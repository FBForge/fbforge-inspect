use super::{metadata::verify_payload, KeyStatus, TrustError};
use crate::production::{bundled_metadata_trust_store, METADATA_KEY_ID, SUITE_UPDATER_PUBLIC_KEY};
use serde_json::{json, Value};

#[test]
fn production_factory_provisions_one_typed_key_with_exact_operator_scopes() {
    let store = bundled_metadata_trust_store().unwrap();
    assert!(store.anchors.is_empty());
    assert_eq!(store.minisign_anchors.len(), 1);
    let anchor = store
        .minisign_anchors
        .get("fbforge-metadata-789a6f09cb345b6e")
        .unwrap();
    assert_eq!(anchor.id, METADATA_KEY_ID);
    assert_eq!(anchor.public_key, SUITE_UPDATER_PUBLIC_KEY);
    assert_eq!(anchor.status, KeyStatus::Active);
    assert_eq!((anchor.not_before, anchor.expires_at), (0, u64::MAX));
    assert_eq!(
        anchor.subjects,
        [
            "catalog",
            "release/fbforge-connect",
            "release/fbforge-inspect"
        ]
    );
}

#[test]
fn production_key_authority_stops_at_the_explicit_metadata_subjects() {
    let store = bundled_metadata_trust_store().unwrap();
    let fixtures: Value =
        serde_json::from_str(include_str!("../../tests/fixtures/minisign-v2.json")).unwrap();
    let mut envelope = fixtures["cases"]["catalog"].clone();
    envelope["keyId"] = json!(METADATA_KEY_ID);
    for subject in [
        "catalog",
        "release/fbforge-connect",
        "release/fbforge-inspect",
    ] {
        // An allowed purpose reaches cryptographic verification, which must
        // reject the real public dummy signature under the production key.
        assert_eq!(
            verify_payload(&envelope.to_string(), &store, subject, 300, None).err(),
            Some(TrustError::InvalidSignature),
            "{subject}"
        );
    }
    for subject in [
        "release/fbforge-suite",
        "release/another-tool",
        "catalog/other",
    ] {
        assert_eq!(
            verify_payload(&envelope.to_string(), &store, subject, 300, None).err(),
            Some(TrustError::WrongSubject),
            "{subject}"
        );
    }
}
