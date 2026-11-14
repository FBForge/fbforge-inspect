mod support;
use fbforge_suite_core::{
    storage::{StateStore, StoreError},
    trust::{verify_catalog, verify_release, verify_release_candidate, TrustError},
};
use semver::Version;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::*;

const TOOL: &str = "fbforge-connect";
fn release(sequence: u64, status: &str) -> String {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap();
    let mut payload: Value =
        serde_json::from_str(fixtures["cases"]["release"]["signed"].as_str().unwrap()).unwrap();
    payload["schemaVersion"] = json!(1);
    payload["sequence"] = json!(sequence);
    payload["body"]["status"] = json!(status);
    sign(&payload, "test-key", 42)
}
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "suite-candidate-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
    fn store(&self) -> StateStore {
        StateStore::open(&self.0.join("state")).unwrap()
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn catalog() -> String {
    sign(&payload("catalog", catalog_body()), "test-key", 42)
}

#[test]
fn discovers_version_only_after_authenticating_the_release_and_product_context() {
    let trust = trust_store();
    let catalog = verify_catalog(&catalog(), &trust, 300, None).unwrap();
    let envelope = release(1, "available");
    let verified = verify_release_candidate(&envelope, &trust, 300, None, &catalog, TOOL).unwrap();
    assert_eq!(verified.metadata_schema_version(), 1);
    assert_eq!(verified.version(), &Version::parse("0.3.0").unwrap());
    assert_eq!(verified.tool_id(), TOOL);
    assert!(verified.is_available());
    assert!(
        verify_release_candidate(&envelope, &trust, 300, None, &catalog, "fbforge-inspect")
            .is_err()
    );
    assert!(verify_release(
        &envelope,
        &trust,
        300,
        None,
        &catalog,
        TOOL,
        &Version::parse("0.4.0").unwrap()
    )
    .is_err());
    let mut envelope: Value = serde_json::from_str(&envelope).unwrap();
    envelope["signed"] = json!(envelope["signed"]
        .as_str()
        .unwrap()
        .replace("0.3.0", "0.4.0"));
    assert!(matches!(
        verify_release_candidate(&envelope.to_string(), &trust, 300, None, &catalog, TOOL),
        Err(TrustError::InvalidSignature)
    ));
}

#[test]
fn accepting_a_candidate_preserves_checkpoints_and_current_time_across_restart() {
    let directory = Directory::new();
    let trust = trust_store();
    {
        let mut store = directory.store();
        store.accept_catalog(&catalog(), &trust, 300).unwrap();
        assert_eq!(store.saved_release_tool_ids().unwrap().count(), 0);
        assert_eq!(
            store
                .accept_release_candidate(&release(2, "available"), &trust, 301, TOOL)
                .unwrap()
                .version(),
            &Version::parse("0.3.0").unwrap()
        );
        assert_eq!(
            store.saved_release_tool_ids().unwrap().collect::<Vec<_>>(),
            vec![TOOL]
        );
    }
    let mut store = directory.store();
    assert!(matches!(
        store.accept_release_candidate(&release(1, "available"), &trust, 302, TOOL),
        Err(StoreError::Trust(_))
    ));
    assert_eq!(
        store
            .load_saved_release(&trust, 303, TOOL)
            .unwrap()
            .version(),
        &Version::parse("0.3.0").unwrap()
    );
    assert!(matches!(
        store.load_saved_release(&trust, 302, TOOL),
        Err(StoreError::Trust(TrustError::ClockRollback))
    ));
    assert!(store.load_saved_release(&trust, 1000, TOOL).is_err());
}

#[test]
fn a_signed_withdrawal_replaces_the_candidate_without_falling_back_to_available_bytes() {
    let directory = Directory::new();
    let trust = trust_store();
    let mut store = directory.store();
    store.accept_catalog(&catalog(), &trust, 300).unwrap();
    store
        .accept_release_candidate(&release(1, "available"), &trust, 301, TOOL)
        .unwrap();
    assert!(!store
        .accept_release_candidate(&release(2, "withdrawn"), &trust, 302, TOOL)
        .unwrap()
        .is_available());
    drop(store);
    let mut store = directory.store();
    assert!(!store
        .load_saved_release(&trust, 303, TOOL)
        .unwrap()
        .is_available());
    assert!(store
        .accept_release_candidate(&release(1, "available"), &trust, 304, TOOL)
        .is_err());
    let mut cache = store.package_cache(Default::default()).unwrap();
    assert!(cache
        .prepare(
            &mut store,
            &trust,
            305,
            TOOL,
            &Version::parse("0.3.0").unwrap(),
            &Default::default()
        )
        .is_err());
}

#[test]
fn the_exact_version_consumer_cannot_advance_a_checkpoint_with_a_different_offer() {
    let directory = Directory::new();
    let trust = trust_store();
    let mut store = directory.store();
    store.accept_catalog(&catalog(), &trust, 300).unwrap();
    store
        .accept_release_candidate(&release(1, "available"), &trust, 301, TOOL)
        .unwrap();
    let envelope: Value = serde_json::from_str(&release(2, "available")).unwrap();
    let mut offered: Value = serde_json::from_str(envelope["signed"].as_str().unwrap()).unwrap();
    offered["body"]["version"] = json!("0.4.0");
    offered["body"]["releaseTag"] = json!("v0.4.0");
    let offered = sign(&offered, "test-key", 42);
    assert!(store
        .accept_release(
            &offered,
            &trust,
            302,
            TOOL,
            &Version::parse("0.3.0").unwrap()
        )
        .is_err());
    let saved = store.load_saved_release(&trust, 303, TOOL).unwrap();
    assert_eq!(saved.version(), &Version::parse("0.3.0").unwrap());
    assert_eq!(
        serde_json::to_value(saved.checkpoint()).unwrap()["sequence"],
        1
    );
    let current = store
        .accept_release_candidate(&offered, &trust, 304, TOOL)
        .unwrap();
    assert_eq!(current.version(), &Version::parse("0.4.0").unwrap());
    assert_eq!(
        serde_json::to_value(current.checkpoint()).unwrap()["sequence"],
        2
    );
    assert!(store
        .load_release(&trust, 305, TOOL, &Version::parse("0.3.0").unwrap())
        .is_err());
}
