use super::TargetError;
use crate::{catalog::is_identifier, discovery::validate_windows_location};
use ring::digest;
use semver::Version;
use serde::Deserialize;
use std::collections::BTreeSet;

pub(super) const MAX_MANIFEST_BYTES: usize = 4 * 1024 * 1024;
pub(super) const MAX_FILES: usize = 8192;
pub(super) const MAX_TOTAL_BYTES: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    schema_version: u32,
    tool_id: String,
    app_id: String,
    version: String,
    files: Vec<FileInput>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileInput {
    path: String,
    size: u64,
    sha256: String,
    #[serde(default)]
    component: Option<String>,
}

/// Structurally checked payload, not proof of NSIS/signature provenance.
#[derive(Debug)]
pub struct PayloadManifest {
    pub(super) tool_id: String,
    pub(super) app_id: String,
    pub(super) version: Version,
    pub(super) files: Vec<PayloadFile>,
    pub(super) total_bytes: u64,
    pub(super) source_sha256: [u8; 32],
    pub(super) selection_sha256: [u8; 32],
}
#[derive(Debug)]
pub struct PayloadFile {
    pub(super) path: String,
    pub(super) size: u64,
    pub(super) sha256: [u8; 32],
}
impl PayloadManifest {
    pub fn tool_id(&self) -> &str {
        &self.tool_id
    }
    pub fn app_id(&self) -> &str {
        &self.app_id
    }
    pub fn version(&self) -> &Version {
        &self.version
    }
    pub fn files(&self) -> &[PayloadFile] {
        &self.files
    }
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
    pub fn source_sha256(&self) -> &[u8; 32] {
        &self.source_sha256
    }
    pub fn selection_sha256(&self) -> &[u8; 32] {
        &self.selection_sha256
    }
}
impl PayloadFile {
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }
}

pub fn parse_payload_manifest(
    bytes: &[u8],
    enabled_components: &[String],
) -> Result<PayloadManifest, TargetError> {
    if bytes.len() > MAX_MANIFEST_BYTES || enabled_components.len() > 8 {
        return Err(TargetError::InvalidManifest);
    }
    let input: Input = serde_json::from_slice(bytes).map_err(|_| TargetError::InvalidManifest)?;
    if input.schema_version != 1
        || !is_identifier(&input.tool_id)
        || !is_identifier(&input.app_id)
        || input.files.is_empty()
        || input.files.len() > MAX_FILES
    {
        return Err(TargetError::InvalidManifest);
    }
    let version = canonical_version(&input.version)?;
    let mut declared = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut bytes_total = 0u64;
    let mut path_units = 0usize;
    for file in &input.files {
        validate_relative(&file.path)?;
        if !paths.insert(file.path.to_ascii_lowercase()) || decode_hash(&file.sha256).is_err() {
            return Err(TargetError::InvalidManifest);
        }
        path_units = path_units
            .checked_add(file.path.encode_utf16().count())
            .ok_or(TargetError::InvalidManifest)?;
        bytes_total = bytes_total
            .checked_add(file.size)
            .ok_or(TargetError::InvalidManifest)?;
        if bytes_total > MAX_TOTAL_BYTES || path_units > 4 * 1024 * 1024 {
            return Err(TargetError::InvalidManifest);
        }
        if let Some(component) = &file.component {
            if !is_identifier(component) || component.len() > 64 {
                return Err(TargetError::InvalidManifest);
            }
            declared.insert(component.clone());
        }
    }
    if declared.len() > 8 {
        return Err(TargetError::InvalidManifest);
    }
    for path in &paths {
        let mut prefix = String::new();
        let parts: Vec<_> = path.split('\\').collect();
        for part in &parts[..parts.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('\\');
            }
            prefix.push_str(part);
            if paths.contains(&prefix) {
                return Err(TargetError::InvalidManifest);
            }
        }
    }
    let mut enabled = BTreeSet::new();
    for component in enabled_components {
        if !declared.contains(component) || !enabled.insert(component.clone()) {
            return Err(TargetError::InvalidManifest);
        }
    }
    let source_sha256 = sha256(bytes);
    let mut selection = digest::Context::new(&digest::SHA256);
    selection.update(b"FBForge Suite active installer payload v1\0");
    selection.update(&source_sha256);
    for component in &enabled {
        selection.update(component.as_bytes());
        selection.update(&[0]);
    }
    let selection_sha256 = selection.finish().as_ref().try_into().expect("SHA256 size");
    let mut files = Vec::new();
    let mut total_bytes = 0;
    for file in input.files {
        if file
            .component
            .as_ref()
            .is_some_and(|component| !enabled.contains(component))
        {
            continue;
        }
        total_bytes += file.size;
        files.push(PayloadFile {
            path: file.path,
            size: file.size,
            sha256: decode_hash(&file.sha256)?,
        });
    }
    if files.is_empty() {
        return Err(TargetError::InvalidManifest);
    }
    Ok(PayloadManifest {
        tool_id: input.tool_id,
        app_id: input.app_id,
        version,
        files,
        total_bytes,
        source_sha256,
        selection_sha256,
    })
}

pub(super) fn canonical_version(value: &str) -> Result<Version, TargetError> {
    if value.len() > 128 {
        return Err(TargetError::InvalidManifest);
    }
    let version = Version::parse(value).map_err(|_| TargetError::InvalidManifest)?;
    if version.to_string() != value {
        return Err(TargetError::InvalidManifest);
    }
    Ok(version)
}
pub(super) fn sha256(bytes: &[u8]) -> [u8; 32] {
    digest::digest(&digest::SHA256, bytes)
        .as_ref()
        .try_into()
        .expect("SHA256 size")
}
pub(super) fn validate_relative(path: &str) -> Result<(), TargetError> {
    if path.contains('/')
        || path.encode_utf16().count() > 512
        || path.split('\\').count() > 32
        || matches!(
            path.split('\\')
                .next()
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str(),
            "uninstall.exe" | ".fbforge-install.pending" | ".fbforge-installation.v1.json"
        )
        || validate_windows_location(&format!("C:\\payload\\{path}")).is_err()
    {
        return Err(TargetError::InvalidManifest);
    }
    Ok(())
}
fn decode_hash(value: &str) -> Result<[u8; 32], TargetError> {
    if value.len() != 64 {
        return Err(TargetError::InvalidManifest);
    }
    let mut hash = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let nibble = |byte| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(TargetError::InvalidManifest),
        };
        hash[index] = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    Ok(hash)
}
