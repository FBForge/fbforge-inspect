mod support;
use fbforge_suite_core::trust::{verify_catalog, verify_release, TrustError, TrustStore};
use semver::Version;
use serde_json::{json, Value};
use std::io::{self, Cursor, Read};
use support::*;

fn release_body() -> Value {
    json!({"schemaVersion":1,"toolId":"fbforge-connect","appId":"org.fitconnect.integration-client",
        "version":"0.3.0","platform":"windows-x64","installer":"nsis-current-user",
        "repository":"FBForge/fbforge-connect","releaseTag":"v0.3.0",
        "assetName":"FBForge.Connect_0.3.0_x64-setup.exe","size":3,
        "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "authenticodePolicyId":"fbforge-connect-production","status":"available",
        "requiredFiles":["fbforge-connect.exe","engine/fbforge-engine.jar"]})
}

#[test]
fn binds_a_signed_release_to_the_catalog_tool_version_platform_and_bytes() {
    let store = trust_store();
    let catalog = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &store,
        300,
        None,
    )
    .unwrap();
    let release = verify_release(
        &sign(
            &payload("release/fbforge-connect", release_body()),
            "test-key",
            42,
        ),
        &store,
        300,
        None,
        &catalog,
        "fbforge-connect",
        &Version::parse("0.3.0").unwrap(),
    )
    .unwrap();
    assert_eq!(release.download_url(), "https://github.com/FBForge/fbforge-connect/releases/download/v0.3.0/FBForge.Connect_0.3.0_x64-setup.exe");
    assert_eq!(
        release
            .verify_package(Cursor::new(b"abc"))
            .unwrap()
            .byte_count(),
        3
    );
    for bytes in [b"abd".as_slice(), b"ab", b"abcd"] {
        assert_eq!(
            release.verify_package(Cursor::new(bytes)).unwrap_err(),
            TrustError::PackageMismatch
        );
    }
    struct Unreadable;
    impl Read for Unreadable {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private path",
            ))
        }
    }
    let error = release.verify_package(Unreadable).unwrap_err();
    assert_eq!(error, TrustError::PackageRead);
    assert!(!error.to_string().contains("private path"));
}

#[test]
fn rejects_package_substitution_and_unsafe_download_or_installation_fields() {
    let store = trust_store();
    let catalog = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &store,
        300,
        None,
    )
    .unwrap();
    for (field, value) in [
        ("toolId", json!("fbforge-inspect")),
        ("appId", json!("de.fbforge.other")),
        ("version", json!("0.2.0")),
        ("platform", json!("linux-x64")),
        ("installer", json!("shell")),
        ("repository", json!("Other/other")),
        ("releaseTag", json!("v0.3.0/../latest")),
        ("assetName", json!("../setup.exe")),
        ("authenticodePolicyId", json!("attacker")),
        ("sha256", json!("00")),
        ("size", json!(0)),
        ("size", json!(17_179_869_185u64)),
        ("requiredFiles", json!(["../escape"])),
        ("requiredFiles", json!(["C:/escape"])),
        ("requiredFiles", json!(["engine/CON.jar"])),
        ("requiredFiles", json!(["engine/file:stream"])),
        ("requiredFiles", json!(["tool.exe", "TOOL.exe"])),
        ("requiredFiles", json!([])),
        ("requiredFiles", json!(["engine/only.jar"])),
        ("status", json!("unknown")),
        ("downloadUrl", json!("https://evil.invalid/tool.exe")),
    ] {
        let mut body = release_body();
        body[field] = value;
        let result = verify_release(
            &sign(&payload("release/fbforge-connect", body), "test-key", 42),
            &store,
            300,
            None,
            &catalog,
            "fbforge-connect",
            &Version::parse("0.3.0").unwrap(),
        );
        assert!(result.is_err(), "{field}");
    }
}

#[test]
fn rechecks_catalog_trust_and_blocks_withdrawn_releases() {
    let store = trust_store();
    let catalog = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &store,
        300,
        None,
    )
    .unwrap();
    let mut body = release_body();
    body["status"] = json!("withdrawn");
    let envelope = sign(&payload("release/fbforge-connect", body), "test-key", 42);
    let version = Version::parse("0.3.0").unwrap();
    let release = verify_release(
        &envelope,
        &store,
        300,
        None,
        &catalog,
        "fbforge-connect",
        &version,
    )
    .unwrap();
    assert_eq!(
        release.verify_package(Cursor::new(b"abc")).unwrap_err(),
        TrustError::WithdrawnRelease
    );
    assert!(verify_release(
        &envelope,
        &store,
        1000,
        None,
        &catalog,
        "fbforge-connect",
        &version
    )
    .is_err());
    // The catalogue signing key is revoked while a different release key stays active.
    let mut release_key = anchor("release-test", 43);
    release_key.subjects = vec!["release/fbforge-connect".into()];
    let changed_store = TrustStore::new(vec![release_key]).unwrap();
    let envelope = sign(
        &payload("release/fbforge-connect", release_body()),
        "release-test",
        43,
    );
    assert_eq!(
        verify_release(
            &envelope,
            &changed_store,
            300,
            None,
            &catalog,
            "fbforge-connect",
            &version
        )
        .unwrap_err(),
        TrustError::UnknownKey
    );
}
