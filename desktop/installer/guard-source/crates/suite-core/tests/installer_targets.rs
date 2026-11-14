use fbforge_suite_core::installer_targets::{parse_payload_manifest, TargetError};
use serde_json::{json, Value};

fn payload(files: Value) -> Value {
    json!({"schemaVersion":1,"toolId":"fbforge-connect","appId":"org.fitconnect.integration-client","version":"0.4.0","files":files})
}
fn file(path: &str, size: u64) -> Value {
    json!({"path":path,"size":size,"sha256":"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"})
}
#[test]
fn active_payload_is_bound_to_explicit_component_selection() {
    let mut scanner = file("resources\\scanner\\engine.dll", 12);
    scanner["component"] = "scanner".into();
    let raw = payload(json!([file("fbforge-connect.exe", 7), scanner])).to_string();
    let base = parse_payload_manifest(raw.as_bytes(), &[]).unwrap();
    assert_eq!(base.files().len(), 1);
    assert_eq!(base.total_bytes(), 7);
    let full = parse_payload_manifest(raw.as_bytes(), &["scanner".to_string()]).unwrap();
    assert_eq!(full.files().len(), 2);
    assert_eq!(full.total_bytes(), 19);
    assert_ne!(base.selection_sha256(), full.selection_sha256());
    assert_eq!(
        parse_payload_manifest(raw.as_bytes(), &["missing".into()]).unwrap_err(),
        TargetError::InvalidManifest
    );
    assert_eq!(
        parse_payload_manifest(raw.as_bytes(), &["scanner".into(), "scanner".into()]).unwrap_err(),
        TargetError::InvalidManifest
    );
}
#[test]
fn relative_windows_paths_cannot_escape_alias_or_overwrite_control_files() {
    for path in [
        "",
        "..\\x",
        "a\\..\\x",
        "C:\\x",
        "\\\\server\\x",
        "a/x",
        "a\\b:stream",
        "NUL.txt",
        "a.\\b",
        "a\\b ",
        ".fbforge-install.pending",
        ".fbforge-installation.v1.json",
        "uninstall.exe",
        "uninstall.exe\\child",
        ".fbforge-install.pending\\child",
    ] {
        assert!(
            parse_payload_manifest(payload(json!([file(path, 0)])).to_string().as_bytes(), &[])
                .is_err(),
            "{path}"
        );
    }
    for files in [
        json!([file("A.dll", 0), file("a.DLL", 0)]),
        json!([file("a", 0), file("a\\b.dll", 0)]),
    ] {
        assert!(parse_payload_manifest(payload(files).to_string().as_bytes(), &[]).is_err());
    }
}
#[test]
fn manifest_is_strict_and_never_reinterprets_sizes_hashes_or_versions() {
    let original = payload(json!([file("fbforge-connect.exe", 0)]));
    assert!(parse_payload_manifest(original.to_string().as_bytes(), &[]).is_ok());
    for (key, value) in [
        ("schemaVersion", json!(2)),
        ("version", json!("v0.4.0")),
        ("shell", json!("x")),
    ] {
        let mut bad = original.clone();
        bad[key] = value;
        assert!(parse_payload_manifest(bad.to_string().as_bytes(), &[]).is_err());
    }
    for (key, value) in [
        ("size", json!(-1)),
        ("sha256", json!("AA".repeat(32))),
        ("command", json!("x")),
    ] {
        let mut bad = original.clone();
        bad["files"][0][key] = value;
        assert!(parse_payload_manifest(bad.to_string().as_bytes(), &[]).is_err());
    }
    assert!(parse_payload_manifest(payload(json!([])).to_string().as_bytes(), &[]).is_err());
    assert!(parse_payload_manifest(
        payload(json!([file("huge.bin", 32 * 1024 * 1024 * 1024 + 1)]))
            .to_string()
            .as_bytes(),
        &[]
    )
    .is_err());
    assert!(parse_payload_manifest(
        payload(json!([file(&vec!["d"; 33].join("\\"), 0)]))
            .to_string()
            .as_bytes(),
        &[]
    )
    .is_err());
}

#[test]
fn path_and_component_limits_match_the_native_nsis_string_budget() {
    let legal = format!("{}\\{}\\x", "a".repeat(254), "b".repeat(254));
    assert_eq!(legal.encode_utf16().count(), 511);
    let mut longest = legal.clone();
    longest.push('x');
    assert!(parse_payload_manifest(
        payload(json!([file(&longest, 0)])).to_string().as_bytes(),
        &[]
    )
    .is_ok());
    longest.push('x');
    assert!(parse_payload_manifest(
        payload(json!([file(&longest, 0)])).to_string().as_bytes(),
        &[]
    )
    .is_err());
    let files: Vec<_> = (0..9)
        .map(|index| {
            let mut entry = file(&format!("file{index}"), 0);
            entry["component"] = format!("component{index}").into();
            entry
        })
        .collect();
    assert!(parse_payload_manifest(payload(json!(files)).to_string().as_bytes(), &[]).is_err());
}

#[test]
fn manifest_size_and_entry_count_are_bounded_before_native_io() {
    assert_eq!(
        parse_payload_manifest(&vec![b' '; 4 * 1024 * 1024 + 1], &[]).unwrap_err(),
        TargetError::InvalidManifest
    );
    let entries: Vec<_> = (0..8193)
        .map(|index| file(&format!("file{index}"), 0))
        .collect();
    assert_eq!(
        parse_payload_manifest(payload(json!(entries)).to_string().as_bytes(), &[]).unwrap_err(),
        TargetError::InvalidManifest
    );
}
