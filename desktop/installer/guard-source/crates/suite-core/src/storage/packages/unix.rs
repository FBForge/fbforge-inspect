use super::{
    files::{Directory, Stamp},
    PackageCacheError,
};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

pub(super) fn open_directory(path: &Path) -> Result<File, PackageCacheError> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)
        .map_err(|_| PackageCacheError::UnsafePath)?;
    let metadata = file.metadata().map_err(|_| PackageCacheError::Io)?;
    // SAFETY: geteuid has no pointer arguments or mutation.
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(PackageCacheError::UnsafePath);
    }
    Ok(file)
}
fn name(text: &str) -> Result<CString, PackageCacheError> {
    if text.is_empty() || text.contains(['/', '\\']) || text == "." || text == ".." {
        return Err(PackageCacheError::UnsafePath);
    }
    CString::new(text).map_err(|_| PackageCacheError::UnsafePath)
}
fn open(directory: &Directory, text: &str, flags: i32) -> Result<File, PackageCacheError> {
    let name = name(text)?;
    // SAFETY: held directory fd, fixed leaf name, bounded flags, private mode.
    let fd = unsafe {
        libc::openat(
            directory.handle.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK,
            0o600,
        )
    };
    if fd < 0 {
        return Err(PackageCacheError::Io);
    }
    // SAFETY: openat returned a new uniquely owned descriptor.
    let file = unsafe { File::from_raw_fd(fd) };
    validate_file(&file)?;
    Ok(file)
}
pub(super) fn open_file(
    directory: &Directory,
    name: &str,
    create: bool,
) -> Result<File, PackageCacheError> {
    open(
        directory,
        name,
        if create {
            libc::O_RDWR | libc::O_CREAT | libc::O_EXCL
        } else {
            libc::O_RDONLY
        },
    )
}
pub(super) fn open_lock(directory: &Directory) -> Result<File, PackageCacheError> {
    open(directory, "state.lock", libc::O_RDWR | libc::O_CREAT)
}
pub(super) fn validate_file(file: &File) -> Result<(), PackageCacheError> {
    crate::storage::unix::validate_file(file, Path::new(""))
        .map_err(|_| PackageCacheError::UnsafePath)
}
pub(super) fn stamp(file: &File) -> Result<Stamp, PackageCacheError> {
    let m = file.metadata().map_err(|_| PackageCacheError::Io)?;
    Ok(Stamp([
        m.dev(),
        m.ino(),
        m.len(),
        m.mtime() as u64,
        m.mtime_nsec() as u64,
        (m.ctime() as u64)
            .wrapping_mul(1_000_000_000)
            .wrapping_add(m.ctime_nsec() as u64),
    ]))
}
fn require_name(directory: &Directory, file: &File, text: &str) -> Result<(), PackageCacheError> {
    let found = open_file(directory, text, false)?;
    let a = file.metadata().map_err(|_| PackageCacheError::Io)?;
    let b = found.metadata().map_err(|_| PackageCacheError::Io)?;
    if a.dev() != b.dev() || a.ino() != b.ino() {
        return Err(PackageCacheError::UnsafePath);
    }
    Ok(())
}
pub(super) fn remove(
    directory: &Directory,
    file: &File,
    text: &str,
) -> Result<(), PackageCacheError> {
    validate_file(file)?;
    require_name(directory, file, text)?;
    let name = name(text)?;
    // SAFETY: anchored directory fd and a single validated leaf.
    if unsafe { libc::unlinkat(directory.handle.as_raw_fd(), name.as_ptr(), 0) } != 0 {
        return Err(PackageCacheError::Io);
    }
    directory
        .handle
        .sync_all()
        .map_err(|_| PackageCacheError::Io)
}
pub(super) fn publish(
    directory: &Directory,
    file: &File,
    source: &str,
    destination: &str,
) -> Result<(), PackageCacheError> {
    validate_file(file)?;
    require_name(directory, file, source)?;
    let source = name(source)?;
    let destination = name(destination)?;
    #[cfg(target_os = "linux")]
    {
        // SAFETY: same held directory, exclusive no-replacement rename. No links
        // are created and no cross-volume fallback or name reopening is used.
        if unsafe {
            libc::renameat2(
                directory.handle.as_raw_fd(),
                source.as_ptr(),
                directory.handle.as_raw_fd(),
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        } != 0
        {
            return Err(PackageCacheError::Indeterminate);
        }
        directory
            .handle
            .sync_all()
            .map_err(|_| PackageCacheError::Indeterminate)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (source, destination);
        Err(PackageCacheError::UnsupportedPlatform)
    }
}
