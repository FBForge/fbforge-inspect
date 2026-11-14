use semver::Version;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Native observations are not executable launch authorizations. Recheck the
/// path, identity and applicable trust policy immediately before native launch.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Installation {
    pub root: PathBuf,
    pub executable: Option<PathBuf>,
    pub reported_version: Option<String>,
    pub version: Option<Version>,
    pub condition: InstallationCondition,
    pub sources: Vec<RegistrySource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallationCondition {
    Detected,
    MissingExecutable,
    MissingDirectory,
    UnsupportedBinary,
    AmbiguousExecutables,
    ConflictingMetadata,
    UnsupportedVersion,
    UnsafeLocation,
    Inaccessible,
    Unconfirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrySource {
    pub hive: RegistryHive,
    pub view: RegistryView,
    pub key_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RegistryHive {
    CurrentUser,
    LocalMachine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RegistryView {
    Registry32,
    Registry64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DetectionIssue {
    NeedsRescan,
    InvalidManifest,
    RegistryUnavailable,
    InvalidRegistryData,
    UnsupportedPlatform,
}

#[derive(Debug)]
pub struct DetectionReport {
    pub installations: Vec<Installation>,
    pub issues: Vec<DetectionIssue>,
}

impl DetectionReport {
    pub fn record_installation(&mut self, mut installation: Installation) {
        // Native discovery supplies canonical paths. Keep distinct paths rather
        // than assuming every Windows directory is case-insensitive.
        if let Some(existing) = self
            .installations
            .iter_mut()
            .find(|entry| entry.root == installation.root)
        {
            if existing.version != installation.version
                || existing.reported_version != installation.reported_version
                || existing.executable != installation.executable
                || existing.condition != installation.condition
            {
                existing.version = None;
                existing.reported_version = None;
                existing.executable = None;
                existing.condition = InstallationCondition::ConflictingMetadata;
            }
            for source in installation.sources.drain(..) {
                if !existing.sources.contains(&source) {
                    existing.sources.push(source);
                }
            }
        } else {
            self.installations.push(installation);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum DetectionState {
    NotScanned,
    Complete {
        installations: Vec<Installation>,
    },
    Incomplete {
        installations: Vec<Installation>,
        issues: Vec<DetectionIssue>,
    },
}

impl DetectionState {
    pub fn installations(&self) -> &[Installation] {
        match self {
            Self::NotScanned => &[],
            Self::Complete { installations } | Self::Incomplete { installations, .. } => {
                installations
            }
        }
    }
}
