//! Parsing is structural validation, never proof of catalogue authenticity.
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fmt};

const MAX_CATALOG_BYTES: usize = 1_048_576;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CatalogInput {
    schema_version: u32,
    tools: Vec<ToolManifest>,
}

#[derive(Debug)]
pub struct Catalog {
    tools: Vec<ToolManifest>,
}

impl Catalog {
    pub fn tools(&self) -> &[ToolManifest] {
        &self.tools
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ToolManifest {
    pub id: String,
    pub product_name: String,
    pub app_id: String,
    pub platform: Platform,
    pub installer: Installer,
    pub registry_names: Vec<String>,
    pub executables: Vec<String>,
    pub release_repository: String,
    pub trust_policy_id: String,
    pub minimum_suite_version: Version,
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Platform {
    #[serde(rename = "windows-x64")]
    WindowsX64,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Installer {
    #[serde(rename = "nsis-current-user")]
    NsisCurrentUser,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Capability {
    Install,
    Launch,
    Update,
    Repair,
    Uninstall,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CatalogError {
    TooLarge,
    InvalidStructure,
    UnsupportedVersion,
    InvalidTool,
    ConflictingIdentity,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TooLarge => "Der Katalog überschreitet die zulässige Größe.",
            Self::InvalidStructure => "Der Katalog entspricht nicht dem Datenvertrag.",
            Self::UnsupportedVersion => "Diese Katalogversion wird nicht unterstützt.",
            Self::InvalidTool => "Eine Tool-Beschreibung enthält unzulässige Werte.",
            Self::ConflictingIdentity => {
                "Tool-Identitäten oder Erkennungsregeln überschneiden sich."
            }
        };
        f.write_str(message)
    }
}

impl std::error::Error for CatalogError {}

pub fn parse_catalog(json: &str) -> Result<Catalog, CatalogError> {
    if json.len() > MAX_CATALOG_BYTES {
        return Err(CatalogError::TooLarge);
    }
    let input: CatalogInput =
        serde_json::from_str(json).map_err(|_| CatalogError::InvalidStructure)?;
    if input.schema_version != 1 {
        return Err(CatalogError::UnsupportedVersion);
    }
    if input.tools.len() > 500 {
        return Err(CatalogError::TooLarge);
    }
    let mut ids = HashSet::new();
    let mut app_ids = HashSet::new();
    let mut registry_names = HashSet::new();
    let mut executables = HashSet::new();
    for tool in &input.tools {
        validate_tool(tool)?;
        if !ids.insert(tool.id.to_ascii_lowercase())
            || !app_ids.insert(tool.app_id.to_ascii_lowercase())
            || tool
                .registry_names
                .iter()
                .any(|name| !registry_names.insert(name.to_ascii_lowercase()))
            || tool
                .executables
                .iter()
                .any(|name| !executables.insert(name.to_ascii_lowercase()))
        {
            return Err(CatalogError::ConflictingIdentity);
        }
    }
    Ok(Catalog { tools: input.tools })
}

pub(crate) fn validate_tool(tool: &ToolManifest) -> Result<(), CatalogError> {
    let repo: Vec<_> = tool.release_repository.split('/').collect();
    let valid = is_identifier(&tool.id)
        && is_identifier(&tool.app_id)
        && is_identifier(&tool.trust_policy_id)
        && is_label(&tool.product_name)
        && repo.len() == 2
        && repo.iter().all(|part| is_identifier(part))
        && valid_list(&tool.registry_names, is_label)
        && valid_list(&tool.executables, is_executable_name)
        && !tool.capabilities.is_empty()
        && tool.capabilities.len() <= 5
        && tool.capabilities.iter().collect::<HashSet<_>>().len() == tool.capabilities.len();
    if valid {
        Ok(())
    } else {
        Err(CatalogError::InvalidTool)
    }
}

pub(crate) fn is_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_'))
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
}

fn is_label(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.trim() == value
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, '\\' | '/' | ':' | '"'))
        && !matches!(value, "." | "..")
}

fn valid_list(values: &[String], predicate: fn(&str) -> bool) -> bool {
    !values.is_empty()
        && values.len() <= 16
        && values.iter().all(|value| predicate(value))
        && values
            .iter()
            .map(|value| value.to_ascii_lowercase())
            .collect::<HashSet<_>>()
            .len()
            == values.len()
}

fn is_executable_name(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let Some(stem) = lower.strip_suffix(".exe") else {
        return false;
    };
    let device = stem.split('.').next().unwrap_or_default();
    let reserved = matches!(device, "con" | "prn" | "aux" | "nul")
        || (device.len() == 4
            && (device.starts_with("com") || device.starts_with("lpt"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'));
    is_identifier(stem) && !reserved
}
