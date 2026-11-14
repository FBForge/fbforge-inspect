use fbforge_suite_core::{
    authenticode::{
        verify_cooperating_program, AuthenticodePolicies, AuthenticodePolicy, IndeterminateReason,
        InvalidReason, PolicyStatus, VerificationFailure,
    },
    catalog::{parse_catalog, ToolManifest},
};
use std::path::Path;

fn tool() -> ToolManifest {
    parse_catalog(include_str!("../../../catalog/tools.v1.json"))
        .unwrap()
        .tools()[0]
        .clone()
}
fn policy(tool: &ToolManifest) -> AuthenticodePolicy {
    AuthenticodePolicy {
        id: tool.trust_policy_id.clone(),
        tool_id: tool.id.clone(),
        app_id: tool.app_id.clone(),
        certificate_sha256: vec![[7; 32]],
        status: PolicyStatus::Active,
    }
}
#[test]
fn manifest_and_active_tool_app_policy_are_required_before_open() {
    let tool = tool();
    let empty = AuthenticodePolicies::new(vec![]).unwrap();
    assert_eq!(
        verify_cooperating_program(Path::new("missing"), &tool, &empty, 300).unwrap_err(),
        VerificationFailure::Indeterminate(IndeterminateReason::PolicyUnavailable)
    );
    for change in ["tool", "app", "revoked"] {
        let mut p = policy(&tool);
        match change {
            "tool" => p.tool_id = "other".into(),
            "app" => p.app_id = "other".into(),
            _ => p.status = PolicyStatus::Revoked,
        }
        let policies = AuthenticodePolicies::new(vec![p]).unwrap();
        assert_eq!(
            verify_cooperating_program(Path::new("missing"), &tool, &policies, 300).unwrap_err(),
            VerificationFailure::Invalid(InvalidReason::PolicyRejected)
        );
    }
    let policies = AuthenticodePolicies::new(vec![policy(&tool)]).unwrap();
    for change in ["id", "app", "policy", "executable", "repository"] {
        let mut invalid = tool.clone();
        match change {
            "id" => invalid.id = "../bad".into(),
            "app" => invalid.app_id = "".into(),
            "policy" => invalid.trust_policy_id = "bad/path".into(),
            "executable" => invalid.executables = vec!["../tool.exe".into()],
            _ => invalid.release_repository = "../bad".into(),
        }
        assert_eq!(
            verify_cooperating_program(Path::new("missing"), &invalid, &policies, 300).unwrap_err(),
            VerificationFailure::Invalid(InvalidReason::InvalidManifest)
        );
    }
}
#[cfg(not(windows))]
#[test]
fn non_windows_cannot_create_installed_image_proof() {
    let tool = tool();
    let policies = AuthenticodePolicies::new(vec![policy(&tool)]).unwrap();
    assert_eq!(
        verify_cooperating_program(Path::new("C:\\product.exe"), &tool, &policies, 300)
            .unwrap_err(),
        VerificationFailure::Indeterminate(IndeterminateReason::UnsupportedPlatform)
    );
}
