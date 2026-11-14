//! A resource-use observation is neither process absence nor mutation authority.
use crate::discovery::validate_windows_location;
use std::collections::BTreeSet;

#[cfg(windows)]
mod windows;

const MAX_RESOURCES: usize = 8192;
const MAX_PATH_UNITS: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceUseIssue {
    InvalidResources,
    UnsupportedPlatform,
    InspectionUnavailable,
    RebootRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceUsers {
    InUse,
    NoUsersReported,
    Unknown(ResourceUseIssue),
}

/// Ask Windows Restart Manager about exactly these native, local file paths.
///
/// The caller must enumerate every affected file and separately retain its path
/// binding, app cooperation and mutation protections. Directories are not
/// recursive resources. Missing files, omitted dependencies, earlier users and
/// future opens are not covered. In particular, `NoUsersReported` must never be
/// converted into global `ProcessStatus::Stopped` or an installation grant.
/// This function never requests application shutdown, restart or termination.
pub fn inspect_resource_users(paths: &[String]) -> ResourceUsers {
    let paths = match encode_resources(paths) {
        Ok(paths) => paths,
        Err(issue) => return ResourceUsers::Unknown(issue),
    };
    #[cfg(windows)]
    {
        windows::inspect(&paths)
    }
    #[cfg(not(windows))]
    {
        let _ = paths;
        ResourceUsers::Unknown(ResourceUseIssue::UnsupportedPlatform)
    }
}

fn encode_resources(paths: &[String]) -> Result<Vec<Vec<u16>>, ResourceUseIssue> {
    if paths.is_empty() || paths.len() > MAX_RESOURCES {
        return Err(ResourceUseIssue::InvalidResources);
    }
    let mut seen = BTreeSet::new();
    let mut units = 0usize;
    let mut encoded = Vec::with_capacity(paths.len());
    for path in paths {
        let normalized =
            validate_windows_location(path).map_err(|_| ResourceUseIssue::InvalidResources)?;
        // No command-line quoting or silent path rewriting at this boundary.
        // This duplicate check is lexical; it does not assert file identity.
        if normalized != *path || !seen.insert(path.replace('/', "\\").to_ascii_lowercase()) {
            return Err(ResourceUseIssue::InvalidResources);
        }
        let value: Vec<_> = path.encode_utf16().chain(Some(0)).collect();
        units = units
            .checked_add(value.len())
            .ok_or(ResourceUseIssue::InvalidResources)?;
        if units > MAX_PATH_UNITS {
            return Err(ResourceUseIssue::InvalidResources);
        }
        encoded.push(value);
    }
    Ok(encoded)
}

#[cfg(any(windows, test))]
fn classify_query(status: u32, needed: u32, returned: u32, reboot: u32) -> ResourceUsers {
    // ERROR_MORE_DATA is already a positive conflict. We do not need names,
    // PIDs or another fallible, racing allocation to tell the user to wait.
    if (status == 0 || status == 234) && (needed > 0 || returned > 0) {
        return ResourceUsers::InUse;
    }
    if status != 0 {
        return ResourceUsers::Unknown(ResourceUseIssue::InspectionUnavailable);
    }
    if reboot != 0 {
        return ResourceUsers::Unknown(ResourceUseIssue::RebootRequired);
    }
    ResourceUsers::NoUsersReported
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_truncated_or_reboot_queries_cannot_report_unused() {
        for status in [5, 121, 234, 1223, u32::MAX] {
            assert_eq!(
                classify_query(status, 0, 0, 0),
                ResourceUsers::Unknown(ResourceUseIssue::InspectionUnavailable)
            );
        }
        assert_eq!(
            classify_query(0, 0, 0, 1),
            ResourceUsers::Unknown(ResourceUseIssue::RebootRequired)
        );
        assert_eq!(classify_query(0, 0, 0, 0), ResourceUsers::NoUsersReported);
    }

    #[test]
    fn an_undersized_process_list_is_already_a_positive_conflict() {
        for (status, needed, returned) in [(234, 1, 0), (0, 1, 0), (0, 0, 1), (234, 2, 0)] {
            assert_eq!(
                classify_query(status, needed, returned, 0),
                ResourceUsers::InUse
            );
        }
    }

    #[test]
    fn resource_budget_counts_utf16_units_including_terminators() {
        let path = format!("C:\\{}\\x.exe", vec!["ä".repeat(120); 12].join("\\"));
        assert!(validate_windows_location(&path).is_ok());
        let paths: Vec<_> = (0..3000).map(|i| format!("{path}.{i}")).collect();
        assert!(paths
            .iter()
            .all(|path| validate_windows_location(path).is_ok()));
        assert_eq!(
            encode_resources(&paths),
            Err(ResourceUseIssue::InvalidResources)
        );
    }
}
