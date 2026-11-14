mod support;
use fbforge_suite_core::{
    authenticode::{InvalidReason, VerificationFailure},
    production::bundled_authenticode_policies,
    trust::{verify_catalog, verify_release, VerifiedRelease},
};
use semver::Version;
use serde_json::json;
use support::*;

fn release_for(tool_id: &str, app_id_override: Option<&str>) -> VerifiedRelease {
    let trust = if tool_id == "fbforge-connect" {
        trust_store()
    } else {
        let mut signer = anchor("test-key", 42);
        signer.subjects.push("release/fbforge-inspect".into());
        fbforge_suite_core::trust::TrustStore::new(vec![signer]).unwrap()
    };
    let mut body = catalog_body();
    if let Some(app_id) = app_id_override {
        let tool = body["tools"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|tool| tool["id"] == tool_id)
            .unwrap();
        tool["appId"] = json!(app_id);
    }
    let catalog = verify_catalog(
        &sign(&payload("catalog", body), "test-key", 42),
        &trust,
        300,
        None,
    )
    .unwrap();
    let tool = catalog
        .catalog()
        .tools()
        .iter()
        .find(|tool| tool.id == tool_id)
        .unwrap();
    let release = json!({"schemaVersion":1,"toolId":tool.id,"appId":tool.app_id,
        "version":"1.0.0","platform":"windows-x64","installer":"nsis-current-user",
        "repository":tool.release_repository,"releaseTag":"v1.0.0",
        "assetName":"fixture-setup.exe","size":3,
        "sha256":"ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "authenticodePolicyId":tool.trust_policy_id,"status":"available",
        "requiredFiles":[tool.executables[0]]});
    verify_release(
        &sign(
            &payload(&format!("release/{tool_id}"), release),
            "test-key",
            42,
        ),
        &trust,
        300,
        None,
        &catalog,
        tool_id,
        &Version::parse("1.0.0").unwrap(),
    )
    .unwrap()
}

#[test]
fn bundled_signer_pins_bind_both_tools_to_the_operator_confirmed_certificate() {
    let policies = bundled_authenticode_policies().unwrap();
    for id in ["fbforge-connect", "fbforge-inspect"] {
        let release = release_for(id, None);
        let policy = policies.policy_for_release(&release).unwrap();
        let actual: String = policy.certificate_sha256[0]
            .iter()
            .map(|byte| format!("{byte:02X}"))
            .collect();
        assert_eq!(
            actual,
            "5536DFF83A3124CCB35449D627E8B98D01EE429BC2465BE9494B5A9806B4D759"
        );
        assert_eq!(policy.tool_id, id);
    }
}

#[test]
fn signed_metadata_cannot_rebind_a_production_tool_to_another_app() {
    let policies = bundled_authenticode_policies().unwrap();
    let substituted = release_for("fbforge-connect", Some("de.example.another-app"));
    assert_eq!(
        policies.policy_for_release(&substituted).unwrap_err(),
        VerificationFailure::Invalid(InvalidReason::PolicyRejected)
    );
}

#[test]
fn production_metadata_uses_only_the_confirmed_minisign_key() {
    use fbforge_suite_core::production::{bundled_metadata_trust_store, METADATA_KEY_ID};
    use fbforge_suite_core::trust::TrustError;
    let store = bundled_metadata_trust_store().unwrap();
    let fixtures: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/minisign-v2.json")).unwrap();
    let mut envelope = fixtures["cases"]["catalog"].clone();
    envelope["keyId"] = json!(METADATA_KEY_ID);
    // Relabelling an authentic public dummy signature cannot grant production trust.
    assert_eq!(
        verify_catalog(&envelope.to_string(), &store, 300, None).unwrap_err(),
        TrustError::InvalidSignature
    );
}

#[test]
fn production_metadata_never_reinterprets_the_minisign_anchor_as_raw_v1() {
    use fbforge_suite_core::production::{bundled_metadata_trust_store, METADATA_KEY_ID};
    use fbforge_suite_core::trust::TrustError;
    let mut envelope: serde_json::Value =
        serde_json::from_str(&sign(&payload("catalog", catalog_body()), "test-key", 42)).unwrap();
    envelope["keyId"] = json!(METADATA_KEY_ID);
    assert_eq!(
        verify_catalog(
            &envelope.to_string(),
            &bundled_metadata_trust_store().unwrap(),
            300,
            None
        )
        .unwrap_err(),
        TrustError::UnknownKey
    );
}
