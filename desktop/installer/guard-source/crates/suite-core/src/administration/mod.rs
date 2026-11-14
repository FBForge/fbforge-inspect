//! Native machine policy snapshots. These never grant consent, trust or execution.
use crate::{
    network::NetworkConfig,
    updates::{AdministrativeUpdatePolicy, UpdateOverrides},
};
use std::fmt;
#[cfg(test)]
mod tests;
#[cfg(all(windows, target_arch = "x86_64"))]
mod windows;

#[derive(Debug, Clone)]
pub enum AdministrativePolicyState {
    Unmanaged,
    Managed(AdministrativePolicy),
}
#[derive(Debug, Clone)]
pub struct AdministrativePolicy {
    updates_allowed: Option<bool>,
    automatic: UpdateOverrides,
    network: Option<NetworkConfig>,
}
impl AdministrativePolicyState {
    pub fn updates_allowed_override(&self) -> Option<bool> {
        match self {
            Self::Unmanaged => None,
            Self::Managed(policy) => policy.updates_allowed,
        }
    }
    pub fn update_policy(&self) -> AdministrativeUpdatePolicy {
        AdministrativeUpdatePolicy {
            updates_allowed: self.updates_allowed_override().unwrap_or(true),
            automatic: match self {
                Self::Unmanaged => UpdateOverrides::default(),
                Self::Managed(policy) => policy.automatic,
            },
        }
    }
    pub fn network_override(&self) -> Option<&NetworkConfig> {
        match self {
            Self::Unmanaged => None,
            Self::Managed(policy) => policy.network.as_ref(),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdministrativePolicyError {
    InvalidPolicy,
    Unavailable,
    UnsupportedPlatform,
}
impl fmt::Display for AdministrativePolicyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPolicy => "Die administrativen Vorgaben der FBForge Suite sind ungültig oder werden nicht unterstützt.",
            Self::Unavailable => "Die administrativen Vorgaben der FBForge Suite konnten nicht sicher gelesen werden.",
            Self::UnsupportedPlatform => "Das Lesen administrativer Vorgaben benötigt Windows x64.",
        })
    }
}
impl std::error::Error for AdministrativePolicyError {}
/// Read the fixed local HKLM 64-bit policy key. Never substitute user settings
/// after an error. Re-read before policy-sensitive operations; this is no lease.
pub fn read_administrative_policy() -> Result<AdministrativePolicyState, AdministrativePolicyError>
{
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        windows::read()
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        Err(AdministrativePolicyError::UnsupportedPlatform)
    }
}

#[cfg(any(all(windows, target_arch = "x86_64"), test))]
const MAX_VALUES: usize = 7;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
const MAX_DATA_BYTES: usize = 642;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
struct RegistryValue {
    name: String,
    kind: u32,
    data: Vec<u8>,
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
fn decode_values(
    values: Option<&[RegistryValue]>,
) -> Result<AdministrativePolicyState, AdministrativePolicyError> {
    use crate::network::ProxyMode;
    use std::collections::BTreeSet;
    let Some(values) = values else {
        return Ok(AdministrativePolicyState::Unmanaged);
    };
    if values.len() > MAX_VALUES {
        return Err(AdministrativePolicyError::InvalidPolicy);
    }
    let mut seen = BTreeSet::new();
    let mut version = None;
    let mut policy = AdministrativePolicy {
        updates_allowed: None,
        automatic: UpdateOverrides::default(),
        network: None,
    };
    let mut mode = None;
    let mut server = None;
    for value in values {
        if value.name.is_empty()
            || !value.name.is_ascii()
            || value.name.len() >= MAX_NAME_UNITS
            || value.data.len() > MAX_DATA_BYTES
            || !seen.insert(value.name.to_ascii_lowercase())
        {
            return Err(AdministrativePolicyError::InvalidPolicy);
        }
        match value.name.to_ascii_lowercase().as_str() {
            "schemaversion" => version = Some(read_dword(value)?),
            "updateallowed" => policy.updates_allowed = Some(read_bool(value)?),
            "automaticsearch" => policy.automatic.search = Some(read_bool(value)?),
            "automaticdownload" => policy.automatic.download = Some(read_bool(value)?),
            "automaticinstall" => policy.automatic.install = Some(read_bool(value)?),
            "proxymode" => mode = Some(read_string(value)?),
            "proxyserver" => server = Some(read_string(value)?),
            _ => return Err(AdministrativePolicyError::InvalidPolicy),
        }
    }
    if version != Some(1) {
        return Err(AdministrativePolicyError::InvalidPolicy);
    }
    policy.network = match (mode.as_deref(), server) {
        (None, None) => None,
        (Some("system"), None) => Some(NetworkConfig::default()),
        (Some("explicit"), Some(server)) => Some(
            NetworkConfig::new(ProxyMode::Explicit { server })
                .map_err(|_| AdministrativePolicyError::InvalidPolicy)?,
        ),
        _ => return Err(AdministrativePolicyError::InvalidPolicy),
    };
    Ok(AdministrativePolicyState::Managed(policy))
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
const MAX_NAME_UNITS: usize = 32;
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
fn read_dword(value: &RegistryValue) -> Result<u32, AdministrativePolicyError> {
    if value.kind != 4 {
        return Err(AdministrativePolicyError::InvalidPolicy);
    }
    let bytes: [u8; 4] = value
        .data
        .as_slice()
        .try_into()
        .map_err(|_| AdministrativePolicyError::InvalidPolicy)?;
    Ok(u32::from_le_bytes(bytes))
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
fn read_bool(value: &RegistryValue) -> Result<bool, AdministrativePolicyError> {
    match read_dword(value)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(AdministrativePolicyError::InvalidPolicy),
    }
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
fn read_string(value: &RegistryValue) -> Result<String, AdministrativePolicyError> {
    if value.kind != 1 || value.data.len() < 2 || !value.data.len().is_multiple_of(2) {
        return Err(AdministrativePolicyError::InvalidPolicy);
    }
    let units: Vec<u16> = value
        .data
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    if units.last() != Some(&0) || units[..units.len() - 1].contains(&0) {
        return Err(AdministrativePolicyError::InvalidPolicy);
    }
    String::from_utf16(&units[..units.len() - 1])
        .map_err(|_| AdministrativePolicyError::InvalidPolicy)
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KeyStamp {
    subkeys: u32,
    values: u32,
    max_name: u32,
    max_data: u32,
    last_write: u64,
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
impl KeyStamp {
    fn validate(self) -> Result<(), AdministrativePolicyError> {
        if self.subkeys != 0
            || self.values > MAX_VALUES as u32
            || self.max_name >= MAX_NAME_UNITS as u32
            || self.max_data > MAX_DATA_BYTES as u32
        {
            Err(AdministrativePolicyError::InvalidPolicy)
        } else {
            Ok(())
        }
    }
}
#[cfg(any(all(windows, target_arch = "x86_64"), test))]
fn decode_snapshot(
    before: KeyStamp,
    values: &[RegistryValue],
    after: KeyStamp,
) -> Result<AdministrativePolicyState, AdministrativePolicyError> {
    before.validate()?;
    after.validate()?;
    if before != after || before.values as usize != values.len() {
        return Err(AdministrativePolicyError::Unavailable);
    }
    decode_values(Some(values))
}
