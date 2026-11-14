use super::*;
use crate::{
    production::METADATA_KEY_ID,
    trust::{KeyStatus, MinisignTrustAnchor, TrustStore},
};
use serde_json::Value;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/minisign-v2.json")).unwrap()
}
fn message(case: &str) -> PreparedMessage {
    let fixtures = fixtures();
    let signed = fixtures["cases"][case]["signed"].as_str().unwrap();
    PreparedMessage::from_bytes(&[metadata::DOMAIN_V2, signed.as_bytes()].concat()).unwrap()
}
fn signature(case: &str) -> String {
    fixtures()["cases"][case]["signature"]
        .as_str()
        .unwrap()
        .into()
}
fn dummy_store() -> TrustStore {
    TrustStore::new(vec![])
        .unwrap()
        .with_minisign_anchors(vec![MinisignTrustAnchor {
            id: METADATA_KEY_ID.into(),
            public_key: fixtures()["publicKey"].as_str().unwrap().into(),
            subjects: vec!["catalog".into(), "release/fbforge-connect".into()],
            not_before: 100,
            expires_at: 10_000,
            status: KeyStatus::Active,
        }])
        .unwrap()
}

#[test]
fn assembly_independently_verifies_complete_box_and_preserves_signed_bytes() {
    let prepared = message("catalog");
    let box_text = signature("catalog");
    let payload: Payload =
        serde_json::from_slice(&prepared.as_bytes()[metadata::DOMAIN_V2.len()..]).unwrap();
    let regenerated = prepare_catalog(
        payload.body.get(),
        MetadataHeader {
            sequence: 1,
            issued_at: 200,
            expires_at: 1000,
        },
    )
    .unwrap();
    assert_eq!(
        regenerated.as_bytes(),
        prepared.as_bytes(),
        "prepare output matches the actual externally signed fixture bytes"
    );
    let envelope = assemble_at(
        &regenerated,
        &box_text,
        AssemblyContext::Catalog,
        &dummy_store(),
        300,
    )
    .unwrap();
    let parsed: Value = serde_json::from_str(&envelope).unwrap();
    assert_eq!(parsed["schemaVersion"], 2);
    assert_eq!(parsed["keyId"], METADATA_KEY_ID);
    assert_eq!(parsed["signature"], box_text);
    assert_eq!(
        parsed["signed"].as_str().unwrap().as_bytes(),
        &prepared.as_bytes()[metadata::DOMAIN_V2.len()..]
    );
    assert!(assemble_at(
        &prepared,
        &box_text,
        AssemblyContext::Catalog,
        &dummy_store(),
        1000
    )
    .is_err());
    assert!(assemble_at(
        &prepared,
        &box_text,
        AssemblyContext::Catalog,
        &dummy_store(),
        150
    )
    .is_err());
    assert!(assemble_at(
        &message("wrongDomain"),
        &signature("wrongDomain"),
        AssemblyContext::Catalog,
        &dummy_store(),
        300
    )
    .is_err());
    let changed = PreparedMessage::from_bytes(
        &String::from_utf8(prepared.as_bytes().to_vec())
            .unwrap()
            .replace("FBForge Connect", "Other Product")
            .into_bytes(),
    )
    .unwrap();
    assert!(assemble_at(
        &changed,
        &box_text,
        AssemblyContext::Catalog,
        &dummy_store(),
        300
    )
    .is_err());
    let truncated = &box_text[..box_text.len() / 2];
    assert!(assemble_at(
        &prepared,
        truncated,
        AssemblyContext::Catalog,
        &dummy_store(),
        300
    )
    .is_err());
    assert_eq!(assemble_catalog(&prepared, &box_text), Err(TrustError::InvalidSignature),
        "the production API must reject this genuine dummy signature cryptographically, before its expired fixture timestamp matters");
}

#[test]
fn release_assembly_rechecks_catalog_purpose_product_and_expected_version() {
    let store = dummy_store();
    let catalog = assemble_at(
        &message("catalog"),
        &signature("catalog"),
        AssemblyContext::Catalog,
        &store,
        300,
    )
    .unwrap();
    let version = Version::parse("0.3.0").unwrap();
    let context = AssemblyContext::Release {
        catalog: &catalog,
        tool_id: "fbforge-connect",
        version: &version,
    };
    let release = assemble_at(
        &message("release"),
        &signature("release"),
        context,
        &store,
        300,
    )
    .unwrap();
    assert!(release.contains(METADATA_KEY_ID));
    assert!(assemble_at(
        &message("release"),
        &signature("release"),
        AssemblyContext::Catalog,
        &store,
        300
    )
    .is_err());
    assert!(assemble_at(
        &message("release"),
        &signature("release"),
        AssemblyContext::Release {
            catalog: &catalog,
            tool_id: "fbforge-inspect",
            version: &version
        },
        &store,
        300
    )
    .is_err());
    assert!(assemble_at(
        &message("release"),
        &signature("release"),
        AssemblyContext::Release {
            catalog: &catalog,
            tool_id: "fbforge-connect",
            version: &Version::parse("0.4.0").unwrap()
        },
        &store,
        300
    )
    .is_err());
    assert!(assemble_at(
        &message("release"),
        &signature("release"),
        AssemblyContext::Release {
            catalog: "{}",
            tool_id: "fbforge-connect",
            version: &version
        },
        &store,
        300
    )
    .is_err());
}
