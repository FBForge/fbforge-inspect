use super::*;
use crate::{
    network::ProxyMode,
    settings::UserSettings,
    updates::{
        AutomaticInstallConsent, UpdateDecision, UpdatePreferences, UpdateStage, UpdateTiming,
        UpdateTrigger,
    },
};
fn dword(name: &str, value: u32) -> RegistryValue {
    RegistryValue {
        name: name.into(),
        kind: 4,
        data: value.to_le_bytes().to_vec(),
    }
}
fn string(name: &str, value: &str) -> RegistryValue {
    RegistryValue {
        name: name.into(),
        kind: 1,
        data: value
            .encode_utf16()
            .chain(Some(0))
            .flat_map(u16::to_le_bytes)
            .collect(),
    }
}
fn schema() -> Vec<RegistryValue> {
    vec![dword("SchemaVersion", 1)]
}
fn read(values: &[RegistryValue]) -> AdministrativePolicyState {
    decode_values(Some(values)).unwrap()
}
fn invalid(values: &[RegistryValue]) {
    assert!(matches!(
        decode_values(Some(values)),
        Err(AdministrativePolicyError::InvalidPolicy)
    ));
}

#[test]
fn missing_key_empty_key_schema_and_removal_have_distinct_meanings() {
    assert!(matches!(
        decode_values(None).unwrap(),
        AdministrativePolicyState::Unmanaged
    ));
    invalid(&[]);
    invalid(&[dword("SchemaVersion", 0)]);
    invalid(&[dword("SchemaVersion", 2)]);
    let values = schema();
    assert!(matches!(
        read(&values),
        AdministrativePolicyState::Managed(_)
    ));
    assert_eq!(read(&values).updates_allowed_override(), None);
    assert!(read(&values).update_policy().updates_allowed);
    assert!(matches!(
        decode_values(None).unwrap(),
        AdministrativePolicyState::Unmanaged
    ));
}
#[test]
fn managed_false_inherit_and_removal_flow_through_the_existing_consumer() {
    let mut settings = UserSettings::default();
    settings.set_global_updates(UpdatePreferences {
        search: true,
        download: true,
        install: false,
    });
    let mut values = schema();
    values.push(dword("AutomaticSearch", 0));
    let managed = read(&values);
    let resolved = settings.effective_update_policy("connect", managed.update_policy());
    assert!(!resolved.search().enabled);
    assert!(resolved.search().is_managed);
    values.pop();
    let inherited = settings.effective_update_policy("connect", read(&values).update_policy());
    assert!(inherited.search().enabled);
    assert!(!inherited.search().is_managed);
    values.push(dword("UpdateAllowed", 0));
    assert_eq!(read(&values).updates_allowed_override(), Some(false));
    assert_eq!(
        settings
            .effective_update_policy("connect", read(&values).update_policy())
            .decide(
                UpdateStage::Search,
                UpdateTrigger::Manual,
                UpdateTiming::default(),
                10
            ),
        UpdateDecision::BlockedByAdministration
    );
    values.pop();
    assert_eq!(
        settings
            .effective_update_policy("connect", read(&values).update_policy())
            .decide(
                UpdateStage::Search,
                UpdateTrigger::Manual,
                UpdateTiming::default(),
                10
            ),
        UpdateDecision::Ready
    );
}
#[test]
fn administrative_install_true_never_grants_consent() {
    let mut values = schema();
    values.extend([dword("AutomaticDownload", 1), dword("AutomaticInstall", 1)]);
    let settings = UserSettings::default();
    assert_eq!(
        settings.automatic_install_consent(),
        AutomaticInstallConsent::NotGranted
    );
    let resolved = settings.effective_update_policy("connect", read(&values).update_policy());
    assert!(resolved.download().enabled && resolved.install().enabled);
    assert!(resolved.download().is_managed && resolved.install().is_managed);
    assert_eq!(
        resolved.decide(
            UpdateStage::Install,
            UpdateTrigger::Automatic,
            UpdateTiming::default(),
            10
        ),
        UpdateDecision::ConsentRequired
    );
}
#[test]
fn network_mode_server_removal_and_user_inheritance_are_explicit() {
    let user = NetworkConfig::new(ProxyMode::Explicit {
        server: "user.invalid:8080".into(),
    })
    .unwrap();
    let mut values = schema();
    assert_eq!(read(&values).network_override().unwrap_or(&user), &user);
    values.push(string("ProxyMode", "system"));
    assert_eq!(
        read(&values).network_override().unwrap().proxy(),
        &ProxyMode::System
    );
    values.push(string("ProxyServer", "managed.invalid:3128"));
    invalid(&values);
    values[1] = string("ProxyMode", "explicit");
    assert_eq!(
        read(&values).network_override().unwrap().proxy(),
        &ProxyMode::Explicit {
            server: "managed.invalid:3128".into()
        }
    );
    values.pop();
    invalid(&values);
    values.pop();
    assert_eq!(read(&values).network_override().unwrap_or(&user), &user);
}
#[test]
fn registry_names_are_case_insensitive_but_duplicates_and_unknowns_are_rejected() {
    let values = [dword("schemaversion", 1), dword("automaticsearch", 0)];
    assert_eq!(read(&values).update_policy().automatic.search, Some(false));
    invalid(&[dword("SchemaVersion", 1), dword("SCHEMAVERSION", 1)]);
    invalid(&[dword("SchemaVersion", 1), dword("AutomaticInstal", 1)]);
    invalid(&[dword("SchemaVersion", 1), dword("", 1)]);
}
#[test]
fn types_boolean_ranges_lengths_and_nulls_fail_closed() {
    for value in [2, u32::MAX] {
        invalid(&[dword("SchemaVersion", 1), dword("UpdateAllowed", value)]);
    }
    for kind in [0, 1, 5, 7, 11] {
        let mut entry = dword("AutomaticInstall", 1);
        entry.kind = kind;
        invalid(&[dword("SchemaVersion", 1), entry]);
    }
    for data in [vec![], vec![1], vec![1, 0, 0, 0, 0]] {
        invalid(&[
            dword("SchemaVersion", 1),
            RegistryValue {
                name: "UpdateAllowed".into(),
                kind: 4,
                data,
            },
        ]);
    }
    for data in [
        vec![],
        vec![0],
        vec![0x00, 0xd8, 0, 0],
        vec![b's', 0],
        vec![0, 0, b's', 0, 0, 0],
        vec![0; MAX_DATA_BYTES + 2],
    ] {
        invalid(&[
            dword("SchemaVersion", 1),
            RegistryValue {
                name: "ProxyMode".into(),
                kind: 1,
                data,
            },
        ]);
    }
    invalid(&[dword("SchemaVersion", 1), string("ProxyMode", "")]);
    invalid(&[dword("SchemaVersion", 1), string("ProxyMode", "System")]);
    let values: Vec<_> = (0..MAX_VALUES + 1)
        .map(|_| dword("SchemaVersion", 1))
        .collect();
    invalid(&values);
}
#[test]
fn managed_proxy_reuses_the_real_strict_network_validator() {
    for server in [
        "",
        "http://proxy.invalid:8080",
        "user:password@proxy.invalid:8080",
        "proxy.invalid:0",
        "proxy.invalid:080",
        "proxy.invalid:80\0evil",
    ] {
        invalid(&[
            dword("SchemaVersion", 1),
            string("ProxyMode", "explicit"),
            string("ProxyServer", server),
        ]);
    }
    let too_long = "p".repeat(321);
    invalid(&[
        dword("SchemaVersion", 1),
        string("ProxyMode", "explicit"),
        string("ProxyServer", &too_long),
    ]);
    let values = [
        dword("SchemaVersion", 1),
        string("ProxyMode", "explicit"),
        string("ProxyServer", "[::1]:8080"),
    ];
    let state = read(&values);
    assert!(state.network_override().is_some());
    assert!(!format!("{state:?}").contains("[::1]"));
}
#[test]
fn failures_never_include_untrusted_data_or_claim_unmanaged_on_other_platforms() {
    for error in [
        AdministrativePolicyError::InvalidPolicy,
        AdministrativePolicyError::Unavailable,
        AdministrativePolicyError::UnsupportedPlatform,
    ] {
        assert!(!error.to_string().contains("proxy.invalid"));
        assert!(!error.to_string().contains("HKLM"));
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    assert!(matches!(
        read_administrative_policy(),
        Err(AdministrativePolicyError::UnsupportedPlatform)
    ));
}

#[test]
fn observed_changes_and_incomplete_enumeration_never_return_a_snapshot() {
    let values = schema();
    let before = KeyStamp {
        subkeys: 0,
        values: 1,
        max_name: 13,
        max_data: 4,
        last_write: 10,
    };
    assert!(decode_snapshot(before, &values, before).is_ok());
    assert!(matches!(
        decode_snapshot(
            before,
            &values,
            KeyStamp {
                last_write: 11,
                ..before
            }
        ),
        Err(AdministrativePolicyError::Unavailable)
    ));
    assert!(matches!(
        decode_snapshot(before, &[], before),
        Err(AdministrativePolicyError::Unavailable)
    ));
    for invalid_stamp in [
        KeyStamp {
            subkeys: 1,
            ..before
        },
        KeyStamp {
            values: 8,
            ..before
        },
        KeyStamp {
            max_name: 32,
            ..before
        },
        KeyStamp {
            max_data: 643,
            ..before
        },
    ] {
        assert!(matches!(
            decode_snapshot(invalid_stamp, &values, invalid_stamp),
            Err(AdministrativePolicyError::InvalidPolicy)
        ));
    }
}
