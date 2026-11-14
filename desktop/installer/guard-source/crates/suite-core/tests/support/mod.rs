use fbforge_suite_core::trust::{KeyStatus, TrustAnchor, TrustStore};
use ring::signature::{Ed25519KeyPair, KeyPair};
use serde_json::{json, Value};

// Deliberately public test seed. Never included in a production trust store.
pub fn key(seed: u8) -> Ed25519KeyPair {
    Ed25519KeyPair::from_seed_unchecked(&[seed; 32]).unwrap()
}

pub fn trust_store() -> TrustStore {
    TrustStore::new(vec![anchor("test-key", 42)]).unwrap()
}

pub fn anchor(id: &str, seed: u8) -> TrustAnchor {
    TrustAnchor {
        id: id.into(),
        public_key: key(seed).public_key().as_ref().try_into().unwrap(),
        subjects: vec!["catalog".into(), "release/fbforge-connect".into()],
        not_before: 100,
        expires_at: 10_000,
        status: KeyStatus::Active,
    }
}

pub fn catalog_body() -> Value {
    serde_json::from_str(include_str!("../../../../catalog/tools.v1.json")).unwrap()
}

pub fn payload(subject: &str, body: Value) -> Value {
    json!({"schemaVersion":1,"subject":subject,"sequence":1,
        "issuedAt":200,"expiresAt":1000,"body":body})
}

pub fn sign(payload: &Value, id: &str, seed: u8) -> String {
    sign_text(&payload.to_string(), id, seed)
}

pub fn sign_text(signed: &str, id: &str, seed: u8) -> String {
    let mut message = b"FBForge Suite metadata v1\0".to_vec();
    message.extend_from_slice(signed.as_bytes());
    let signature = key(seed).sign(&message);
    let hex: String = signature
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    json!({"schemaVersion":1,"keyId":id,"signed":signed,"signature":hex}).to_string()
}
