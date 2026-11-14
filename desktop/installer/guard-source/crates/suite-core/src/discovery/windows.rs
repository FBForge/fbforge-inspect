use super::{inspect_windows_x64_image, validate_windows_location, DetectionFailure};
use crate::{catalog::ToolManifest, inventory::*};
use semver::Version;
use std::{
    fs::{self, File},
    io,
    os::windows::fs::MetadataExt,
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
    Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT,
    System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE,
        KEY_QUERY_VALUE, KEY_WOW64_32KEY, KEY_WOW64_64KEY, REG_SZ,
    },
};

const UNINSTALL_ROOT: &str = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

pub(super) fn discover_tool(tool: &ToolManifest) -> DetectionReport {
    let mut report = DetectionReport {
        installations: vec![],
        issues: vec![],
    };
    for (hive, handle) in [
        (RegistryHive::CurrentUser, HKEY_CURRENT_USER),
        (RegistryHive::LocalMachine, HKEY_LOCAL_MACHINE),
    ] {
        for (view, flag) in [
            (RegistryView::Registry64, KEY_WOW64_64KEY),
            (RegistryView::Registry32, KEY_WOW64_32KEY),
        ] {
            for name in &tool.registry_names {
                let source = RegistrySource {
                    hive,
                    view,
                    key_name: name.clone(),
                };
                match read_installation(tool, handle, flag, source) {
                    Ok(Some(found)) => report.record_installation(found),
                    Ok(None) => {}
                    Err(issue) => {
                        if !report.issues.contains(&issue) {
                            report.issues.push(issue);
                        }
                    }
                }
            }
        }
    }
    report
}

struct RegistryKey(HKEY);
impl Drop for RegistryKey {
    fn drop(&mut self) {
        // SAFETY: this handle was returned by a successful RegOpenKeyExW and
        // remains exclusively owned by this wrapper until drop.
        unsafe {
            RegCloseKey(self.0);
        }
    }
}

fn read_installation(
    tool: &ToolManifest,
    hive: HKEY,
    view: u32,
    source: RegistrySource,
) -> Result<Option<Installation>, DetectionIssue> {
    let Some(key) = open_uninstall_key(hive, view, &source.key_name)? else {
        return Ok(None);
    };
    let raw = read_string(&key, "InstallLocation")?.ok_or(DetectionIssue::InvalidRegistryData)?;
    let root = PathBuf::from(
        validate_windows_location(&raw).map_err(|_| DetectionIssue::InvalidRegistryData)?,
    );
    let reported_version = read_string(&key, "DisplayVersion")?;
    if reported_version
        .as_ref()
        .is_some_and(|version| version.len() > 128)
    {
        return Err(DetectionIssue::InvalidRegistryData);
    }
    let version = reported_version
        .as_deref()
        .and_then(|version| Version::parse(version).ok());
    let mut installation = Installation {
        root,
        executable: None,
        reported_version,
        version,
        condition: InstallationCondition::Detected,
        sources: vec![source],
    };
    match inspect_directory(&installation.root, tool) {
        Ok((root, executable)) => {
            installation.root = root;
            installation.executable = Some(executable);
            if installation.version.is_none() {
                installation.condition = InstallationCondition::UnsupportedVersion;
            }
        }
        Err(condition) => installation.condition = condition,
    }
    Ok(Some(installation))
}

fn open_uninstall_key(
    hive: HKEY,
    view: u32,
    name: &str,
) -> Result<Option<RegistryKey>, DetectionIssue> {
    let key_name = wide(&format!("{UNINSTALL_ROOT}\\{name}"));
    let mut handle = 0;
    // SAFETY: terminated UTF-16 key name and valid output pointer. Only query
    // access is requested, with one explicit registry view. Nothing is written.
    let status = unsafe {
        RegOpenKeyExW(
            hive,
            key_name.as_ptr(),
            0,
            KEY_QUERY_VALUE | view,
            &mut handle,
        )
    };
    if matches!(status, ERROR_FILE_NOT_FOUND | ERROR_PATH_NOT_FOUND) {
        return Ok(None);
    }
    if status != ERROR_SUCCESS {
        return Err(DetectionIssue::RegistryUnavailable);
    }
    Ok(Some(RegistryKey(handle)))
}

/// Read the unresolved registration spelling with the same bounded REG_SZ
/// reader as discovery. It is a location observation, never target authority.
pub(crate) fn read_registered_current_user_root(
    source: &RegistrySource,
) -> Result<String, DetectionIssue> {
    if source.hive != RegistryHive::CurrentUser {
        return Err(DetectionIssue::InvalidRegistryData);
    }
    let view = match source.view {
        RegistryView::Registry32 => KEY_WOW64_32KEY,
        RegistryView::Registry64 => KEY_WOW64_64KEY,
    };
    let key = open_uninstall_key(HKEY_CURRENT_USER, view, &source.key_name)?
        .ok_or(DetectionIssue::RegistryUnavailable)?;
    let raw = read_string(&key, "InstallLocation")?.ok_or(DetectionIssue::InvalidRegistryData)?;
    validate_windows_location(&raw).map_err(|_| DetectionIssue::InvalidRegistryData)
}

fn read_string(key: &RegistryKey, name: &str) -> Result<Option<String>, DetectionIssue> {
    let name = wide(name);
    let mut buffer = [0u16; 4096];
    let mut bytes = std::mem::size_of_val(&buffer) as u32;
    let mut value_type = 0;
    // SAFETY: the writable buffer is aligned, sized in bytes, and all pointers
    // remain valid for this call. Never resize from a registry-provided length.
    let status = unsafe {
        RegQueryValueExW(
            key.0,
            name.as_ptr(),
            ptr::null(),
            &mut value_type,
            buffer.as_mut_ptr().cast(),
            &mut bytes,
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != ERROR_SUCCESS
        || value_type != REG_SZ
        || bytes < 2
        || bytes as usize > std::mem::size_of_val(&buffer)
        || !bytes.is_multiple_of(2)
    {
        return Err(DetectionIssue::InvalidRegistryData);
    }
    let count = bytes as usize / 2;
    if buffer[count - 1] != 0 || buffer[..count - 1].contains(&0) {
        return Err(DetectionIssue::InvalidRegistryData);
    }
    String::from_utf16(&buffer[..count - 1])
        .map(Some)
        .map_err(|_| DetectionIssue::InvalidRegistryData)
}

fn inspect_directory(
    root: &Path,
    tool: &ToolManifest,
) -> Result<(PathBuf, PathBuf), InstallationCondition> {
    let mut ancestor = PathBuf::new();
    for component in root.components() {
        ancestor.push(component);
        if ancestor.is_absolute() {
            let metadata = fs::symlink_metadata(&ancestor).map_err(directory_error)?;
            if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(InstallationCondition::UnsafeLocation);
            }
        }
    }
    let canonical_root = fs::canonicalize(root).map_err(directory_error)?;
    let mut executables = Vec::new();
    for name in &tool.executables {
        let path = canonical_root.join(name);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => return Err(InstallationCondition::Inaccessible),
        };
        if !metadata.is_file() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return Err(InstallationCondition::UnsafeLocation);
        }
        let mut file = File::open(&path).map_err(|_| InstallationCondition::Inaccessible)?;
        inspect_windows_x64_image(&mut file).map_err(|error| match error {
            DetectionFailure::Unreadable => InstallationCondition::Inaccessible,
            _ => InstallationCondition::UnsupportedBinary,
        })?;
        let executable =
            fs::canonicalize(&path).map_err(|_| InstallationCondition::Inaccessible)?;
        if !executable.starts_with(&canonical_root) {
            return Err(InstallationCondition::UnsafeLocation);
        }
        executables.push(executable);
    }
    match executables.len() {
        0 => Err(InstallationCondition::MissingExecutable),
        1 => Ok((canonical_root, executables.remove(0))),
        _ => Err(InstallationCondition::AmbiguousExecutables),
    }
}

fn directory_error(error: io::Error) -> InstallationCondition {
    if error.kind() == io::ErrorKind::NotFound {
        InstallationCondition::MissingDirectory
    } else {
        InstallationCondition::Inaccessible
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}
