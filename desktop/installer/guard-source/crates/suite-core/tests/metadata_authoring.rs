use fbforge_suite_core::{
    catalog::parse_catalog,
    trust::authoring::{prepare_catalog, prepare_release, MetadataHeader, PreparedMessage},
};
use semver::Version;
use serde_json::Value;

fn fixtures() -> Value {
    serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap()
}
fn body(case: &str) -> String {
    let signed = fixtures()["cases"][case]["signed"]
        .as_str()
        .unwrap()
        .to_owned();
    serde_json::from_str::<Value>(&signed).unwrap()["body"].to_string()
}
fn header() -> MetadataHeader {
    MetadataHeader {
        sequence: 7,
        issued_at: 200,
        expires_at: 1000,
    }
}

#[test]
fn prepares_exact_domain_and_utf8_payload_without_claiming_trust() {
    let body = body("catalog").replace("\"schemaVersion\":1", "\"schemaVersion\" : 1");
    let prepared = prepare_catalog(&body, header()).unwrap();
    let bytes = prepared.as_bytes();
    assert!(bytes.starts_with(b"FBForge Suite metadata v2\0{"));
    let signed = std::str::from_utf8(&bytes[b"FBForge Suite metadata v2\0".len()..]).unwrap();
    assert_eq!(signed, format!("{{\"schemaVersion\":2,\"subject\":\"catalog\",\"sequence\":7,\"issuedAt\":200,\"expiresAt\":1000,\"body\":{body}}}"));
    let roundtrip = PreparedMessage::from_bytes(bytes).unwrap();
    assert_eq!(roundtrip.as_bytes(), bytes);
    for invalid in [
        bytes[1..].to_vec(),
        b"FBForge Suite metadata v2\\0{}".to_vec(),
        [b"FBForge Suite metadata v2\0".as_slice(), &[0xff]].concat(),
    ] {
        assert!(PreparedMessage::from_bytes(&invalid).is_err());
    }
}

#[test]
fn preparation_rejects_duplicate_bodies_bad_intervals_and_unbounded_messages() {
    let catalog = body("catalog");
    let duplicate = catalog.replacen(
        "\"schemaVersion\":1",
        "\"schemaVersion\":1,\"schemaVersion\":1",
        1,
    );
    assert!(prepare_catalog(&duplicate, header()).is_err());
    for header in [
        MetadataHeader {
            sequence: 0,
            ..header()
        },
        MetadataHeader {
            expires_at: 200,
            ..header()
        },
        MetadataHeader {
            expires_at: 199,
            ..header()
        },
        MetadataHeader {
            expires_at: 200 + 30 * 24 * 60 * 60 + 1,
            ..header()
        },
    ] {
        assert!(prepare_catalog(&catalog, header).is_err());
    }
    assert!(prepare_catalog(
        &catalog,
        MetadataHeader {
            expires_at: 200 + 30 * 24 * 60 * 60,
            ..header()
        }
    )
    .is_ok());
    assert!(prepare_catalog(&" ".repeat(1_048_577), header()).is_err());
    let message = prepare_catalog(&catalog, header()).unwrap();
    let changed = String::from_utf8(message.as_bytes().to_vec())
        .unwrap()
        .replacen("\"sequence\":7", "\"sequence\":7,\"sequence\":7", 1);
    assert!(PreparedMessage::from_bytes(changed.as_bytes()).is_err());
}

#[test]
fn unsigned_release_preparation_reuses_catalog_and_release_identity_contracts() {
    let catalog = parse_catalog(&body("catalog")).unwrap();
    let release = body("release");
    let version = Version::parse("0.3.0").unwrap();
    let prepared =
        prepare_release(&release, &catalog, "fbforge-connect", &version, header()).unwrap();
    assert!(String::from_utf8_lossy(prepared.as_bytes())
        .contains("\"subject\":\"release/fbforge-connect\""));
    assert!(prepare_release(&release, &catalog, "fbforge-inspect", &version, header()).is_err());
    assert!(prepare_release(
        &release,
        &catalog,
        "fbforge-connect",
        &Version::parse("0.4.0").unwrap(),
        header()
    )
    .is_err());
    for (field, value) in [
        ("appId", Value::from("other.app")),
        ("sha256", Value::from("bad")),
        ("size", Value::from(0)),
        ("authenticodePolicyId", Value::from("other-policy")),
    ] {
        let mut changed: Value = serde_json::from_str(&release).unwrap();
        changed[field] = value;
        assert!(
            prepare_release(
                &changed.to_string(),
                &catalog,
                "fbforge-connect",
                &version,
                header()
            )
            .is_err(),
            "{field}"
        );
    }
    let duplicate = release.replacen(
        "\"schemaVersion\":1",
        "\"schemaVersion\":1,\"schemaVersion\":1",
        1,
    );
    assert!(prepare_release(&duplicate, &catalog, "fbforge-connect", &version, header()).is_err());
}
