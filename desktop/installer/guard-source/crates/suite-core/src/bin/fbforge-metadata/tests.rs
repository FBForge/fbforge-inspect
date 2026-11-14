use super::*;
use fbforge_suite_core::trust::{KeyStatus, MinisignTrustAnchor};
use serde_json::Value;
fn fixtures() -> Value {
    serde_json::from_str(include_str!("../../../tests/fixtures/minisign-v2.json")).unwrap()
}
fn dummy_store() -> TrustStore {
    TrustStore::new(vec![])
        .unwrap()
        .with_minisign_anchors(vec![MinisignTrustAnchor {
            id: fbforge_suite_core::production::METADATA_KEY_ID.into(),
            public_key: fixtures()["publicKey"].as_str().unwrap().into(),
            subjects: vec!["catalog".into(), "release/fbforge-connect".into()],
            not_before: 100,
            expires_at: 10_000,
            status: KeyStatus::Active,
        }])
        .unwrap()
}
fn envelope(name: &str) -> String {
    let mut envelope = fixtures()["cases"][name].clone();
    envelope["keyId"] = Value::from(fbforge_suite_core::production::METADATA_KEY_ID);
    envelope.to_string()
}

#[test]
fn package_command_rechecks_product_version_and_time_before_file_io() {
    let store = dummy_store();
    let policies = bundled_authenticode_policies().unwrap();
    for (tool, version, now) in [
        ("fbforge-inspect", "0.3.0", 300),
        ("fbforge-connect", "0.4.0", 300),
        ("fbforge-connect", "0.3.0", 1000),
    ] {
        let error = check_package_at(
            &envelope("catalog"),
            &envelope("release"),
            (tool, &Version::parse(version).unwrap()),
            Path::new("missing-package.exe"),
            &store,
            &policies,
            now,
        )
        .unwrap_err();
        assert!(matches!(error, CliError::Trust(_)), "{error:?}");
    }
}

#[cfg(not(windows))]
#[test]
fn unix_hash_match_is_an_explicit_non_success_and_wrong_bytes_are_rejected() {
    let path = std::env::temp_dir().join(format!("suite-authoring-package-{}", std::process::id()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    file.write_all(b"abc").unwrap();
    drop(file);
    let store = dummy_store();
    let policies = bundled_authenticode_policies().unwrap();
    let version = Version::parse("0.3.0").unwrap();
    let result = check_package_at(
        &envelope("catalog"),
        &envelope("release"),
        ("fbforge-connect", &version),
        &path,
        &store,
        &policies,
        300,
    );
    let error = result.unwrap_err();
    assert!(matches!(error, CliError::WindowsUnavailable));
    assert_eq!(error.exit_code(), 2);
    std::fs::write(&path, b"bad").unwrap();
    assert!(matches!(
        check_package_at(
            &envelope("catalog"),
            &envelope("release"),
            ("fbforge-connect", &version),
            &path,
            &store,
            &policies,
            300
        ),
        Err(CliError::Trust(TrustError::PackageMismatch))
    ));
    std::fs::remove_file(path).unwrap();
}
