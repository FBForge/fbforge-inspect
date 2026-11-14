mod support;
use fbforge_suite_core::{
    catalog::parse_catalog,
    inventory::Inventory,
    settings::UserSettings,
    storage::{StateStore, StoreError},
    trust::{KeyStatus, MinisignTrustAnchor, TrustError, TrustStore},
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

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-release-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn root(&self) -> PathBuf {
        self.0.join("state")
    }
    fn file(&self) -> PathBuf {
        self.root().join("state-v1.json")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap()
}
fn cache_fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/release-cache-v2.json")).unwrap()
}
fn v2(name: &str) -> String {
    cache_fixtures()["cases"][name].to_string()
}
fn trust() -> TrustStore {
    trust_store()
        .with_minisign_anchors(vec![MinisignTrustAnchor {
            id: "release-cache-test".into(),
            public_key: cache_fixtures()["publicKey"].as_str().unwrap().into(),
            subjects: vec!["catalog".into(), format!("release/{TOOL}")],
            not_before: 100,
            expires_at: 10_000,
            status: KeyStatus::Active,
        }])
        .unwrap()
}
fn version(value: &str) -> Version {
    Version::parse(value).unwrap()
}
fn release(sequence: u64, version: &str, status: &str) -> String {
    let mut signed: Value =
        serde_json::from_str(fixtures()["cases"]["release"]["signed"].as_str().unwrap()).unwrap();
    signed["schemaVersion"] = json!(1);
    signed["sequence"] = json!(sequence);
    signed["body"]["version"] = json!(version);
    signed["body"]["releaseTag"] = json!(format!("v{version}"));
    signed["body"]["status"] = json!(status);
    sign(&signed, "test-key", 42)
}
fn inspect_release() -> String {
    let mut inspect: Value =
        serde_json::from_str(fixtures()["cases"]["release"]["signed"].as_str().unwrap()).unwrap();
    inspect["schemaVersion"] = json!(1);
    inspect["subject"] = json!("release/fbforge-inspect");
    inspect["body"]["toolId"] = json!("fbforge-inspect");
    inspect["body"]["appId"] = json!("de.fbforge.inspect");
    inspect["body"]["repository"] = json!("FBForge/fbforge-inspect");
    inspect["body"]["authenticodePolicyId"] = json!("fbforge-inspect-production");
    inspect["body"]["requiredFiles"] = json!(["fbforge-inspect.exe"]);
    sign(&inspect, "test-key", 42)
}
fn open(directory: &Directory) -> StateStore {
    StateStore::open(&directory.root()).unwrap()
}
fn accept_catalog(store: &mut StateStore, trust: &TrustStore, now: u64) {
    store
        .accept_catalog(
            &sign(&payload("catalog", catalog_body()), "test-key", 42),
            trust,
            now,
        )
        .unwrap();
}
fn saved(directory: &Directory) -> Value {
    let envelope: Value = serde_json::from_slice(&fs::read(directory.file()).unwrap()).unwrap();
    serde_json::from_str(envelope["payload"].as_str().unwrap()).unwrap()
}
fn replace_saved(directory: &Directory, state: &Value) {
    replace_raw_payload(directory, &state.to_string());
}
fn replace_raw_payload(directory: &Directory, payload: &str) {
    let checksum: String = ring::digest::digest(&ring::digest::SHA256, payload.as_bytes())
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    fs::write(
        directory.file(),
        json!({"schemaVersion":1,"payload":payload,"sha256":checksum}).to_string(),
    )
    .unwrap();
}

#[test]
fn release_envelope_and_checkpoint_survive_restart_without_losing_other_state() {
    let directory = Directory::new();
    let trust = trust();
    let envelope = format!(" \n{}\n ", release(2, "0.3.0", "available"));
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        let mut settings = UserSettings::default();
        settings.set_search_interval_seconds(900).unwrap();
        store.save_settings(&settings).unwrap();
        let previous = saved(&directory);
        let verified = store
            .accept_release(&envelope, &trust, 300, TOOL, &version("0.3.0"))
            .unwrap();
        assert!(verified.is_available());
        let next = saved(&directory);
        assert_eq!(next["catalog"], previous["catalog"]);
        assert_eq!(next["inventory"], previous["inventory"]);
        assert_eq!(next["settings"], previous["settings"]);
        assert_eq!(next["releases"][TOOL]["envelope"], envelope);
    }
    let mut store = open(&directory);
    assert_eq!(
        store.load_settings().unwrap().search_interval_seconds(),
        900
    );
    assert_eq!(
        store
            .load_release(&trust, 301, TOOL, &version("0.3.0"))
            .unwrap()
            .version(),
        &version("0.3.0")
    );
    let before = fs::read(directory.file()).unwrap();
    assert_eq!(saved(&directory)["releases"][TOOL]["envelope"], envelope);
    assert_eq!(
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                301,
                TOOL,
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
    assert_eq!(before, fs::read(directory.file()).unwrap());
}

#[test]
fn release_revalidation_persists_its_clock_checkpoint_before_returning() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
        store
            .load_release(&trust, 350, TOOL, &version("0.3.0"))
            .unwrap();
        assert_eq!(
            saved(&directory)["releases"][TOOL]["checkpoint"]["lastVerifiedAt"],
            350
        );
    }
    assert_eq!(
        open(&directory)
            .load_release(&trust, 349, TOOL, &version("0.3.0"))
            .unwrap_err(),
        StoreError::Trust(TrustError::ClockRollback)
    );
}

#[test]
fn persisted_withdrawal_blocks_an_older_available_release() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
        assert!(!store
            .accept_release(
                &release(2, "0.3.0", "withdrawn"),
                &trust,
                301,
                TOOL,
                &version("0.3.0")
            )
            .unwrap()
            .is_available());
    }
    let mut store = open(&directory);
    assert!(!store
        .load_release(&trust, 302, TOOL, &version("0.3.0"))
        .unwrap()
        .is_available());
    assert_eq!(
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                302,
                TOOL,
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
}

#[test]
fn cached_v2_release_never_downgrades_to_a_newer_sequence_in_v1() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        store.accept_catalog(&v2("catalog"), &trust, 300).unwrap();
        store
            .accept_release(&v2("release1"), &trust, 300, TOOL, &version("0.3.0"))
            .unwrap();
    }
    let mut store = open(&directory);
    store
        .load_release(&trust, 301, TOOL, &version("0.3.0"))
        .unwrap();
    assert_eq!(
        store
            .accept_release(
                &release(2, "0.3.0", "available"),
                &trust,
                301,
                TOOL,
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
}

#[test]
fn actual_v2_release_progress_and_withdrawal_survive_restart_and_reject_older_versions() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        store.accept_catalog(&v2("catalog"), &trust, 300).unwrap();
        store
            .accept_release(&v2("release1"), &trust, 300, TOOL, &version("0.3.0"))
            .unwrap();
        store
            .accept_release(&v2("release2"), &trust, 301, TOOL, &version("0.4.0"))
            .unwrap();
    }
    {
        let mut store = open(&directory);
        assert_eq!(
            store
                .load_release(&trust, 302, TOOL, &version("0.3.0"))
                .unwrap_err(),
            StoreError::Trust(TrustError::InvalidRelease)
        );
        store
            .load_release(&trust, 302, TOOL, &version("0.4.0"))
            .unwrap();
        assert_eq!(
            store
                .accept_release(&v2("release1"), &trust, 302, TOOL, &version("0.3.0"))
                .unwrap_err(),
            StoreError::Trust(TrustError::Rollback)
        );
        assert!(!store
            .accept_release(&v2("withdrawn3"), &trust, 303, TOOL, &version("0.4.0"))
            .unwrap()
            .is_available());
    }
    {
        let mut store = open(&directory);
        assert!(!store
            .load_release(&trust, 350, TOOL, &version("0.4.0"))
            .unwrap()
            .is_available());
        assert_eq!(
            store
                .accept_release(&v2("release2"), &trust, 350, TOOL, &version("0.4.0"))
                .unwrap_err(),
            StoreError::Trust(TrustError::Rollback)
        );
        assert_eq!(
            store
                .accept_release(
                    &release(4, "0.4.0", "available"),
                    &trust,
                    350,
                    TOOL,
                    &version("0.4.0")
                )
                .unwrap_err(),
            StoreError::Trust(TrustError::Rollback)
        );
    }
    assert_eq!(
        open(&directory)
            .load_release(&trust, 349, TOOL, &version("0.4.0"))
            .unwrap_err(),
        StoreError::Trust(TrustError::ClockRollback)
    );
    assert_eq!(
        saved(&directory)["releases"][TOOL]["envelope"],
        v2("withdrawn3")
    );
}

#[test]
fn tool_version_current_catalog_and_current_trust_are_required() {
    let directory = Directory::new();
    let trust = trust();
    let mut store = open(&directory);
    assert_eq!(
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                300,
                TOOL,
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::MissingCatalog
    );
    accept_catalog(&mut store, &trust, 300);
    for (tool, requested) in [("fbforge-inspect", "0.3.0"), (TOOL, "0.4.0")] {
        assert!(store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                300,
                tool,
                &version(requested)
            )
            .is_err());
    }
    store
        .accept_release(
            &release(1, "0.3.0", "available"),
            &trust,
            300,
            TOOL,
            &version("0.3.0"),
        )
        .unwrap();
    assert_eq!(
        store
            .load_release(&trust, 300, TOOL, &version("0.4.0"))
            .unwrap_err(),
        StoreError::Trust(TrustError::InvalidRelease)
    );
    assert_eq!(
        store
            .load_release(&trust, 1000, TOOL, &version("0.3.0"))
            .unwrap_err(),
        StoreError::Trust(TrustError::Expired)
    );
    assert!(store
        .load_release(
            &TrustStore::new(vec![]).unwrap(),
            300,
            TOOL,
            &version("0.3.0")
        )
        .is_err());
    let mut changed = payload("catalog", json!({"schemaVersion":1,"tools":[]}));
    changed["sequence"] = json!(2);
    store
        .accept_catalog(&sign(&changed, "test-key", 42), &trust, 300)
        .unwrap();
    assert_eq!(
        store
            .load_release(&trust, 300, TOOL, &version("0.3.0"))
            .unwrap_err(),
        StoreError::Trust(TrustError::InvalidRelease)
    );
    assert_eq!(
        saved(&directory)["releases"][TOOL]["checkpoint"]["sequence"],
        1
    );
}

#[test]
fn old_state_without_release_entries_remains_compatible_and_missing_is_explicit() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
    }
    let mut state = saved(&directory);
    state.as_object_mut().unwrap().remove("releases");
    replace_saved(&directory, &state);
    let mut store = open(&directory);
    assert_eq!(
        store
            .load_release(&trust, 300, TOOL, &version("0.3.0"))
            .unwrap_err(),
        StoreError::MissingRelease
    );
    store
        .accept_release(
            &release(1, "0.3.0", "available"),
            &trust,
            300,
            TOOL,
            &version("0.3.0"),
        )
        .unwrap();
}

#[test]
fn corrupt_release_pairs_and_duplicate_or_excessive_entries_are_never_reset() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
    }
    let original = saved(&directory);
    for field in [
        "sequence",
        "metadataSchemaVersion",
        "payloadSha256",
        "subject",
    ] {
        let mut state = original.clone();
        state["releases"][TOOL]["checkpoint"][field] = match field {
            "sequence" => json!(2),
            "metadataSchemaVersion" => json!(2),
            _ => json!("corrupt"),
        };
        replace_saved(&directory, &state);
        let before = fs::read(directory.file()).unwrap();
        assert!(
            matches!(
                StateStore::open(&directory.root()),
                Err(StoreError::CorruptState)
            ),
            "{field}"
        );
        assert_eq!(fs::read(directory.file()).unwrap(), before);
    }
    let entry = original["releases"][TOOL].to_string();
    let duplicate = original.to_string().replace(
        &format!("\"releases\":{{\"{TOOL}\":{entry}}}"),
        &format!("\"releases\":{{\"{TOOL}\":{entry},\"{TOOL}\":{entry}}}"),
    );
    assert_ne!(duplicate, original.to_string());
    replace_raw_payload(&directory, &duplicate);
    assert!(matches!(
        StateStore::open(&directory.root()),
        Err(StoreError::CorruptState)
    ));
    let mut state = original.clone();
    state["releases"] = json!(null);
    replace_saved(&directory, &state);
    assert!(matches!(
        StateStore::open(&directory.root()),
        Err(StoreError::CorruptState)
    ));
    let mut entries = serde_json::Map::new();
    for index in 0..501 {
        entries.insert(format!("tool-{index}"), original["releases"][TOOL].clone());
    }
    state["releases"] = Value::Object(entries);
    replace_saved(&directory, &state);
    assert!(matches!(
        StateStore::open(&directory.root()),
        Err(StoreError::CorruptState)
    ));
}

#[test]
fn failed_release_acceptance_or_revalidation_never_returns_verified_success() {
    let directory = Directory::new();
    let trust = trust();
    let mut store = open(&directory);
    accept_catalog(&mut store, &trust, 300);
    store
        .accept_release(
            &release(1, "0.3.0", "available"),
            &trust,
            300,
            TOOL,
            &version("0.3.0"),
        )
        .unwrap();
    let backup = directory.root().join("original.json");
    fs::rename(directory.file(), &backup).unwrap();
    fs::create_dir(directory.file()).unwrap();
    assert!(store
        .accept_release(
            &release(2, "0.3.0", "available"),
            &trust,
            301,
            TOOL,
            &version("0.3.0")
        )
        .is_err());
    assert!(store
        .load_release(&trust, 302, TOOL, &version("0.3.0"))
        .is_err());
    fs::remove_dir(directory.file()).unwrap();
    fs::rename(backup, directory.file()).unwrap();
    assert_eq!(
        saved(&directory)["releases"][TOOL]["checkpoint"]["sequence"],
        1
    );
    store
        .load_release(&trust, 301, TOOL, &version("0.3.0"))
        .unwrap();
}

#[test]
fn revoked_release_key_does_not_erase_its_high_water_mark_or_select_an_older_key() {
    let directory = Directory::new();
    let mut release_anchor = anchor("release-key", 43);
    release_anchor.subjects = vec![format!("release/{TOOL}")];
    let initial = TrustStore::new(vec![anchor("test-key", 42), release_anchor]).unwrap();
    let mut signed: Value =
        serde_json::from_str(fixtures()["cases"]["release"]["signed"].as_str().unwrap()).unwrap();
    signed["schemaVersion"] = json!(1);
    signed["sequence"] = json!(2);
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &initial, 300);
        store
            .accept_release(
                &sign(&signed, "release-key", 43),
                &initial,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
    }
    let mut revoked = anchor("release-key", 43);
    revoked.status = KeyStatus::Revoked;
    revoked.subjects = vec![format!("release/{TOOL}")];
    let trust = TrustStore::new(vec![anchor("test-key", 42), revoked]).unwrap();
    let mut store = open(&directory);
    let previous = fs::read(directory.file()).unwrap();
    assert_eq!(
        store
            .load_release(&trust, 301, TOOL, &version("0.3.0"))
            .unwrap_err(),
        StoreError::Trust(TrustError::KeyUnavailable)
    );
    assert_eq!(
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                301,
                TOOL,
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
    assert_eq!(fs::read(directory.file()).unwrap(), previous);
}

#[test]
fn release_checkpoints_are_independent_for_each_tool() {
    let directory = Directory::new();
    let mut key = anchor("test-key", 42);
    key.subjects.push("release/fbforge-inspect".into());
    let trust = TrustStore::new(vec![key]).unwrap();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        store
            .accept_release(
                &release(50, "0.3.0", "available"),
                &trust,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
        store
            .accept_release(
                &inspect_release(),
                &trust,
                300,
                "fbforge-inspect",
                &version("0.3.0"),
            )
            .unwrap();
    }
    let mut store = open(&directory);
    store
        .load_release(&trust, 301, TOOL, &version("0.3.0"))
        .unwrap();
    store
        .load_release(&trust, 301, "fbforge-inspect", &version("0.3.0"))
        .unwrap();
    assert_eq!(
        saved(&directory)["releases"][TOOL]["checkpoint"]["sequence"],
        50
    );
    assert_eq!(
        saved(&directory)["releases"]["fbforge-inspect"]["checkpoint"]["sequence"],
        1
    );
}

#[test]
fn malformed_release_contract_and_lost_v2_format_binding_are_rejected_at_open() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        store.accept_catalog(&v2("catalog"), &trust, 300).unwrap();
        store
            .accept_release(&v2("release1"), &trust, 300, TOOL, &version("0.3.0"))
            .unwrap();
    }
    let original = saved(&directory);
    for change in [
        "missingFormat",
        "wrongKey",
        "badBody",
        "unknownCacheField",
        "oversizeEnvelope",
    ] {
        let mut state = original.clone();
        match change {
            "missingFormat" => {
                state["releases"][TOOL]["checkpoint"]
                    .as_object_mut()
                    .unwrap()
                    .remove("metadataSchemaVersion");
            }
            "wrongKey" => {
                let cache = state["releases"][TOOL].take();
                state["releases"].as_object_mut().unwrap().remove(TOOL);
                state["releases"]["fbforge-inspect"] = cache;
            }
            "unknownCacheField" => state["releases"][TOOL]["untrustedKey"] = json!("ignored"),
            "oversizeEnvelope" => {
                state["releases"][TOOL]["envelope"] = json!("x".repeat(2_097_153))
            }
            _ => {
                let mut envelope: Value =
                    serde_json::from_str(state["releases"][TOOL]["envelope"].as_str().unwrap())
                        .unwrap();
                let mut signed: Value =
                    serde_json::from_str(envelope["signed"].as_str().unwrap()).unwrap();
                signed["body"]["size"] = json!(0);
                let text = signed.to_string();
                let hash: String = ring::digest::digest(&ring::digest::SHA256, text.as_bytes())
                    .as_ref()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                state["releases"][TOOL]["checkpoint"]["payloadSha256"] = json!(hash);
                envelope["signed"] = json!(text);
                state["releases"][TOOL]["envelope"] = json!(envelope.to_string());
            }
        }
        replace_saved(&directory, &state);
        let before = fs::read(directory.file()).unwrap();
        assert!(
            matches!(
                StateStore::open(&directory.root()),
                Err(StoreError::CorruptState)
            ),
            "{change}"
        );
        assert_eq!(fs::read(directory.file()).unwrap(), before);
    }
}

#[test]
fn total_state_size_is_bounded_without_committing_a_partial_release_cache() {
    let directory = Directory::new();
    let original = catalog_body()["tools"][0].clone();
    let tools: Vec<Value> = (0..12)
        .map(|index| {
            let id = format!("release-fixture-{index}");
            let mut tool = original.clone();
            tool["id"] = json!(id);
            tool["appId"] = json!(format!("org.example.{id}"));
            tool["registryNames"] = json!([format!("Release Fixture {index}")]);
            tool["executables"] = json!([format!("{id}.exe")]);
            tool
        })
        .collect();
    let mut key = anchor("test-key", 42);
    key.subjects = vec!["catalog".into()];
    key.subjects.extend(
        tools
            .iter()
            .map(|tool| format!("release/{}", tool["id"].as_str().unwrap())),
    );
    let trust = TrustStore::new(vec![key]).unwrap();
    let mut store = open(&directory);
    store
        .accept_catalog(
            &sign(
                &payload("catalog", json!({"schemaVersion":1,"tools":tools})),
                "test-key",
                42,
            ),
            &trust,
            300,
        )
        .unwrap();
    let padding: Vec<String> = (0..8190)
        .map(|index| format!("fixture-{index:04}-{}.bin", "x".repeat(90)))
        .collect();
    let mut reached_limit = false;
    for index in 0..12 {
        let id = format!("release-fixture-{index}");
        let mut signed: Value =
            serde_json::from_str(fixtures()["cases"]["release"]["signed"].as_str().unwrap())
                .unwrap();
        signed["schemaVersion"] = json!(1);
        signed["subject"] = json!(format!("release/{id}"));
        signed["body"]["toolId"] = json!(id);
        signed["body"]["appId"] = json!(format!("org.example.{id}"));
        let mut files = padding.clone();
        files.push(format!("{id}.exe"));
        signed["body"]["requiredFiles"] = json!(files);
        assert!(signed.to_string().len() <= 1_048_576);
        let before = fs::read(directory.file()).unwrap();
        match store.accept_release(
            &sign(&signed, "test-key", 42),
            &trust,
            300,
            &id,
            &version("0.3.0"),
        ) {
            Ok(_) => {}
            Err(StoreError::TooLarge) => {
                assert!(index > 0);
                assert_eq!(fs::read(directory.file()).unwrap(), before);
                assert_eq!(
                    store
                        .load_release(&trust, 300, &id, &version("0.3.0"))
                        .unwrap_err(),
                    StoreError::MissingRelease
                );
                reached_limit = true;
                break;
            }
            Err(error) => panic!("unexpected bounded-cache error: {error:?}"),
        }
    }
    assert!(reached_limit);
}

#[test]
fn five_hundred_historical_tools_remain_valid_but_a_new_release_cannot_evict_a_checkpoint() {
    let directory = Directory::new();
    let mut key = anchor("test-key", 42);
    key.subjects.push("release/fbforge-inspect".into());
    let trust = TrustStore::new(vec![key]).unwrap();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        store
            .accept_release(
                &release(2, "0.3.0", "available"),
                &trust,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
    }
    let mut state = saved(&directory);
    for index in 0..499 {
        let id = format!("historical-{index}");
        let mut signed: Value =
            serde_json::from_str(fixtures()["cases"]["release"]["signed"].as_str().unwrap())
                .unwrap();
        signed["schemaVersion"] = json!(1);
        signed["subject"] = json!(format!("release/{id}"));
        signed["body"]["toolId"] = json!(id);
        signed["body"]["appId"] = json!(format!("org.example.{id}"));
        let text = signed.to_string();
        let checksum: String = ring::digest::digest(&ring::digest::SHA256, text.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        state["releases"][&id] = json!({"envelope":sign_text(&text, "test-key", 42), "checkpoint":{"metadataSchemaVersion":1,"subject":format!("release/{id}"),"sequence":1,"payloadSha256":checksum,"lastVerifiedAt":300}});
    }
    replace_saved(&directory, &state);
    let mut store = open(&directory);
    store
        .load_release(&trust, 300, TOOL, &version("0.3.0"))
        .unwrap();
    store
        .accept_release(
            &release(3, "0.3.0", "available"),
            &trust,
            300,
            TOOL,
            &version("0.3.0"),
        )
        .unwrap();
    let before = fs::read(directory.file()).unwrap();
    assert_eq!(
        store
            .accept_release(
                &inspect_release(),
                &trust,
                300,
                "fbforge-inspect",
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::TooLarge
    );
    assert_eq!(before, fs::read(directory.file()).unwrap());
    assert_eq!(
        saved(&directory)["releases"].as_object().unwrap().len(),
        500
    );
}

#[test]
fn settings_inventory_and_catalog_writes_preserve_release_high_water_marks() {
    let directory = Directory::new();
    let trust = trust();
    {
        let mut store = open(&directory);
        accept_catalog(&mut store, &trust, 300);
        store
            .accept_release(
                &release(2, "0.3.0", "withdrawn"),
                &trust,
                300,
                TOOL,
                &version("0.3.0"),
            )
            .unwrap();
        let expected = saved(&directory)["releases"].clone();
        let mut settings = UserSettings::default();
        settings.set_search_interval_seconds(900).unwrap();
        store.save_settings(&settings).unwrap();
        assert_eq!(saved(&directory)["releases"], expected);
        let builtin = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
        store
            .save_inventory(&Inventory::from_catalog(&builtin).unwrap())
            .unwrap();
        assert_eq!(saved(&directory)["releases"], expected);
        let mut next_catalog = payload("catalog", catalog_body());
        next_catalog["sequence"] = json!(2);
        store
            .accept_catalog(&sign(&next_catalog, "test-key", 42), &trust, 301)
            .unwrap();
        assert_eq!(saved(&directory)["releases"], expected);
    }
    let mut store = open(&directory);
    assert!(!store
        .load_release(&trust, 302, TOOL, &version("0.3.0"))
        .unwrap()
        .is_available());
    assert_eq!(
        store
            .accept_release(
                &release(1, "0.3.0", "available"),
                &trust,
                302,
                TOOL,
                &version("0.3.0")
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
}
