mod support;
use fbforge_suite_core::{
    catalog::parse_catalog,
    inventory::{
        DetectionReport, DetectionState, Installation, InstallationCondition, Inventory,
        RegistryHive, RegistrySource, RegistryView,
    },
    storage::{CatalogCacheStatus, StateStore, StoreError},
    trust::TrustError,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::*;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-state-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf {
        self.0.join("FBForgeSuite")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn persisted_removed_installation_survives_restart_as_unconfirmed() {
    let directory = Directory::new();
    let trust = trust_store();
    let builtin = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let mut inventory = Inventory::from_catalog(&builtin).unwrap();
    inventory
        .record_detection(
            "fbforge-connect",
            DetectionReport {
                installations: vec![Installation {
                    root: PathBuf::from("C:/Tools/Connect"),
                    executable: Some(PathBuf::from("C:/Tools/Connect/fbforge-connect.exe")),
                    reported_version: Some("0.3.0".into()),
                    version: Some(semver::Version::parse("0.3.0").unwrap()),
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
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store.save_inventory(&inventory).unwrap();
        let empty = payload("catalog", json!({"schemaVersion":1,"tools":[]}));
        store
            .accept_catalog(&sign(&empty, "test-key", 42), &trust, 300)
            .unwrap();
    }
    let mut store = StateStore::open(&directory.store()).unwrap();
    let recovered = store.load_inventory(&builtin, &trust, 301).unwrap();
    assert_eq!(recovered.catalog_status, CatalogCacheStatus::Current);
    let entry = recovered.inventory.get("fbforge-connect").unwrap();
    assert!(!entry.is_listed());
    assert!(!entry.can_offer_installation());
    assert!(matches!(
        entry.detection(),
        DetectionState::Incomplete { .. }
    ));
    let installation = &entry.detection().installations()[0];
    assert_eq!(installation.condition, InstallationCondition::Unconfirmed);
    assert_eq!(installation.root, PathBuf::from("C:/Tools/Connect"));
    assert_eq!(
        installation.executable,
        Some(PathBuf::from("C:/Tools/Connect/fbforge-connect.exe"))
    );
}

#[test]
fn cached_revalidation_persists_the_newest_clock_checkpoint() {
    let directory = Directory::new();
    let trust = trust_store();
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store
            .accept_catalog(
                &sign(&payload("catalog", catalog_body()), "test-key", 42),
                &trust,
                300,
            )
            .unwrap();
        store.load_catalog(&trust, 301).unwrap();
    }
    let mut store = StateStore::open(&directory.store()).unwrap();
    assert_eq!(
        store.load_catalog(&trust, 300).unwrap_err(),
        StoreError::Trust(TrustError::ClockRollback)
    );
}

#[test]
fn catalog_and_checkpoint_survive_restart_and_reject_rollback() {
    let directory = Directory::new();
    let trust = trust_store();
    let first = payload("catalog", catalog_body());
    let mut second = first.clone();
    second["sequence"] = json!(2);
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store
            .accept_catalog(&sign(&second, "test-key", 42), &trust, 300)
            .unwrap();
    }
    let mut store = StateStore::open(&directory.store()).unwrap();
    assert_eq!(
        store
            .accept_catalog(&sign(&first, "test-key", 42), &trust, 301)
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
    let before = fs::read(directory.store().join("state-v1.json")).unwrap();
    assert!(store
        .accept_catalog(&sign(&second, "test-key", 43), &trust, 301)
        .is_err());
    assert_eq!(
        before,
        fs::read(directory.store().join("state-v1.json")).unwrap()
    );
    assert_eq!(
        store
            .load_catalog(&trust, 302)
            .unwrap()
            .catalog()
            .tools()
            .len(),
        2
    );
}

#[test]
fn offline_restart_reports_expiry_but_preserves_known_tools() {
    let directory = Directory::new();
    let trust = trust_store();
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store
            .accept_catalog(
                &sign(&payload("catalog", catalog_body()), "test-key", 42),
                &trust,
                300,
            )
            .unwrap();
    }
    let mut store = StateStore::open(&directory.store()).unwrap();
    let builtin = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let recovered = store.load_inventory(&builtin, &trust, 1000).unwrap();
    assert_eq!(
        recovered.catalog_status,
        CatalogCacheStatus::Unavailable(TrustError::Expired)
    );
    assert!(recovered.inventory.get("fbforge-connect").is_some());
    assert!(!recovered
        .inventory
        .get("fbforge-connect")
        .unwrap()
        .can_offer_installation());
}

#[test]
fn interrupted_temporary_files_do_not_replace_committed_state() {
    let directory = Directory::new();
    let trust = trust_store();
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store
            .accept_catalog(
                &sign(&payload("catalog", catalog_body()), "test-key", 42),
                &trust,
                300,
            )
            .unwrap();
    }
    let orphan = directory
        .store()
        .join(".state-0123456789abcdef0123456789abcdef.tmp");
    fs::write(&orphan, b"partial data").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&orphan, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let unrelated = directory.store().join("unrelated.tmp");
    fs::write(&unrelated, b"keep").unwrap();
    let mut store = StateStore::open(&directory.store()).unwrap();
    assert!(store.load_catalog(&trust, 301).is_ok());
    assert!(!orphan.exists());
    assert!(unrelated.exists());
}

#[test]
fn corrupt_state_is_an_error_and_is_never_reset_or_overwritten() {
    let directory = Directory::new();
    drop(StateStore::open(&directory.store()).unwrap());
    let path = directory.store().join("state-v1.json");
    fs::write(&path, b"{damaged").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let result = StateStore::open(&directory.store());
    assert!(
        matches!(&result, Err(StoreError::CorruptState)),
        "unexpected state error: {:?}",
        result.err()
    );
    assert_eq!(fs::read(&path).unwrap(), b"{damaged");
}

#[test]
fn mismatched_persisted_checkpoint_is_rejected_before_a_catalog_can_replace_it() {
    let directory = Directory::new();
    let trust = trust_store();
    let mut newer = payload("catalog", catalog_body());
    newer["sequence"] = json!(2);
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store
            .accept_catalog(&sign(&newer, "test-key", 42), &trust, 300)
            .unwrap();
    }
    let path = directory.store().join("state-v1.json");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut state: serde_json::Value =
        serde_json::from_str(envelope["payload"].as_str().unwrap()).unwrap();
    state["catalog"]["checkpoint"]["sequence"] = json!(1);
    let payload = state.to_string();
    envelope["sha256"] = json!(
        ring::digest::digest(&ring::digest::SHA256, payload.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    envelope["payload"] = json!(payload);
    let corrupted = envelope.to_string();
    fs::write(&path, &corrupted).unwrap();
    assert!(matches!(
        StateStore::open(&directory.store()),
        Err(StoreError::CorruptState)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), corrupted);
}

#[test]
fn checksum_detects_otherwise_valid_json_corruption_without_resetting_state() {
    let directory = Directory::new();
    let trust = trust_store();
    {
        let mut store = StateStore::open(&directory.store()).unwrap();
        store
            .accept_catalog(
                &sign(&payload("catalog", catalog_body()), "test-key", 42),
                &trust,
                300,
            )
            .unwrap();
    }
    let path = directory.store().join("state-v1.json");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut state: serde_json::Value =
        serde_json::from_str(envelope["payload"].as_str().unwrap()).unwrap();
    state["catalog"]["checkpoint"]["lastVerifiedAt"] = json!(299);
    envelope["payload"] = json!(state.to_string());
    let corrupted = envelope.to_string();
    fs::write(&path, &corrupted).unwrap();
    assert!(matches!(
        StateStore::open(&directory.store()),
        Err(StoreError::CorruptState)
    ));
    assert_eq!(fs::read_to_string(&path).unwrap(), corrupted);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_state_or_shared_directories_without_touching_targets() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let directory = Directory::new();
    drop(StateStore::open(&directory.store()).unwrap());
    let target = directory.0.join("foreign.json");
    fs::write(&target, b"do not alter").unwrap();
    symlink(&target, directory.store().join("state-v1.json")).unwrap();
    assert!(StateStore::open(&directory.store()).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"do not alter");
    fs::remove_file(directory.store().join("state-v1.json")).unwrap();
    fs::set_permissions(directory.store(), fs::Permissions::from_mode(0o777)).unwrap();
    assert!(StateStore::open(&directory.store()).is_err());
}
