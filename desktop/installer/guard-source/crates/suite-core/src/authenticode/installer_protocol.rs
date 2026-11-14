//! Narrow VERSIONINFO assertion reader; parsing alone is not publisher proof.
use super::{InvalidReason, VerificationFailure};
use std::collections::BTreeSet;

fn rejected() -> VerificationFailure {
    VerificationFailure::Invalid(InvalidReason::CooperationRejected)
}

struct Block<'a> {
    key: String,
    kind: u16,
    value: &'a [u8],
    children: &'a [u8],
    length: usize,
}

fn word(bytes: &[u8], offset: usize) -> Result<u16, VerificationFailure> {
    let value = bytes.get(offset..offset + 2).ok_or_else(rejected)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn align(value: usize) -> Result<usize, VerificationFailure> {
    value.checked_add(3).map(|n| n & !3).ok_or_else(rejected)
}

fn zeros(bytes: &[u8]) -> Result<(), VerificationFailure> {
    if bytes.iter().any(|n| *n != 0) {
        Err(rejected())
    } else {
        Ok(())
    }
}

fn terminated(bytes: &[u8]) -> Result<String, VerificationFailure> {
    if !bytes.len().is_multiple_of(2) || bytes.len() < 2 {
        return Err(rejected());
    }
    let units: Vec<_> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    if units.last() != Some(&0) || units[..units.len() - 1].contains(&0) {
        return Err(rejected());
    }
    String::from_utf16(&units[..units.len() - 1]).map_err(|_| rejected())
}

fn block(bytes: &[u8]) -> Result<Block<'_>, VerificationFailure> {
    let length = word(bytes, 0)? as usize;
    let value_length = word(bytes, 2)? as usize;
    let kind = word(bytes, 4)?;
    if length < 8 || length > bytes.len() || kind > 1 {
        return Err(rejected());
    }
    let bytes = &bytes[..length];
    let mut key_end = 6;
    loop {
        if key_end > 6 + 2 * 128 {
            return Err(rejected());
        }
        let unit = word(bytes, key_end)?;
        key_end += 2;
        if unit == 0 {
            break;
        }
    }
    let key = terminated(&bytes[6..key_end])?;
    if key.is_empty() {
        return Err(rejected());
    }
    let start = align(key_end)?;
    if start > length {
        return Err(rejected());
    }
    zeros(&bytes[key_end..start])?;
    let size = value_length
        .checked_mul(if kind == 1 { 2 } else { 1 })
        .ok_or_else(rejected)?;
    let end = start.checked_add(size).ok_or_else(rejected)?;
    if end > length {
        return Err(rejected());
    }
    let children = if end == length {
        &bytes[length..]
    } else {
        let next = align(end)?;
        if next > length {
            return Err(rejected());
        }
        zeros(&bytes[end..next])?;
        &bytes[next..]
    };
    Ok(Block {
        key,
        kind,
        value: &bytes[start..end],
        children,
        length,
    })
}

fn children<'a>(parent: &Block<'a>) -> Result<Vec<Block<'a>>, VerificationFailure> {
    let mut result = Vec::new();
    let mut keys = BTreeSet::new();
    let mut bytes = parent.children;
    while !bytes.is_empty() {
        if result.len() >= 128 {
            return Err(rejected());
        }
        let child = block(bytes)?;
        if !keys.insert(child.key.to_ascii_lowercase()) {
            return Err(rejected());
        }
        let consumed = child.length;
        result.push(child);
        if consumed == bytes.len() {
            break;
        }
        let next = align(consumed)?;
        if next > bytes.len() {
            return Err(rejected());
        }
        zeros(&bytes[consumed..next])?;
        bytes = &bytes[next..];
    }
    Ok(result)
}

fn container(node: &Block<'_>, key: &str) -> Result<(), VerificationFailure> {
    // NSIS3.13 uses binary(0), SDK resource compilers also use text(1) for
    // empty containers. With value length0 neither form changes value units.
    if node.key != key || !node.value.is_empty() {
        Err(rejected())
    } else {
        Ok(())
    }
}

struct Fields {
    app_id: Option<String>,
    protocol: Option<String>,
    product_version: String,
    payload_manifest_sha256: Option<String>,
}

fn fields(bytes: &[u8], required_language: Option<u16>) -> Result<Fields, VerificationFailure> {
    // wLength is a WORD. Installers require en-US/Unicode; the separate program
    // data reader also accepts Tauri's neutral/Unicode table. Both have one
    // table and unambiguous values, independent of the UI language.
    if bytes.len() > u16::MAX as usize {
        return Err(rejected());
    }
    // NSIS3.13 leaves dwStrucVersion zero; SDK RC uses0x10000. Both
    // still require the fixed signature and exact protocol/version strings.
    let root = block(bytes)?;
    if root.length != bytes.len()
        || root.key != "VS_VERSION_INFO"
        || root.kind != 0
        || root.value.len() != 52
        || root.value[..4] != 0xfeef04bdu32.to_le_bytes()
        || (root.value[4..8] != 0u32.to_le_bytes() && root.value[4..8] != 0x10000u32.to_le_bytes())
    {
        return Err(rejected());
    }
    let mut app_id = None;
    let mut protocol = None;
    let mut product_version = None;
    let mut payload_manifest_sha256 = None;
    let mut table_language = None;
    let mut translation_language = None;
    for node in children(&root)? {
        match node.key.as_str() {
            "StringFileInfo" => {
                container(&node, "StringFileInfo")?;
                let tables = children(&node)?;
                if tables.len() != 1 {
                    return Err(rejected());
                }
                let table = &tables[0];
                let language = if table.key.eq_ignore_ascii_case("040904b0") {
                    1033
                } else if table.key.eq_ignore_ascii_case("000004b0") {
                    0
                } else {
                    return Err(rejected());
                };
                if !table.value.is_empty() || required_language.is_some_and(|n| n != language) {
                    return Err(rejected());
                }
                table_language = Some(language);
                for field in children(table)? {
                    if field.kind != 1 || !field.children.is_empty() {
                        return Err(rejected());
                    }
                    let value = terminated(field.value)?;
                    match field.key.as_str() {
                        "FBForgeAppId" => app_id = Some(value),
                        "FBForgeInstallerProtocol" => protocol = Some(value),
                        "ProductVersion" => product_version = Some(value),
                        "FBForgePayloadManifestSha256" => payload_manifest_sha256 = Some(value),
                        key if key.eq_ignore_ascii_case("FBForgeAppId")
                            || key.eq_ignore_ascii_case("FBForgeInstallerProtocol")
                            || key.eq_ignore_ascii_case("ProductVersion")
                            || key.eq_ignore_ascii_case("FBForgePayloadManifestSha256") =>
                        {
                            return Err(rejected())
                        }
                        _ => {}
                    }
                }
            }
            "VarFileInfo" => {
                container(&node, "VarFileInfo")?;
                let entries = children(&node)?;
                if entries.len() != 1 {
                    return Err(rejected());
                }
                let translation = &entries[0];
                if translation.key != "Translation"
                    || translation.kind != 0
                    || !translation.children.is_empty()
                    || translation.value.len() != 4
                    || translation.value[2..] != [0xb0, 0x04]
                {
                    return Err(rejected());
                }
                translation_language = Some(u16::from_le_bytes([
                    translation.value[0],
                    translation.value[1],
                ]));
            }
            _ => return Err(rejected()),
        }
    }
    if translation_language.is_some() && translation_language != table_language {
        return Err(rejected());
    }
    let product_version = product_version.ok_or(VerificationFailure::Invalid(
        InvalidReason::CooperationMissing,
    ))?;
    Ok(Fields {
        app_id,
        protocol,
        product_version,
        payload_manifest_sha256,
    })
}

fn canonical_version(value: &str) -> Result<semver::Version, VerificationFailure> {
    let version = semver::Version::parse(value).map_err(|_| rejected())?;
    if version.to_string() != value {
        return Err(rejected());
    }
    Ok(version)
}

pub(super) fn product_version(bytes: &[u8]) -> Result<semver::Version, VerificationFailure> {
    canonical_version(&fields(bytes, None)?.product_version)
}

pub(super) fn identity(
    bytes: &[u8],
) -> Result<super::DeclaredInstallerIdentity, VerificationFailure> {
    let data = fields(bytes, Some(1033))?;
    let app_id = data.app_id.ok_or_else(rejected)?;
    if !crate::catalog::is_identifier(&app_id) || data.protocol.as_deref() != Some("1") {
        return Err(rejected());
    }
    Ok(super::DeclaredInstallerIdentity {
        app_id,
        version: canonical_version(&data.product_version)?,
        payload_manifest_sha256: data
            .payload_manifest_sha256
            .as_deref()
            .map(payload_digest)
            .transpose()?,
    })
}

fn payload_digest(value: &str) -> Result<[u8; 32], VerificationFailure> {
    if value.len() != 64 {
        return Err(rejected());
    }
    let mut result = [0; 32];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let digit = |byte| match byte {
            b'0'..=b'9' => Ok(byte - b'0'),
            b'a'..=b'f' => Ok(byte - b'a' + 10),
            _ => Err(rejected()),
        };
        result[index] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(result)
}

pub(super) fn require_identity(
    data: &super::DeclaredInstallerIdentity,
    app_id: &str,
    version: &str,
) -> Result<(), VerificationFailure> {
    if app_id.is_empty() || app_id.len() > 128 || !app_id.is_ascii() {
        return Err(rejected());
    }
    if data.app_id() != app_id || data.version().to_string() != version || data.protocol() != 1 {
        return Err(rejected());
    }
    Ok(())
}
#[cfg(test)]
fn inspect(bytes: &[u8], app_id: &str, version: &str) -> Result<(), VerificationFailure> {
    require_identity(&identity(bytes)?, app_id, version)
}
#[cfg(test)]
mod tests;
