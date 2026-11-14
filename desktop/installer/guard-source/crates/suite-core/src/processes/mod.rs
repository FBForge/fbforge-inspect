//! Native process observations and explicit activation of existing windows.
//! These observations identify neither trusted executing bytes nor an exclusive
//! installation window. They never authorize launch, termination or installation.
mod resources;
#[cfg(windows)]
mod windows;
pub use resources::{inspect_resource_users, ResourceUseIssue, ResourceUsers};

use crate::{
    catalog::{parse_catalog, ToolManifest},
    inventory::{DetectionReport, Installation, InstallationCondition, Inventory},
    operations::ProcessStatus,
};
use serde_json::json;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessIssue {
    InvalidManifest,
    InvalidInstallation,
    InstallationNotCurrent,
    UnsupportedPlatform,
    EnumerationUnavailable,
    EnumerationLimit,
    ProcessUnavailable,
    PathUnavailable,
    IncompleteCoverage,
    ProcessChanged,
    WindowEnumerationUnavailable,
    WindowUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessReport {
    pub status: ProcessStatus,
    pub matched_processes: usize,
    pub issues: Vec<ProcessIssue>,
}

impl ProcessReport {
    fn unknown(issue: ProcessIssue) -> Self {
        Self {
            status: ProcessStatus::Unknown,
            matched_processes: 0,
            issues: vec![issue],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationOutcome {
    Activated,
    ForegroundDenied,
    NoWindow,
    Ambiguous,
    Unknown(ProcessIssue),
}

/// Positive path matches can establish Running. Missing matches cannot establish
/// Stopped: renamed binaries, hosted runtimes and older updaters are not covered
/// by the current manifest contract. Busy needs a separate cooperating-app signal.
pub fn inspect_processes(tool: &ToolManifest, installation: &Installation) -> ProcessReport {
    if let Err(issue) = validate_target(tool, installation) {
        return ProcessReport::unknown(issue);
    }
    #[cfg(windows)]
    {
        windows::inspect_processes(tool, installation)
    }
    #[cfg(not(windows))]
    {
        ProcessReport::unknown(ProcessIssue::UnsupportedPlatform)
    }
}

/// Call only for an explicit user request. The target is re-discovered and its
/// processes/windows are resolved inside this call, never from renderer handles.
/// A denied foreground request does not cause a retry, forced focus or new launch.
pub fn activate_existing(tool: &ToolManifest, installation: &Installation) -> ActivationOutcome {
    if let Err(issue) = validate_target(tool, installation) {
        return ActivationOutcome::Unknown(issue);
    }
    #[cfg(windows)]
    {
        windows::activate_existing(tool, installation)
    }
    #[cfg(not(windows))]
    {
        ActivationOutcome::Unknown(ProcessIssue::UnsupportedPlatform)
    }
}

fn validate_target(tool: &ToolManifest, installation: &Installation) -> Result<(), ProcessIssue> {
    let catalog = parse_catalog(&json!({"schemaVersion":1,"tools":[tool]}).to_string())
        .map_err(|_| ProcessIssue::InvalidManifest)?;
    if installation.condition != InstallationCondition::Detected
        || installation.executable.is_none()
        || installation.version.is_none()
    {
        return Err(ProcessIssue::InvalidInstallation);
    }
    // Reuse the native snapshot contract for local paths, program membership,
    // registry sources and consistent reported/parsed versions before any OS access.
    let mut inventory =
        Inventory::from_catalog(&catalog).map_err(|_| ProcessIssue::InvalidManifest)?;
    inventory
        .record_detection(
            &tool.id,
            DetectionReport {
                installations: vec![installation.clone()],
                issues: vec![],
            },
        )
        .map_err(|_| ProcessIssue::InvalidInstallation)?;
    inventory
        .snapshot_json()
        .map_err(|_| ProcessIssue::InvalidInstallation)?;
    Ok(())
}
