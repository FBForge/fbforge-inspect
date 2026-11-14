//! Native, cached-only Windows publisher verification. Evidence is not an
//! execution authorization and deliberately retains its checked file handle.
use crate::{
    catalog::{is_identifier, validate_tool, ToolManifest},
    storage::CachedPackage,
    trust::VerifiedRelease,
};
use std::{collections::BTreeMap, fmt, path::Path};

#[cfg(any(windows, test))]
mod cooperation;
#[cfg(any(windows, test))]
mod installer_protocol;
#[cfg(any(windows, test))]
mod program_permissions;
mod start;
pub use start::{ProgramStartFailure, ProgramStartOutcome};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::installer_targets_bridge;

#[cfg(windows)]
pub(crate) use windows::InstallerTemporaryDirectory;

/// Structural data only. Native target code must authenticate this same held
/// x64 file separately; this reader creates no publisher or installation proof.
#[cfg(any(windows, test))]
pub(crate) fn read_program_product_version(
    file: &mut (impl std::io::Read + std::io::Seek),
) -> Result<semver::Version, VerificationFailure> {
    cooperation::read_program_product_version(file)
}

/// Parsed section-backed data only; the caller retains the same-file signature
/// binding. This is not publisher evidence or a mutation grant.
#[cfg(any(windows, test))]
#[derive(Debug)]
pub(crate) struct DeclaredInstallerIdentity {
    app_id: String,
    version: semver::Version,
    payload_manifest_sha256: Option<[u8; 32]>,
}
#[cfg(any(windows, test))]
impl DeclaredInstallerIdentity {
    pub(crate) fn app_id(&self) -> &str {
        &self.app_id
    }
    pub(crate) fn version(&self) -> &semver::Version {
        &self.version
    }
    pub(crate) fn payload_manifest_sha256(&self) -> Option<&[u8; 32]> {
        self.payload_manifest_sha256.as_ref()
    }
    pub(crate) fn protocol(&self) -> u16 {
        1
    }
}

#[cfg(any(windows, test))]
pub(crate) fn read_installer_identity(
    file: &mut (impl std::io::Read + std::io::Seek),
) -> Result<DeclaredInstallerIdentity, VerificationFailure> {
    cooperation::read_installer_identity(file)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyStatus {
    Active,
    Revoked,
}

/// Local configuration only. Metadata can name a policy, never provision it.
#[derive(Debug)]
pub struct AuthenticodePolicy {
    pub id: String,
    pub tool_id: String,
    pub app_id: String,
    pub certificate_sha256: Vec<[u8; 32]>,
    pub status: PolicyStatus,
}

#[derive(Debug)]
pub struct AuthenticodePolicies {
    policies: BTreeMap<String, AuthenticodePolicy>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    InvalidConfiguration,
}

impl AuthenticodePolicies {
    pub fn new(policies: Vec<AuthenticodePolicy>) -> Result<Self, PolicyError> {
        if policies.len() > 128 {
            return Err(PolicyError::InvalidConfiguration);
        }
        let mut entries = BTreeMap::new();
        for policy in policies {
            let pins = &policy.certificate_sha256;
            if !is_identifier(&policy.id)
                || !is_identifier(&policy.tool_id)
                || !is_identifier(&policy.app_id)
                || pins.is_empty()
                || pins.len() > 32
                || pins.contains(&[0; 32])
                || pins.iter().collect::<std::collections::HashSet<_>>().len() != pins.len()
                || entries.contains_key(&policy.id)
            {
                return Err(PolicyError::InvalidConfiguration);
            }
            entries.insert(policy.id.clone(), policy);
        }
        Ok(Self { policies: entries })
    }

    pub fn policy_for_release(
        &self,
        release: &VerifiedRelease,
    ) -> Result<&AuthenticodePolicy, VerificationFailure> {
        let policy = self.policies.get(release.authenticode_policy_id()).ok_or(
            VerificationFailure::Indeterminate(IndeterminateReason::PolicyUnavailable),
        )?;
        if policy.status != PolicyStatus::Active
            || policy.tool_id != release.tool_id()
            || policy.app_id != release.app_id()
        {
            return Err(VerificationFailure::Invalid(InvalidReason::PolicyRejected));
        }
        Ok(policy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidReason {
    PolicyRejected,
    InvalidManifest,
    CooperationMissing,
    CooperationRejected,
    MetadataExpired,
    WithdrawnRelease,
    UnsafeFile,
    InvalidImage,
    PackageMismatch,
    Unsigned,
    SignatureRejected,
    SignerMismatch,
    Revoked,
    TimestampRejected,
    ChainRejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndeterminateReason {
    UnsupportedPlatform,
    PolicyUnavailable,
    PackageUnavailable,
    ProviderUnavailable,
    ProviderStateUnavailable,
    RevocationUnavailable,
    ChainUnavailable,
    UnknownWindowsStatus,
}

/// Fixed reason codes contain no filesystem paths, certificate text or OS errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerificationFailure {
    Invalid(InvalidReason),
    Indeterminate(IndeterminateReason),
}

impl fmt::Display for VerificationFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid(_) => "Die Windows-Paketprüfung hat das Paket abgewiesen.",
            Self::Indeterminate(_) => "Die Windows-Paketprüfung konnte nicht abgeschlossen werden.",
        })
    }
}
impl std::error::Error for VerificationFailure {}

/// This value cannot be deserialized or constructed from frontend assertions.
/// The retained handle protects the verified file during this value's lifetime.
/// It does not establish that a future path-based launch selects this file.
pub struct VerifiedWindowsPackage {
    #[cfg(windows)]
    _package: windows::LockedPackageFile,
    sha256: String,
    byte_count: u64,
    signer_certificate_sha256: [u8; 32],
}

impl fmt::Debug for VerifiedWindowsPackage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedWindowsPackage")
            .field("byte_count", &self.byte_count)
            .finish_non_exhaustive()
    }
}

impl VerifiedWindowsPackage {
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    pub fn byte_count(&self) -> u64 {
        self.byte_count
    }

    pub fn signer_certificate_sha256(&self) -> &[u8; 32] {
        &self.signer_certificate_sha256
    }
}

/// Verify current release evidence and package bytes before accepting a Windows
/// publisher. Requires a validated timestamp and cached revocation evidence.
/// Only primary embedded signatures and fixed local drives are supported.
/// Timestamped signatures may remain valid after publisher-certificate expiry.
/// No network retrieval, certificate installation or process launch is allowed.
/// The caller must obtain `release` again with current native metadata rules.
pub fn verify_package(
    path: &Path,
    release: &VerifiedRelease,
    policies: &AuthenticodePolicies,
    now: u64,
) -> Result<VerifiedWindowsPackage, VerificationFailure> {
    let policy = validate_release_policy(release, policies, now)?;
    #[cfg(windows)]
    {
        windows::verify_package(path, release, policy, now)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, policy);
        Err(VerificationFailure::Indeterminate(
            IndeterminateReason::UnsupportedPlatform,
        ))
    }
}

pub(crate) fn validate_release_policy<'a>(
    release: &VerifiedRelease,
    policies: &'a AuthenticodePolicies,
    now: u64,
) -> Result<&'a AuthenticodePolicy, VerificationFailure> {
    let policy = policies.policy_for_release(release)?;
    if now >= release.expires_at() {
        return Err(VerificationFailure::Invalid(InvalidReason::MetadataExpired));
    }
    if !release.is_available() {
        return Err(VerificationFailure::Invalid(
            InvalidReason::WithdrawnRelease,
        ));
    }
    Ok(policy)
}

/// Publisher evidence that owns the original private cache guard. Its file,
/// source directories and OS locks stay held until this proof is dropped.
/// This is not an execution grant or a claim of future metadata freshness.
///
/// ```compile_fail
/// use fbforge_suite_core::authenticode::VerifiedCachedWindowsPackage;
/// use std::io::Write;
/// fn overwrite(package: &mut VerifiedCachedWindowsPackage<'_>) {
///     package.write_all(b"changed").unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use fbforge_suite_core::{
///     authenticode::{verify_cached_package, AuthenticodePolicies},
///     network::CancellationToken, storage::StateStore,
///     trust::{TrustStore, VerifiedRelease},
/// };
/// fn close_owner(mut store: StateStore, trust: &TrustStore,
///     release: &VerifiedRelease, policies: &AuthenticodePolicies) {
///     let mut cache = store.package_cache(Default::default()).unwrap();
///     let cancel = CancellationToken::default();
///     let mut stage = cache.prepare(&mut store, trust, 300,
///         release.tool_id(), release.version(), &cancel).unwrap();
///     stage.verify_bytes(None, &cancel).unwrap();
///     let package = stage.finish(&mut store, trust, || 300, &cancel).unwrap();
///     let _proof = verify_cached_package(package, release, policies, 300).unwrap();
///     drop(cache);
/// }
/// ```
pub struct VerifiedCachedWindowsPackage<'cache> {
    _package: CachedPackage<'cache>,
    sha256: String,
    byte_count: u64,
    signer_certificate_sha256: [u8; 32],
}

impl fmt::Debug for VerifiedCachedWindowsPackage<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedCachedWindowsPackage")
            .field("byte_count", &self.byte_count)
            .finish_non_exhaustive()
    }
}
impl VerifiedCachedWindowsPackage<'_> {
    pub(crate) fn matches_release(&self, release: &VerifiedRelease) -> bool {
        self._package.matches_release(release)
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub fn byte_count(&self) -> u64 {
        self.byte_count
    }
    pub fn signer_certificate_sha256(&self) -> &[u8; 32] {
        &self.signer_certificate_sha256
    }
}

pub(crate) fn prepare_installer_package<'cache>(
    mut package: CachedPackage<'cache>,
    release: &VerifiedRelease,
    policies: &AuthenticodePolicies,
    now: u64,
) -> Result<VerifiedCachedWindowsPackage<'cache>, VerificationFailure> {
    let policy = validate_release_policy(release, policies, now)?;
    if release.metadata_schema_version() != 2 || !package.matches_release(release) {
        return Err(VerificationFailure::Invalid(InvalidReason::PackageMismatch));
    }
    #[cfg(windows)]
    {
        package
            .prepare_installer_read()
            .map_err(|error| match error {
                crate::storage::PackageCacheError::UnsafePath => {
                    VerificationFailure::Invalid(InvalidReason::UnsafeFile)
                }
                crate::storage::PackageCacheError::Corrupt => {
                    VerificationFailure::Invalid(InvalidReason::PackageMismatch)
                }
                _ => VerificationFailure::Indeterminate(IndeterminateReason::PackageUnavailable),
            })?;
        windows::prepare_installer_package(package, release, policy, now)
    }
    #[cfg(not(windows))]
    {
        let _ = (&mut package, policy);
        Err(VerificationFailure::Indeterminate(
            IndeterminateReason::UnsupportedPlatform,
        ))
    }
}

#[cfg(windows)]
pub(crate) fn installer_path(
    proof: &VerifiedCachedWindowsPackage<'_>,
) -> Result<Vec<u16>, VerificationFailure> {
    windows::checked_installer_path(proof._package.file_ref())
}

/// Verify the existing held cache file without reopening a name or relaxing its
/// sharing restrictions. Obtain `release` from current native trust/time first,
/// then release the StateStore borrow before this long hash/WinTrust operation.
/// Metadata, clock, withdrawal and local policy must be checked again immediately
/// before any later execution; this evidence alone never authorizes execution.
pub fn verify_cached_package<'cache>(
    package: CachedPackage<'cache>,
    release: &VerifiedRelease,
    policies: &AuthenticodePolicies,
    now: u64,
) -> Result<VerifiedCachedWindowsPackage<'cache>, VerificationFailure> {
    let policy = validate_release_policy(release, policies, now)?;
    if !package.matches_release(release) {
        return Err(VerificationFailure::Invalid(InvalidReason::PackageMismatch));
    }
    #[cfg(windows)]
    {
        windows::verify_cached_package(package, release, policy, now)
    }
    #[cfg(not(windows))]
    {
        let _ = policy;
        Err(VerificationFailure::Indeterminate(
            IndeterminateReason::UnsupportedPlatform,
        ))
    }
}

/// Publisher-bound assertion of coordination protocol 1 for one installed x64
/// program. Owns the read-only locked file and its retained ancestor handles.
/// No process identity, external JAR, application version, Stopped state or
/// execution authorization follows from this assertion. Recheck current native
/// policy/time/metadata and coordination/process gates before later mutations.
///
/// ```compile_fail
/// use fbforge_suite_core::authenticode::OwnedVerifiedCooperatingProgram;
/// use std::io::Write;
/// fn overwrite(proof: &mut OwnedVerifiedCooperatingProgram) {
///     proof.write_all(b"changed").unwrap();
/// }
/// ```
///
/// ```compile_fail
/// use fbforge_suite_core::authenticode::OwnedVerifiedCooperatingProgram;
/// fn forged() -> OwnedVerifiedCooperatingProgram {
///     OwnedVerifiedCooperatingProgram { app_id: "de.fake".into() }
/// }
/// ```
pub struct OwnedVerifiedCooperatingProgram {
    #[cfg(windows)]
    _program: windows::LockedPackageFile,
    app_id: String,
    binding: start::ProgramBinding,
}
impl fmt::Debug for OwnedVerifiedCooperatingProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedVerifiedCooperatingProgram")
            .field("app_id", &self.app_id)
            .field("protocol", &1u16)
            .finish_non_exhaustive()
    }
}
impl OwnedVerifiedCooperatingProgram {
    pub fn app_id(&self) -> &str {
        &self.app_id
    }
    pub fn protocol(&self) -> u16 {
        1
    }

    /// Check and retain the existing installation tree before the caller's fresh
    /// native manifest/policy/process checks. No runtime version is authenticated.
    ///
    /// ```compile_fail
    /// use fbforge_suite_core::{authenticode::{OwnedVerifiedCooperatingProgram, AuthenticodePolicies}, catalog::ToolManifest};
    /// fn skip_tree_check(proof: OwnedVerifiedCooperatingProgram, tool: &ToolManifest, policies: &AuthenticodePolicies) {
    ///     proof.request_start(tool, policies);
    /// }
    /// ```
    pub fn prepare_start(self) -> Result<PreparedProgramStart, ProgramStartFailure> {
        #[cfg(windows)]
        {
            windows::prepare_program_start(self)
        }
        #[cfg(not(windows))]
        {
            Err(ProgramStartFailure::UnsupportedPlatform)
        }
    }
}

/// One immediate start preparation. The private owner retains the verified EXE,
/// its ancestors and every checked installation object until process creation.
/// Neither paths nor proof setters are exposed.
pub struct PreparedProgramStart {
    program: OwnedVerifiedCooperatingProgram,
    #[cfg(windows)]
    _tree: windows::LockedProgramTree,
}
impl fmt::Debug for PreparedProgramStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreparedProgramStart")
            .finish_non_exhaustive()
    }
}
impl PreparedProgramStart {
    /// Consume this held-file preparation for one explicit native start request.
    /// The caller must freshly resolve the installation/current native manifest
    /// and release its same-thread cooperation lease immediately before this call.
    /// No process absence, Ready state, metadata version or installation authority
    /// follows. The child must reacquire the app lease before application startup.
    ///
    /// ```compile_fail
    /// use fbforge_suite_core::{authenticode::{PreparedProgramStart, AuthenticodePolicies}, catalog::ToolManifest};
    /// fn select_another_path(proof: PreparedProgramStart, tool: &ToolManifest, policies: &AuthenticodePolicies) {
    ///     proof.request_start(tool, policies, std::path::Path::new("C:/other.exe"));
    /// }
    /// ```
    pub fn request_start(
        self,
        tool: &ToolManifest,
        policies: &AuthenticodePolicies,
    ) -> Result<ProgramStartOutcome, ProgramStartFailure> {
        start::validate_binding(&self.program, tool, policies)?;
        start::check_time(
            self.program.binding.verified_at,
            std::time::SystemTime::now(),
        )?;
        #[cfg(windows)]
        {
            windows::request_program_start(self, tool)
        }
        #[cfg(not(windows))]
        {
            Err(ProgramStartFailure::UnsupportedPlatform)
        }
    }
}

/// Verify an installed x64 program on one held file, without loading code or
/// reopening its path. Only a structurally validated native manifest and an
/// active locally provisioned tool/app/policy binding are accepted. The caller
/// must source that manifest from its current trusted native catalogue.
/// Timestamp, publisher pin and cached revocation rules equal package checks.
/// This potentially long operation is not a freshness or execution grant.
pub fn verify_cooperating_program(
    path: &Path,
    tool: &ToolManifest,
    policies: &AuthenticodePolicies,
    now: u64,
) -> Result<OwnedVerifiedCooperatingProgram, VerificationFailure> {
    validate_tool(tool)
        .map_err(|_| VerificationFailure::Invalid(InvalidReason::InvalidManifest))?;
    let policy =
        policies
            .policies
            .get(&tool.trust_policy_id)
            .ok_or(VerificationFailure::Indeterminate(
                IndeterminateReason::PolicyUnavailable,
            ))?;
    if policy.status != PolicyStatus::Active
        || policy.tool_id != tool.id
        || policy.app_id != tool.app_id
    {
        return Err(VerificationFailure::Invalid(InvalidReason::PolicyRejected));
    }
    #[cfg(windows)]
    {
        windows::verify_cooperating_program(path, tool, policy, now)
    }
    #[cfg(not(windows))]
    {
        let _ = (path, now);
        Err(VerificationFailure::Indeterminate(
            IndeterminateReason::UnsupportedPlatform,
        ))
    }
}
