mod support;

use fbforge_suite_core::{
    authenticode::{
        verify_package, AuthenticodePolicies, AuthenticodePolicy, IndeterminateReason,
        InvalidReason, PolicyError, PolicyStatus, VerificationFailure,
    },
    trust::{verify_catalog, verify_release, VerifiedRelease},
};
use semver::Version;
use serde_json::json;
use std::path::Path;
use support::*;

fn policy() -> AuthenticodePolicy {
    AuthenticodePolicy {
        id: "fbforge-connect-production".into(),
        tool_id: "fbforge-connect".into(),
        app_id: "org.fitconnect.integration-client".into(),
        certificate_sha256: vec![[7; 32]],
        status: PolicyStatus::Active,
    }
}

fn release() -> VerifiedRelease {
    let store = trust_store();
    let catalog = verify_catalog(
        &sign(&payload("catalog", catalog_body()), "test-key", 42),
        &store,
        300,
        None,
    )
    .unwrap();
    let body = json!({"schemaVersion":1,"toolId":"fbforge-connect",
        "appId":"org.fitconnect.integration-client","version":"0.3.0",
        "platform":"windows-x64","installer":"nsis-current-user",
        "repository":"FBForge/fbforge-connect","releaseTag":"v0.3.0",
        "assetName":"setup.exe","size":3,
        "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "authenticodePolicyId":"fbforge-connect-production","status":"available",
        "requiredFiles":["fbforge-connect.exe"]});
    verify_release(
        &sign(&payload("release/fbforge-connect", body), "test-key", 42),
        &store,
        300,
        None,
        &catalog,
        "fbforge-connect",
        &Version::parse("0.3.0").unwrap(),
    )
    .unwrap()
}

#[test]
fn signed_metadata_cannot_provision_its_own_signer_policy() {
    let policies = AuthenticodePolicies::new(vec![]).unwrap();
    assert_eq!(
        policies.policy_for_release(&release()).unwrap_err(),
        VerificationFailure::Indeterminate(IndeterminateReason::PolicyUnavailable)
    );
}

#[test]
fn policy_is_bound_to_tool_and_application_and_can_be_revoked() {
    let release = release();
    let policies = AuthenticodePolicies::new(vec![policy()]).unwrap();
    assert_eq!(
        policies
            .policy_for_release(&release)
            .unwrap()
            .certificate_sha256,
        vec![[7; 32]]
    );
    for change in ["tool", "app", "revoked"] {
        let mut entry = policy();
        match change {
            "tool" => entry.tool_id = "another-tool".into(),
            "app" => entry.app_id = "another-app".into(),
            _ => entry.status = PolicyStatus::Revoked,
        }
        let policies = AuthenticodePolicies::new(vec![entry]).unwrap();
        assert_eq!(
            policies.policy_for_release(&release).unwrap_err(),
            VerificationFailure::Invalid(InvalidReason::PolicyRejected)
        );
    }
}

#[test]
fn local_policy_rejects_empty_ambiguous_and_unbounded_configuration() {
    assert_eq!(
        AuthenticodePolicies::new(vec![policy(), policy()]).unwrap_err(),
        PolicyError::InvalidConfiguration
    );
    for pins in [
        vec![],
        vec![[0; 32]],
        vec![[7; 32], [7; 32]],
        vec![[7; 32]; 33],
    ] {
        let mut entry = policy();
        entry.certificate_sha256 = pins;
        assert!(AuthenticodePolicies::new(vec![entry]).is_err());
    }
    for field in ["id", "tool", "app"] {
        let mut entry = policy();
        match field {
            "id" => entry.id = "../policy".into(),
            "tool" => entry.tool_id = "".into(),
            _ => entry.app_id = "bad/app".into(),
        }
        assert!(AuthenticodePolicies::new(vec![entry]).is_err());
    }
    assert!(AuthenticodePolicies::new(
        (0..129)
            .map(|index| {
                let mut entry = policy();
                entry.id = format!("policy-{index}");
                entry
            })
            .collect()
    )
    .is_err());
}

#[test]
fn expired_release_is_blocked_before_any_file_operation() {
    let policies = AuthenticodePolicies::new(vec![policy()]).unwrap();
    assert_eq!(
        verify_package(Path::new("/not/a/package"), &release(), &policies, 1000).unwrap_err(),
        VerificationFailure::Invalid(InvalidReason::MetadataExpired)
    );
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_never_produces_windows_trust_or_exposes_a_path() {
    let policies = AuthenticodePolicies::new(vec![policy()]).unwrap();
    let result = verify_package(
        Path::new("/private/SECRET-path\ncredential=value"),
        &release(),
        &policies,
        300,
    );
    let failure = result.unwrap_err();
    assert_eq!(
        failure,
        VerificationFailure::Indeterminate(IndeterminateReason::UnsupportedPlatform)
    );
    for message in [failure.to_string(), format!("{failure:?}")] {
        assert!(!message.contains("SECRET"));
        assert!(!message.contains('/'));
        assert!(!message.contains('\n'));
        assert!(!message.contains("credential"));
    }
}
