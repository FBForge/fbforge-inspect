//! Read-only installation discovery. Observations never authorize execution.
mod validation;
pub use validation::{
    inspect_windows_installer_image, inspect_windows_x64_image, validate_windows_location,
    DetectionFailure,
};
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use windows::read_registered_current_user_root;

use crate::{
    catalog::{parse_catalog, ToolManifest},
    inventory::{DetectionIssue, DetectionReport},
};
use serde_json::json;

/// Scan only the names in a structurally valid built-in or verified tool record.
/// A failed scan must remain distinguishable from confirmed absence.
pub fn discover_tool(tool: &ToolManifest) -> DetectionReport {
    if parse_catalog(&json!({"schemaVersion":1,"tools":[tool]}).to_string()).is_err() {
        return DetectionReport {
            installations: vec![],
            issues: vec![DetectionIssue::InvalidManifest],
        };
    }
    #[cfg(windows)]
    {
        windows::discover_tool(tool)
    }
    #[cfg(not(windows))]
    {
        DetectionReport {
            installations: vec![],
            issues: vec![DetectionIssue::UnsupportedPlatform],
        }
    }
}
