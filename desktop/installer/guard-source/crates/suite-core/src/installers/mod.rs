//! Native cache-installer launch mechanics, not target or mutation authority.
use crate::{
    authenticode::{
        self, AuthenticodePolicies, InvalidReason, VerificationFailure,
        VerifiedCachedWindowsPackage,
    },
    storage::CachedPackage,
    trust::VerifiedRelease,
};
use std::{fmt, time::Duration};

#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerError {
    Verification(VerificationFailure),
    UnsupportedPlatform,
    ClockUnavailable,
    ClockRollback,
    InvalidTimeout,
    LaunchUnavailable,
    ElevationNotAllowed,
    ProcessUnavailable,
    TemporaryDirectoryUnavailable,
}
impl From<VerificationFailure> for InstallerError {
    fn from(error: VerificationFailure) -> Self {
        if error
            == VerificationFailure::Indeterminate(
                crate::authenticode::IndeterminateReason::UnsupportedPlatform,
            )
        {
            Self::UnsupportedPlatform
        } else {
            Self::Verification(error)
        }
    }
}
impl fmt::Display for InstallerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Verification(_) => "Die Installerdatei wurde nicht freigegeben.",
            Self::UnsupportedPlatform => "Der Installerstart benötigt Windows.",
            Self::ClockUnavailable => "Die native Zeit konnte nicht gelesen werden.",
            Self::ClockRollback => "Die native Zeit ist zurückgestellt.",
            Self::InvalidTimeout => "Die Wartezeit überschreitet die zulässige Grenze.",
            Self::LaunchUnavailable => "Der Installer konnte nicht gestartet werden.",
            Self::ElevationNotAllowed => {
                "Der Installerstart unter erhöhten Rechten ist nicht freigegeben."
            }
            Self::ProcessUnavailable => {
                "Der Installerprozess konnte nicht sicher beobachtet werden."
            }
            Self::TemporaryDirectoryUnavailable => {
                "Das temporäre Verzeichnis ist für den Installerstart nicht freigegeben."
            }
        })
    }
}
impl std::error::Error for InstallerError {}

/// Newly verified read-shared installer bytes and protocol1 assertion. This
/// owns the private cache/root guards, exposes no file or path, and grants no
/// installation target, consent, process absence or product mutation authority.
///
/// ```compile_fail
/// use fbforge_suite_core::installers::PreparedInstaller;
/// fn forge() { let _ = PreparedInstaller {}; }
/// ```
/// ```compile_fail
/// use fbforge_suite_core::installers::PreparedInstaller;
/// use std::io::Write;
/// fn modify(value: &mut PreparedInstaller<'_>) { value.write_all(b"changed").unwrap(); }
/// ```
/// ```compile_fail
/// use fbforge_suite_core::{installers::prepare_cached_installer,
///     authenticode::AuthenticodePolicies, network::CancellationToken,
///     storage::{PackageCache, StateStore}, trust::{TrustStore, VerifiedRelease}};
/// fn close_owner(mut cache: PackageCache, store: &mut StateStore,
///     trust: &TrustStore, release: &VerifiedRelease, policies: &AuthenticodePolicies) {
///     let cancel = CancellationToken::default();
///     let mut stage = cache.prepare(store, trust, 300, release.tool_id(),
///         release.version(), &cancel).unwrap();
///     stage.verify_bytes(None, &cancel).unwrap();
///     let package = stage.finish(store, trust, || 300, &cancel).unwrap();
///     let prepared = prepare_cached_installer(package, release, policies, 300);
///     drop(cache);
///     drop(prepared);
/// }
/// ```
/// ```compile_fail
/// use fbforge_suite_core::installers::PreparedInstaller;
/// fn duplicate(value: PreparedInstaller<'_>) { let _copy = value.clone(); }
/// ```
pub struct PreparedInstaller<'cache> {
    proof: VerifiedCachedWindowsPackage<'cache>,
    policy_id: String,
    checked_at: u64,
}
impl fmt::Debug for PreparedInstaller<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PreparedInstaller")
            .finish_non_exhaustive()
    }
}

/// Consume cache bytes, change sharing by same-object handles, and establish
/// entirely new VERSIONINFO/protocol, PE, hash and Windows publisher evidence.
/// No previous publisher proof survives the handle transition. The caller
/// supplies current native Store/trust metadata and active publisher policy.
pub fn prepare_cached_installer<'cache>(
    package: CachedPackage<'cache>,
    release: &VerifiedRelease,
    policies: &AuthenticodePolicies,
    now: u64,
) -> Result<PreparedInstaller<'cache>, InstallerError> {
    let proof = authenticode::prepare_installer_package(package, release, policies, now)?;
    Ok(PreparedInstaller {
        proof,
        policy_id: release.authenticode_policy_id().into(),
        checked_at: now,
    })
}

impl<'cache> PreparedInstaller<'cache> {
    /// Trusted native caller only: revalidate current Store/trust, administrative
    /// update/install policy, consent, target and mutation guards immediately
    /// before this call. `metadata_checked_at` is that native Store clock.
    /// This function reads the native clock again, checks current release/policy
    /// binding, and starts only this held image with fixed `/P` (never `/R`).
    /// It neither accepts CLI paths/arguments nor elevates or uses a shell.
    pub fn start(
        self,
        current: &VerifiedRelease,
        policies: &AuthenticodePolicies,
        metadata_checked_at: u64,
    ) -> Result<InstallerProcess<'cache>, InstallerError> {
        let validate = || {
            let now = native_time()?;
            if now < self.checked_at || now < metadata_checked_at {
                return Err(InstallerError::ClockRollback);
            }
            let policy = authenticode::validate_release_policy(current, policies, now)?;
            if current.metadata_schema_version() != 2
                || !self.proof.matches_release(current)
                || current.authenticode_policy_id() != self.policy_id
                || !policy
                    .certificate_sha256
                    .contains(self.proof.signer_certificate_sha256())
            {
                return Err(VerificationFailure::Invalid(InvalidReason::PackageMismatch).into());
            }
            Ok(())
        };
        validate()?;
        #[cfg(windows)]
        {
            // Repeat the clock/binding checks after final path and launch-buffer
            // setup, immediately before CreateProcessW's side effect.
            let process = windows::start(&self.proof, validate)?;
            Ok(InstallerProcess {
                process,
                _prepared: self,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = (self, current, policies, metadata_checked_at);
            Err(InstallerError::UnsupportedPlatform)
        }
    }
}

/// Observation of this exact child handle, never an installation success.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallerProcessState {
    Running,
    Exited { code: u32 },
}

/// Retains the read-shared source and cache/root guards while owned. Native
/// orchestration must retain this guard until observed exit. Dropping it early
/// detaches observation and releases guards; it NEVER terminates the child and
/// must not be interpreted as completed installation or permission to mutate.
/// ```compile_fail
/// use fbforge_suite_core::installers::InstallerProcess;
/// fn duplicate(value: InstallerProcess<'_>) { let _copy = value.clone(); }
/// ```
pub struct InstallerProcess<'cache> {
    #[cfg(windows)]
    process: windows::InstallerChild,
    _prepared: PreparedInstaller<'cache>,
}
impl fmt::Debug for InstallerProcess<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InstallerProcess")
            .finish_non_exhaustive()
    }
}
impl InstallerProcess<'_> {
    /// Zero is a poll; waits are limited to60s and run on a native worker.
    /// Timeout returns Running without cancellation. Exit259 remains a real
    /// exit code after the process handle was signaled, never an infinite poll.
    pub fn wait(&mut self, timeout: Duration) -> Result<InstallerProcessState, InstallerError> {
        let milliseconds = wait_milliseconds(timeout)?;
        #[cfg(windows)]
        {
            windows::wait(&self.process, milliseconds)
        }
        #[cfg(not(windows))]
        {
            let _ = milliseconds;
            Err(InstallerError::UnsupportedPlatform)
        }
    }
}

fn wait_milliseconds(timeout: Duration) -> Result<u32, InstallerError> {
    let milliseconds =
        timeout.as_millis() + u128::from(!timeout.subsec_nanos().is_multiple_of(1_000_000));
    if milliseconds > 60_000 {
        Err(InstallerError::InvalidTimeout)
    } else {
        Ok(milliseconds as u32)
    }
}

// Match the pinned Unicode NSIS3.13 stub, including its NUL capacity.
#[cfg(any(windows, test))]
const NSIS_TEMP_CAPACITY: usize = 1024;

#[cfg(any(windows, test))]
fn temporary_directory_path(
    buffer: &[u16; NSIS_TEMP_CAPACITY],
    count: u32,
) -> Result<std::path::PathBuf, InstallerError> {
    let denied = InstallerError::TemporaryDirectoryUnavailable;
    let count = count as usize;
    if count == 0
        || count >= NSIS_TEMP_CAPACITY
        || buffer[count] != 0
        || buffer[..count].contains(&0)
    {
        return Err(denied);
    }
    let text = String::from_utf16(&buffer[..count]).map_err(|_| denied)?;
    // NSIS validates/sanitizes this path before use. Accept only an unchanged
    // DOS spelling; a rejected path must never select a different directory.
    let directory = text.strip_suffix('\\').ok_or(denied)?;
    if directory.contains('/')
        || crate::discovery::validate_windows_location(directory).as_deref() != Ok(directory)
    {
        return Err(denied);
    }
    Ok(directory.into())
}

#[cfg(any(windows, test))]
fn system_windows_directory_path(
    buffer: &[u16; NSIS_TEMP_CAPACITY],
    count: u32,
) -> Result<std::path::PathBuf, InstallerError> {
    let count = count as usize;
    if count == 0 || count >= NSIS_TEMP_CAPACITY - 1 || buffer[count] != 0 {
        return Err(InstallerError::TemporaryDirectoryUnavailable);
    }
    // GetSystemWindowsDirectoryW omits the trailing separator. Feed the same
    // bounded DOS validation without accepting an environment-supplied root.
    let mut terminated = *buffer;
    terminated[count] = b'\\' as u16;
    terminated[count + 1] = 0;
    temporary_directory_path(&terminated, count as u32 + 1)
}

fn native_time() -> Result<u64, InstallerError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(|_| InstallerError::ClockUnavailable)
}

#[cfg(test)]
mod temporary_directory_tests {
    use super::*;

    fn result(path: &str) -> Result<std::path::PathBuf, InstallerError> {
        let units: Vec<_> = path.encode_utf16().collect();
        let mut buffer = [0; NSIS_TEMP_CAPACITY];
        buffer[..units.len()].copy_from_slice(&units);
        temporary_directory_path(&buffer, units.len() as u32)
    }

    #[test]
    fn native_temp_path_keeps_normal_private_user_paths_and_utf16_unit_counts() {
        for path in [
            r"C:\Users\Standard\AppData\Local\Temp",
            r"D:\Users\Müller\Temp",
        ] {
            assert_eq!(
                result(&format!("{path}\\")),
                Ok(std::path::PathBuf::from(path))
            );
        }
    }

    #[test]
    fn shared_windows_directory_response_is_bounded_and_does_not_select_temp() {
        let mut buffer = [0; NSIS_TEMP_CAPACITY];
        let directory = r"C:\Windows";
        let units: Vec<_> = directory.encode_utf16().collect();
        buffer[..units.len()].copy_from_slice(&units);
        assert_eq!(
            system_windows_directory_path(&buffer, units.len() as u32),
            Ok(std::path::PathBuf::from(directory))
        );
        for count in [0, NSIS_TEMP_CAPACITY as u32 - 1, u32::MAX] {
            assert_eq!(
                system_windows_directory_path(&buffer, count),
                Err(InstallerError::TemporaryDirectoryUnavailable)
            );
        }
        buffer[3] = 0;
        assert!(system_windows_directory_path(&buffer, units.len() as u32).is_err());
    }

    #[test]
    fn native_temp_path_rejects_gettemppath_errors_truncation_and_invalid_utf16() {
        let mut buffer = [0; NSIS_TEMP_CAPACITY];
        for count in [0, NSIS_TEMP_CAPACITY as u32, u32::MAX] {
            assert_eq!(
                temporary_directory_path(&buffer, count),
                Err(InstallerError::TemporaryDirectoryUnavailable)
            );
        }
        buffer[0] = 0xd800;
        buffer[1] = b'\\' as u16;
        assert!(temporary_directory_path(&buffer, 2).is_err());
        let units: Vec<_> = r"C:\Users\Standard\Temp\".encode_utf16().collect();
        buffer.fill(0);
        buffer[..units.len()].copy_from_slice(&units);
        buffer[units.len()] = 7;
        assert!(temporary_directory_path(&buffer, units.len() as u32).is_err());
        buffer[units.len()] = 0;
        buffer[5] = 0;
        assert!(temporary_directory_path(&buffer, units.len() as u32).is_err());
    }

    #[test]
    fn native_temp_path_cannot_be_silently_rewritten_by_nsis_filename_validation() {
        for path in [
            r"C:\",
            r"C:\Users\..\Temp\",
            r"C:\Temp.\",
            "C:\\Temp \\",
            r"C:\Temp",
            r"\\server\share\Temp\",
            r"\\?\C:\Temp\",
            r"relative\Temp\",
            "C:/Users/Temp/",
            "\"C:\\Temp\\\"",
            "C:\\Te\0mp\\",
        ] {
            assert_eq!(
                result(path),
                Err(InstallerError::TemporaryDirectoryUnavailable),
                "{path:?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waits_are_bounded_and_never_round_a_positive_budget_to_zero() {
        assert_eq!(wait_milliseconds(Duration::ZERO), Ok(0));
        assert_eq!(wait_milliseconds(Duration::from_nanos(1)), Ok(1));
        assert_eq!(wait_milliseconds(Duration::from_secs(60)), Ok(60_000));
        assert_eq!(
            wait_milliseconds(Duration::from_secs(60) + Duration::from_nanos(1)),
            Err(InstallerError::InvalidTimeout)
        );
        assert_eq!(
            wait_milliseconds(Duration::MAX),
            Err(InstallerError::InvalidTimeout)
        );
    }
}
