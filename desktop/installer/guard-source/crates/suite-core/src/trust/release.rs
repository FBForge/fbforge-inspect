use super::{
    encoding::{decode_hex, encode_hex},
    metadata::verify_payload,
    verify_catalog, MetadataCheckpoint, TrustError, TrustStore, VerifiedCatalog,
};
use crate::catalog::{is_identifier, Installer, Platform, ToolManifest};
use ring::digest;
use semver::Version;
use serde::Deserialize;
use std::{collections::HashSet, io::Read};

const MAX_PACKAGE_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Availability {
    Available,
    Withdrawn,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReleaseManifest {
    schema_version: u32,
    tool_id: String,
    app_id: String,
    version: Version,
    platform: Platform,
    installer: Installer,
    repository: String,
    release_tag: String,
    asset_name: String,
    size: u64,
    sha256: String,
    authenticode_policy_id: String,
    status: Availability,
    required_files: Vec<String>,
}

#[derive(Debug)]
pub struct VerifiedRelease {
    manifest: ReleaseManifest,
    checkpoint: MetadataCheckpoint,
    expires_at: u64,
    digest: [u8; 32],
}

impl VerifiedRelease {
    pub fn metadata_schema_version(&self) -> u32 {
        self.checkpoint.metadata_schema_version
    }

    pub fn tool_id(&self) -> &str {
        &self.manifest.tool_id
    }

    pub fn app_id(&self) -> &str {
        &self.manifest.app_id
    }

    pub fn authenticode_policy_id(&self) -> &str {
        &self.manifest.authenticode_policy_id
    }

    pub fn checkpoint(&self) -> &MetadataCheckpoint {
        &self.checkpoint
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }

    pub fn version(&self) -> &Version {
        &self.manifest.version
    }

    pub fn package_size(&self) -> u64 {
        self.manifest.size
    }

    pub fn package_sha256(&self) -> &[u8; 32] {
        &self.digest
    }

    pub fn required_files(&self) -> &[String] {
        &self.manifest.required_files
    }

    pub fn is_available(&self) -> bool {
        self.manifest.status == Availability::Available
    }

    pub fn download_url(&self) -> String {
        format!(
            "https://github.com/{}/releases/download/{}/{}",
            self.manifest.repository, self.manifest.release_tag, self.manifest.asset_name
        )
    }

    /// Only verifies bytes, not Authenticode, metadata freshness at execution,
    /// file ownership or post-installation health. The executor must recheck all.
    pub fn verify_package(&self, reader: impl Read) -> Result<PackageIntegrity, TrustError> {
        if !self.is_available() {
            return Err(TrustError::WithdrawnRelease);
        }
        let mut reader = reader.take(self.manifest.size + 1);
        let mut context = digest::Context::new(&digest::SHA256);
        let mut count = 0;
        let mut buffer = [0; 65536];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => return Err(TrustError::PackageRead),
            };
            if read == 0 {
                break;
            }
            count += read as u64;
            if count > self.manifest.size {
                return Err(TrustError::PackageMismatch);
            }
            context.update(&buffer[..read]);
        }
        let actual = context.finish();
        if count != self.manifest.size || actual.as_ref() != self.digest {
            return Err(TrustError::PackageMismatch);
        }
        Ok(PackageIntegrity {
            byte_count: count,
            sha256: encode_hex(actual.as_ref()),
        })
    }
}

/// Integrity evidence alone never authorizes execution of a Windows program.
#[derive(Debug)]
pub struct PackageIntegrity {
    byte_count: u64,
    sha256: String,
}

impl PackageIntegrity {
    pub fn byte_count(&self) -> u64 {
        self.byte_count
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

pub fn verify_release(
    envelope: &str,
    store: &TrustStore,
    now: u64,
    checkpoint: Option<&MetadataCheckpoint>,
    catalog: &VerifiedCatalog,
    tool_id: &str,
    expected_version: &Version,
) -> Result<VerifiedRelease, TrustError> {
    let release = verify_release_candidate(envelope, store, now, checkpoint, catalog, tool_id)?;
    if release.version() != expected_version {
        return Err(TrustError::InvalidRelease);
    }
    Ok(release)
}

/// Authenticate the publisher's offered version before exposing it. This does
/// not establish global publication freshness or select an installation version.
/// A subsequent user-selected download still uses verify_release's exact version.
pub fn verify_release_candidate(
    envelope: &str,
    store: &TrustStore,
    now: u64,
    checkpoint: Option<&MetadataCheckpoint>,
    catalog: &VerifiedCatalog,
    tool_id: &str,
) -> Result<VerifiedRelease, TrustError> {
    // An earlier catalogue check cannot outlive expiry or local key revocation.
    let current_catalog = verify_catalog(&catalog.source, store, now, Some(catalog.checkpoint()))?;
    let tool = current_catalog
        .catalog()
        .tools()
        .iter()
        .find(|tool| tool.id == tool_id)
        .ok_or(TrustError::InvalidRelease)?;
    let subject = format!("release/{tool_id}");
    let verified = verify_payload(envelope, store, &subject, now, checkpoint)?;
    let manifest: ReleaseManifest =
        serde_json::from_str(verified.body.get()).map_err(|_| TrustError::InvalidRelease)?;
    validate_release(&manifest, tool, &manifest.version)?;
    let digest = decode_hex::<32>(&manifest.sha256).map_err(|_| TrustError::InvalidRelease)?;
    Ok(VerifiedRelease {
        manifest,
        digest,
        checkpoint: verified.checkpoint,
        expires_at: verified.expires_at.min(current_catalog.expires_at()),
    })
}

/// Structural cache/checkpoint binding only. Current key/time/catalog authority
/// is checked separately by verify_release whenever the cached release is used.
pub(crate) fn validate_cached_release(
    envelope: &str,
    checkpoint: &MetadataCheckpoint,
    tool_id: &str,
) -> Result<(), TrustError> {
    let body = super::metadata::validate_cached_payload(
        envelope,
        checkpoint,
        &format!("release/{tool_id}"),
    )?;
    let manifest: ReleaseManifest =
        serde_json::from_str(body.get()).map_err(|_| TrustError::InvalidRelease)?;
    if manifest.tool_id != tool_id || !has_valid_release_shape(&manifest) {
        return Err(TrustError::InvalidRelease);
    }
    Ok(())
}

fn validate_release(
    release: &ReleaseManifest,
    tool: &ToolManifest,
    version: &Version,
) -> Result<(), TrustError> {
    let valid = has_valid_release_shape(release)
        && release.tool_id == tool.id
        && release.app_id == tool.app_id
        && release.version == *version
        && release.platform == tool.platform
        && release.installer == tool.installer
        && release.repository == tool.release_repository
        && release.authenticode_policy_id == tool.trust_policy_id
        && tool.executables.iter().any(|exe| {
            release
                .required_files
                .iter()
                .any(|path| path.eq_ignore_ascii_case(exe))
        });
    if valid {
        Ok(())
    } else {
        Err(TrustError::InvalidRelease)
    }
}

// Structural authoring help only. This never constructs VerifiedRelease or
// turns an unsigned catalog/body into trusted release evidence.
pub(super) fn validate_unsigned_release(
    body: &str,
    tool: &ToolManifest,
    version: &Version,
) -> Result<(), TrustError> {
    let release: ReleaseManifest =
        serde_json::from_str(body).map_err(|_| TrustError::InvalidRelease)?;
    validate_release(&release, tool, version)
}

fn has_valid_release_shape(release: &ReleaseManifest) -> bool {
    let has_valid_repository = release
        .repository
        .split_once('/')
        .is_some_and(|(owner, name)| is_identifier(owner) && is_identifier(name));
    let tag_version = release
        .release_tag
        .strip_prefix('v')
        .unwrap_or(&release.release_tag);
    release.schema_version == 1
        && is_identifier(&release.tool_id)
        && is_identifier(&release.app_id)
        && is_identifier(&release.authenticode_policy_id)
        && has_valid_repository
        && tag_version == release.version.to_string()
        && release.size > 0
        && release.size <= MAX_PACKAGE_BYTES
        && decode_hex::<32>(&release.sha256).is_ok()
        && release.asset_name.ends_with(".exe")
        && safe_file_component(&release.asset_name)
        && !release.required_files.is_empty()
        && release.required_files.len() <= 8192
        && release
            .required_files
            .iter()
            .all(|path| path.len() <= 1024 && path.split('/').all(safe_file_component))
        && release
            .required_files
            .iter()
            .map(|path| path.to_ascii_lowercase())
            .collect::<HashSet<_>>()
            .len()
            == release.required_files.len()
}

fn safe_file_component(value: &str) -> bool {
    if !is_identifier(value) {
        return false;
    }
    let lower = value.to_ascii_lowercase();
    let device = lower.split('.').next().unwrap_or_default();
    !matches!(device, "con" | "prn" | "aux" | "nul")
        && !(device.len() == 4
            && (device.starts_with("com") || device.starts_with("lpt"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
}
