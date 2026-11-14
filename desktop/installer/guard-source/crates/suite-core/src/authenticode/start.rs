use super::{AuthenticodePolicies, OwnedVerifiedCooperatingProgram, PolicyStatus};
use crate::catalog::{validate_tool, Capability, ToolManifest};
use std::{
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStartOutcome {
    StartRequested,
}

/// Fixed native failures contain neither paths, handles, PIDs nor OS messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramStartFailure {
    ManifestRejected,
    PolicyRejected,
    ClockUnavailable,
    ClockRollback,
    ProgramUnavailable,
    UnsafeInstallation,
    UnsupportedInstallation,
    OperatingSystem,
    UnsupportedPlatform,
}
impl fmt::Display for ProgramStartFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Der native Programmstart konnte nicht bestätigt werden.")
    }
}
impl std::error::Error for ProgramStartFailure {}

pub(super) struct ProgramBinding {
    pub tool_id: String,
    pub policy_id: String,
    pub certificate_sha256: [u8; 32],
    pub verified_at: u64,
}

pub(super) fn validate_binding(
    program: &OwnedVerifiedCooperatingProgram,
    tool: &ToolManifest,
    policies: &AuthenticodePolicies,
) -> Result<(), ProgramStartFailure> {
    validate_tool(tool).map_err(|_| ProgramStartFailure::ManifestRejected)?;
    if !tool.capabilities.contains(&Capability::Launch) {
        return Err(ProgramStartFailure::ManifestRejected);
    }
    let binding = &program.binding;
    let policy = policies
        .policies
        .get(&tool.trust_policy_id)
        .ok_or(ProgramStartFailure::PolicyRejected)?;
    if tool.id != binding.tool_id
        || tool.app_id != program.app_id
        || tool.trust_policy_id != binding.policy_id
        || policy.status != PolicyStatus::Active
        || policy.tool_id != tool.id
        || policy.app_id != tool.app_id
        || !policy
            .certificate_sha256
            .contains(&binding.certificate_sha256)
    {
        return Err(ProgramStartFailure::PolicyRejected);
    }
    Ok(())
}

pub(super) fn check_time(verified_at: u64, now: SystemTime) -> Result<(), ProgramStartFailure> {
    let current = now
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ProgramStartFailure::ClockUnavailable)?
        .as_secs();
    if current < verified_at {
        return Err(ProgramStartFailure::ClockRollback);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn native_clock_after_signature_must_not_move_backwards() {
        assert_eq!(
            check_time(300, UNIX_EPOCH + Duration::from_secs(301)),
            Ok(())
        );
        assert_eq!(
            check_time(300, UNIX_EPOCH + Duration::from_secs(300)),
            Ok(())
        );
        assert_eq!(
            check_time(300, UNIX_EPOCH + Duration::from_secs(299)),
            Err(ProgramStartFailure::ClockRollback)
        );
        assert_eq!(
            check_time(300, UNIX_EPOCH - Duration::from_secs(1)),
            Err(ProgramStartFailure::ClockUnavailable)
        );
    }
}
