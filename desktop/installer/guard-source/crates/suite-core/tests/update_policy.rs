use fbforge_suite_core::updates::*;

#[test]
fn defaults_search_automatically_but_require_download_and_install_opt_in() {
    let policy = resolve_update_policy(
        UpdatePreferences::default(),
        UpdateOverrides::default(),
        AdministrativeUpdatePolicy::default(),
        AutomaticInstallConsent::NotGranted,
    );
    assert!(policy.search().enabled);
    assert!(!policy.download().enabled);
    assert!(!policy.install().enabled);
    assert_eq!(
        policy.decide(
            UpdateStage::Search,
            UpdateTrigger::Automatic,
            UpdateTiming::default(),
            100
        ),
        UpdateDecision::Ready
    );
    assert_eq!(
        policy.decide(
            UpdateStage::Download,
            UpdateTrigger::Automatic,
            UpdateTiming::default(),
            100
        ),
        UpdateDecision::Disabled
    );
}

#[test]
fn tool_preferences_override_globals_but_managed_values_take_precedence() {
    let mut managed = AdministrativeUpdatePolicy::default();
    managed.automatic.download = Some(false);
    let policy = resolve_update_policy(
        UpdatePreferences::default(),
        UpdateOverrides {
            search: Some(false),
            download: Some(true),
            install: None,
        },
        managed,
        AutomaticInstallConsent::Granted,
    );
    assert!(!policy.search().enabled);
    assert!(!policy.search().is_managed);
    assert!(!policy.download().enabled);
    assert!(policy.download().is_managed);
}

#[test]
fn managed_automatic_installation_never_replaces_deliberate_user_consent() {
    let mut managed = AdministrativeUpdatePolicy::default();
    managed.automatic.install = Some(true);
    for consent in [
        AutomaticInstallConsent::NotGranted,
        AutomaticInstallConsent::Granted,
        AutomaticInstallConsent::NotGranted,
    ] {
        let policy = resolve_update_policy(
            UpdatePreferences::default(),
            UpdateOverrides::default(),
            managed,
            consent,
        );
        assert_eq!(
            policy.decide(
                UpdateStage::Install,
                UpdateTrigger::Automatic,
                UpdateTiming::default(),
                100
            ),
            if consent == AutomaticInstallConsent::Granted {
                UpdateDecision::Ready
            } else {
                UpdateDecision::ConsentRequired
            }
        );
        assert_eq!(
            policy.decide(
                UpdateStage::Install,
                UpdateTrigger::Manual,
                UpdateTiming::default(),
                100
            ),
            UpdateDecision::Ready
        );
    }
}

#[test]
fn administrative_denial_applies_to_manual_and_automatic_operations() {
    let managed = AdministrativeUpdatePolicy {
        updates_allowed: false,
        ..Default::default()
    };
    let policy = resolve_update_policy(
        UpdatePreferences::default(),
        UpdateOverrides::default(),
        managed,
        AutomaticInstallConsent::Granted,
    );
    for trigger in [UpdateTrigger::Manual, UpdateTrigger::Automatic] {
        for stage in [
            UpdateStage::Search,
            UpdateStage::Download,
            UpdateStage::Install,
        ] {
            assert_eq!(
                policy.decide(stage, trigger, UpdateTiming::default(), 100),
                UpdateDecision::BlockedByAdministration
            );
        }
    }
}

#[test]
fn pause_and_release_deferral_do_not_prevent_an_explicit_manual_request() {
    let policy = resolve_update_policy(
        UpdatePreferences {
            search: true,
            download: true,
            install: true,
        },
        UpdateOverrides::default(),
        AdministrativeUpdatePolicy::default(),
        AutomaticInstallConsent::Granted,
    );
    let timing = UpdateTiming {
        paused_until: Some(200),
        deferred_until: Some(300),
    };
    for stage in [
        UpdateStage::Search,
        UpdateStage::Download,
        UpdateStage::Install,
    ] {
        assert_eq!(
            policy.decide(stage, UpdateTrigger::Automatic, timing, 100),
            UpdateDecision::PausedUntil(200)
        );
        assert_eq!(
            policy.decide(stage, UpdateTrigger::Manual, timing, 100),
            UpdateDecision::Ready
        );
    }
    assert_eq!(
        policy.decide(UpdateStage::Search, UpdateTrigger::Automatic, timing, 200),
        UpdateDecision::Ready
    );
    assert_eq!(
        policy.decide(UpdateStage::Install, UpdateTrigger::Automatic, timing, 200),
        UpdateDecision::DeferredUntil(300)
    );
    assert_eq!(
        policy.decide(UpdateStage::Install, UpdateTrigger::Automatic, timing, 300),
        UpdateDecision::Ready
    );
}

#[test]
fn periodic_search_has_bounded_intervals_and_handles_clock_rollback() {
    assert!(SearchInterval::new(899).is_err());
    assert!(SearchInterval::new(604_801).is_err());
    let interval = SearchInterval::new(900).unwrap();
    assert_eq!(interval.next_search(None, 100), SearchDue::Now);
    assert_eq!(interval.next_search(Some(100), 999), SearchDue::At(1000));
    assert_eq!(interval.next_search(Some(100), 1000), SearchDue::Now);
    assert_eq!(
        interval.next_search(Some(100), 99),
        SearchDue::ClockRollback
    );
    assert_eq!(
        interval.next_search(Some(u64::MAX), u64::MAX),
        SearchDue::ClockOverflow
    );
}

#[test]
fn removing_overrides_restores_inheritance_and_disabling_installation_stops_automation() {
    let mut global = UpdatePreferences::default();
    let mut tool = UpdateOverrides {
        download: Some(true),
        ..Default::default()
    };
    let mut managed = AdministrativeUpdatePolicy::default();
    assert!(
        resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted)
            .download()
            .enabled
    );
    global.download = true;
    tool.download = Some(false);
    assert!(
        !resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted)
            .download()
            .enabled
    );
    managed.automatic.download = Some(true);
    assert!(
        resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted)
            .download()
            .enabled
    );
    managed.automatic.download = None;
    assert!(
        !resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted)
            .download()
            .enabled
    );
    tool.download = None;
    assert!(
        resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted)
            .download()
            .enabled
    );
    global.install = true;
    let enabled = resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted);
    assert_eq!(
        enabled.decide(
            UpdateStage::Install,
            UpdateTrigger::Automatic,
            UpdateTiming::default(),
            100
        ),
        UpdateDecision::Ready
    );
    global.install = false;
    let disabled = resolve_update_policy(global, tool, managed, AutomaticInstallConsent::Granted);
    assert_eq!(
        disabled.decide(
            UpdateStage::Install,
            UpdateTrigger::Automatic,
            UpdateTiming::default(),
            100
        ),
        UpdateDecision::Disabled
    );
}
