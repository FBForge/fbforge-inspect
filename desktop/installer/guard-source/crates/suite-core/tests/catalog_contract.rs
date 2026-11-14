use fbforge_suite_core::catalog::parse_catalog;
use serde_json::{json, Value};

fn catalog() -> Value {
    serde_json::from_str(include_str!("../../../catalog/tools.v1.json")).unwrap()
}

#[test]
fn accepts_both_products_and_a_third_data_only_tool() {
    let mut input = catalog();
    let mut third = input["tools"][1].clone();
    third["id"] = json!("fbforge-example");
    third["appId"] = json!("de.fbforge.example");
    third["productName"] = json!("FBForge Example");
    third["registryNames"] = json!(["FBForge Example"]);
    third["executables"] = json!(["fbforge-example.exe"]);
    third["releaseRepository"] = json!("FBForge/fbforge-example");
    third["trustPolicyId"] = json!("fbforge-example-test");
    input["tools"].as_array_mut().unwrap().push(third);
    assert_eq!(parse_catalog(&input.to_string()).unwrap().tools().len(), 3);
}

#[test]
fn rejects_unsupported_contracts_and_executable_extensions() {
    for (field, value) in [("schemaVersion", json!(2)), ("shell", json!("cmd.exe"))] {
        let mut input = catalog();
        input[field] = value;
        assert!(parse_catalog(&input.to_string()).is_err(), "{field}");
    }
    let mut input = catalog();
    input["tools"][0]["arguments"] = json!(["/S", "&calc"]);
    assert!(parse_catalog(&input.to_string()).is_err());
}

#[test]
fn rejects_unsafe_paths_sources_identities_and_versions() {
    for (field, value) in [
        ("executables", json!(["../escape.exe"])),
        ("executables", json!(["C:\\escape.exe"])),
        ("executables", json!(["tool.exe:stream"])),
        ("executables", json!(["CON.exe"])),
        ("executables", json!(["tool.EXE", "TOOL.exe"])),
        ("executables", json!([])),
        ("releaseRepository", json!("https://evil.invalid/tool")),
        ("registryNames", json!(["Software\\OtherApp"])),
        ("id", json!("../other")),
        ("trustPolicyId", json!("")),
        ("platform", json!("linux-x64")),
        ("installer", json!("shell")),
        ("minimumSuiteVersion", json!("1.0")),
        ("capabilities", json!(["run-command"])),
    ] {
        let mut input = catalog();
        input["tools"][0][field] = value;
        assert!(parse_catalog(&input.to_string()).is_err(), "{field}");
    }
}

#[test]
fn rejects_duplicate_tool_ids_app_ids_and_detection_rules() {
    for field in ["id", "appId", "registryNames", "executables"] {
        let mut input = catalog();
        input["tools"][1][field] = input["tools"][0][field].clone();
        assert!(parse_catalog(&input.to_string()).is_err(), "{field}");
    }
}

#[test]
fn bounds_untrusted_input_and_does_not_include_it_in_errors() {
    assert!(parse_catalog(&" ".repeat(1_048_577)).is_err());
    let error = parse_catalog(r#"{"secret":"do-not-log"}"#).unwrap_err();
    assert!(!error.to_string().contains("do-not-log"));
}
