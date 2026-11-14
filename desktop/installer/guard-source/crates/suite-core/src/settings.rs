//! Private user preferences, distinct from native administrative policy and trust.
//! Persisted consent is never accepted from a generic renderer settings payload.
use crate::{
    catalog::is_identifier,
    network::NetworkConfig,
    updates::{
        resolve_update_policy, AdministrativeUpdatePolicy, AutomaticInstallConsent,
        EffectiveUpdatePolicy, SearchInterval, UpdateOverrides, UpdatePreferences,
    },
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

const MAX_SETTINGS_BYTES: usize = 65_536;
const MAX_TOOL_OVERRIDES: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserSettings {
    schema_version: u32,
    updates: UpdatePreferences,
    #[serde(deserialize_with = "deserialize_tool_updates")]
    tool_updates: BTreeMap<String, UpdateOverrides>,
    automatic_install_consent: AutomaticInstallConsent,
    search_interval_seconds: u64,
    network: NetworkConfig,
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            schema_version: 1,
            updates: UpdatePreferences::default(),
            tool_updates: BTreeMap::new(),
            automatic_install_consent: AutomaticInstallConsent::NotGranted,
            search_interval_seconds: 6 * 60 * 60,
            network: NetworkConfig::default(),
        }
    }
}

impl UserSettings {
    /// Only for the private native state document, not a renderer command input.
    pub fn from_json(json: &str) -> Result<Self, SettingsError> {
        if json.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::TooLarge);
        }
        let settings: Self = serde_json::from_str(json).map_err(|_| SettingsError::Invalid)?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn to_json(&self) -> Result<String, SettingsError> {
        self.validate()?;
        let json = serde_json::to_string(self).map_err(|_| SettingsError::Invalid)?;
        if json.len() > MAX_SETTINGS_BYTES {
            return Err(SettingsError::TooLarge);
        }
        Ok(json)
    }

    fn validate(&self) -> Result<(), SettingsError> {
        if self.schema_version != 1
            || self.tool_updates.len() > MAX_TOOL_OVERRIDES
            || self.tool_updates.keys().any(|id| !is_identifier(id))
            || SearchInterval::new(self.search_interval_seconds).is_err()
        {
            return Err(SettingsError::Invalid);
        }
        NetworkConfig::new(self.network.proxy().clone()).map_err(|_| SettingsError::Invalid)?;
        Ok(())
    }

    pub fn global_updates(&self) -> UpdatePreferences {
        self.updates
    }
    pub fn set_global_updates(&mut self, updates: UpdatePreferences) {
        self.updates = updates;
    }
    pub fn tool_override(&self, tool_id: &str) -> UpdateOverrides {
        self.tool_updates.get(tool_id).copied().unwrap_or_default()
    }
    pub fn set_tool_override(
        &mut self,
        tool_id: &str,
        preferences: Option<UpdateOverrides>,
    ) -> Result<(), SettingsError> {
        if !is_identifier(tool_id) {
            return Err(SettingsError::Invalid);
        }
        let mut next = self.clone();
        match preferences {
            Some(preferences) => {
                next.tool_updates.insert(tool_id.to_owned(), preferences);
            }
            None => {
                next.tool_updates.remove(tool_id);
            }
        }
        // Keep the previous state when either the entry or serialized size cap
        // would be exceeded. Explicit false and inherited None remain distinct.
        next.to_json()?;
        *self = next;
        Ok(())
    }
    pub fn search_interval_seconds(&self) -> u64 {
        self.search_interval_seconds
    }
    pub fn set_search_interval_seconds(&mut self, seconds: u64) -> Result<(), SettingsError> {
        SearchInterval::new(seconds).map_err(|_| SettingsError::Invalid)?;
        self.search_interval_seconds = seconds;
        Ok(())
    }
    pub fn automatic_install_consent(&self) -> AutomaticInstallConsent {
        self.automatic_install_consent
    }
    /// The native consent action must present its own deliberate activation.
    /// Ordinary preference changes must not call this to infer user consent.
    pub fn set_automatic_install_consent(&mut self, consent: AutomaticInstallConsent) {
        self.automatic_install_consent = consent;
    }
    pub fn network(&self) -> &NetworkConfig {
        &self.network
    }
    pub fn set_network(&mut self, network: NetworkConfig) {
        self.network = network;
    }
    pub fn effective_update_policy(
        &self,
        tool_id: &str,
        administration: AdministrativeUpdatePolicy,
    ) -> EffectiveUpdatePolicy {
        resolve_update_policy(
            self.updates,
            self.tool_override(tool_id),
            administration,
            self.automatic_install_consent,
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SettingsError {
    Invalid,
    TooLarge,
}
impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Invalid => "Die Suite-Einstellungen sind ungültig oder werden nicht unterstützt.",
            Self::TooLarge => "Die Suite-Einstellungen überschreiten die zulässige Größe.",
        })
    }
}
impl std::error::Error for SettingsError {}

fn deserialize_tool_updates<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, UpdateOverrides>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Overrides;
    impl<'de> serde::de::Visitor<'de> for Overrides {
        type Value = BTreeMap<String, UpdateOverrides>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("eindeutige begrenzte Werkzeugeinstellungen")
        }
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut entries = BTreeMap::new();
            while let Some(id) = map.next_key::<String>()? {
                if !is_identifier(&id)
                    || entries.len() >= MAX_TOOL_OVERRIDES
                    || entries.contains_key(&id)
                {
                    return Err(serde::de::Error::custom("Ungültige Werkzeugeinstellungen."));
                }
                entries.insert(id, map.next_value()?);
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_map(Overrides)
}
