use super::{journal::RECEIPT, manifest, parse_payload_manifest, TargetError};
use crate::catalog::ToolManifest;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    path::Path,
    time::{Duration, Instant},
};

pub(super) const MAX_RECEIPT_BYTES: usize = manifest::MAX_MANIFEST_BYTES + 65536;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    schema_version: u32,
    tool_id: String,
    app_id: String,
    version: String,
    source_sha256: String,
    selection_sha256: String,
    files: Vec<SavedFile>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct SavedFile {
    path: String,
    size: u64,
    sha256: String,
}

// Additional deny guard only. The caller separately proves the private tree,
// actual signed old product version and current lease before any mutation.
// A private receipt is bookkeeping, never a publisher/ownership assertion.
pub(super) fn check_existing(
    root: &Path,
    bytes: &[u8],
    tool: &ToolManifest,
    version: &Version,
) -> Result<(), TargetError> {
    let denied = TargetError::ManualMigrationRequired;
    if bytes.len() > MAX_RECEIPT_BYTES {
        return Err(denied);
    }
    let receipt: Receipt = serde_json::from_slice(bytes).map_err(|_| denied)?;
    let digest = |s: &str| {
        s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    if receipt.schema_version != 1
        || receipt.tool_id != tool.id
        || receipt.app_id != tool.app_id
        || receipt.version != version.to_string()
        || !digest(&receipt.source_sha256)
        || !digest(&receipt.selection_sha256)
    {
        return Err(denied);
    }
    // Reuse existing size/path/hash/version/duplicate limits after direct typed
    // decoding has already rejected duplicate and unknown receipt/file fields.
    let raw = serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"toolId":receipt.tool_id,"appId":receipt.app_id,"version":receipt.version,"files":receipt.files})).map_err(|_| denied)?;
    let payload = parse_payload_manifest(&raw, &[]).map_err(|_| denied)?;
    let mut files: BTreeSet<String> = [RECEIPT.to_string(), "uninstall.exe".to_string()].into();
    let mut directories = BTreeSet::new();
    for file in payload.files() {
        let path = file.path().to_ascii_lowercase();
        for (offset, _) in path.match_indices('\\') {
            directories.insert(path[..offset].to_string());
        }
        files.insert(path);
    }
    // Read the actual namespace each time, including empty directories. An
    // earlier MutationTree snapshot cannot account for additions during staging.
    let started = Instant::now();
    let mut pending = vec![(root.to_path_buf(), 0)];
    let (mut objects, mut units) = (0usize, 0usize);
    while let Some((parent, depth)) = pending.pop() {
        for child in fs::read_dir(parent).map_err(|_| denied)? {
            if started.elapsed() > Duration::from_secs(5) {
                return Err(denied);
            }
            let path = child.map_err(|_| denied)?.path();
            objects += 1;
            units += path.as_os_str().to_string_lossy().encode_utf16().count();
            if objects > manifest::MAX_FILES || depth >= 32 || units > 4 * 1024 * 1024 {
                return Err(denied);
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|_| denied)?
                .to_str()
                .ok_or(denied)?
                .replace(std::path::MAIN_SEPARATOR, "\\")
                .to_ascii_lowercase();
            let metadata = fs::symlink_metadata(&path).map_err(|_| denied)?;
            #[cfg(windows)]
            let reparse = {
                use std::os::windows::fs::MetadataExt;
                metadata.file_attributes() & 0x400 != 0
            };
            #[cfg(not(windows))]
            let reparse = metadata.file_type().is_symlink();
            if reparse {
                return Err(denied);
            }
            if metadata.is_dir() && directories.contains(&relative) {
                pending.push((path, depth + 1));
            } else if !metadata.is_file() || !files.contains(&relative) {
                return Err(denied);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "fbforge-preserve-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            fs::create_dir(root.join("resources")).unwrap();
            fs::write(root.join("old-name.exe"), b"old fixture").unwrap();
            fs::write(root.join("resources/retired.dll"), b"old resource").unwrap();
            fs::write(root.join(RECEIPT), serde_json::to_vec(&receipt()).unwrap()).unwrap();
            fs::write(root.join("uninstall.exe"), b"inert owned control fixture").unwrap();
            Self(root)
        }
        fn check(&self, receipt: &[u8]) -> Result<(), TargetError> {
            let catalog =
                crate::catalog::parse_catalog(include_str!("../../../../catalog/tools.v1.json"))
                    .unwrap();
            check_existing(
                &self.0,
                receipt,
                &catalog.tools()[0],
                &Version::new(0, 4, 0),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
    fn receipt() -> Value {
        json!({"schemaVersion":1,"toolId":"fbforge-connect","appId":"org.fitconnect.integration-client","version":"0.4.0","sourceSha256":"00".repeat(32),"selectionSha256":"00".repeat(32),"files":[{"path":"old-name.exe","size":11,"sha256":"00".repeat(32)},{"path":"resources\\retired.dll","size":12,"sha256":"00".repeat(32)}]})
    }
    #[test]
    fn old_receipt_allows_retired_or_renamed_payload_without_using_new_payload() {
        let fixture = Fixture::new();
        assert_eq!(
            fixture.check(&serde_json::to_vec(&receipt()).unwrap()),
            Ok(())
        );
    }
    #[test]
    fn additional_file_or_empty_directory_blocks_before_moving_anything() {
        for directory in [false, true] {
            let fixture = Fixture::new();
            let extra = fixture.0.join("private-notes");
            if directory {
                fs::create_dir(&extra).unwrap()
            } else {
                fs::write(&extra, b"user data").unwrap()
            }
            assert_eq!(
                fixture.check(&serde_json::to_vec(&receipt()).unwrap()),
                Err(TargetError::ManualMigrationRequired)
            );
            assert!(extra.exists());
            assert!(fixture.0.join("old-name.exe").exists());
        }
    }
    #[test]
    fn late_additions_are_checked_again_instead_of_using_a_preparation_snapshot() {
        for directory in [false, true] {
            let fixture = Fixture::new();
            let receipt = serde_json::to_vec(&receipt()).unwrap();
            fixture.check(&receipt).unwrap();
            let extra = fixture.0.join("added-after-prepare");
            if directory {
                fs::create_dir(&extra).unwrap()
            } else {
                fs::write(&extra, b"user data").unwrap()
            }
            assert_eq!(
                fixture.check(&receipt),
                Err(TargetError::ManualMigrationRequired)
            );
        }
    }
    #[test]
    fn missing_foreign_invalid_duplicate_or_oversized_receipts_never_allow_swap() {
        let fixture = Fixture::new();
        for (field, value) in [
            ("toolId", json!("other")),
            ("appId", json!("other.app")),
            ("version", json!("0.5.0")),
            ("schemaVersion", json!(2)),
            ("sourceSha256", json!("not-a-hash")),
            ("extra", json!(true)),
        ] {
            let mut bad = receipt();
            bad[field] = value;
            assert_eq!(
                fixture.check(&serde_json::to_vec(&bad).unwrap()),
                Err(TargetError::ManualMigrationRequired)
            );
        }
        let raw = serde_json::to_string(&receipt()).unwrap();
        let duplicate = raw.replacen("{", "{\"schemaVersion\":1,", 1);
        let nested_duplicate = raw.replacen("\"path\":", "\"size\":1,\"path\":", 1);
        for bad in [
            Vec::new(),
            duplicate.into_bytes(),
            nested_duplicate.into_bytes(),
            vec![b' '; MAX_RECEIPT_BYTES + 1],
        ] {
            assert_eq!(
                fixture.check(&bad),
                Err(TargetError::ManualMigrationRequired)
            );
        }
        for path in ["..\\user-data", "C:\\private", "uninstall.exe"] {
            let mut bad = receipt();
            bad["files"][0]["path"] = path.into();
            assert_eq!(
                fixture.check(&serde_json::to_vec(&bad).unwrap()),
                Err(TargetError::ManualMigrationRequired)
            );
        }
    }
}
