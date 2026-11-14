use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
};
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-authoring-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn file(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn run(arguments: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fbforge-metadata"))
        .args(arguments)
        .env_clear()
        .output()
        .unwrap()
}
fn body(case: &str) -> String {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap();
    serde_json::from_str::<Value>(fixtures["cases"][case]["signed"].as_str().unwrap()).unwrap()
        ["body"]
        .to_string()
}
fn prepare(input: &Path, output: &Path) -> Output {
    run(&[
        "prepare-catalog".as_ref(),
        "--body".as_ref(),
        input.as_os_str(),
        "--sequence".as_ref(),
        "1".as_ref(),
        "--issued-at".as_ref(),
        "200".as_ref(),
        "--expires-at".as_ref(),
        "1000".as_ref(),
        "--out".as_ref(),
        output.as_os_str(),
    ])
}
#[test]
fn cli_prepares_binary_message_and_never_clobbers_existing_output() {
    let directory = Directory::new();
    let input = directory.file("body.json");
    let output = directory.file("message.bin");
    fs::write(&input, body("catalog")).unwrap();
    let first = prepare(&input, &output);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let bytes = fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"FBForge Suite metadata v2\0{"));
    let second = prepare(&input, &output);
    assert!(!second.status.success());
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert!(!String::from_utf8_lossy(&second.stderr).contains("suite-authoring-"));
    assert!(!prepare(&input, &input).status.success());
    assert_eq!(fs::read_to_string(&input).unwrap(), body("catalog"));
}

#[test]
fn help_with_trailing_arguments_never_executes_a_later_command() {
    let directory = Directory::new();
    let input = directory.file("body.json");
    let output = directory.file("message.bin");
    fs::write(&input, body("catalog")).unwrap();
    let result = run(&[
        "--help".as_ref(),
        "ignored".as_ref(),
        "prepare-catalog".as_ref(),
        "--body".as_ref(),
        input.as_os_str(),
        "--sequence".as_ref(),
        "1".as_ref(),
        "--issued-at".as_ref(),
        "200".as_ref(),
        "--expires-at".as_ref(),
        "1000".as_ref(),
        "--out".as_ref(),
        output.as_os_str(),
    ]);
    assert!(!result.status.success());
    assert!(!output.exists());
}
#[test]
fn cli_requires_explicit_headers_rejects_overrides_and_bounds_input() {
    for arguments in [
        vec!["prepare-catalog"],
        vec!["prepare-catalog", "--now", "300"],
        vec!["prepare-catalog", "--key-id", "dummy"],
        vec!["prepare-catalog", "--sequence", "1", "--sequence", "2"],
        vec!["check-package", "--skip-authenticode", "true"],
    ] {
        let arguments: Vec<_> = arguments.iter().map(std::ffi::OsStr::new).collect();
        assert!(!run(&arguments).status.success());
    }
    let directory = Directory::new();
    let input = directory.file("too-large.json");
    let output = directory.file("message.bin");
    fs::write(&input, vec![b' '; 2_097_153]).unwrap();
    assert!(!prepare(&input, &output).status.success());
    assert!(!output.exists());
}
#[test]
fn public_cli_rejects_a_real_dummy_signature_without_creating_an_envelope() {
    let directory = Directory::new();
    let input = directory.file("message.bin");
    let sig = directory.file("message.bin.sig");
    let output = directory.file("suite-catalog.v2.json");
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap();
    fs::write(
        &input,
        [
            b"FBForge Suite metadata v2\0".as_slice(),
            fixtures["cases"]["catalog"]["signed"]
                .as_str()
                .unwrap()
                .as_bytes(),
        ]
        .concat(),
    )
    .unwrap();
    fs::write(
        &sig,
        fixtures["cases"]["catalog"]["signature"].as_str().unwrap(),
    )
    .unwrap();
    let result = run(&[
        "assemble-catalog".as_ref(),
        "--message".as_ref(),
        input.as_os_str(),
        "--signature".as_ref(),
        sig.as_os_str(),
        "--out".as_ref(),
        output.as_os_str(),
    ]);
    assert!(!result.status.success());
    assert!(!output.exists());
}

#[test]
fn cli_release_preparation_uses_explicit_unsigned_catalog_and_expected_version() {
    let directory = Directory::new();
    let catalog = directory.file("catalog-body.json");
    let release = directory.file("release-body.json");
    let output = directory.file("release-message.bin");
    fs::write(&catalog, body("catalog")).unwrap();
    fs::write(&release, body("release")).unwrap();
    let arguments = [
        "prepare-release".as_ref(),
        "--body".as_ref(),
        release.as_os_str(),
        "--catalog-body".as_ref(),
        catalog.as_os_str(),
        "--tool".as_ref(),
        "fbforge-connect".as_ref(),
        "--version".as_ref(),
        "0.3.0".as_ref(),
        "--sequence".as_ref(),
        "1".as_ref(),
        "--issued-at".as_ref(),
        "200".as_ref(),
        "--expires-at".as_ref(),
        "1000".as_ref(),
        "--out".as_ref(),
        output.as_os_str(),
    ];
    let result = run(&arguments);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = fs::read(&output).unwrap();
    assert!(String::from_utf8_lossy(&bytes).contains("\"subject\":\"release/fbforge-connect\""));
}

#[cfg(unix)]
#[test]
fn cli_rejects_symlink_and_fifo_inputs_and_does_not_follow_existing_output_links() {
    use std::{ffi::CString, os::unix::fs::symlink};
    let directory = Directory::new();
    let input = directory.file("body.json");
    let output = directory.file("message.bin");
    let link = directory.file("link.json");
    fs::write(&input, body("catalog")).unwrap();
    symlink(&input, &link).unwrap();
    assert!(!prepare(&link, &output).status.success());
    assert!(!output.exists());
    symlink(&input, &output).unwrap();
    assert!(!prepare(&input, &output).status.success());
    assert_eq!(fs::read_to_string(&input).unwrap(), body("catalog"));
    let fifo = directory.file("input.fifo");
    let name = CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    // SAFETY: fixture-owned temporary pathname and private FIFO permissions.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    assert!(!prepare(&fifo, &directory.file("fifo-message.bin"))
        .status
        .success());
}
