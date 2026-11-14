//! The installer host is selected by the OS, never by an NSIS path argument.
#[cfg(any(windows, test))]
use super::{parse_payload_manifest, PayloadManifest};
use super::{InstallerTargetPlan, TargetError};
use crate::catalog::{parse_catalog, ToolManifest};
use fbforge_suite_coordination::Lease;
use semver::Version;
use std::{fmt, path::Path};

pub const PAYLOAD_MANIFEST_LEAF: &str = "fbforge-payload.v1.json";

/// Same-file publisher/identity assertion for this process's native installer.
/// It is not a lease, an execution grant, or evidence about previously loaded
/// instructions. The existing threat model trusts code already running as the
/// current SID. No caller-controlled host path or public proof constructor exists.
pub struct VerifiedInstallerHost {
    tool: ToolManifest,
    version: Version,
    manifest_sha256: [u8; 32],
    host_sha256: [u8; 32],
    #[cfg(windows)]
    _held: crate::authenticode::installer_targets_bridge::HeldInstallerHost,
}
impl fmt::Debug for VerifiedInstallerHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VerifiedInstallerHost")
            .field("tool_id", &self.tool.id)
            .field("version", &self.version)
            .finish_non_exhaustive()
    }
}
impl VerifiedInstallerHost {
    pub fn verify_current(tool_id: &str) -> Result<Self, TargetError> {
        let tool = closed_tool(tool_id)?;
        #[cfg(windows)]
        {
            let (held, version, manifest_sha256, host_sha256) =
                crate::authenticode::installer_targets_bridge::current_installer(&tool)?;
            Ok(Self {
                tool,
                version,
                manifest_sha256,
                host_sha256,
                _held: held,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = tool;
            Err(TargetError::UnsupportedPlatform)
        }
    }
    pub fn sha256(&self) -> &[u8; 32] {
        &self.host_sha256
    }
    pub fn app_id(&self) -> &str {
        &self.tool.app_id
    }
    pub fn tool_id(&self) -> &str {
        &self.tool.id
    }
    pub fn version(&self) -> &Version {
        &self.version
    }
    pub fn payload_manifest_sha256(&self) -> &[u8; 32] {
        &self.manifest_sha256
    }

    /// Read only the fixed private PLUGINSDIR leaf and bind its exact bytes to
    /// the digest inside the held host's signed VERSIONINFO section.
    pub fn prepare_target<'lease>(
        &self,
        lease: &'lease Lease,
        plugin_directory: &Path,
        enabled_components: &[String],
    ) -> Result<InstallerTargetPlan<'lease>, TargetError> {
        #[cfg(windows)]
        {
            let text = plugin_directory.to_str().ok_or(TargetError::UnsafeTarget)?;
            if text.encode_utf16().count() > 960
                || crate::discovery::validate_windows_location(text).as_deref() != Ok(text)
            {
                return Err(TargetError::UnsafeTarget);
            }
            use std::io::Read;
            let file = crate::authenticode::installer_targets_bridge::private_file(
                &plugin_directory.join(PAYLOAD_MANIFEST_LEAF),
            )?;
            let mut bytes = Vec::new();
            file.take(super::manifest::MAX_MANIFEST_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|_| TargetError::InspectionUnavailable)?;
            if bytes.len() > super::manifest::MAX_MANIFEST_BYTES {
                return Err(TargetError::InvalidManifest);
            }
            let payload = bound_payload(
                &self.tool,
                &self.version,
                &self.manifest_sha256,
                &bytes,
                enabled_components,
            )?;
            InstallerTargetPlan::prepare(lease, &self.tool, payload)
        }
        #[cfg(not(windows))]
        {
            let _ = (lease, plugin_directory, enabled_components);
            Err(TargetError::UnsupportedPlatform)
        }
    }
}

fn closed_tool(tool_id: &str) -> Result<ToolManifest, TargetError> {
    let catalog = parse_catalog(include_str!("../../../../catalog/tools.v1.json"))
        .map_err(|_| TargetError::InspectionUnavailable)?;
    catalog
        .tools()
        .iter()
        .find(|tool| tool.id == tool_id)
        .cloned()
        .ok_or(TargetError::UnsupportedTarget)
}
#[cfg(any(windows, test))]
fn bound_payload(
    tool: &ToolManifest,
    version: &Version,
    expected: &[u8; 32],
    bytes: &[u8],
    components: &[String],
) -> Result<PayloadManifest, TargetError> {
    let payload = parse_payload_manifest(bytes, components)?;
    if payload.source_sha256() != expected
        || payload.tool_id() != tool.id
        || payload.app_id() != tool.app_id
        || payload.version() != version
    {
        return Err(TargetError::PayloadMismatch);
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn host_factory_is_closed_and_does_not_authorize_suite_or_arbitrary_apps() {
        assert_eq!(
            closed_tool("fbforge-connect").unwrap().app_id,
            "org.fitconnect.integration-client"
        );
        assert_eq!(
            closed_tool("fbforge-inspect").unwrap().app_id,
            "de.fbforge.inspect"
        );
        for id in [
            "fbforge-suite",
            "FBFORGE-CONNECT",
            "de.fbforge.inspect",
            "../fbforge-connect",
            "",
        ] {
            assert!(closed_tool(id).is_err());
        }
    }
    #[test]
    fn unsigned_manifest_binding_checks_exact_bytes_and_host_identity_independently() {
        // Structural/hash facts only. No Windows host/publisher proof is faked.
        let tool = closed_tool("fbforge-inspect").unwrap();
        let version = Version::new(0, 3, 0);
        let value = json!({"schemaVersion":1,"toolId":tool.id,"appId":tool.app_id,"version":"0.3.0","files":[{"path":"fbforge-inspect.exe","size":0,"sha256":"00".repeat(32)}]});
        let raw = serde_json::to_vec(&value).unwrap();
        let hash = super::super::manifest::sha256(&raw);
        assert!(bound_payload(&tool, &version, &hash, &raw, &[]).is_ok());
        assert_eq!(
            bound_payload(&tool, &version, &[0; 32], &raw, &[]).unwrap_err(),
            TargetError::PayloadMismatch
        );
        let mut changed = raw.clone();
        changed.push(b' ');
        assert_eq!(
            bound_payload(&tool, &version, &hash, &changed, &[]).unwrap_err(),
            TargetError::PayloadMismatch
        );
        for (field, value) in [
            ("toolId", "fbforge-connect"),
            ("appId", "de.other"),
            ("version", "0.4.0"),
        ] {
            let mut changed: serde_json::Value = serde_json::from_slice(&raw).unwrap();
            changed[field] = value.into();
            let changed = serde_json::to_vec(&changed).unwrap();
            let digest = super::super::manifest::sha256(&changed);
            assert_eq!(
                bound_payload(&tool, &version, &digest, &changed, &[]).unwrap_err(),
                TargetError::PayloadMismatch
            );
        }
    }
    #[cfg(not(windows))]
    #[test]
    fn a_non_windows_host_is_never_a_publisher_success() {
        assert!(matches!(
            VerifiedInstallerHost::verify_current("fbforge-inspect"),
            Err(TargetError::UnsupportedPlatform)
        ));
    }
}
