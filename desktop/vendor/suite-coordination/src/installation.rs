use std::{fs, io, path::Path};

/// Reserved native installer marker. Ordinary startup never removes this file.
pub const PENDING_INSTALLATION_MARKER: &str = ".fbforge-install.pending";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingInstallation {
    Detected,
    Unavailable,
}

/// Call under the app lease before creating UI, Java or other ordinary work.
/// Source the directory natively from the executable, never a renderer value.
/// A marker of any kind blocks startup, including a link or malformed data.
/// Absence only clears this interrupted-install signal; it is no integrity,
/// process-state, coordination or execution authorization.
pub fn check_pending_installation(directory: &Path) -> Result<(), PendingInstallation> {
    let is_directory = fs::symlink_metadata(directory).is_ok_and(|metadata| metadata.is_dir());
    if !is_directory {
        return Err(PendingInstallation::Unavailable);
    }
    match fs::symlink_metadata(directory.join(PENDING_INSTALLATION_MARKER)) {
        Ok(_) => Err(PendingInstallation::Detected),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(PendingInstallation::Unavailable),
    }
}
