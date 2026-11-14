use super::StoreError;
use std::{
    fs::{self, File, Metadata, OpenOptions},
    os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
    path::Path,
};

pub(super) fn validate_root_syntax(_root: &Path) -> Result<(), StoreError> {
    Ok(())
}

pub(super) fn validate_ancestor(metadata: &Metadata) -> Result<(), StoreError> {
    // SAFETY: reads the effective identity without pointers or side effects.
    let current_user = unsafe { libc::geteuid() };
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || (metadata.uid() != 0 && metadata.uid() != current_user)
        || (metadata.mode() & 0o022 != 0 && metadata.mode() & 0o1000 == 0)
    {
        return Err(StoreError::UnsafePath);
    }
    Ok(())
}
pub(super) fn create_directory(root: &Path) -> Result<(), StoreError> {
    match fs::DirBuilder::new().mode(0o700).create(root) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(StoreError::Io),
    }
    // The first committed state also depends on the root directory entry itself.
    // Synchronize its parent before any successful state commit is possible.
    File::open(root.parent().ok_or(StoreError::UnsafePath)?)
        .and_then(|parent| parent.sync_all())
        .map_err(|_| StoreError::Io)
}
pub(super) fn validate_directory(root: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(root).map_err(|_| StoreError::Io)?;
    // SAFETY: geteuid takes no pointers and only reads the process identity.
    if !metadata.is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(StoreError::UnsafePath);
    }
    Ok(())
}
pub(super) fn configure_open(options: &mut OpenOptions) {
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
}
pub(super) fn validate_file(file: &File, _path: &Path) -> Result<(), StoreError> {
    let metadata = file.metadata().map_err(|_| StoreError::Io)?;
    // SAFETY: identity lookup takes no pointers and cannot mutate filesystem state.
    if !metadata.is_file()
        || metadata.nlink() != 1
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(StoreError::UnsafePath);
    }
    Ok(())
}
pub(super) fn replace_file(source: &Path, destination: &Path) -> Result<(), StoreError> {
    fs::rename(source, destination).map_err(|_| StoreError::Indeterminate)
}
pub(super) fn sync_directory(root: &Path) -> Result<(), StoreError> {
    File::open(root)
        .and_then(|file| file.sync_all())
        .map_err(|_| StoreError::Indeterminate)
}
