mod support;
use fbforge_suite_core::{
    network::{CancellationToken, DownloadResult, NetworkClient, NetworkError},
    settings::UserSettings,
    storage::{PackageCacheError, PackageCacheLimits, StateStore, StoreError},
    trust::{TrustError, TrustStore},
};
use semver::Version;
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::*;

const TOOL: &str = "fbforge-connect";
const BYTES: &[u8] = b"a small genuine package fixture";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-package-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn root(&self) -> PathBuf {
        self.0.join("state")
    }
    fn cache(&self) -> PathBuf {
        self.root().join("packages-v1")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn version() -> Version {
    Version::parse("0.3.0").unwrap()
}
fn sha(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn release(bytes: &[u8], seq: u64, status: &str) -> String {
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap();
    let mut signed: Value =
        serde_json::from_str(fixtures["cases"]["release"]["signed"].as_str().unwrap()).unwrap();
    signed["schemaVersion"] = json!(1);
    signed["sequence"] = json!(seq);
    signed["body"]["size"] = json!(bytes.len());
    signed["body"]["sha256"] = json!(sha(bytes));
    signed["body"]["status"] = json!(status);
    sign(&signed, "test-key", 42)
}
fn store(directory: &Directory, trust: &TrustStore) -> StateStore {
    let mut store = StateStore::open(&directory.root()).unwrap();
    store
        .accept_catalog(
            &sign(&payload("catalog", catalog_body()), "test-key", 42),
            trust,
            300,
        )
        .unwrap();
    store
        .accept_release(
            &release(BYTES, 1, "available"),
            trust,
            300,
            TOOL,
            &version(),
        )
        .unwrap();
    store
}
fn received(n: usize) -> Result<DownloadResult, NetworkError> {
    Ok(DownloadResult {
        bytes_received: n as u64,
    })
}
fn package_files(directory: &Directory) -> Vec<PathBuf> {
    fs::read_dir(directory.cache())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|e| e == "pkg"))
        .collect()
}

#[test]
fn publishes_only_checked_content_and_rechecks_a_hit_after_restart() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    {
        let mut store = store(&directory, &trust);
        let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
        let mut stage = cache
            .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
            .unwrap();
        assert!(!stage.is_cached());
        stage.write_all(BYTES).unwrap();
        assert!(package_files(&directory).is_empty());
        stage
            .verify_bytes(Some(received(BYTES.len())), &cancel)
            .unwrap();
        let mut package = stage.finish(&mut store, &trust, || 301, &cancel).unwrap();
        let mut actual = Vec::new();
        package.read_to_end(&mut actual).unwrap();
        assert_eq!(actual, BYTES);
        assert_eq!(
            package_files(&directory),
            vec![directory.cache().join(format!("{}.pkg", sha(BYTES)))]
        );
    }
    let mut store = StateStore::open(&directory.root()).unwrap();
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 302, TOOL, &version(), &cancel)
        .unwrap();
    assert!(stage.is_cached());
    stage.verify_bytes(None, &cancel).unwrap();
    stage.finish(&mut store, &trust, || 303, &cancel).unwrap();
    drop(cache);
    drop(store);
    let mut store = StateStore::open(&directory.root()).unwrap();
    assert!(matches!(
        store.load_release(&trust, 302, TOOL, &version()),
        Err(StoreError::Trust(TrustError::ClockRollback))
    ));
}

#[test]
fn a_native_cache_hit_verifies_bytes_without_requesting_a_network_transfer() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store.package_cache(Default::default()).unwrap();
    {
        let mut stage = cache
            .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
            .unwrap();
        stage.write_all(BYTES).unwrap();
        stage
            .verify_bytes(Some(received(BYTES.len())), &cancel)
            .unwrap();
        drop(stage.finish(&mut store, &trust, || 301, &cancel).unwrap());
    }
    let mut stage = cache
        .prepare(&mut store, &trust, 302, TOOL, &version(), &cancel)
        .unwrap();
    assert!(stage.is_cached());
    stage
        .download_and_verify_bytes(&NetworkClient::new(Default::default()), &cancel, |_| {
            panic!("a cache hit must not invent network progress")
        })
        .unwrap();
    let mut package = stage.finish(&mut store, &trust, || 303, &cancel).unwrap();
    let mut bytes = Vec::new();
    package.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, BYTES);
}

#[cfg(not(windows))]
#[test]
fn unsupported_native_transfer_never_publishes_or_finishes_download_staging() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store.package_cache(Default::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    assert!(!stage.is_cached());
    assert!(matches!(
        stage.download_and_verify_bytes(
            &NetworkClient::new(Default::default()),
            &cancel,
            |_| panic!("unsupported transport must not report received bytes")
        ),
        Err(PackageCacheError::Network(
            NetworkError::UnsupportedPlatform
        ))
    ));
    assert!(stage.write_all(BYTES).is_err());
    assert!(matches!(
        stage.finish(&mut store, &trust, || 301, &cancel),
        Err(PackageCacheError::InvalidState)
    ));
    assert!(package_files(&directory).is_empty());
}

#[test]
fn cancelled_native_cache_verification_never_publishes() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store.package_cache(Default::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    cancel.cancel();
    assert!(matches!(
        stage.download_and_verify_bytes(&NetworkClient::new(Default::default()), &cancel, |_| {}),
        Err(PackageCacheError::Cancelled)
    ));
    assert!(stage.write_all(BYTES).is_err());
    assert!(stage.finish(&mut store, &trust, || 301, &cancel).is_err());
    assert!(package_files(&directory).is_empty());
}

#[test]
fn native_source_setup_error_poisons_staging_before_any_transfer() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    // A structurally valid signed version may exceed the transport URL bound.
    let version = Version::parse(&format!("0.3.0+{}", "a".repeat(40_000))).unwrap();
    let envelope: Value = serde_json::from_str(&release(BYTES, 2, "available")).unwrap();
    let mut payload: Value = serde_json::from_str(envelope["signed"].as_str().unwrap()).unwrap();
    payload["body"]["version"] = json!(version.to_string());
    payload["body"]["releaseTag"] = json!(format!("v{version}"));
    store
        .accept_release(&sign(&payload, "test-key", 42), &trust, 301, TOOL, &version)
        .unwrap();
    let mut cache = store.package_cache(Default::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 302, TOOL, &version, &cancel)
        .unwrap();
    assert!(matches!(
        stage.download_and_verify_bytes(
            &NetworkClient::new(Default::default()),
            &cancel,
            |_| panic!("source setup failure cannot report network progress")
        ),
        Err(PackageCacheError::Network(NetworkError::InvalidUrl))
    ));
    assert!(stage.write_all(BYTES).is_err());
    assert!(matches!(
        stage.finish(&mut store, &trust, || 303, &cancel),
        Err(PackageCacheError::InvalidState)
    ));
    assert!(package_files(&directory).is_empty());
}

#[test]
fn transfer_error_or_cancellation_and_unsigned_bytes_never_publish() {
    for scenario in [
        "error",
        "cancel",
        "short",
        "hash",
        "oversize",
        "sealed-write",
    ] {
        let directory = Directory::new();
        let trust = trust_store();
        let cancel = CancellationToken::default();
        let mut store = store(&directory, &trust);
        let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
        let mut stage = cache
            .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
            .unwrap();
        match scenario {
            "short" => stage.write_all(&BYTES[..3]).unwrap(),
            "hash" => stage.write_all(&[b'x'; BYTES.len()]).unwrap(),
            "oversize" => {
                assert!(stage.write_all(&[b'x'; BYTES.len() + 1]).is_err());
            }
            _ => stage.write_all(BYTES).unwrap(),
        }
        if scenario == "cancel" {
            cancel.cancel();
        }
        let result = if scenario == "error" {
            Err(NetworkError::Offline)
        } else {
            received(BYTES.len())
        };
        let checked = stage.verify_bytes(Some(result), &cancel);
        if scenario == "sealed-write" {
            checked.unwrap();
            assert!(stage.write_all(b"x").is_err());
        } else {
            assert!(checked.is_err(), "{scenario}");
        }
        assert!(
            stage.finish(&mut store, &trust, || 301, &cancel).is_err(),
            "{scenario}"
        );
        assert!(package_files(&directory).is_empty(), "{scenario}");
    }
}

#[test]
fn settings_and_withdrawal_can_change_during_download_and_hash_phase() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    let mut settings = UserSettings::default();
    settings.set_search_interval_seconds(900).unwrap();
    store.save_settings(&settings).unwrap();
    stage.write_all(BYTES).unwrap();
    stage
        .verify_bytes(Some(received(BYTES.len())), &cancel)
        .unwrap();
    store
        .accept_release(
            &release(BYTES, 2, "withdrawn"),
            &trust,
            301,
            TOOL,
            &version(),
        )
        .unwrap();
    assert!(matches!(
        stage.finish(&mut store, &trust, || 302, &cancel),
        Err(PackageCacheError::Trust(TrustError::WithdrawnRelease))
    ));
    assert!(package_files(&directory).is_empty());
    assert_eq!(
        store.load_settings().unwrap().search_interval_seconds(),
        900
    );
}

#[test]
fn cache_owns_the_original_store_lock_and_rejects_foreign_store_pairing() {
    let directory = Directory::new();
    let other = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let store = store(&directory, &trust);
    let mut foreign = StateStore::open(&other.root()).unwrap();
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    assert!(matches!(
        store.package_cache(PackageCacheLimits::default()),
        Err(PackageCacheError::Busy)
    ));
    assert!(matches!(
        cache.prepare(&mut foreign, &trust, 300, TOOL, &version(), &cancel),
        Err(PackageCacheError::WrongStore)
    ));
    drop(store);
    assert!(matches!(
        StateStore::open(&directory.root()),
        Err(StoreError::Busy)
    ));
    drop(cache);
    assert!(StateStore::open(&directory.root()).is_ok());
}

fn publish_fixture(directory: &Directory, store: &mut StateStore, trust: &TrustStore) {
    let cancel = CancellationToken::default();
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let mut stage = cache
        .prepare(store, trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    stage.write_all(BYTES).unwrap();
    stage
        .verify_bytes(Some(received(BYTES.len())), &cancel)
        .unwrap();
    stage.finish(store, trust, || 300, &cancel).unwrap();
    assert_eq!(package_files(directory).len(), 1);
}
fn private_file(path: &std::path::Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
    }
}

#[test]
fn limits_are_native_bounded_and_include_the_reserved_staging_size() {
    assert_eq!(
        PackageCacheLimits::default(),
        PackageCacheLimits::new(4 * 1024 * 1024 * 1024, 16).unwrap()
    );
    for (bytes, files) in [(0, 1), (1, 0), (32 * 1024 * 1024 * 1024 + 1, 1), (1, 65)] {
        assert_eq!(
            PackageCacheLimits::new(bytes, files),
            Err(PackageCacheError::InvalidLimits)
        );
    }
    assert!(PackageCacheLimits::new(32 * 1024 * 1024 * 1024, 64).is_ok());
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store
        .package_cache(PackageCacheLimits::new(BYTES.len() as u64 - 1, 1).unwrap())
        .unwrap();
    assert!(matches!(
        cache.prepare(&mut store, &trust, 300, TOOL, &version(), &cancel),
        Err(PackageCacheError::Full)
    ));
    assert!(package_files(&directory).is_empty());
}

#[test]
fn abandoned_staging_is_cleaned_on_restart_and_drop_does_not_publish() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    {
        let mut stage = cache
            .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
            .unwrap();
        stage.write_all(b"partial").unwrap();
    }
    assert_eq!(fs::read_dir(directory.cache()).unwrap().count(), 1);
    drop(cache);
    drop(store);
    let orphan = directory
        .cache()
        .join(".package-0123456789abcdef0123456789abcdef.part");
    private_file(&orphan, b"interrupted transfer");
    let store = StateStore::open(&directory.root()).unwrap();
    let _cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    assert!(!orphan.exists());
    assert!(package_files(&directory).is_empty());
}

#[test]
fn every_cache_hit_requires_fresh_bytes_and_fresh_finish() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    publish_fixture(&directory, &mut store, &trust);
    let file = package_files(&directory).pop().unwrap();
    private_file(&file, &[b'x'; BYTES.len()]);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    assert!(stage.is_cached());
    assert!(matches!(
        stage.verify_bytes(None, &cancel),
        Err(PackageCacheError::Trust(TrustError::PackageMismatch))
    ));
    assert!(stage.finish(&mut store, &trust, || 301, &cancel).is_err());
    drop(cache);
    private_file(&file, BYTES);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    assert!(stage.is_cached());
    assert!(matches!(
        stage.finish(&mut store, &trust, || 301, &cancel),
        Err(PackageCacheError::InvalidState)
    ));
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    assert!(stage.write_all(b"x").is_err());
    assert!(stage.verify_bytes(None, &cancel).is_err());
}

#[test]
fn finish_rechecks_current_key_time_product_and_package_identity() {
    for scenario in [
        "expired",
        "revoked",
        "changed-bytes",
        "clock",
        "foreign-store",
        "cancel-before-publish",
    ] {
        let directory = Directory::new();
        let other = Directory::new();
        let trust = trust_store();
        let cancel = CancellationToken::default();
        let mut store = store(&directory, &trust);
        let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
        let mut stage = cache
            .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
            .unwrap();
        stage.write_all(BYTES).unwrap();
        stage
            .verify_bytes(Some(received(BYTES.len())), &cancel)
            .unwrap();
        if scenario == "changed-bytes" {
            store
                .accept_release(
                    &release(b"different signed bytes", 2, "available"),
                    &trust,
                    301,
                    TOOL,
                    &version(),
                )
                .unwrap();
        }
        let mut revoked = anchor("test-key", 42);
        revoked.status = fbforge_suite_core::trust::KeyStatus::Revoked;
        let revoked = TrustStore::new(vec![revoked]).unwrap();
        let mut foreign = StateStore::open(&other.root()).unwrap();
        let current = if scenario == "foreign-store" {
            &mut foreign
        } else {
            &mut store
        };
        let now = match scenario {
            "expired" => 1000,
            "clock" => 299,
            _ => 302,
        };
        let error = stage
            .finish(
                current,
                if scenario == "revoked" {
                    &revoked
                } else {
                    &trust
                },
                || {
                    if scenario == "cancel-before-publish" {
                        cancel.cancel();
                    }
                    now
                },
                &cancel,
            )
            .err()
            .unwrap();
        match scenario {
            "expired" => assert_eq!(
                error,
                PackageCacheError::Store(StoreError::Trust(TrustError::Expired))
            ),
            "revoked" => assert_eq!(
                error,
                PackageCacheError::Store(StoreError::Trust(TrustError::KeyUnavailable))
            ),
            "changed-bytes" => assert_eq!(error, PackageCacheError::Corrupt),
            "clock" => assert_eq!(
                error,
                PackageCacheError::Store(StoreError::Trust(TrustError::ClockRollback))
            ),
            "foreign-store" => assert_eq!(error, PackageCacheError::WrongStore),
            _ => assert_eq!(error, PackageCacheError::Cancelled),
        }
        assert!(package_files(&directory).is_empty(), "{scenario}");
    }
}

#[test]
fn cache_hit_withdrawal_blocks_after_hash_without_losing_the_checkpoint() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    publish_fixture(&directory, &mut store, &trust);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    assert!(stage.is_cached());
    stage.verify_bytes(None, &cancel).unwrap();
    store
        .accept_release(
            &release(BYTES, 2, "withdrawn"),
            &trust,
            301,
            TOOL,
            &version(),
        )
        .unwrap();
    assert!(matches!(
        stage.finish(&mut store, &trust, || 302, &cancel),
        Err(PackageCacheError::Trust(TrustError::WithdrawnRelease))
    ));
    drop(cache);
    drop(store);
    let mut store = StateStore::open(&directory.root()).unwrap();
    assert_eq!(
        store
            .accept_release(
                &release(BYTES, 1, "available"),
                &trust,
                302,
                TOOL,
                &version()
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
}

#[test]
fn eviction_limits_bytes_and_entries_without_deleting_release_checkpoints() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    publish_fixture(&directory, &mut store, &trust);
    let next = b"a different package";
    store
        .accept_release(
            &release(next, 2, "available"),
            &trust,
            300,
            TOOL,
            &version(),
        )
        .unwrap();
    let mut cache = store
        .package_cache(PackageCacheLimits::new(BYTES.len() as u64, 1).unwrap())
        .unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    assert!(package_files(&directory).is_empty());
    stage.write_all(next).unwrap();
    stage
        .verify_bytes(Some(received(next.len())), &cancel)
        .unwrap();
    stage.finish(&mut store, &trust, || 300, &cancel).unwrap();
    assert_eq!(
        package_files(&directory),
        vec![directory.cache().join(format!("{}.pkg", sha(next)))]
    );
    drop(cache);
    drop(store);
    let mut store = StateStore::open(&directory.root()).unwrap();
    assert_eq!(
        store
            .accept_release(
                &release(BYTES, 1, "available"),
                &trust,
                300,
                TOOL,
                &version()
            )
            .unwrap_err(),
        StoreError::Trust(TrustError::Rollback)
    );
}

#[cfg(unix)]
#[test]
fn changed_or_replaced_staging_after_hash_never_publishes_or_deletes_a_foreign_leaf() {
    for replaced in [false, true] {
        let directory = Directory::new();
        let trust = trust_store();
        let cancel = CancellationToken::default();
        let mut store = store(&directory, &trust);
        let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
        let mut stage = cache
            .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
            .unwrap();
        stage.write_all(BYTES).unwrap();
        stage
            .verify_bytes(Some(received(BYTES.len())), &cancel)
            .unwrap();
        let path = fs::read_dir(directory.cache())
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p.extension().is_some_and(|s| s == "part"))
            .unwrap();
        if replaced {
            fs::rename(&path, directory.0.join("retained-original")).unwrap();
            private_file(&path, b"do not delete this replacement");
        } else {
            private_file(&path, &[b'x'; BYTES.len()]);
        }
        assert!(stage.finish(&mut store, &trust, || 301, &cancel).is_err());
        assert!(package_files(&directory).is_empty());
        if replaced {
            assert_eq!(fs::read(path).unwrap(), b"do not delete this replacement");
        }
    }
}

#[cfg(unix)]
#[test]
fn symlink_and_hardlink_entries_are_rejected_without_touching_the_target() {
    use std::os::unix::fs::symlink;
    for scenario in ["directory", "leaf", "hardlink"] {
        let directory = Directory::new();
        let trust = trust_store();
        let mut store = store(&directory, &trust);
        let outside = directory.0.join("outside");
        fs::create_dir(&outside).unwrap();
        let target = outside.join("keep");
        private_file(&target, b"keep external data");
        if scenario == "directory" {
            symlink(&outside, directory.cache()).unwrap();
        } else {
            let cache = store.package_cache(PackageCacheLimits::default()).unwrap();
            drop(cache);
            let name = directory.cache().join(format!("{}.pkg", sha(BYTES)));
            if scenario == "leaf" {
                symlink(&target, name).unwrap();
            } else {
                fs::hard_link(&target, name).unwrap();
            }
        }
        assert!(store.package_cache(PackageCacheLimits::default()).is_err());
        assert_eq!(fs::read(target).unwrap(), b"keep external data");
        // No state checkpoint is reset by disposable-cache errors.
        assert!(store
            .load_release(&trust, 300, TOOL, &version())
            .unwrap()
            .is_available());
    }
}

#[test]
fn failure_to_persist_final_revalidation_never_returns_a_package() {
    let directory = Directory::new();
    let trust = trust_store();
    let cancel = CancellationToken::default();
    let mut store = store(&directory, &trust);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let mut stage = cache
        .prepare(&mut store, &trust, 300, TOOL, &version(), &cancel)
        .unwrap();
    stage.write_all(BYTES).unwrap();
    stage
        .verify_bytes(Some(received(BYTES.len())), &cancel)
        .unwrap();
    let path = directory.root().join("state-v1.json");
    let backup = directory.root().join("saved.json");
    fs::rename(&path, &backup).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(matches!(
        stage.finish(&mut store, &trust, || 301, &cancel),
        Err(PackageCacheError::Store(_))
    ));
    assert!(package_files(&directory).is_empty());
    fs::remove_dir(path).unwrap();
    fs::rename(backup, directory.root().join("state-v1.json")).unwrap();
    assert!(store.load_release(&trust, 300, TOOL, &version()).is_ok());
}

#[test]
fn reopening_with_a_lower_native_budget_evicts_bytes_but_keeps_metadata() {
    let directory = Directory::new();
    let trust = trust_store();
    let mut store = store(&directory, &trust);
    let cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    drop(cache);
    for i in 0..3 {
        private_file(&directory.cache().join(format!("{i:064x}.pkg")), b"four");
    }
    let before = fs::read(directory.root().join("state-v1.json")).unwrap();
    let cache = store
        .package_cache(PackageCacheLimits::new(4, 1).unwrap())
        .unwrap();
    assert_eq!(package_files(&directory).len(), 1);
    assert_eq!(
        fs::read(directory.root().join("state-v1.json")).unwrap(),
        before
    );
    drop(cache);
    assert!(store.load_release(&trust, 300, TOOL, &version()).is_ok());
}

#[test]
fn malformed_names_and_excessive_directory_entries_fail_closed_without_state_reset() {
    for excessive in [false, true] {
        let directory = Directory::new();
        let trust = trust_store();
        let mut store = store(&directory, &trust);
        let cache = store.package_cache(PackageCacheLimits::default()).unwrap();
        drop(cache);
        if excessive {
            for i in 0..129 {
                private_file(&directory.cache().join(format!("{i:064x}.pkg")), b"x");
            }
        } else {
            private_file(
                &directory.cache().join("not-a-cache-package.exe"),
                b"keep unknown file",
            );
        }
        assert!(store.package_cache(PackageCacheLimits::default()).is_err());
        if !excessive {
            assert_eq!(
                fs::read(directory.cache().join("not-a-cache-package.exe")).unwrap(),
                b"keep unknown file"
            );
        }
        assert!(store.load_release(&trust, 300, TOOL, &version()).is_ok());
    }
}
