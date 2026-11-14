use fbforge_suite_core::{catalog::parse_catalog, inventory::*};
use semver::Version;
use serde_json::{json, Value};
use std::path::PathBuf;

fn catalog() -> Value {
    serde_json::from_str(include_str!("../../../catalog/tools.v1.json")).unwrap()
}

fn detected(root: &str) -> Installation {
    Installation {
        root: PathBuf::from(root),
        executable: Some(PathBuf::from(root).join("fbforge-connect.exe")),
        reported_version: Some("0.3.0".into()),
        version: Some(Version::parse("0.3.0").unwrap()),
        condition: InstallationCondition::Detected,
        sources: vec![RegistrySource {
            hive: RegistryHive::CurrentUser,
            view: RegistryView::Registry64,
            key_name: "FIT-Connect Test-Client".into(),
        }],
    }
}

fn inventory() -> Inventory {
    Inventory::from_catalog(&parse_catalog(&catalog().to_string()).unwrap()).unwrap()
}

#[test]
fn combines_registry_sources_for_one_root_and_rejects_conflicting_versions() {
    let first = detected("C:/Tools/Connect");
    let mut second = detected("C:/Tools/Connect");
    second.sources[0].hive = RegistryHive::LocalMachine;
    let mut report = DetectionReport {
        installations: vec![],
        issues: vec![],
    };
    report.record_installation(first);
    report.record_installation(second.clone());
    assert_eq!(report.installations.len(), 1);
    assert_eq!(report.installations[0].sources.len(), 2);
    second.version = Some(Version::parse("0.2.0").unwrap());
    second.reported_version = Some("0.2.0".into());
    report.record_installation(second);
    assert!(report.installations[0].version.is_none());
    assert!(report.installations[0].executable.is_none());
    assert_eq!(
        report.installations[0].condition,
        InstallationCondition::ConflictingMetadata
    );
    assert_eq!(report.installations[0].sources.len(), 2);
}

#[test]
fn keeps_distinct_case_sensitive_installation_roots_separate() {
    let mut report = DetectionReport {
        installations: vec![],
        issues: vec![],
    };
    report.record_installation(detected("C:/Tools/Connect"));
    report.record_installation(detected("C:/Tools/connect"));
    assert_eq!(report.installations.len(), 2);
}

#[test]
fn only_offers_installation_when_the_tool_supports_it() {
    let mut input = catalog();
    input["tools"][0]["capabilities"] = json!(["launch"]);
    let mut inventory =
        Inventory::from_catalog(&parse_catalog(&input.to_string()).unwrap()).unwrap();
    assert!(inventory.get("fbforge-connect").unwrap().is_listed());
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![],
                issues: vec![],
            },
        )
        .unwrap();
    assert!(!inventory
        .get("fbforge-connect")
        .unwrap()
        .can_offer_installation());
}

#[test]
fn retains_legacy_detection_rules_when_a_listed_tool_changes_its_package_name() {
    let mut inventory = inventory();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect")],
                issues: vec![],
            },
        )
        .unwrap();
    let mut next = catalog();
    next["tools"][0]["executables"] = json!(["fbforge-connect-next.exe"]);
    next["tools"][0]["registryNames"] = json!(["FBForge Connect Next"]);
    inventory
        .apply_catalog(&parse_catalog(&next.to_string()).unwrap())
        .unwrap();
    let tool = inventory.get("fbforge-connect").unwrap().manifest();
    assert!(tool
        .executables
        .iter()
        .any(|name| name == "fbforge-connect.exe"));
    assert!(tool
        .registry_names
        .iter()
        .any(|name| name == "FIT-Connect Test-Client"));
}

#[test]
fn distinguishes_not_scanned_from_confirmed_absence_and_unavailable_detection() {
    let mut inventory = inventory();
    assert!(!inventory
        .get("fbforge-connect")
        .unwrap()
        .can_offer_installation());
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![],
                issues: vec![DetectionIssue::RegistryUnavailable],
            },
        )
        .unwrap();
    assert!(!inventory
        .get("fbforge-connect")
        .unwrap()
        .can_offer_installation());
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![],
                issues: vec![],
            },
        )
        .unwrap();
    assert!(inventory
        .get("fbforge-connect")
        .unwrap()
        .can_offer_installation());
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect")],
                issues: vec![],
            },
        )
        .unwrap();
    assert!(!inventory
        .get("fbforge-connect")
        .unwrap()
        .can_offer_installation());
}

#[test]
fn retains_removed_installed_tools_and_their_last_launch_information() {
    let mut inventory = inventory();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect")],
                issues: vec![],
            },
        )
        .unwrap();
    inventory
        .apply_catalog(&parse_catalog(r#"{"schemaVersion":1,"tools":[]}"#).unwrap())
        .unwrap();
    let entry = inventory.get("fbforge-connect").unwrap();
    assert!(!entry.is_listed());
    assert_eq!(entry.manifest().product_name, "FBForge Connect");
    assert_eq!(
        entry.detection().installations()[0].executable,
        Some(PathBuf::from("C:/Tools/Connect/fbforge-connect.exe"))
    );
    assert!(!entry.can_offer_installation());
    // Confirmed absence removes a remembered tool, while uncertain detection retains it.
    inventory
        .record_detection(
            "fbforge-inspect",
            DetectionReport {
                installations: vec![],
                issues: vec![DetectionIssue::RegistryUnavailable],
            },
        )
        .unwrap();
    assert!(inventory.get("fbforge-inspect").is_some());
    inventory
        .record_detection(
            "fbforge-inspect",
            DetectionReport {
                installations: vec![],
                issues: vec![],
            },
        )
        .unwrap();
    assert!(inventory.get("fbforge-inspect").is_none());
}

#[test]
fn failed_rescan_preserves_prior_installations_as_unconfirmed_then_recovers() {
    let mut inventory = inventory();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect")],
                issues: vec![],
            },
        )
        .unwrap();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![],
                issues: vec![DetectionIssue::RegistryUnavailable],
            },
        )
        .unwrap();
    let installations = inventory
        .get("fbforge-connect")
        .unwrap()
        .detection()
        .installations();
    assert_eq!(installations.len(), 1);
    assert_eq!(
        installations[0].condition,
        InstallationCondition::Unconfirmed
    );
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect")],
                issues: vec![],
            },
        )
        .unwrap();
    assert_eq!(
        inventory
            .get("fbforge-connect")
            .unwrap()
            .detection()
            .installations()[0]
            .condition,
        InstallationCondition::Detected
    );
}

#[test]
fn preserves_distinct_installations_and_exposes_damage_without_reinstalling_over_them() {
    let mut inventory = inventory();
    let mut damaged = detected("C:/Tools/Broken");
    damaged.executable = None;
    damaged.condition = InstallationCondition::MissingExecutable;
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect"), damaged],
                issues: vec![],
            },
        )
        .unwrap();
    let entry = inventory.get("fbforge-connect").unwrap();
    assert_eq!(entry.detection().installations().len(), 2);
    assert!(!entry.can_offer_installation());
    assert_eq!(
        entry.detection().installations()[1].condition,
        InstallationCondition::MissingExecutable
    );
}

#[test]
fn rejects_identity_changes_and_takeover_of_remembered_detection_rules_atomically() {
    let mut inventory = inventory();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![detected("C:/Tools/Connect")],
                issues: vec![],
            },
        )
        .unwrap();
    let mut changed = catalog();
    changed["tools"][0]["appId"] = json!("org.other.app");
    assert!(inventory
        .apply_catalog(&parse_catalog(&changed.to_string()).unwrap())
        .is_err());
    assert_eq!(
        inventory.get("fbforge-connect").unwrap().manifest().app_id,
        "org.fitconnect.integration-client"
    );
    inventory
        .apply_catalog(&parse_catalog(r#"{"schemaVersion":1,"tools":[]}"#).unwrap())
        .unwrap();
    let mut replacement = catalog();
    replacement["tools"].as_array_mut().unwrap().truncate(1);
    replacement["tools"][0]["id"] = json!("replacement-tool");
    assert!(inventory
        .apply_catalog(&parse_catalog(&replacement.to_string()).unwrap())
        .is_err());
    assert!(inventory.get("replacement-tool").is_none());
    assert!(inventory.get("fbforge-connect").is_some());
}
