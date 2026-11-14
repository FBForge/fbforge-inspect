use super::*;
use crate::discovery::validate_windows_location;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const MAX_SNAPSHOT_BYTES: usize = 4_194_304;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedInventory {
    schema_version: u32,
    tools: Vec<SavedTool>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedTool {
    manifest: ToolManifest,
    installations: Vec<Installation>,
}

impl Inventory {
    pub fn snapshot_json(&self) -> Result<String, InventoryError> {
        let saved = SavedInventory {
            schema_version: 1,
            tools: self
                .entries
                .values()
                .map(|entry| SavedTool {
                    manifest: entry.manifest.clone(),
                    installations: entry.detection.installations().to_vec(),
                })
                .collect(),
        };
        validate_saved(&saved)?;
        let json = serde_json::to_string(&saved).map_err(|_| InventoryError::InvalidSnapshot)?;
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(InventoryError::InvalidSnapshot);
        }
        Ok(json)
    }

    /// Saved paths are retained for display and native re-discovery. No saved
    /// scan result is promoted to current evidence or an execution permission.
    pub fn restore_snapshot(json: &str, current: &Catalog) -> Result<Self, InventoryError> {
        if json.len() > MAX_SNAPSHOT_BYTES {
            return Err(InventoryError::InvalidSnapshot);
        }
        let saved: SavedInventory =
            serde_json::from_str(json).map_err(|_| InventoryError::InvalidSnapshot)?;
        validate_saved(&saved)?;
        let mut inventory = Self::default();
        for mut tool in saved.tools {
            for installation in &mut tool.installations {
                installation.condition = InstallationCondition::Unconfirmed;
            }
            let detection = if tool.installations.is_empty() {
                DetectionState::NotScanned
            } else {
                DetectionState::Incomplete {
                    installations: tool.installations,
                    issues: vec![DetectionIssue::NeedsRescan],
                }
            };
            inventory.entries.insert(
                tool.manifest.id.clone(),
                ToolEntry {
                    manifest: tool.manifest,
                    is_listed: false,
                    detection,
                },
            );
        }
        inventory.apply_catalog(current)?;
        Ok(inventory)
    }
}

fn validate_saved(saved: &SavedInventory) -> Result<(), InventoryError> {
    if saved.schema_version != 1 || saved.tools.len() > 500 {
        return Err(InventoryError::InvalidSnapshot);
    }
    let manifests: Vec<_> = saved.tools.iter().map(|tool| &tool.manifest).collect();
    parse_catalog(&json!({"schemaVersion":1,"tools":manifests}).to_string())
        .map_err(|_| InventoryError::InvalidSnapshot)?;
    for tool in &saved.tools {
        if tool.installations.len() > 64 {
            return Err(InventoryError::InvalidSnapshot);
        }
        let mut roots = HashSet::new();
        for installation in &tool.installations {
            if !valid_installation(installation, &tool.manifest)
                || !roots.insert(&installation.root)
            {
                return Err(InventoryError::InvalidSnapshot);
            }
        }
    }
    Ok(())
}

fn valid_installation(installation: &Installation, tool: &ToolManifest) -> bool {
    let Some(root) = installation.root.to_str() else {
        return false;
    };
    let local = root.strip_prefix(r"\\?\").unwrap_or(root);
    if validate_windows_location(local).is_err()
        || installation.sources.is_empty()
        || installation.sources.len() > 64
    {
        return false;
    }
    if !installation.sources.iter().all(|source| {
        tool.registry_names
            .iter()
            .any(|name| name.eq_ignore_ascii_case(&source.key_name))
    }) {
        return false;
    }
    if let Some(reported) = &installation.reported_version {
        if reported.len() > 128 || reported.chars().any(char::is_control) {
            return false;
        }
    }
    if installation
        .reported_version
        .as_deref()
        .and_then(|value| semver::Version::parse(value).ok())
        != installation.version
    {
        return false;
    }
    if let Some(executable) = &installation.executable {
        let Some(executable) = executable.to_str() else {
            return false;
        };
        let root = root.replace('\\', "/");
        let executable = executable.replace('\\', "/");
        if !tool
            .executables
            .iter()
            .any(|name| executable.eq_ignore_ascii_case(&format!("{root}/{name}")))
        {
            return false;
        }
    }
    true
}
