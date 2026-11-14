use fbforge_suite_coordination::AppIdentity;
#[cfg(not(windows))]
use fbforge_suite_coordination::{Acquisition, Lease, UnavailableReason};

#[test]
fn identity_preserves_stable_catalog_app_ids_and_normalizes_ascii_case() {
    for identity in [
        "org.fitconnect.integration-client",
        "de.fbforge.inspect",
        "a",
        "tool_id",
    ] {
        assert_eq!(AppIdentity::new(identity).unwrap().as_str(), identity);
    }
    assert_eq!(
        AppIdentity::new("ORG.FitConnect.Integration-Client").unwrap(),
        AppIdentity::new("org.fitconnect.integration-client").unwrap()
    );
    assert!(AppIdentity::new(&"a".repeat(128)).is_ok());
}

#[test]
fn identity_rejects_namespaces_paths_controls_and_unbounded_input() {
    for identity in [
        "",
        ".tool",
        "tool-",
        r"Global\tool",
        r"C:\tool",
        "tool:stream",
        "tool\0other",
        "tool\nother",
        "tööl",
        "tool name",
    ] {
        assert!(AppIdentity::new(identity).is_err(), "{identity:?}");
    }
    assert!(AppIdentity::new(&"a".repeat(129)).is_err());
}

#[cfg(not(windows))]
#[test]
fn non_windows_does_not_simulate_a_coordination_lease() {
    let identity = AppIdentity::new("example.coordination-fixture").unwrap();
    assert!(matches!(
        Lease::try_acquire(&identity),
        Acquisition::Unavailable(UnavailableReason::UnsupportedPlatform)
    ));
}
