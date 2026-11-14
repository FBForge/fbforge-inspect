use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "cooperation marker-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn marker_matches_configured_native_app_identity_and_exact_v1_bytes() {
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    assert_eq!(config["identifier"].as_str(), Some(APP_ID));
    assert_eq!(APP_ID, crate::app_coordination::APP_ID);
    let expected_app = "de.fbforge.inspect";
    let mut expected = b"FBFORGE\0\x01\0\x01\0".to_vec();
    expected.extend_from_slice(&(expected_app.len() as u16).to_le_bytes());
    expected.extend_from_slice(expected_app.as_bytes());
    assert_eq!(marker_bytes(), expected);
    assert_eq!(expected.len(), 14 + expected_app.len());
}

#[test]
fn cargo_target_embeds_on_both_windows_targets_without_host_cfg() {
    let fixture = Fixture::new();
    for target in ["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"] {
        let rc = resource_for_target(target, &fixture.0).unwrap().unwrap();
        assert!(rc.starts_with("LANGUAGE 0, 0\n17986 RCDATA \""));
        assert_eq!(
            fs::read(fixture.0.join("fbforge-cooperation.bin")).unwrap(),
            marker_bytes()
        );
    }
}

#[test]
fn other_targets_do_not_generate_or_include_marker() {
    let fixture = Fixture::new();
    for target in [
        "x86_64-unknown-linux-gnu",
        "aarch64-apple-darwin",
        "wasm32-unknown-unknown",
    ] {
        assert_eq!(resource_for_target(target, &fixture.0).unwrap(), None);
    }
    assert_eq!(fs::read_dir(&fixture.0).unwrap().count(), 0);
}

#[test]
fn rc_filename_escapes_backslashes_quotes_and_rejects_control_characters() {
    assert_eq!(
        rc_filename(r#"C:\Build path\a"b\marker.bin"#).unwrap(),
        r#""C:\\Build path\\a""b\\marker.bin""#
    );
    for text in ["bad\npath", "bad\rpath", "bad\0path", "bad\tpath"] {
        assert!(rc_filename(text).is_err());
    }
}
