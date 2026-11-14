//! Planning decisions are not execution authorizations. The native installer must
//! recheck evidence while holding its process-wide lock immediately before use.
use semver::Version;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureStatus {
    Unchecked,
    Checking,
    Trusted,
    Invalid,
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessStatus {
    Stopped,
    Running,
    Busy,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseStatus {
    Available,
    Withdrawn,
    Unknown,
}

pub struct Preconditions {
    pub signature: SignatureStatus,
    pub process: ProcessStatus,
    pub package_identity_matches: bool,
    pub has_verified_bytes: bool,
    pub has_install_rights: bool,
    pub has_disk_space: bool,
    pub has_exclusive_lock: bool,
    pub release: ReleaseStatus,
    pub is_allowed_by_policy: bool,
    pub is_platform_supported: bool,
    pub is_suite_compatible: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstallDecision {
    Ready,
    WaitingForTool,
    Blocked(BlockReason),
}

#[derive(Debug, PartialEq, Eq)]
pub enum BlockReason {
    Trust,
    PackageIdentity,
    Integrity,
    Rights,
    DiskSpace,
    ConcurrentOperation,
    ReleaseAvailability,
    AdministrativePolicy,
    UnsupportedPlatform,
    SuiteVersion,
}

pub fn evaluate_installation(evidence: &Preconditions) -> InstallDecision {
    let checks = [
        (
            evidence.signature == SignatureStatus::Trusted,
            BlockReason::Trust,
        ),
        (evidence.has_verified_bytes, BlockReason::Integrity),
        (
            evidence.package_identity_matches,
            BlockReason::PackageIdentity,
        ),
        (
            evidence.release == ReleaseStatus::Available,
            BlockReason::ReleaseAvailability,
        ),
        (
            evidence.is_allowed_by_policy,
            BlockReason::AdministrativePolicy,
        ),
        (
            evidence.is_platform_supported,
            BlockReason::UnsupportedPlatform,
        ),
        (evidence.is_suite_compatible, BlockReason::SuiteVersion),
        (evidence.has_install_rights, BlockReason::Rights),
        (evidence.has_disk_space, BlockReason::DiskSpace),
        (
            evidence.has_exclusive_lock,
            BlockReason::ConcurrentOperation,
        ),
    ];
    for (passes, reason) in checks {
        if !passes {
            return InstallDecision::Blocked(reason);
        }
    }
    if evidence.process != ProcessStatus::Stopped {
        return InstallDecision::WaitingForTool;
    }
    InstallDecision::Ready
}

pub fn has_stable_update(installed: &Version, available: &Version) -> bool {
    available.pre.is_empty() && available.cmp_precedence(installed).is_gt()
}

pub fn can_repair_version(installed: &Version, available: &Version) -> bool {
    available.pre.is_empty() && available.cmp_precedence(installed).is_ge()
}
