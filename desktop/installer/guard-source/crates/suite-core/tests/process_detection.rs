use fbforge_suite_core::{
    catalog::{parse_catalog, ToolManifest},
    inventory::{Installation, InstallationCondition, RegistryHive, RegistrySource, RegistryView},
    operations::ProcessStatus,
    processes::{activate_existing, inspect_processes, ActivationOutcome, ProcessIssue},
};
use std::path::PathBuf;

fn target() -> (ToolManifest, Installation) {
    let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let tool = catalog.tools()[0].clone();
    let installation = Installation {
        root: PathBuf::from(r"C:\Tools\Connect"),
        executable: Some(PathBuf::from(r"C:\Tools\Connect\fbforge-connect.exe")),
        reported_version: Some("0.3.0".into()),
        version: Some("0.3.0".parse().unwrap()),
        condition: InstallationCondition::Detected,
        sources: vec![RegistrySource {
            hive: RegistryHive::CurrentUser,
            view: RegistryView::Registry64,
            key_name: "FBForge Connect".into(),
        }],
    };
    (tool, installation)
}

fn assert_rejected(tool: &ToolManifest, installation: &Installation, issue: ProcessIssue) {
    let report = inspect_processes(tool, installation);
    assert_eq!(report.status, ProcessStatus::Unknown);
    assert_eq!(report.matched_processes, 0);
    assert_eq!(report.issues, [issue]);
    assert_eq!(
        activate_existing(tool, installation),
        ActivationOutcome::Unknown(issue)
    );
}

#[test]
fn process_entry_points_revalidate_the_manifest_before_native_access() {
    let (mut tool, installation) = target();
    tool.executables = vec![r"..\other.exe".into()];
    assert_rejected(&tool, &installation, ProcessIssue::InvalidManifest);
}

#[test]
fn remembered_or_damaged_installations_are_not_current_process_targets() {
    for condition in [
        InstallationCondition::Unconfirmed,
        InstallationCondition::ConflictingMetadata,
    ] {
        let (tool, mut installation) = target();
        installation.condition = condition;
        assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
    }
}

#[test]
fn unsafe_paths_and_programs_outside_the_manifest_are_rejected() {
    for executable in [
        r"\\server\share\fbforge-connect.exe",
        r"C:\Tools\Other\fbforge-connect.exe",
        r"C:\Tools\Connect\javaw.exe",
    ] {
        let (tool, mut installation) = target();
        installation.executable = Some(PathBuf::from(executable));
        assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
    }
    let (tool, mut installation) = target();
    installation.root = PathBuf::from(r"C:\Tools\..\Connect");
    assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
}

#[test]
fn inconsistent_versions_and_unrelated_registry_sources_are_rejected() {
    let (tool, mut installation) = target();
    installation.reported_version = Some("0.4.0".into());
    assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
    let (tool, mut installation) = target();
    installation.sources[0].key_name = "Unrelated Product".into();
    assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
}

#[test]
fn missing_program_version_or_native_source_cannot_form_a_process_target() {
    let (tool, mut installation) = target();
    installation.executable = None;
    assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
    let (tool, mut installation) = target();
    installation.version = None;
    installation.reported_version = None;
    assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
    let (tool, mut installation) = target();
    installation.sources.clear();
    assert_rejected(&tool, &installation, ProcessIssue::InvalidInstallation);
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_never_becomes_stopped_or_an_activation_success() {
    let (tool, installation) = target();
    assert_rejected(&tool, &installation, ProcessIssue::UnsupportedPlatform);
}
