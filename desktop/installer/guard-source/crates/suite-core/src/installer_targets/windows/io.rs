use super::super::TargetError;
use crate::authenticode::installer_targets_bridge as proof;
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    fs::{self, File},
    io::{Read, Write},
    os::windows::ffi::OsStrExt,
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetDiskFreeSpaceExW, MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
};

pub(super) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn nonce() -> Result<String, TargetError> {
    let mut bytes = [0; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| TargetError::OperatingSystem)?;
    Ok(hex(&bytes))
}
pub(super) fn wide(path: &Path) -> Vec<u16> {
    // All inputs originate from the native DOS-path plan. Use the explicit
    // long-path spelling so NSIS process manifest defaults cannot truncate it.
    std::ffi::OsStr::new(r"\\?\")
        .encode_wide()
        .chain(path.as_os_str().encode_wide())
        .chain(Some(0))
        .collect()
}
pub(super) fn exists(path: &Path) -> Result<bool, TargetError> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => Err(TargetError::InspectionUnavailable),
    }
}
pub(super) fn read(path: &Path, limit: usize) -> Result<Vec<u8>, TargetError> {
    let file = proof::private_file(path)?;
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| TargetError::OperatingSystem)?;
    if bytes.len() > limit {
        return Err(TargetError::RecoveryRequired);
    }
    Ok(bytes)
}
pub(super) fn create_bytes(path: &Path, bytes: &[u8]) -> Result<(), TargetError> {
    let mut file = proof::create_file(path)?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| TargetError::OperatingSystem)
}
pub(super) fn replace_control(path: &Path, bytes: &[u8]) -> Result<(), TargetError> {
    if bytes.len() > super::super::journal::MAX_JOURNAL_BYTES {
        return Err(TargetError::RecoveryRequired);
    }
    if exists(path)? {
        drop(proof::private_file(path)?);
    }
    let parent = path.parent().ok_or(TargetError::UnsafeTarget)?;
    let temp = parent.join(format!(".fbforge-control-{}.tmp", nonce()?));
    create_bytes(&temp, bytes)?;
    // Replacement is confined to our validated private journal/locator/receipt,
    // never to the old installation tree or either directory publication move.
    // SAFETY: both terminated native paths remain live for the duration of this call.
    if unsafe {
        MoveFileExW(
            wide(&temp).as_ptr(),
            wide(path).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(TargetError::OperatingSystem);
    }
    drop(proof::private_file(path)?);
    Ok(())
}
pub(super) fn rename_directory(from: &Path, to: &Path) -> Result<(), TargetError> {
    if exists(to)? {
        return Err(TargetError::RecoveryRequired);
    }
    // No REPLACE_EXISTING, COPY_ALLOWED, delay-until-reboot or fallback copy.
    // SAFETY: both terminated native paths remain live for the duration of this call.
    if unsafe {
        MoveFileExW(
            wide(from).as_ptr(),
            wide(to).as_ptr(),
            MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(TargetError::OperatingSystem);
    }
    Ok(())
}
pub(super) fn remove_private_file(path: &Path) -> Result<(), TargetError> {
    if !exists(path)? {
        return Ok(());
    }
    drop(proof::private_file(path)?);
    fs::remove_file(path).map_err(|_| TargetError::OperatingSystem)
}
pub(super) fn require_space(parent: &Path, payload: u64) -> Result<(), TargetError> {
    let mut available = 0;
    // SAFETY: terminated held-parent path and writable scalar output remain live.
    if unsafe {
        GetDiskFreeSpaceExW(
            wide(parent).as_ptr(),
            &mut available,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(TargetError::InspectionUnavailable);
    }
    if available
        < payload
            .checked_add(128 * 1024 * 1024)
            .ok_or(TargetError::InvalidManifest)?
    {
        return Err(TargetError::NotEnoughSpace);
    }
    Ok(())
}
pub(super) fn copy_bounded(
    mut file: File,
    reader: &mut impl Read,
    limit: u64,
    expected: Option<[u8; 32]>,
) -> Result<(), TargetError> {
    let mut total = 0u64;
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut buffer = [0; 64 * 1024];
    loop {
        let size = reader
            .read(&mut buffer)
            .map_err(|_| TargetError::OperatingSystem)?;
        if size == 0 {
            break;
        }
        total = total
            .checked_add(size as u64)
            .ok_or(TargetError::PayloadMismatch)?;
        if total > limit {
            return Err(TargetError::PayloadMismatch);
        }
        hash.update(&buffer[..size]);
        file.write_all(&buffer[..size])
            .map_err(|_| TargetError::OperatingSystem)?;
    }
    if expected.is_some_and(|expected| total != limit || hash.finish().as_ref() != expected) {
        return Err(TargetError::PayloadMismatch);
    }
    file.sync_all().map_err(|_| TargetError::OperatingSystem)
}
