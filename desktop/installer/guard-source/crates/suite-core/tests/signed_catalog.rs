mod support;
use fbforge_suite_core::trust::{verify_catalog, KeyStatus, TrustError, TrustStore};
use serde_json::{json, Value};
use support::*;

#[test]
fn verifies_real_ed25519_signatures_and_binds_the_exact_payload() {
    let store = trust_store();
    let input = sign(&payload("catalog", catalog_body()), "test-key", 42);
    let verified = verify_catalog(&input, &store, 300, None).unwrap();
    assert_eq!(verified.catalog().tools().len(), 2);
    let mut tampered: Value = serde_json::from_str(&input).unwrap();
    tampered["signed"] = json!(tampered["signed"]
        .as_str()
        .unwrap()
        .replace("Inspect", "Other"));
    assert_eq!(
        verify_catalog(&tampered.to_string(), &store, 300, None).unwrap_err(),
        TrustError::InvalidSignature
    );
    let wrong_key = sign(&payload("catalog", catalog_body()), "test-key", 43);
    assert_eq!(
        verify_catalog(&wrong_key, &store, 300, None).unwrap_err(),
        TrustError::InvalidSignature
    );
}

#[test]
fn rejects_unprovisioned_revoked_expired_and_wrong_scope_keys() {
    let input = sign(&payload("catalog", catalog_body()), "test-key", 42);
    let empty = TrustStore::new(vec![]).unwrap();
    assert_eq!(
        verify_catalog(&input, &empty, 300, None).unwrap_err(),
        TrustError::UnknownKey
    );
    let mut entry = anchor("test-key", 42);
    entry.status = KeyStatus::Revoked;
    let store = TrustStore::new(vec![entry]).unwrap();
    assert_eq!(
        verify_catalog(&input, &store, 300, None).unwrap_err(),
        TrustError::KeyUnavailable
    );
    let mut entry = anchor("test-key", 42);
    entry.expires_at = 250;
    let store = TrustStore::new(vec![entry]).unwrap();
    assert_eq!(
        verify_catalog(&input, &store, 300, None).unwrap_err(),
        TrustError::KeyUnavailable
    );
    let mut entry = anchor("test-key", 42);
    entry.subjects = vec!["release/fbforge-connect".into()];
    let store = TrustStore::new(vec![entry]).unwrap();
    assert_eq!(
        verify_catalog(&input, &store, 300, None).unwrap_err(),
        TrustError::WrongSubject
    );
}

#[test]
fn rejects_duplicate_fields_inside_the_signed_catalog_body() {
    let ambiguous = r#"{"schemaVersion":1,"subject":"catalog","sequence":1,"issuedAt":200,"expiresAt":1000,"body":{"schemaVersion":2,"schemaVersion":1,"tools":[]}}"#;
    assert_eq!(
        verify_catalog(
            &sign_text(ambiguous, "test-key", 42),
            &trust_store(),
            300,
            None
        )
        .unwrap_err(),
        TrustError::InvalidCatalog
    );
}

#[test]
fn enforces_expiry_not_yet_valid_and_key_lifetime_without_an_offline_bypass() {
    let store = trust_store();
    for (issued, expires, now) in [
        (200, 1000, 1000),
        (301, 1000, 300),
        (200, 200, 300),
        (99, 1000, 300),
        (200, 10001, 300),
    ] {
        let mut data = payload("catalog", catalog_body());
        data["issuedAt"] = json!(issued);
        data["expiresAt"] = json!(expires);
        assert!(verify_catalog(&sign(&data, "test-key", 42), &store, now, None).is_err());
    }
}

#[test]
fn permits_rechecking_cache_but_rejects_rollback_and_same_sequence_equivocation() {
    let store = trust_store();
    let first = payload("catalog", catalog_body());
    let first_envelope = sign(&first, "test-key", 42);
    let verified = verify_catalog(&first_envelope, &store, 300, None).unwrap();
    assert!(verify_catalog(&first_envelope, &store, 301, Some(verified.checkpoint())).is_ok());
    assert!(verify_catalog(&first_envelope, &store, 299, Some(verified.checkpoint())).is_err());
    let mut changed = first.clone();
    changed["expiresAt"] = json!(1001);
    assert_eq!(
        verify_catalog(
            &sign(&changed, "test-key", 42),
            &store,
            300,
            Some(verified.checkpoint())
        )
        .unwrap_err(),
        TrustError::ConflictingSequence
    );
    changed["sequence"] = json!(2);
    let newer = verify_catalog(
        &sign(&changed, "test-key", 42),
        &store,
        300,
        Some(verified.checkpoint()),
    )
    .unwrap();
    assert_eq!(
        verify_catalog(&first_envelope, &store, 300, Some(newer.checkpoint())).unwrap_err(),
        TrustError::Rollback
    );
}

#[test]
fn supports_provisioned_rotation_and_never_imports_keys_from_an_envelope() {
    let store = TrustStore::new(vec![anchor("old-test", 42), anchor("new-test", 43)]).unwrap();
    let first = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "old-test", 42),
        &store,
        300,
        None,
    )
    .unwrap();
    let mut second = payload("catalog", catalog_body());
    second["sequence"] = json!(2);
    assert!(verify_catalog(
        &sign(&second, "new-test", 43),
        &store,
        300,
        Some(first.checkpoint())
    )
    .is_ok());
    let mut injected: Value = serde_json::from_str(&sign(&second, "unknown", 44)).unwrap();
    injected["publicKey"] = json!("attacker supplied key");
    assert_eq!(
        verify_catalog(&injected.to_string(), &store, 300, None).unwrap_err(),
        TrustError::InvalidEnvelope
    );
    assert!(TrustStore::new(vec![anchor("duplicate", 42), anchor("duplicate", 43)]).is_err());
}

#[test]
fn rejects_wrong_document_type_invalid_catalog_and_unbounded_input() {
    let store = trust_store();
    assert_eq!(
        verify_catalog(
            &sign(
                &payload("release/fbforge-connect", catalog_body()),
                "test-key",
                42
            ),
            &store,
            300,
            None
        )
        .unwrap_err(),
        TrustError::WrongSubject
    );
    assert_eq!(
        verify_catalog(
            &sign(
                &payload("catalog", json!({"schemaVersion":2,"tools":[]})),
                "test-key",
                42
            ),
            &store,
            300,
            None
        )
        .unwrap_err(),
        TrustError::InvalidCatalog
    );
    assert!(verify_catalog(&" ".repeat(2_097_153), &store, 300, None).is_err());
    let duplicate = r#"{"schemaVersion":1,"schemaVersion":1,"subject":"catalog","sequence":1,"issuedAt":200,"expiresAt":1000,"body":{}}"#;
    assert_eq!(
        verify_catalog(&sign_text(duplicate, "test-key", 42), &store, 300, None).unwrap_err(),
        TrustError::InvalidEnvelope
    );
}
