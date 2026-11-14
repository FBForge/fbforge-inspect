use super::{ActivationOutcome, ProcessIssue, ProcessReport};
use crate::{
    catalog::ToolManifest,
    discovery::{discover_tool, validate_windows_location},
    inventory::{Installation, InstallationCondition},
    operations::ProcessStatus,
};
use std::{
    fs,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_NO_MORE_FILES, FILETIME, HANDLE, HWND,
        INVALID_HANDLE_VALUE, LPARAM, WAIT_TIMEOUT,
    },
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT,
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{
            GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, WaitForSingleObject,
            PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        },
    },
    UI::WindowsAndMessaging::{
        EnumWindows, GetWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
        SetForegroundWindow, ShowWindowAsync, GW_OWNER, SW_RESTORE,
    },
};

const MAX_PROCESS_ENTRIES: usize = 16_384;
const MAX_MATCHED_PROCESSES: usize = 64;
const MAX_PATH_CHARACTERS: usize = 32_768;
const MAX_WINDOW_ENTRIES: usize = 16_384;

struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: only successfully acquired, non-inherited handles enter this wrapper.
        unsafe { CloseHandle(self.0) };
    }
}

struct ObservedProcess {
    handle: OwnedHandle,
    pid: u32,
    created_at: u64,
}

impl ObservedProcess {
    fn is_current(&self) -> bool {
        // SAFETY: this wrapper retains the process handle throughout the query.
        (unsafe { WaitForSingleObject(self.handle.0, 0) }) == WAIT_TIMEOUT
            && creation_time(&self.handle).ok() == Some(self.created_at)
    }
}

struct ProcessSnapshot {
    matches: Vec<ObservedProcess>,
    issues: Vec<ProcessIssue>,
}

pub(super) fn inspect_processes(tool: &ToolManifest, installation: &Installation) -> ProcessReport {
    let target = match rediscover_target(tool, installation) {
        Ok(target) => target,
        Err(issue) => return ProcessReport::unknown(issue),
    };
    match enumerate_processes(&target) {
        Ok(mut snapshot) => {
            snapshot.matches.retain(ObservedProcess::is_current);
            ProcessReport {
                status: if snapshot.matches.is_empty() {
                    ProcessStatus::Unknown
                } else {
                    ProcessStatus::Running
                },
                matched_processes: snapshot.matches.len(),
                issues: snapshot.issues,
            }
        }
        Err(issue) => ProcessReport::unknown(issue),
    }
}

pub(super) fn activate_existing(
    tool: &ToolManifest,
    installation: &Installation,
) -> ActivationOutcome {
    let target = match rediscover_target(tool, installation) {
        Ok(target) => target,
        Err(issue) => return ActivationOutcome::Unknown(issue),
    };
    let mut snapshot = match enumerate_processes(&target) {
        Ok(snapshot) => snapshot,
        Err(issue) => return ActivationOutcome::Unknown(issue),
    };
    snapshot.matches.retain(ObservedProcess::is_current);
    if snapshot.matches.is_empty() {
        return ActivationOutcome::Unknown(ProcessIssue::IncompleteCoverage);
    }
    activate_window(&snapshot.matches)
}

fn rediscover_target(
    tool: &ToolManifest,
    installation: &Installation,
) -> Result<PathBuf, ProcessIssue> {
    let current = discover_tool(tool);
    if !current.issues.is_empty() {
        return Err(ProcessIssue::InstallationNotCurrent);
    }
    current
        .installations
        .into_iter()
        .find(|found| {
            found.condition == InstallationCondition::Detected
                && found.root == installation.root
                && found.executable == installation.executable
                && found.version == installation.version
                && found.reported_version == installation.reported_version
        })
        .and_then(|found| found.executable)
        .ok_or(ProcessIssue::InstallationNotCurrent)
}

fn enumerate_processes(target: &Path) -> Result<ProcessSnapshot, ProcessIssue> {
    // SAFETY: process enumeration only; no module access or inherited handles.
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE || raw == 0 {
        return Err(ProcessIssue::EnumerationUnavailable);
    }
    let snapshot = OwnedHandle(raw);
    let mut result = ProcessSnapshot {
        matches: Vec::new(),
        // Even a complete process list cannot exclude hosted/renamed instances
        // or coordinate another launcher/updater with the manifest contract today.
        issues: vec![ProcessIssue::IncompleteCoverage],
    };
    // SAFETY: this POD structure is zero-initializable; dwSize is set before use.
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
    // SAFETY: snapshot handle is live and the writable entry has its declared size.
    let mut has_entry = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    let mut visited = 0;
    while has_entry != 0 {
        visited += 1;
        if visited > MAX_PROCESS_ENTRIES {
            return Err(ProcessIssue::EnumerationLimit);
        }
        // Do not use szExeFile as an absence filter. Inspect full image paths;
        // inaccessible processes remain an explicit coverage gap.
        if entry.th32ProcessID != 0 {
            match query_matching_process(entry.th32ProcessID, target) {
                Ok(Some(process)) => {
                    if result.matches.len() == MAX_MATCHED_PROCESSES {
                        return Err(ProcessIssue::EnumerationLimit);
                    }
                    result.matches.push(process);
                }
                Ok(None) => {}
                Err(issue) if !result.issues.contains(&issue) => result.issues.push(issue),
                Err(_) => {}
            }
        }
        // SAFETY: same valid snapshot and writable sized entry throughout iteration.
        has_entry = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    // SAFETY: inspect the error immediately after the failed first/next call.
    if unsafe { GetLastError() } != ERROR_NO_MORE_FILES {
        return Err(ProcessIssue::EnumerationUnavailable);
    }
    Ok(result)
}

fn query_matching_process(
    pid: u32,
    target: &Path,
) -> Result<Option<ObservedProcess>, ProcessIssue> {
    // SAFETY: bounded rights only, no inheritance, caller supplies an enumerated PID.
    let raw = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    if raw == 0 {
        return Err(ProcessIssue::ProcessUnavailable);
    }
    let handle = OwnedHandle(raw);
    let created_at = creation_time(&handle)?;
    let image = image_path(&handle)?;
    let image_text = normalized_path(&image).ok_or(ProcessIssue::PathUnavailable)?;
    let target_text = normalized_path(target).ok_or(ProcessIssue::PathUnavailable)?;
    // This full-path filter is only an optimization for positive matches. The
    // subsequent canonical comparison preserves case-sensitive directory identity.
    if !image_text.eq_ignore_ascii_case(&target_text) {
        return Ok(None);
    }
    if canonical_image(&image)? != target {
        return Ok(None);
    }
    let process = ObservedProcess {
        handle,
        pid,
        created_at,
    };
    Ok(process.is_current().then_some(process))
}

fn image_path(handle: &OwnedHandle) -> Result<PathBuf, ProcessIssue> {
    let mut buffer = vec![0u16; MAX_PATH_CHARACTERS];
    let mut size = buffer.len() as u32;
    // SAFETY: retained query-capable handle and a bounded writable UTF-16 buffer.
    if unsafe { QueryFullProcessImageNameW(handle.0, 0, buffer.as_mut_ptr(), &mut size) } == 0
        || size == 0
        || size as usize >= buffer.len()
    {
        return Err(ProcessIssue::PathUnavailable);
    }
    let text =
        String::from_utf16(&buffer[..size as usize]).map_err(|_| ProcessIssue::PathUnavailable)?;
    Ok(PathBuf::from(text))
}

fn normalized_path(path: &Path) -> Option<String> {
    let text = path.to_str()?;
    Some(
        text.strip_prefix(r"\\?\")
            .unwrap_or(text)
            .replace('/', "\\"),
    )
}

fn canonical_image(path: &Path) -> Result<PathBuf, ProcessIssue> {
    let text = normalized_path(path).ok_or(ProcessIssue::PathUnavailable)?;
    validate_windows_location(&text).map_err(|_| ProcessIssue::PathUnavailable)?;
    // Reject known reparse ancestors before canonicalization can follow a remote
    // target. This is an observation, not a race-proof executing-image identity.
    for ancestor in path.ancestors().collect::<Vec<_>>().into_iter().rev() {
        if !ancestor.is_absolute() {
            continue;
        }
        let metadata = fs::symlink_metadata(ancestor).map_err(|_| ProcessIssue::PathUnavailable)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(ProcessIssue::PathUnavailable);
        }
    }
    fs::canonicalize(path).map_err(|_| ProcessIssue::PathUnavailable)
}

fn creation_time(handle: &OwnedHandle) -> Result<u64, ProcessIssue> {
    let mut created = FILETIME {
        dwLowDateTime: 0,
        dwHighDateTime: 0,
    };
    let mut exited = created;
    let mut kernel = created;
    let mut user = created;
    // SAFETY: retained query-capable handle and four distinct writable FILETIMEs.
    if unsafe { GetProcessTimes(handle.0, &mut created, &mut exited, &mut kernel, &mut user) } == 0
    {
        return Err(ProcessIssue::ProcessUnavailable);
    }
    Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
}

struct WindowSearch<'a> {
    processes: &'a [ObservedProcess],
    matches: Vec<(HWND, usize)>,
    visited: usize,
    limit_reached: bool,
}

unsafe extern "system" fn collect_window(window: HWND, context: LPARAM) -> i32 {
    // SAFETY: EnumWindows receives the live exclusive WindowSearch pointer below
    // and invokes this callback synchronously on this thread.
    let search = unsafe { &mut *(context as *mut WindowSearch<'_>) };
    search.visited += 1;
    if search.visited > MAX_WINDOW_ENTRIES {
        search.limit_reached = true;
        return 0;
    }
    // SAFETY: these OS queries accept an enumerated HWND and do not mutate it.
    if unsafe { IsWindowVisible(window) } == 0 || unsafe { GetWindow(window, GW_OWNER) } != 0 {
        return 1;
    }
    let mut pid = 0;
    // SAFETY: writable PID output. A failed query does not identify a candidate.
    if unsafe { GetWindowThreadProcessId(window, &mut pid) } != 0 {
        if let Some(index) = search
            .processes
            .iter()
            .position(|process| process.pid == pid)
        {
            search.matches.push((window, index));
            if search.matches.len() == 2 {
                return 0;
            }
        }
    }
    1
}

fn activate_window(processes: &[ObservedProcess]) -> ActivationOutcome {
    let mut search = WindowSearch {
        processes,
        matches: Vec::new(),
        visited: 0,
        limit_reached: false,
    };
    // SAFETY: the callback borrows this live state only for the synchronous call.
    let completed = unsafe { EnumWindows(Some(collect_window), &mut search as *mut _ as LPARAM) };
    if search.limit_reached {
        return ActivationOutcome::Unknown(ProcessIssue::EnumerationLimit);
    }
    if search.matches.len() > 1 {
        return ActivationOutcome::Ambiguous;
    }
    if completed == 0 {
        return ActivationOutcome::Unknown(ProcessIssue::WindowEnumerationUnavailable);
    }
    let Some(&(window, index)) = search.matches.first() else {
        return if processes.iter().any(ObservedProcess::is_current) {
            ActivationOutcome::NoWindow
        } else {
            ActivationOutcome::Unknown(ProcessIssue::ProcessChanged)
        };
    };
    let process = &processes[index];
    if !window_belongs_to(window, process) {
        return ActivationOutcome::Unknown(ProcessIssue::ProcessChanged);
    }
    // SAFETY: revalidated existing window; the asynchronous call avoids waiting
    // for an unresponsive application's UI thread. It does not force foreground.
    if unsafe { IsIconic(window) } != 0 && unsafe { ShowWindowAsync(window, SW_RESTORE) } == 0 {
        return ActivationOutcome::Unknown(ProcessIssue::WindowUnavailable);
    }
    if !window_belongs_to(window, process) {
        return ActivationOutcome::Unknown(ProcessIssue::ProcessChanged);
    }
    // SAFETY: only one explicit activation request for the revalidated HWND.
    if unsafe { SetForegroundWindow(window) } != 0 {
        ActivationOutcome::Activated
    } else if window_belongs_to(window, process) {
        ActivationOutcome::ForegroundDenied
    } else {
        ActivationOutcome::Unknown(ProcessIssue::ProcessChanged)
    }
}

fn window_belongs_to(window: HWND, process: &ObservedProcess) -> bool {
    let mut pid = 0;
    // SAFETY: valid output pointer; a destroyed/reassigned HWND fails identity.
    (unsafe { GetWindowThreadProcessId(window, &mut pid) }) != 0
        && pid == process.pid
        && process.is_current()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_process_snapshot_finds_this_executable_without_claiming_complete_coverage() {
        let target = fs::canonicalize(std::env::current_exe().unwrap()).unwrap();
        let snapshot = enumerate_processes(&target).unwrap();
        let process = snapshot
            .matches
            .iter()
            .find(|process| process.pid == std::process::id())
            .unwrap();
        assert!(process.is_current());
        assert!(snapshot.issues.contains(&ProcessIssue::IncompleteCoverage));
    }
}
