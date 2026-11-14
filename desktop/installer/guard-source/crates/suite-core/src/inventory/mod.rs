//! Retained tool descriptions and local observations have independent lifecycles.
mod snapshot;
mod types;
pub use types::*;

use crate::catalog::{parse_catalog, Capability, Catalog, ToolManifest};
use serde::Serialize;
use serde_json::json;
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolEntry {
    manifest: ToolManifest,
    is_listed: bool,
    detection: DetectionState,
}

impl ToolEntry {
    pub fn manifest(&self) -> &ToolManifest {
        &self.manifest
    }
    pub fn is_listed(&self) -> bool {
        self.is_listed
    }
    pub fn detection(&self) -> &DetectionState {
        &self.detection
    }

    /// UI affordance only. The installer still needs a current native scan,
    /// verified package and all execution prerequisites under its own lock.
    pub fn can_offer_installation(&self) -> bool {
        self.is_listed
            && self.manifest.capabilities.contains(&Capability::Install)
            && matches!(&self.detection, DetectionState::Complete { installations } if installations.is_empty())
    }
}

#[derive(Debug, Default, Clone)]
pub struct Inventory {
    entries: BTreeMap<String, ToolEntry>,
}

impl Inventory {
    pub fn from_catalog(catalog: &Catalog) -> Result<Self, InventoryError> {
        let mut inventory = Self::default();
        inventory.apply_catalog(catalog)?;
        Ok(inventory)
    }

    pub fn get(&self, tool_id: &str) -> Option<&ToolEntry> {
        self.entries.get(tool_id)
    }
    pub fn entries(&self) -> impl Iterator<Item = &ToolEntry> {
        self.entries.values()
    }

    /// The caller supplies a trusted built-in or verified online/cache catalog.
    /// Validate the combined set before replacing state, including remembered
    /// tools, so another tool cannot take over their detection identities.
    pub fn apply_catalog(&mut self, catalog: &Catalog) -> Result<(), InventoryError> {
        let mut next = self.entries.clone();
        for (id, entry) in &mut next {
            entry.is_listed = catalog.tools().iter().any(|tool| tool.id == *id);
        }
        next.retain(|_, entry| entry.is_listed || !matches!(&entry.detection, DetectionState::Complete { installations } if installations.is_empty()));
        for tool in catalog.tools() {
            if let Some(entry) = next.get_mut(&tool.id) {
                if entry.manifest.app_id != tool.app_id {
                    return Err(InventoryError::IdentityChanged);
                }
                let mut updated = tool.clone();
                if !matches!(&entry.detection, DetectionState::Complete { installations } if installations.is_empty())
                {
                    retain_detection_names(
                        &mut updated.registry_names,
                        &entry.manifest.registry_names,
                    );
                    retain_detection_names(&mut updated.executables, &entry.manifest.executables);
                }
                entry.manifest = updated;
                entry.is_listed = true;
            } else {
                next.insert(
                    tool.id.clone(),
                    ToolEntry {
                        manifest: tool.clone(),
                        is_listed: true,
                        detection: DetectionState::NotScanned,
                    },
                );
            }
        }
        let tools: Vec<_> = next.values().map(|entry| &entry.manifest).collect();
        parse_catalog(&json!({"schemaVersion":1,"tools":tools}).to_string())
            .map_err(|_| InventoryError::ConflictingCatalog)?;
        self.entries = next;
        Ok(())
    }

    pub fn record_detection(
        &mut self,
        tool_id: &str,
        mut report: DetectionReport,
    ) -> Result<(), InventoryError> {
        let entry = self
            .entries
            .get_mut(tool_id)
            .ok_or(InventoryError::UnknownTool)?;
        if report.issues.is_empty() {
            entry.detection = DetectionState::Complete {
                installations: report.installations,
            };
        } else {
            // An incomplete scan is not evidence that earlier installations
            // disappeared. Keep them visible, clearly marked as unconfirmed.
            for previous in entry.detection.installations() {
                if !report
                    .installations
                    .iter()
                    .any(|current| current.root == previous.root)
                {
                    let mut retained = previous.clone();
                    retained.condition = InstallationCondition::Unconfirmed;
                    report.installations.push(retained);
                }
            }
            entry.detection = DetectionState::Incomplete {
                installations: report.installations,
                issues: report.issues,
            };
        }
        let can_remove = !entry.is_listed
            && matches!(&entry.detection, DetectionState::Complete { installations } if installations.is_empty());
        if can_remove {
            self.entries.remove(tool_id);
        }
        Ok(())
    }
}

fn retain_detection_names(current: &mut Vec<String>, previous: &[String]) {
    for name in previous {
        if !current.iter().any(|value| value.eq_ignore_ascii_case(name)) {
            current.push(name.clone());
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InventoryError {
    UnknownTool,
    IdentityChanged,
    ConflictingCatalog,
    InvalidSnapshot,
}

impl fmt::Display for InventoryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnknownTool => "Das Tool ist im lokalen Bestand nicht bekannt.",
            Self::IdentityChanged => "Die App-Identität einer vorhandenen Tool-Beschreibung wurde geändert.",
            Self::ConflictingCatalog => "Der Katalog überschneidet sich mit dem gemerkten Tool-Bestand oder überschreitet dessen Grenzen.",
            Self::InvalidSnapshot => "Der gespeicherte Tool-Bestand ist beschädigt oder wird nicht unterstützt.",
        })
    }
}
impl std::error::Error for InventoryError {}
