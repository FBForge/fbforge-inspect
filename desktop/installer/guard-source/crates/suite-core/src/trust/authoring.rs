//! Local unsigned preparation and production-bound assembly. Prepared bytes
//! carry no trust, and are never a VerifiedCatalog/VerifiedRelease substitute.
use super::{
    metadata::{self, Payload},
    verify_catalog, verify_release, TrustError, TrustStore,
};
use crate::{
    catalog::{parse_catalog, Catalog},
    production::{bundled_metadata_trust_store, METADATA_KEY_ID},
};
use semver::Version;
use serde::Serialize;
use serde_json::value::RawValue;
use std::time::{SystemTime, UNIX_EPOCH};

/// Explicit publisher input. No sequence, issuance or expiry defaults exist.
#[derive(Debug, Clone, Copy)]
pub struct MetadataHeader {
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: u64,
}

/// Untrusted, byte-exact signing input. This type grants no metadata authority.
#[derive(Debug)]
pub struct PreparedMessage {
    bytes: Vec<u8>,
}
impl PreparedMessage {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Decode an untrusted prepared file without normalizing its signed bytes.
    /// Contextual body and production trust checks still happen at assembly.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TrustError> {
        let signed = bytes
            .strip_prefix(metadata::DOMAIN_V2)
            .ok_or(TrustError::InvalidEnvelope)?;
        if signed.len() > metadata::MAX_SIGNED_BYTES {
            return Err(TrustError::TooLarge);
        }
        let payload: Payload =
            serde_json::from_slice(signed).map_err(|_| TrustError::InvalidEnvelope)?;
        if payload.schema_version != 2 || !super::valid_subject(&payload.subject) {
            return Err(TrustError::InvalidEnvelope);
        }
        metadata::validate_validity(payload.sequence, payload.issued_at, payload.expires_at)?;
        Ok(Self {
            bytes: bytes.to_vec(),
        })
    }
}

pub fn prepare_catalog(body: &str, header: MetadataHeader) -> Result<PreparedMessage, TrustError> {
    parse_catalog(body).map_err(|_| TrustError::InvalidCatalog)?;
    prepare(body, "catalog".into(), header)
}

/// `catalog` is only a parsed structural contract, not authenticated authority.
pub fn prepare_release(
    body: &str,
    catalog: &Catalog,
    tool_id: &str,
    version: &Version,
    header: MetadataHeader,
) -> Result<PreparedMessage, TrustError> {
    if body.len() > metadata::MAX_SIGNED_BYTES {
        return Err(TrustError::TooLarge);
    }
    let tool = catalog
        .tools()
        .iter()
        .find(|tool| tool.id == tool_id)
        .ok_or(TrustError::InvalidRelease)?;
    super::release::validate_unsigned_release(body, tool, version)?;
    prepare(body, format!("release/{tool_id}"), header)
}

fn prepare(
    body: &str,
    subject: String,
    header: MetadataHeader,
) -> Result<PreparedMessage, TrustError> {
    metadata::validate_validity(header.sequence, header.issued_at, header.expires_at)?;
    let body = RawValue::from_string(body.to_owned()).map_err(|_| TrustError::InvalidEnvelope)?;
    let signed = serde_json::to_vec(&Payload {
        schema_version: 2,
        subject,
        sequence: header.sequence,
        issued_at: header.issued_at,
        expires_at: header.expires_at,
        body,
    })
    .map_err(|_| TrustError::InvalidEnvelope)?;
    if signed.len() > metadata::MAX_SIGNED_BYTES {
        return Err(TrustError::TooLarge);
    }
    Ok(PreparedMessage {
        bytes: [metadata::DOMAIN_V2, &signed].concat(),
    })
}

/// Assembly accepts only the locally provisioned production key and native time.
/// A valid output is metadata evidence, never an execution authorization.
pub fn assemble_catalog(message: &PreparedMessage, signature: &str) -> Result<String, TrustError> {
    assemble_at(
        message,
        signature,
        AssemblyContext::Catalog,
        &bundled_metadata_trust_store()?,
        native_time()?,
    )
}

pub fn assemble_release(
    message: &PreparedMessage,
    signature: &str,
    catalog: &str,
    tool_id: &str,
    version: &Version,
) -> Result<String, TrustError> {
    assemble_at(
        message,
        signature,
        AssemblyContext::Release {
            catalog,
            tool_id,
            version,
        },
        &bundled_metadata_trust_store()?,
        native_time()?,
    )
}

fn native_time() -> Result<u64, TrustError> {
    // No public clock argument or environment override. A clock before the Unix
    // epoch is invalid and must fail closed rather than invent a verification time.
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(|_| TrustError::ClockRollback)
}

enum AssemblyContext<'a> {
    Catalog,
    Release {
        catalog: &'a str,
        tool_id: &'a str,
        version: &'a Version,
    },
}

fn assemble_at(
    message: &PreparedMessage,
    signature: &str,
    context: AssemblyContext<'_>,
    store: &TrustStore,
    now: u64,
) -> Result<String, TrustError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Envelope<'a> {
        schema_version: u32,
        key_id: &'a str,
        signed: &'a str,
        signature: &'a str,
    }
    // PreparedMessage never exposes mutable bytes or admits another domain.
    let signed = std::str::from_utf8(&message.as_bytes()[metadata::DOMAIN_V2.len()..])
        .map_err(|_| TrustError::InvalidEnvelope)?;
    if signature.len() > 16_384 {
        return Err(TrustError::TooLarge);
    }
    let envelope = serde_json::to_string(&Envelope {
        schema_version: 2,
        key_id: METADATA_KEY_ID,
        signed,
        signature,
    })
    .map_err(|_| TrustError::InvalidEnvelope)?;
    match context {
        AssemblyContext::Catalog => {
            verify_catalog(&envelope, store, now, None)?;
        }
        AssemblyContext::Release {
            catalog,
            tool_id,
            version,
        } => {
            let catalog = verify_catalog(catalog, store, now, None)?;
            verify_release(&envelope, store, now, None, &catalog, tool_id, version)?;
        }
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests;
