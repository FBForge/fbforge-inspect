mod support;
use base64::{engine::general_purpose::STANDARD, Engine};
use fbforge_suite_core::{
    catalog::parse_catalog,
    storage::{CatalogCacheStatus, StateStore, StoreError},
    trust::{
        verify_catalog, verify_release, KeyStatus, MetadataCheckpoint, MinisignTrustAnchor,
        TrustError, TrustStore,
    },
};
use semver::Version;
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::*;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap()
}
fn envelope(name: &str) -> String {
    fixtures()["cases"][name].to_string()
}
fn minisign_anchor() -> MinisignTrustAnchor {
    MinisignTrustAnchor {
        id: "minisign-test".into(),
        public_key: fixtures()["publicKey"].as_str().unwrap().into(),
        subjects: vec!["catalog".into(), "release/fbforge-connect".into()],
        not_before: 100,
        expires_at: 10_000,
        status: KeyStatus::Active,
    }
}
fn store() -> TrustStore {
    trust_store()
        .with_minisign_anchors(vec![minisign_anchor()])
        .unwrap()
}

#[test]
fn accepts_real_tauri_prehashed_signatures_and_keeps_v1_compatible() {
    let store = store();
    let verified = verify_catalog(&envelope("catalog"), &store, 300, None).unwrap();
    assert_eq!(verified.catalog().tools().len(), 2);
    assert_eq!(
        serde_json::to_value(verified.checkpoint()).unwrap()["metadataSchemaVersion"],
        2
    );
    assert_eq!(verified.expires_at(), 1000);
    assert!(verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &store,
        300,
        None
    )
    .is_ok());
}

#[test]
fn cloned_local_trust_retains_both_key_types_and_authenticated_format_getter() {
    let cloned = store().clone();
    let v2 = verify_catalog(&envelope("catalog"), &cloned, 300, None).unwrap();
    assert_eq!(v2.metadata_schema_version(), 2);
    let v1 = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &cloned,
        300,
        None,
    )
    .unwrap();
    assert_eq!(v1.metadata_schema_version(), 1);
}

#[test]
fn verifies_domain_exact_bytes_key_id_and_the_authenticated_comment() {
    let store = store();
    let mut altered = fixtures()["cases"]["catalog"].clone();
    altered["signed"] = json!(altered["signed"]
        .as_str()
        .unwrap()
        .replace("FBForge Connect", "Other Product"));
    assert_eq!(
        verify_catalog(&altered.to_string(), &store, 300, None).unwrap_err(),
        TrustError::InvalidSignature
    );
    assert_eq!(
        verify_catalog(&envelope("wrongDomain"), &store, 300, None).unwrap_err(),
        TrustError::InvalidSignature
    );
    for change in ["comment", "global", "keyId", "trailing", "missingGlobal"] {
        let mut altered = fixtures()["cases"]["catalog"].clone();
        let decoded = String::from_utf8(
            STANDARD
                .decode(altered["signature"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        let mut lines: Vec<String> = decoded.lines().map(str::to_owned).collect();
        match change {
            "comment" => lines[2] = lines[2].replace("version:0.1.0", "version:0.2.0"),
            "global" => lines[3] = STANDARD.encode([0u8; 64]),
            "keyId" => {
                let mut signature = STANDARD.decode(&lines[1]).unwrap();
                signature[2] ^= 1;
                lines[1] = STANDARD.encode(signature);
            }
            "missingGlobal" => {
                lines.pop();
            }
            _ => lines.push("ignored content must not be accepted".into()),
        }
        altered["signature"] = json!(STANDARD.encode(lines.join("\n")));
        assert!(
            verify_catalog(&altered.to_string(), &store, 300, None).is_err(),
            "{change}"
        );
    }
}

#[test]
fn provisioned_minisign_keys_are_typed_purpose_scoped_and_revocable() {
    assert_eq!(
        verify_catalog(&envelope("catalog"), &trust_store(), 300, None).unwrap_err(),
        TrustError::UnknownKey
    );
    for change in [
        "purpose", "revoked", "expired", "future", "keyId", "lifetime",
    ] {
        let mut anchor = minisign_anchor();
        match change {
            "purpose" => anchor.subjects = vec!["release/fbforge-inspect".into()],
            "revoked" => anchor.status = KeyStatus::Revoked,
            "expired" => anchor.expires_at = 299,
            "future" => anchor.not_before = 301,
            "lifetime" => anchor.expires_at = 900,
            _ => {
                let key = String::from_utf8(STANDARD.decode(&anchor.public_key).unwrap()).unwrap();
                let mut lines: Vec<String> = key.lines().map(str::to_owned).collect();
                let mut bytes = STANDARD.decode(&lines[1]).unwrap();
                bytes[2] ^= 1;
                lines[1] = STANDARD.encode(bytes);
                anchor.public_key = STANDARD.encode(lines.join("\n"));
            }
        }
        let store = TrustStore::new(vec![])
            .unwrap()
            .with_minisign_anchors(vec![anchor])
            .unwrap();
        assert!(
            verify_catalog(&envelope("catalog"), &store, 300, None).is_err(),
            "{change}"
        );
    }
    let mut altered = fixtures()["cases"]["catalog"].clone();
    altered["keyId"] = json!("test-key");
    assert_eq!(
        verify_catalog(&altered.to_string(), &store(), 300, None).unwrap_err(),
        TrustError::UnknownKey
    );
}

#[test]
fn malformed_or_ambiguous_local_minisign_anchors_are_rejected() {
    for value in ["", "invalid", &STANDARD.encode("not a minisign public key")] {
        let mut anchor = minisign_anchor();
        anchor.public_key = value.into();
        assert!(trust_store().with_minisign_anchors(vec![anchor]).is_err());
    }
    assert!(trust_store()
        .with_minisign_anchors(vec![minisign_anchor(), minisign_anchor()])
        .is_err());
    let mut collision = minisign_anchor();
    collision.id = "test-key".into();
    assert!(trust_store()
        .with_minisign_anchors(vec![collision])
        .is_err());
    let mut invalid = minisign_anchor();
    invalid.subjects = vec!["../catalog".into()];
    assert!(trust_store().with_minisign_anchors(vec![invalid]).is_err());
}

#[test]
fn rejects_format_confusion_duplicate_fields_invalid_lifetimes_and_metadata_replay() {
    let store = store();
    for name in ["innerV1", "duplicatePayload", "duplicateBody"] {
        assert!(
            verify_catalog(&envelope(name), &store, 300, None).is_err(),
            "{name}"
        );
    }
    assert_eq!(
        verify_catalog(&envelope("catalog"), &store, 1000, None).unwrap_err(),
        TrustError::Expired
    );
    assert_eq!(
        verify_catalog(&envelope("catalog"), &store, 150, None).unwrap_err(),
        TrustError::NotYetValid
    );
    let first = verify_catalog(&envelope("catalog2"), &store, 300, None).unwrap();
    assert!(verify_catalog(&envelope("catalog2"), &store, 301, Some(first.checkpoint())).is_ok());
    assert_eq!(
        verify_catalog(&envelope("catalog2"), &store, 299, Some(first.checkpoint())).unwrap_err(),
        TrustError::ClockRollback
    );
    assert_eq!(
        verify_catalog(&envelope("catalog"), &store, 301, Some(first.checkpoint())).unwrap_err(),
        TrustError::Rollback
    );
    assert_eq!(
        verify_catalog(
            &envelope("catalogConflict"),
            &store,
            301,
            Some(first.checkpoint())
        )
        .unwrap_err(),
        TrustError::ConflictingSequence
    );
    let duplicate = envelope("catalog").replacen(
        "\"schemaVersion\":2",
        "\"schemaVersion\":2,\"schemaVersion\":2",
        1,
    );
    assert!(verify_catalog(&duplicate, &store, 300, None).is_err());
    let mut confused = fixtures()["cases"]["catalog"].clone();
    confused["schemaVersion"] = json!(1);
    assert!(verify_catalog(&confused.to_string(), &store, 300, None).is_err());
    confused["schemaVersion"] = json!(2);
    confused["publicKey"] = fixtures()["publicKey"].clone();
    assert!(verify_catalog(&confused.to_string(), &store, 300, None).is_err());

    let raw: Value =
        serde_json::from_str(&sign(&payload("catalog", catalog_body()), "test-key", 42)).unwrap();
    let mut wrong_format = fixtures()["cases"]["catalog"].clone();
    wrong_format["signature"] = raw["signature"].clone();
    assert!(verify_catalog(&wrong_format.to_string(), &store, 300, None).is_err());
    let mut wrong_format = raw;
    wrong_format["signature"] = fixtures()["cases"]["catalog"]["signature"].clone();
    assert!(verify_catalog(&wrong_format.to_string(), &store, 300, None).is_err());
}

#[test]
fn migration_requires_a_new_sequence_and_never_reverts_to_v1() {
    let store = store();
    let first = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &store,
        300,
        None,
    )
    .unwrap();
    assert_eq!(
        verify_catalog(&envelope("catalog"), &store, 301, Some(first.checkpoint())).unwrap_err(),
        TrustError::ConflictingSequence
    );
    let next =
        verify_catalog(&envelope("catalog2"), &store, 301, Some(first.checkpoint())).unwrap();
    let mut v1 = payload("catalog", catalog_body());
    v1["sequence"] = json!(3);
    assert_eq!(
        verify_catalog(
            &sign(&v1, "test-key", 42),
            &store,
            302,
            Some(next.checkpoint())
        )
        .unwrap_err(),
        TrustError::Rollback
    );
    let mut legacy = serde_json::to_value(first.checkpoint()).unwrap();
    legacy
        .as_object_mut()
        .unwrap()
        .remove("metadataSchemaVersion");
    let restored: MetadataCheckpoint = serde_json::from_value(legacy).unwrap();
    assert!(verify_catalog(&envelope("catalog2"), &store, 301, Some(&restored)).is_ok());
}

#[test]
fn release_and_package_identity_checks_apply_to_v2_envelopes() {
    let store = store();
    let catalog = verify_catalog(&envelope("catalog"), &store, 300, None).unwrap();
    let verify = |name| {
        verify_release(
            &envelope(name),
            &store,
            300,
            None,
            &catalog,
            "fbforge-connect",
            &Version::parse("0.3.0").unwrap(),
        )
    };
    let release = verify("release").unwrap();
    assert_eq!(
        release
            .verify_package(b"abc".as_slice())
            .unwrap()
            .byte_count(),
        3
    );
    assert_eq!(
        release.verify_package(b"abd".as_slice()).unwrap_err(),
        TrustError::PackageMismatch
    );
    assert_eq!(
        verify("wrongPackage").unwrap_err(),
        TrustError::InvalidRelease
    );
    assert_eq!(
        verify("withdrawnRelease")
            .unwrap()
            .verify_package(b"abc".as_slice())
            .unwrap_err(),
        TrustError::WithdrawnRelease
    );
}

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-minisign-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf {
        self.0.join("state")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn cached_v2_catalog_survives_restart_and_blocks_format_downgrade() {
    let directory = Directory::new();
    let trust = store();
    {
        let mut saved = StateStore::open(&directory.store()).unwrap();
        saved
            .accept_catalog(&envelope("catalog2"), &trust, 300)
            .unwrap();
    }
    let mut saved = StateStore::open(&directory.store()).unwrap();
    let verified = saved.load_catalog(&trust, 301).unwrap();
    assert_eq!(
        serde_json::to_value(verified.checkpoint()).unwrap()["metadataSchemaVersion"],
        2
    );
    let mut older = payload("catalog", catalog_body());
    older["sequence"] = json!(3);
    assert_eq!(
        saved
            .accept_catalog(&sign(&older, "test-key", 42), &trust, 302)
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
    let builtin = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let recovered = saved.load_inventory(&builtin, &trust, 1000).unwrap();
    assert_eq!(
        recovered.catalog_status,
        CatalogCacheStatus::Unavailable(TrustError::Expired)
    );
    assert_eq!(recovered.inventory.entries().count(), 2);
}

#[test]
fn cached_v2_checkpoint_cannot_lose_its_format_binding() {
    use ring::digest;
    let directory = Directory::new();
    {
        let mut saved = StateStore::open(&directory.store()).unwrap();
        saved
            .accept_catalog(&envelope("catalog2"), &store(), 300)
            .unwrap();
    }
    let path = directory.store().join("state-v1.json");
    let original: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for remove_field in [false, true] {
        let mut envelope = original.clone();
        let mut state: Value = serde_json::from_str(envelope["payload"].as_str().unwrap()).unwrap();
        if remove_field {
            state["catalog"]["checkpoint"]
                .as_object_mut()
                .unwrap()
                .remove("metadataSchemaVersion");
        } else {
            state["catalog"]["checkpoint"]["metadataSchemaVersion"] = json!(1);
        }
        let encoded = state.to_string();
        let checksum: String = digest::digest(&digest::SHA256, encoded.as_bytes())
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        envelope["payload"] = json!(encoded);
        envelope["sha256"] = json!(checksum);
        fs::write(&path, envelope.to_string()).unwrap();
        assert!(matches!(
            StateStore::open(&directory.store()),
            Err(StoreError::CorruptState)
        ));
    }
}
