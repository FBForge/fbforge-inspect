mod support;
use fbforge_suite_core::{
    authenticode::{
        verify_cached_package, AuthenticodePolicies, AuthenticodePolicy, IndeterminateReason,
        InvalidReason, PolicyStatus, VerificationFailure,
    },
    network::{CancellationToken, DownloadResult},
    storage::{CachedPackage, PackageCache, PackageCacheLimits, StateStore},
    trust::{verify_catalog, verify_release, TrustStore, VerifiedRelease},
};
use semver::Version;
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
use support::*;
const TOOL: &str = "fbforge-connect";
const BYTES: &[u8] = b"abc";
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-cached-auth-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn root(&self) -> PathBuf {
        self.0.join("state")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
fn trust() -> TrustStore {
    let mut key = anchor("test-key", 42);
    key.subjects.push("release/fbforge-inspect".into());
    TrustStore::new(vec![key]).unwrap()
}
fn release_body(tool: &str, version: &str, status: &str) -> Value {
    let catalog = catalog_body();
    let tool = catalog["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == tool)
        .unwrap();
    json!({"schemaVersion":1,"toolId":tool["id"],"appId":tool["appId"],"version":version,
    "platform":tool["platform"],"installer":tool["installer"],"repository":tool["releaseRepository"],"releaseTag":format!("v{version}"),"assetName":"setup.exe","size":3,
    "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad", "authenticodePolicyId":tool["trustPolicyId"],"status":status,"requiredFiles":tool["executables"]})
}
fn metadata(tool: &str, version: &str, status: &str) -> VerifiedRelease {
    metadata_with_body(tool, version, release_body(tool, version, status))
}
fn metadata_with_body(tool: &str, version: &str, body: Value) -> VerifiedRelease {
    let trust = trust();
    let catalog = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &trust,
        300,
        None,
    )
    .unwrap();
    verify_release(
        &sign(&payload(&format!("release/{tool}"), body), "test-key", 42),
        &trust,
        300,
        None,
        &catalog,
        tool,
        &Version::parse(version).unwrap(),
    )
    .unwrap()
}
fn policies(tool: &str, status: PolicyStatus) -> AuthenticodePolicies {
    let release = metadata(tool, "0.3.0", "available");
    AuthenticodePolicies::new(vec![AuthenticodePolicy {
        id: release.authenticode_policy_id().into(),
        tool_id: tool.into(),
        app_id: release.app_id().into(),
        certificate_sha256: vec![[7; 32]],
        status,
    }])
    .unwrap()
}
fn store(directory: &Directory) -> StateStore {
    let mut store = StateStore::open(&directory.root()).unwrap();
    let trust = trust_store();
    store
        .accept_catalog(
            &sign(&payload("catalog", catalog_body()), "test-key", 42),
            &trust,
            300,
        )
        .unwrap();
    store
        .accept_release(
            &sign(
                &payload(
                    &format!("release/{TOOL}"),
                    release_body(TOOL, "0.3.0", "available"),
                ),
                "test-key",
                42,
            ),
            &trust,
            300,
            TOOL,
            &Version::parse("0.3.0").unwrap(),
        )
        .unwrap();
    store
}
fn cached<'a>(cache: &'a mut PackageCache, store: &mut StateStore) -> CachedPackage<'a> {
    let cancel = CancellationToken::default();
    let trust = trust();
    let mut stage = cache
        .prepare(
            store,
            &trust,
            300,
            TOOL,
            &Version::parse("0.3.0").unwrap(),
            &cancel,
        )
        .unwrap();
    let transfer = if stage.is_cached() {
        None
    } else {
        stage.write_all(BYTES).unwrap();
        Some(Ok(DownloadResult { bytes_received: 3 }))
    };
    stage.verify_bytes(transfer, &cancel).unwrap();
    stage.finish(store, &trust, || 300, &cancel).unwrap()
}

#[test]
fn current_policy_expiry_withdrawal_and_product_binding_precede_any_os_call() {
    for scenario in [
        "missing-policy",
        "revoked-policy",
        "expired",
        "withdrawn",
        "product",
        "version",
        "hash",
        "size",
    ] {
        let directory = Directory::new();
        let mut store = store(&directory);
        let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
        let package = cached(&mut cache, &mut store);
        let tool = if scenario == "product" {
            "fbforge-inspect"
        } else {
            TOOL
        };
        let version = if scenario == "version" {
            "0.4.0"
        } else {
            "0.3.0"
        };
        let status = if scenario == "withdrawn" {
            "withdrawn"
        } else {
            "available"
        };
        let mut body = release_body(tool, version, status);
        match scenario {
            "hash" => body["sha256"] = json!("00".repeat(32)),
            "size" => body["size"] = json!(4),
            _ => {}
        }
        let release = metadata_with_body(tool, version, body);
        let policies = if scenario == "missing-policy" {
            AuthenticodePolicies::new(vec![]).unwrap()
        } else {
            policies(
                tool,
                if scenario == "revoked-policy" {
                    PolicyStatus::Revoked
                } else {
                    PolicyStatus::Active
                },
            )
        };
        let failure = verify_cached_package(
            package,
            &release,
            &policies,
            if scenario == "expired" { 1000 } else { 300 },
        )
        .unwrap_err();
        let expected = match scenario {
            "missing-policy" => {
                VerificationFailure::Indeterminate(IndeterminateReason::PolicyUnavailable)
            }
            "revoked-policy" => VerificationFailure::Invalid(InvalidReason::PolicyRejected),
            "expired" => VerificationFailure::Invalid(InvalidReason::MetadataExpired),
            "withdrawn" => VerificationFailure::Invalid(InvalidReason::WithdrawnRelease),
            _ => VerificationFailure::Invalid(InvalidReason::PackageMismatch),
        };
        assert_eq!(failure, expected, "{scenario}");
        // Error consumes/drops the file guard without leaking its cache borrow.
        drop(cached(&mut cache, &mut store));
    }
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_cannot_produce_a_cached_windows_proof() {
    let directory = Directory::new();
    let mut store = store(&directory);
    let mut cache = store.package_cache(PackageCacheLimits::default()).unwrap();
    let package = cached(&mut cache, &mut store);
    let failure = verify_cached_package(
        package,
        &metadata(TOOL, "0.3.0", "available"),
        &policies(TOOL, PolicyStatus::Active),
        300,
    )
    .unwrap_err();
    assert_eq!(
        failure,
        VerificationFailure::Indeterminate(IndeterminateReason::UnsupportedPlatform)
    );
    assert!(!format!("{failure:?} {failure}").contains("suite-cached-auth"));
}
