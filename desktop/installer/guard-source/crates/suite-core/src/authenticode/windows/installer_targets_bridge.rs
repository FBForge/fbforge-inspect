//! Same-file native facts for installer_targets. No public file/authority API.
use super::{program_tree as permissions, verify_publisher};
use crate::{
    authenticode::{self, InvalidReason, VerificationFailure},
    catalog::ToolManifest,
    installer_targets::TargetError,
    production::bundled_authenticode_policies,
};
pub(crate) use permissions::{DirectoryAnchor, MutationTree};
use semver::Version;
use std::{fs::File, path::Path};

pub(crate) fn require_policy(tool: &ToolManifest) -> Result<(), TargetError> {
    let policies =
        bundled_authenticode_policies().map_err(|_| TargetError::InspectionUnavailable)?;
    let policy = policies
        .policies
        .get(&tool.trust_policy_id)
        .ok_or(TargetError::PublisherRejected)?;
    if policy.tool_id != tool.id
        || policy.app_id != tool.app_id
        || policy.status != authenticode::PolicyStatus::Active
    {
        return Err(TargetError::PublisherRejected);
    }
    Ok(())
}

pub(crate) fn directory(path: &Path, private: bool) -> Result<DirectoryAnchor, TargetError> {
    permissions::lock_directory(path, private, false).map_err(map_permission)
}
pub(crate) fn create_directory(path: &Path) -> Result<DirectoryAnchor, TargetError> {
    permissions::create_private_directory(path).map_err(map_permission)
}
pub(crate) fn create_file(path: &Path) -> Result<File, TargetError> {
    permissions::create_private_file(path).map_err(map_permission)
}
pub(crate) fn private_file(path: &Path) -> Result<File, TargetError> {
    permissions::lock_private_file(path).map_err(map_permission)
}
pub(crate) fn program_tree(
    path: &Path,
    tool: &ToolManifest,
    version: Option<&Version>,
    now: u64,
) -> Result<(MutationTree, Version), TargetError> {
    let policies =
        bundled_authenticode_policies().map_err(|_| TargetError::InspectionUnavailable)?;
    let mut program = authenticode::verify_cooperating_program(path, tool, &policies, now)
        .map_err(|failure| match failure {
            VerificationFailure::Invalid(InvalidReason::CooperationMissing) => {
                TargetError::ManualMigrationRequired
            }
            VerificationFailure::Invalid(_) => TargetError::PublisherRejected,
            _ => TargetError::InspectionUnavailable,
        })?;
    let actual = authenticode::read_program_product_version(&mut program._program.file)
        .map_err(map_verification)?;
    if version.is_some_and(|expected| expected != &actual) {
        return Err(TargetError::PayloadMismatch);
    }
    let tree = permissions::lock_mutation_tree(program._program).map_err(map_permission)?;
    Ok((tree, actual))
}
pub(crate) fn uninstaller(
    file: &mut File,
    tool: &ToolManifest,
    version: &Version,
    now: u64,
) -> Result<(), TargetError> {
    crate::discovery::inspect_windows_installer_image(file)
        .map_err(|_| TargetError::PublisherRejected)?;
    let identity = authenticode::read_installer_identity(file).map_err(map_verification)?;
    if identity.app_id() != tool.app_id || identity.version() != version || identity.protocol() != 1
    {
        return Err(TargetError::PublisherRejected);
    }
    let policies =
        bundled_authenticode_policies().map_err(|_| TargetError::InspectionUnavailable)?;
    let policy = policies
        .policies
        .get(&tool.trust_policy_id)
        .ok_or(TargetError::PublisherRejected)?;
    if policy.tool_id != tool.id
        || policy.app_id != tool.app_id
        || policy.status != authenticode::PolicyStatus::Active
    {
        return Err(TargetError::PublisherRejected);
    }
    verify_publisher(file, policy, now).map_err(map_verification)?;
    Ok(())
}
fn map_verification(failure: VerificationFailure) -> TargetError {
    match failure {
        VerificationFailure::Invalid(_) => TargetError::PublisherRejected,
        VerificationFailure::Indeterminate(_) => TargetError::InspectionUnavailable,
    }
}
pub(crate) fn map_permission(failure: authenticode::ProgramStartFailure) -> TargetError {
    match failure {
        authenticode::ProgramStartFailure::UnsafeInstallation => TargetError::UnsafeTarget,
        authenticode::ProgramStartFailure::UnsupportedInstallation => {
            TargetError::UnsupportedTarget
        }
        _ => TargetError::InspectionUnavailable,
    }
}

pub(crate) struct HeldInstallerHost {
    _file: super::LockedPackageFile,
}

pub(crate) fn current_installer(
    tool: &ToolManifest,
) -> Result<(HeldInstallerHost, Version, [u8; 32], [u8; 32]), TargetError> {
    use std::io::{Read, Seek, SeekFrom};
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows_sys::Win32::System::LibraryLoader::GetModuleFileNameW;
    let mut buffer = [0u16; 4097];
    // SAFETY: module0 selects the native process image, not this plugin DLL;
    // fixed writable storage spans the exact declared UTF16 capacity.
    let count = unsafe { GetModuleFileNameW(0, buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if count == 0 || count >= buffer.len() || buffer[count] != 0 || buffer[..count].contains(&0) {
        return Err(TargetError::InspectionUnavailable);
    }
    let text = String::from_utf16(&buffer[..count]).map_err(|_| TargetError::UnsupportedTarget)?;
    let path = PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(&text));
    let mut host = super::LockedPackageFile::open(&path).map_err(map_verification)?;
    permissions::inspect_installer_host(&host).map_err(map_permission)?;
    let identity =
        authenticode::read_installer_identity(&mut host.file).map_err(map_verification)?;
    if identity.app_id() != tool.app_id || identity.protocol() != 1 {
        return Err(TargetError::PublisherRejected);
    }
    let digest = *identity
        .payload_manifest_sha256()
        .ok_or(TargetError::PublisherRejected)?;
    let version = identity.version().clone();
    let policies =
        bundled_authenticode_policies().map_err(|_| TargetError::InspectionUnavailable)?;
    let policy = policies
        .policies
        .get(&tool.trust_policy_id)
        .ok_or(TargetError::PublisherRejected)?;
    if policy.tool_id != tool.id
        || policy.app_id != tool.app_id
        || policy.status != authenticode::PolicyStatus::Active
    {
        return Err(TargetError::PublisherRejected);
    }
    // Bound the complete same-held host read, including NSIS's signed overlay.
    // The digest is not a replacement for the pinned Windows publisher check.
    host.file
        .seek(SeekFrom::Start(0))
        .map_err(|_| TargetError::InspectionUnavailable)?;
    let mut read = 0u64;
    let mut bytes = [0u8; 64 * 1024];
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    loop {
        let count = host
            .file
            .read(&mut bytes)
            .map_err(|_| TargetError::InspectionUnavailable)?;
        if count == 0 {
            break;
        }
        read = read
            .checked_add(count as u64)
            .ok_or(TargetError::UnsupportedTarget)?;
        if read > 32 * 1024 * 1024 * 1024 {
            return Err(TargetError::UnsupportedTarget);
        }
        hash.update(&bytes[..count]);
    }
    let host_sha256 = hash.finish().as_ref().try_into().expect("SHA256 size");
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TargetError::InspectionUnavailable)?
        .as_secs();
    verify_publisher(&host.file, policy, now).map_err(map_verification)?;
    if SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| TargetError::InspectionUnavailable)?
        .as_secs()
        < now
    {
        return Err(TargetError::InspectionUnavailable);
    }
    Ok((
        HeldInstallerHost { _file: host },
        version,
        digest,
        host_sha256,
    ))
}
