use super::{
    system_windows_directory_path, temporary_directory_path, InstallerError, InstallerProcessState,
    NSIS_TEMP_CAPACITY,
};
use crate::authenticode::{self, InstallerTemporaryDirectory, VerifiedCachedWindowsPackage};
use std::{
    mem::{size_of, zeroed},
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{
        GetLastError, ERROR_ELEVATION_REQUIRED, HANDLE, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
    },
    Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY},
    Storage::FileSystem::GetTempPathW,
    System::{
        SystemInformation::GetSystemWindowsDirectoryW,
        Threading::{
            CreateProcessW, GetCurrentProcess, GetExitCodeProcess, OpenProcessToken,
            WaitForSingleObject, PROCESS_INFORMATION, STARTUPINFOW,
        },
    },
};

pub(super) struct InstallerChild {
    process: OwnedHandle,
    _temporary_directory: InstallerTemporaryDirectory,
}

fn current_temporary_directory() -> Result<std::path::PathBuf, InstallerError> {
    let mut buffer = [0; NSIS_TEMP_CAPACITY];
    // SAFETY: complete fixed UTF16 output buffer matches the pinned NSIS stub.
    let count = unsafe { GetTempPathW(buffer.len() as u32, buffer.as_mut_ptr()) };
    if count == 0 {
        let _error = unsafe { GetLastError() };
    }
    temporary_directory_path(&buffer, count)
}

fn shared_windows_directory() -> Result<std::path::PathBuf, InstallerError> {
    let mut buffer = [0; NSIS_TEMP_CAPACITY];
    // SAFETY: complete fixed UTF16 buffer. This queries the shared OS directory,
    // including Terminal Services, rather than a user-specific Windows folder.
    let count = unsafe { GetSystemWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
    if count == 0 {
        let _error = unsafe { GetLastError() };
    }
    system_windows_directory_path(&buffer, count)
}

fn require_standard_token() -> Result<(), InstallerError> {
    let mut token = 0;
    // SAFETY: query this process's primary token, matching CreateProcessW's
    // security context even when the calling thread happens to impersonate.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        let _error = unsafe { GetLastError() };
        return Err(InstallerError::LaunchUnavailable);
    }
    let token = unsafe { OwnedHandle::from_raw_handle(token as _) };
    let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
    let mut returned = 0;
    if unsafe {
        GetTokenInformation(
            token.as_raw_handle() as HANDLE,
            TokenElevation,
            (&mut elevation as *mut TOKEN_ELEVATION).cast(),
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut returned,
        )
    } == 0
    {
        let _error = unsafe { GetLastError() };
        return Err(InstallerError::LaunchUnavailable);
    }
    if returned != size_of::<TOKEN_ELEVATION>() as u32 {
        return Err(InstallerError::LaunchUnavailable);
    }
    if elevation.TokenIsElevated != 0 {
        return Err(InstallerError::ElevationNotAllowed);
    }
    Ok(())
}

pub(super) fn start(
    proof: &VerifiedCachedWindowsPackage<'_>,
    validate: impl FnOnce() -> Result<(), InstallerError>,
) -> Result<InstallerChild, InstallerError> {
    let application = authenticode::installer_path(proof)?;
    let mut command = Vec::with_capacity(application.len() + 6);
    command.push(b'"' as u16);
    command.extend_from_slice(&application[..application.len() - 1]);
    command.extend("\" /P\0".encode_utf16());
    if command.len() > 32767 {
        return Err(InstallerError::LaunchUnavailable);
    }
    let last_separator = application[..application.len() - 1]
        .iter()
        .rposition(|n| *n == b'\\' as u16)
        .ok_or(InstallerError::LaunchUnavailable)?;
    let mut directory = application[..last_separator].to_vec();
    directory.push(0);
    // SAFETY: complete POD structures; unspecified STARTUPINFO fields are zero.
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    let mut result: PROCESS_INFORMATION = unsafe { zeroed() };
    require_standard_token()?;
    let temporary_path = current_temporary_directory()?;
    let windows_directory = shared_windows_directory()?;
    let temporary_directory =
        InstallerTemporaryDirectory::lock(&temporary_path, &windows_directory)
            .map_err(|_| InstallerError::TemporaryDirectoryUnavailable)?;
    temporary_directory
        .recheck()
        .map_err(|_| InstallerError::TemporaryDirectoryUnavailable)?;
    if current_temporary_directory()? != temporary_path {
        return Err(InstallerError::TemporaryDirectoryUnavailable);
    }
    validate()?;
    // SAFETY: absolute held-file application path, distinct mutable command
    // buffer, fixed /P only, fixed cache CWD, no inherited handles or new token.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            0,
            ptr::null(),
            directory.as_ptr(),
            &startup,
            &mut result,
        )
    };
    if created == 0 {
        // Preserve the OS error before any other OS call or owned-handle Drop.
        let error = unsafe { GetLastError() };
        return Err(if error == ERROR_ELEVATION_REQUIRED {
            InstallerError::ElevationNotAllowed
        } else {
            InstallerError::LaunchUnavailable
        });
    }
    // SAFETY: successful CreateProcessW transfers exactly these two owned
    // handles. Closing the primary-thread handle never terminates that thread.
    let thread = unsafe { OwnedHandle::from_raw_handle(result.hThread as _) };
    let process = unsafe { OwnedHandle::from_raw_handle(result.hProcess as _) };
    drop(thread);
    Ok(InstallerChild {
        process,
        _temporary_directory: temporary_directory,
    })
}

pub(super) fn wait(
    child: &InstallerChild,
    milliseconds: u32,
) -> Result<InstallerProcessState, InstallerError> {
    // SAFETY: borrowed owned process handle remains live for the entire wait.
    let status =
        unsafe { WaitForSingleObject(child.process.as_raw_handle() as HANDLE, milliseconds) };
    match status {
        WAIT_TIMEOUT => Ok(InstallerProcessState::Running),
        WAIT_OBJECT_0 => {
            let mut code = 0;
            if unsafe { GetExitCodeProcess(child.process.as_raw_handle() as HANDLE, &mut code) }
                == 0
            {
                let _error = unsafe { GetLastError() };
                return Err(InstallerError::ProcessUnavailable);
            }
            Ok(InstallerProcessState::Exited { code })
        }
        WAIT_FAILED => {
            let _error = unsafe { GetLastError() };
            Err(InstallerError::ProcessUnavailable)
        }
        _ => Err(InstallerError::ProcessUnavailable),
    }
}
