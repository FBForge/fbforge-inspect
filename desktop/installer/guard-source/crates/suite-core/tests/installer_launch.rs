use fbforge_suite_core::{
    authenticode::{
        AuthenticodePolicies, AuthenticodePolicy, InvalidReason, PolicyStatus, VerificationFailure,
    },
    installers::{prepare_cached_installer, InstallerError},
    network::{CancellationToken, DownloadResult},
    storage::{CachedPackage, PackageCache, StateStore},
    trust::{KeyStatus, MinisignTrustAnchor, TrustStore, VerifiedRelease},
};
use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-installer-launch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/release-cache-v2.json")).unwrap()
}
fn setup() -> (Directory, TrustStore, StateStore, VerifiedRelease) {
    let directory = Directory::new();
    let fixture = fixture();
    let trust = TrustStore::new(vec![])
        .unwrap()
        .with_minisign_anchors(vec![MinisignTrustAnchor {
            id: "release-cache-test".into(),
            public_key: fixture["publicKey"].as_str().unwrap().into(),
            subjects: vec!["catalog".into(), "release/fbforge-connect".into()],
            not_before: 100,
            expires_at: 10_000,
            status: KeyStatus::Active,
        }])
        .unwrap();
    let mut store = StateStore::open(&directory.0.join("state")).unwrap();
    store
        .accept_catalog(&fixture["cases"]["catalog"].to_string(), &trust, 300)
        .unwrap();
    let release = store
        .accept_release(
            &fixture["cases"]["release1"].to_string(),
            &trust,
            300,
            "fbforge-connect",
            &"0.3.0".parse().unwrap(),
        )
        .unwrap();
    (directory, trust, store, release)
}
fn policies(release: &VerifiedRelease, status: PolicyStatus) -> AuthenticodePolicies {
    AuthenticodePolicies::new(vec![AuthenticodePolicy {
        id: release.authenticode_policy_id().into(),
        tool_id: release.tool_id().into(),
        app_id: release.app_id().into(),
        certificate_sha256: vec![[7; 32]],
        status,
    }])
    .unwrap()
}
fn package<'cache>(
    cache: &'cache mut PackageCache,
    store: &mut StateStore,
    trust: &TrustStore,
) -> CachedPackage<'cache> {
    let cancel = CancellationToken::default();
    let mut stage = cache
        .prepare(
            store,
            trust,
            300,
            "fbforge-connect",
            &"0.3.0".parse().unwrap(),
            &cancel,
        )
        .unwrap();
    stage.write_all(b"abc").unwrap();
    stage
        .verify_bytes(Some(Ok(DownloadResult { bytes_received: 3 })), &cancel)
        .unwrap();
    stage.finish(store, trust, || 300, &cancel).unwrap()
}
#[test]
fn installer_preparation_rejects_revoked_policy_and_expiry_before_os_handoff() {
    for (status, now, reason) in [
        (PolicyStatus::Revoked, 300, InvalidReason::PolicyRejected),
        (PolicyStatus::Active, 1000, InvalidReason::MetadataExpired),
    ] {
        let (_directory, trust, mut store, release) = setup();
        let mut cache = store.package_cache(Default::default()).unwrap();
        let package = package(&mut cache, &mut store, &trust);
        assert_eq!(
            prepare_cached_installer(package, &release, &policies(&release, status), now)
                .unwrap_err(),
            InstallerError::Verification(VerificationFailure::Invalid(reason))
        );
    }
}
#[test]
fn installer_preparation_rejects_a_different_current_release() {
    let (_directory, trust, mut store, release) = setup();
    let mut cache = store.package_cache(Default::default()).unwrap();
    let package = package(&mut cache, &mut store, &trust);
    let other = store
        .accept_release(
            &fixture()["cases"]["release2"].to_string(),
            &trust,
            300,
            "fbforge-connect",
            &"0.4.0".parse().unwrap(),
        )
        .unwrap();
    assert_eq!(
        prepare_cached_installer(
            package,
            &other,
            &policies(&release, PolicyStatus::Active),
            300
        )
        .unwrap_err(),
        InstallerError::Verification(VerificationFailure::Invalid(InvalidReason::PackageMismatch))
    );
}
#[cfg(not(windows))]
#[test]
fn portable_preparation_cannot_manufacture_a_trusted_windows_installer() {
    let (_directory, trust, mut store, release) = setup();
    let mut cache = store.package_cache(Default::default()).unwrap();
    let package = package(&mut cache, &mut store, &trust);
    assert_eq!(
        prepare_cached_installer(
            package,
            &release,
            &policies(&release, PolicyStatus::Active),
            300
        )
        .unwrap_err(),
        InstallerError::UnsupportedPlatform
    );
}
