//! Metadata authenticity is separate from Windows Authenticode and package integrity.
//! Trust anchors are provisioned locally, never deserialized from a download.
pub mod authoring;
pub(crate) mod encoding;
mod metadata;
mod minisign;
#[cfg(test)]
mod production_tests;
pub(crate) use metadata::validate_cached_catalog;
mod release;
pub(crate) use release::validate_cached_release;
pub use release::{verify_release, verify_release_candidate, PackageIntegrity, VerifiedRelease};

use crate::catalog::{is_identifier, parse_catalog, Catalog};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyStatus {
    Active,
    Revoked,
}

#[derive(Clone)]
pub struct TrustAnchor {
    pub id: String,
    pub public_key: [u8; 32],
    pub subjects: Vec<String>,
    pub not_before: u64,
    pub expires_at: u64,
    pub status: KeyStatus,
}

/// Locally provisioned Tauri-encoded Minisign key. This is a separate key type
/// and authority from the raw Ed25519 anchors used by the v1 contract.
#[derive(Clone)]
pub struct MinisignTrustAnchor {
    pub id: String,
    pub public_key: String,
    pub subjects: Vec<String>,
    pub not_before: u64,
    pub expires_at: u64,
    pub status: KeyStatus,
}

#[derive(Clone)]
pub struct TrustStore {
    anchors: BTreeMap<String, TrustAnchor>,
    minisign_anchors: BTreeMap<String, MinisignTrustAnchor>,
}

impl TrustStore {
    pub fn new(anchors: Vec<TrustAnchor>) -> Result<Self, TrustError> {
        if anchors.len() > 128 {
            return Err(TrustError::InvalidTrustStore);
        }
        let mut entries = BTreeMap::new();
        for anchor in anchors {
            if !is_identifier(&anchor.id)
                || anchor.expires_at <= anchor.not_before
                || anchor.subjects.is_empty()
                || anchor.subjects.len() > 501
                || !anchor.subjects.iter().all(|subject| valid_subject(subject))
                || entries.contains_key(&anchor.id)
            {
                return Err(TrustError::InvalidTrustStore);
            }
            entries.insert(anchor.id.clone(), anchor);
        }
        Ok(Self {
            anchors: entries,
            minisign_anchors: BTreeMap::new(),
        })
    }

    pub fn with_minisign_anchors(
        mut self,
        anchors: Vec<MinisignTrustAnchor>,
    ) -> Result<Self, TrustError> {
        if anchors.len() > 128
            || self.anchors.len() + self.minisign_anchors.len() + anchors.len() > 128
        {
            return Err(TrustError::InvalidTrustStore);
        }
        for anchor in anchors {
            if !is_identifier(&anchor.id)
                || anchor.expires_at <= anchor.not_before
                || anchor.subjects.is_empty()
                || anchor.subjects.len() > 501
                || !anchor.subjects.iter().all(|subject| valid_subject(subject))
                || self.anchors.contains_key(&anchor.id)
                || self.minisign_anchors.contains_key(&anchor.id)
            {
                return Err(TrustError::InvalidTrustStore);
            }
            minisign::decode_public_key(&anchor.public_key)
                .map_err(|_| TrustError::InvalidTrustStore)?;
            self.minisign_anchors.insert(anchor.id.clone(), anchor);
        }
        Ok(self)
    }
}

fn valid_subject(value: &str) -> bool {
    value == "catalog" || value.strip_prefix("release/").is_some_and(is_identifier)
}

/// Local rollback state. Persist only after validation, atomically with the cached
/// envelope. It is not a trust anchor and does not bypass signature/time checks.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetadataCheckpoint {
    #[serde(default = "legacy_metadata_version")]
    metadata_schema_version: u32,
    subject: String,
    sequence: u64,
    payload_sha256: String,
    last_verified_at: u64,
}

fn legacy_metadata_version() -> u32 {
    1
}

#[derive(Debug)]
pub struct VerifiedCatalog {
    catalog: Catalog,
    checkpoint: MetadataCheckpoint,
    expires_at: u64,
    source: String,
}

impl VerifiedCatalog {
    /// Authenticated envelope format, for native consumers that require v2.
    pub fn metadata_schema_version(&self) -> u32 {
        self.checkpoint.metadata_schema_version
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn checkpoint(&self) -> &MetadataCheckpoint {
        &self.checkpoint
    }

    pub fn expires_at(&self) -> u64 {
        self.expires_at
    }
}

pub fn verify_catalog(
    envelope: &str,
    store: &TrustStore,
    now: u64,
    checkpoint: Option<&MetadataCheckpoint>,
) -> Result<VerifiedCatalog, TrustError> {
    let verified = metadata::verify_payload(envelope, store, "catalog", now, checkpoint)?;
    let catalog = parse_catalog(verified.body.get()).map_err(|_| TrustError::InvalidCatalog)?;
    Ok(VerifiedCatalog {
        catalog,
        checkpoint: verified.checkpoint,
        expires_at: verified.expires_at,
        source: envelope.to_owned(),
    })
}

#[derive(Debug, PartialEq, Eq)]
pub enum TrustError {
    TooLarge,
    InvalidEnvelope,
    InvalidSignature,
    InvalidTrustStore,
    UnknownKey,
    KeyUnavailable,
    WrongSubject,
    Expired,
    NotYetValid,
    Rollback,
    ClockRollback,
    ConflictingSequence,
    InvalidCatalog,
    InvalidRelease,
    PackageMismatch,
    WithdrawnRelease,
    PackageRead,
}

impl fmt::Display for TrustError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::TooLarge => "Die signierten Metadaten sind zu groß.",
            Self::InvalidEnvelope => "Die Signaturhülle entspricht nicht dem Vertrag.",
            Self::InvalidSignature => "Die Metadatensignatur ist ungültig.",
            Self::InvalidTrustStore => "Die Vertrauensregeln sind nicht eindeutig oder ungültig.",
            Self::UnknownKey => "Der Signierschlüssel ist nicht freigegeben.",
            Self::KeyUnavailable => "Der Signierschlüssel ist gesperrt oder zeitlich nicht gültig.",
            Self::WrongSubject => "Die Metadaten gehören nicht zum freigegebenen Zweck.",
            Self::Expired => "Die Metadaten sind abgelaufen.",
            Self::NotYetValid => "Die Metadaten sind noch nicht gültig.",
            Self::Rollback => "Die Metadaten sind älter als der bereits geprüfte Stand.",
            Self::ClockRollback => "Die Systemzeit liegt vor der letzten Metadatenprüfung. Prüfen Sie Datum und Uhrzeit.",
            Self::ConflictingSequence => {
                "Für dieselbe Metadatenfolge liegen unterschiedliche Inhalte vor."
            }
            Self::InvalidCatalog => "Der signierte Katalog entspricht nicht dem Vertrag.",
            Self::InvalidRelease => "Der signierte Paketnachweis passt nicht zum Tool-Vertrag.",
            Self::PackageMismatch => {
                "Die Paketbytes stimmen nicht mit dem signierten Nachweis überein."
            }
            Self::WithdrawnRelease => "Diese Veröffentlichung wurde zurückgezogen.",
            Self::PackageRead => "Das Paket konnte nicht vollständig gelesen werden.",
        };
        f.write_str(message)
    }
}

impl std::error::Error for TrustError {}
