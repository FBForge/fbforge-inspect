mod support;
use fbforge_suite_core::{
    catalog::parse_catalog,
    inventory::{
        DetectionReport, Installation, InstallationCondition, Inventory, RegistryHive,
        RegistrySource, RegistryView,
    },
    network::{NetworkConfig, ProxyMode},
    settings::UserSettings,
    storage::{StateStore, StoreError},
    trust::TrustError,
    updates::{
        AdministrativeUpdatePolicy, AutomaticInstallConsent, UpdateDecision, UpdateOverrides,
        UpdatePreferences, UpdateStage, UpdateTiming, UpdateTrigger,
    },
};
use serde_json::json;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};
use support::{catalog_body, payload, sign, trust_store};

#[test]
fn preferences_round_trip_without_turning_missing_overrides_into_false() {
    let mut settings = UserSettings::default();
    assert_eq!(settings.global_updates(), UpdatePreferences::default());
    settings.set_global_updates(UpdatePreferences {
        search: true,
        download: true,
        install: true,
    });
    settings
        .set_tool_override(
            "fbforge-connect",
            Some(UpdateOverrides {
                search: Some(false),
                download: None,
                install: None,
            }),
        )
        .unwrap();
    let restored = UserSettings::from_json(&settings.to_json().unwrap()).unwrap();
    let policy =
        restored.effective_update_policy("fbforge-connect", AdministrativeUpdatePolicy::default());
    assert!(!policy.search().enabled);
    assert!(policy.download().enabled);
    assert_eq!(
        policy.decide(
            UpdateStage::Install,
            UpdateTrigger::Automatic,
            UpdateTiming::default(),
            100
        ),
        UpdateDecision::ConsentRequired
    );
    settings.set_tool_override("fbforge-connect", None).unwrap();
    assert!(
        settings
            .effective_update_policy("fbforge-connect", AdministrativeUpdatePolicy::default())
            .search()
            .enabled
    );
}

#[test]
fn explicit_consent_survives_restart_and_revocation_blocks_every_tool_override() {
    let mut settings = UserSettings::default();
    settings
        .set_tool_override(
            "fbforge-inspect",
            Some(UpdateOverrides {
                install: Some(true),
                ..Default::default()
            }),
        )
        .unwrap();
    settings.set_automatic_install_consent(AutomaticInstallConsent::Granted);
    let mut restored = UserSettings::from_json(&settings.to_json().unwrap()).unwrap();
    let automatic_install = |settings: &UserSettings| {
        settings
            .effective_update_policy("fbforge-inspect", AdministrativeUpdatePolicy::default())
            .decide(
                UpdateStage::Install,
                UpdateTrigger::Automatic,
                UpdateTiming::default(),
                100,
            )
    };
    assert_eq!(automatic_install(&restored), UpdateDecision::Ready);
    restored.set_automatic_install_consent(AutomaticInstallConsent::NotGranted);
    assert_eq!(
        automatic_install(&restored),
        UpdateDecision::ConsentRequired
    );
    let managed = AdministrativeUpdatePolicy {
        updates_allowed: false,
        ..Default::default()
    };
    assert_eq!(
        restored
            .effective_update_policy("fbforge-inspect", managed)
            .decide(
                UpdateStage::Install,
                UpdateTrigger::Manual,
                UpdateTiming::default(),
                100
            ),
        UpdateDecision::BlockedByAdministration
    );
}

#[test]
fn settings_reject_unknown_versions_fields_invalid_intervals_and_unbounded_overrides() {
    let settings = UserSettings::default();
    let source: serde_json::Value = serde_json::from_str(&settings.to_json().unwrap()).unwrap();
    for (key, value) in [
        ("schemaVersion", json!(2)),
        ("searchIntervalSeconds", json!(0)),
        ("searchIntervalSeconds", json!(604801)),
        ("managed", json!(true)),
    ] {
        let mut candidate = source.clone();
        candidate[key] = value;
        assert!(UserSettings::from_json(&candidate.to_string()).is_err());
    }
    let mut candidate = source.clone();
    candidate["toolUpdates"] =
        json!({"../tool": {"search": false, "download": null, "install": null}});
    assert!(UserSettings::from_json(&candidate.to_string()).is_err());
    assert!(UserSettings::from_json(&" ".repeat(65_537)).is_err());
    let mut settings = settings;
    assert!(settings.set_search_interval_seconds(900).is_ok());
    assert!(settings.set_search_interval_seconds(604800).is_ok());
    assert!(settings.set_search_interval_seconds(604801).is_err());
    assert_eq!(settings.search_interval_seconds(), 604800);
    for i in 0..500 {
        settings
            .set_tool_override(
                &format!("tool-{i}"),
                Some(UpdateOverrides {
                    search: Some(false),
                    ..Default::default()
                }),
            )
            .unwrap();
    }
    assert!(settings
        .set_tool_override("tool-501", Some(UpdateOverrides::default()))
        .is_err());
    assert_eq!(
        settings.tool_override("tool-501"),
        UpdateOverrides::default()
    );
    assert_eq!(settings.tool_override("tool-0").search, Some(false));
    settings.set_tool_override("tool-0", None).unwrap();
    assert_eq!(settings.tool_override("tool-0"), UpdateOverrides::default());
}

#[test]
fn network_settings_reuse_transport_validation_and_reject_ambiguous_override_maps() {
    let mut settings = UserSettings::default();
    let proxy = NetworkConfig::new(ProxyMode::Explicit {
        server: "proxy.example:8080".into(),
    })
    .unwrap();
    settings.set_network(proxy.clone());
    assert_eq!(
        UserSettings::from_json(&settings.to_json().unwrap())
            .unwrap()
            .network(),
        &proxy
    );
    let mut document: serde_json::Value =
        serde_json::from_str(&settings.to_json().unwrap()).unwrap();
    document["network"] =
        json!({"proxy":{"mode":"explicit","server":"user:secret@proxy.example:8080"}});
    assert!(UserSettings::from_json(&document.to_string()).is_err());
    let document = settings.to_json().unwrap().replace(
        "\"toolUpdates\":{}",
        "\"toolUpdates\":{\"tool\":{\"search\":false},\"tool\":{\"search\":true}}",
    );
    assert!(UserSettings::from_json(&document).is_err());
}

#[test]
fn settings_are_committed_with_inventory_state_and_recovered_after_reopening() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "suite-settings-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let root = directory.join("state");
    let mut settings = UserSettings::default();
    settings.set_global_updates(UpdatePreferences {
        search: false,
        download: true,
        install: false,
    });
    settings.set_search_interval_seconds(900).unwrap();
    {
        let mut store = StateStore::open(&root).unwrap();
        assert_eq!(
            store.load_settings().unwrap().global_updates(),
            UpdatePreferences::default()
        );
        let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
        let inventory = Inventory::from_catalog(&catalog).unwrap();
        store.save_inventory(&inventory).unwrap();
        store.save_settings(&settings).unwrap();
        store.save_inventory(&inventory).unwrap();
    }
    {
        let store = StateStore::open(&root).unwrap();
        let restored = store.load_settings().unwrap();
        assert_eq!(restored.global_updates(), settings.global_updates());
        assert_eq!(restored.search_interval_seconds(), 900);
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn older_state_gets_defaults_and_invalid_saved_preferences_do_not_reset_state() {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let directory = std::env::temp_dir().join(format!(
        "suite-settings-compat-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&directory).unwrap();
    let root = directory.join("state");
    {
        let mut store = StateStore::open(&root).unwrap();
        store.save_settings(&UserSettings::default()).unwrap();
    }
    let path = root.join("state-v1.json");
    let mut envelope: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let mut payload: serde_json::Value =
        serde_json::from_str(envelope["payload"].as_str().unwrap()).unwrap();
    payload.as_object_mut().unwrap().remove("settings");
    let write_payload = |envelope: &mut serde_json::Value, payload: &serde_json::Value| {
        let text = payload.to_string();
        let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
        envelope["sha256"] = json!(digest
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>());
        envelope["payload"] = json!(text);
        fs::write(&path, envelope.to_string()).unwrap();
    };
    write_payload(&mut envelope, &payload);
    {
        let store = StateStore::open(&root).unwrap();
        assert_eq!(
            store.load_settings().unwrap().global_updates(),
            UpdatePreferences::default()
        );
    }
    let mut settings: serde_json::Value =
        serde_json::from_str(&UserSettings::default().to_json().unwrap()).unwrap();
    settings["network"] = json!({"proxy":{"mode":"explicit","server":"user:password@proxy:80"}});
    payload["settings"] = json!(settings.to_string());
    write_payload(&mut envelope, &payload);
    let before = fs::read(&path).unwrap();
    assert!(matches!(
        StateStore::open(&root),
        Err(StoreError::CorruptState)
    ));
    assert_eq!(fs::read(&path).unwrap(), before);
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn saving_preferences_preserves_a_signed_catalog_checkpoint_and_installed_version() {
    let directory =
        std::env::temp_dir().join(format!("suite-settings-checkpoint-{}", std::process::id()));
    fs::create_dir(&directory).unwrap();
    let root = directory.join("state");
    let trust = trust_store();
    let builtin = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let mut document = payload("catalog", catalog_body());
    document["sequence"] = json!(2);
    {
        let mut store = StateStore::open(&root).unwrap();
        store
            .accept_catalog(&sign(&document, "test-key", 42), &trust, 300)
            .unwrap();
        let mut inventory = store
            .load_inventory(&builtin, &trust, 300)
            .unwrap()
            .inventory;
        inventory
            .record_detection(
                "fbforge-connect",
                DetectionReport {
                    installations: vec![Installation {
                        root: "C:/Tools/Connect".into(),
                        executable: Some("C:/Tools/Connect/fbforge-connect.exe".into()),
                        reported_version: Some("0.3.0".into()),
                        version: Some("0.3.0".parse().unwrap()),
                        condition: InstallationCondition::Detected,
                        sources: vec![RegistrySource {
                            hive: RegistryHive::CurrentUser,
                            view: RegistryView::Registry64,
                            key_name: "FIT-Connect Test-Client".into(),
                        }],
                    }],
                    issues: vec![],
                },
            )
            .unwrap();
        store.save_inventory(&inventory).unwrap();
        let mut settings = UserSettings::default();
        settings.set_search_interval_seconds(900).unwrap();
        store.save_settings(&settings).unwrap();
    }
    {
        let mut store = StateStore::open(&root).unwrap();
        assert_eq!(
            store
                .load_catalog(&trust, 301)
                .unwrap()
                .catalog()
                .tools()
                .len(),
            2
        );
        assert_eq!(
            store.load_settings().unwrap().search_interval_seconds(),
            900
        );
        let recovered = store.load_inventory(&builtin, &trust, 301).unwrap();
        let installation = &recovered
            .inventory
            .get("fbforge-connect")
            .unwrap()
            .detection()
            .installations()[0];
        assert_eq!(installation.version.as_ref().unwrap().to_string(), "0.3.0");
        assert_eq!(installation.condition, InstallationCondition::Unconfirmed);
        assert_eq!(
            installation.executable.as_ref().unwrap(),
            &std::path::PathBuf::from("C:/Tools/Connect/fbforge-connect.exe")
        );
        let older = sign(&payload("catalog", catalog_body()), "test-key", 42);
        assert_eq!(
            store.accept_catalog(&older, &trust, 302).unwrap_err(),
            StoreError::Trust(TrustError::Rollback)
        );
    }
    fs::remove_dir_all(directory).unwrap();
}
