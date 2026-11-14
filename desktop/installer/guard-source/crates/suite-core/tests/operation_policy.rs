use fbforge_suite_core::operations::*;
use semver::Version;

fn ready() -> Preconditions {
    Preconditions {
        signature: SignatureStatus::Trusted,
        process: ProcessStatus::Stopped,
        package_identity_matches: true,
        has_verified_bytes: true,
        has_install_rights: true,
        has_disk_space: true,
        has_exclusive_lock: true,
        release: ReleaseStatus::Available,
        is_allowed_by_policy: true,
        is_platform_supported: true,
        is_suite_compatible: true,
    }
}

#[test]
fn requires_positive_current_evidence_at_every_gate() {
    assert_eq!(evaluate_installation(&ready()), InstallDecision::Ready);
    for status in [
        SignatureStatus::Unchecked,
        SignatureStatus::Checking,
        SignatureStatus::Invalid,
        SignatureStatus::Indeterminate,
    ] {
        let mut conditions = ready();
        conditions.signature = status;
        assert_eq!(
            evaluate_installation(&conditions),
            InstallDecision::Blocked(BlockReason::Trust)
        );
    }
    for status in [
        ProcessStatus::Running,
        ProcessStatus::Busy,
        ProcessStatus::Unknown,
    ] {
        let mut conditions = ready();
        conditions.process = status;
        assert_eq!(
            evaluate_installation(&conditions),
            InstallDecision::WaitingForTool
        );
    }
    for status in [ReleaseStatus::Unknown, ReleaseStatus::Withdrawn] {
        let mut conditions = ready();
        conditions.release = status;
        assert_eq!(
            evaluate_installation(&conditions),
            InstallDecision::Blocked(BlockReason::ReleaseAvailability)
        );
    }
    for gate in 0..9 {
        let mut conditions = ready();
        match gate {
            0 => conditions.package_identity_matches = false,
            1 => conditions.has_verified_bytes = false,
            2 => conditions.has_install_rights = false,
            3 => conditions.has_disk_space = false,
            4 => conditions.has_exclusive_lock = false,
            5 => conditions.release = ReleaseStatus::Withdrawn,
            6 => conditions.is_allowed_by_policy = false,
            7 => conditions.is_platform_supported = false,
            _ => conditions.is_suite_compatible = false,
        }
        assert!(
            matches!(
                evaluate_installation(&conditions),
                InstallDecision::Blocked(_)
            ),
            "gate {gate}"
        );
    }
}

#[test]
fn compares_stable_versions_by_semver_precedence_not_text_or_build_metadata() {
    let version = |value| Version::parse(value).unwrap();
    assert!(has_stable_update(&version("0.9.0"), &version("0.10.0")));
    assert!(!has_stable_update(
        &version("1.0.0"),
        &version("1.1.0-rc.1")
    ));
    assert!(!has_stable_update(
        &version("1.0.0+one"),
        &version("1.0.0+two")
    ));
    assert!(!has_stable_update(&version("2.0.0"), &version("1.0.0")));
    assert!(has_stable_update(&version("1.0.0-rc.1"), &version("1.0.0")));
}

#[test]
fn repair_never_silently_downgrades_or_uses_a_prerelease() {
    let version = |value| Version::parse(value).unwrap();
    assert!(can_repair_version(
        &version("1.2.0"),
        &version("1.2.0+rebuild")
    ));
    assert!(can_repair_version(&version("1.2.0"), &version("1.3.0")));
    assert!(!can_repair_version(&version("1.3.0"), &version("1.2.0")));
    assert!(!can_repair_version(
        &version("1.3.0"),
        &version("1.4.0-rc.1")
    ));
}
