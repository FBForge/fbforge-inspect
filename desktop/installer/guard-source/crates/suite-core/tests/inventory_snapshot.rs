use fbforge_suite_core::{catalog::parse_catalog, inventory::*};
use semver::Version;
use serde_json::{json, Value};
use std::path::PathBuf;

fn inventory() -> Inventory {
    let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let mut inventory = Inventory::from_catalog(&catalog).unwrap();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![Installation {
                    root: PathBuf::from("C:/Tools/Connect"),
                    executable: Some(PathBuf::from("C:/Tools/Connect/fbforge-connect.exe")),
                    reported_version: Some("0.3.0".into()),
                    version: Some(Version::parse("0.3.0").unwrap()),
                    condition: InstallationCondition::Detected,
                    sources: vec![RegistrySource {
                        hive: RegistryHive::CurrentUser,
                        view: RegistryView::Registry64,
                        key_name: "FIT-Connect Test-Client".into(),
                    }],
                }],
                issues: vec![],
            },
        )
        .unwrap();
    inventory
}

#[test]
fn restart_retains_removed_tools_and_paths_but_requires_fresh_native_observations() {
    let inventory = inventory();
    let snapshot = inventory.snapshot_json().unwrap();
    let empty = parse_catalog(r#"{"schemaVersion":1,"tools":[]}"#).unwrap();
    let restored = Inventory::restore_snapshot(&snapshot, &empty).unwrap();
    let entry = restored.get("fbforge-connect").unwrap();
    assert!(!entry.is_listed());
    assert!(!entry.can_offer_installation());
    assert_eq!(
        entry.detection().installations()[0].root,
        PathBuf::from("C:/Tools/Connect")
    );
    assert_eq!(
        entry.detection().installations()[0].condition,
        InstallationCondition::Unconfirmed
    );
    assert!(matches!(
        entry.detection(),
        DetectionState::Incomplete { .. }
    ));
}

#[test]
fn validates_saved_contracts_sources_paths_and_version_consistency() {
    let data: Value = serde_json::from_str(&inventory().snapshot_json().unwrap()).unwrap();
    let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    for (field, value) in [
        ("root", json!("//server/share/Tool")),
        ("root", json!("C:/Tools/../Other")),
        ("executable", json!("C:/Other/evil.exe")),
        ("reportedVersion", json!("0.2.0")),
        ("sources", json!([])),
    ] {
        let mut changed = data.clone();
        changed["tools"][0]["installations"][0][field] = value;
        assert!(
            Inventory::restore_snapshot(&changed.to_string(), &catalog).is_err(),
            "{field}"
        );
    }
    let mut changed = data.clone();
    changed["tools"][0]["installations"][0]["sources"][0]["keyName"] = json!("Unrelated");
    assert!(Inventory::restore_snapshot(&changed.to_string(), &catalog).is_err());
    let mut changed = data.clone();
    changed["tools"][0]["manifest"]["appId"] = json!("org.other.app");
    assert!(Inventory::restore_snapshot(&changed.to_string(), &catalog).is_err());
    let mut changed = data;
    changed["tools"][0]["installations"][0]["command"] = json!("cmd.exe");
    assert!(Inventory::restore_snapshot(&changed.to_string(), &catalog).is_err());
}

#[test]
fn never_turns_corrupt_or_unsupported_state_into_an_empty_inventory() {
    let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    for input in [
        "{}",
        r#"{"schemaVersion":2,"tools":[]}"#,
        r#"{"schemaVersion":1,"tools":[],"extra":true}"#,
    ] {
        assert!(Inventory::restore_snapshot(input, &catalog).is_err());
    }
    assert!(Inventory::restore_snapshot(&" ".repeat(4_194_305), &catalog).is_err());
}
