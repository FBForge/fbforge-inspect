use super::{
    encoding::{decode_hex, encode_hex},
    KeyStatus, MetadataCheckpoint, TrustError, TrustStore,
};
use ring::{digest, signature};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

const DOMAIN_V1: &[u8] = b"FBForge Suite metadata v1\0";
pub(super) const DOMAIN_V2: &[u8] = b"FBForge Suite metadata v2\0";
pub(super) const MAX_SIGNED_BYTES: usize = 1_048_576;
const MAX_ENVELOPE_BYTES: usize = 2_097_152;
const MAX_VALIDITY_SECONDS: u64 = 30 * 24 * 60 * 60;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Envelope {
    schema_version: u32,
    key_id: String,
    signed: String,
    signature: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Payload {
    pub schema_version: u32,
    pub subject: String,
    pub sequence: u64,
    pub issued_at: u64,
    pub expires_at: u64,
    pub body: Box<RawValue>,
}

pub(super) fn validate_validity(
    sequence: u64,
    issued_at: u64,
    expires_at: u64,
) -> Result<(), TrustError> {
    let lifetime = expires_at
        .checked_sub(issued_at)
        .ok_or(TrustError::InvalidEnvelope)?;
    if sequence == 0 || lifetime == 0 || lifetime > MAX_VALIDITY_SECONDS {
        return Err(TrustError::InvalidEnvelope);
    }
    Ok(())
}

pub(super) struct VerifiedPayload {
    pub body: Box<RawValue>,
    pub checkpoint: MetadataCheckpoint,
    pub expires_at: u64,
}

// Structural consistency only. This does not grant trust or bypass verification
// against current keys and time when cached metadata is actually used.
pub(crate) fn validate_cached_catalog(
    json: &str,
    checkpoint: &MetadataCheckpoint,
) -> Result<(), TrustError> {
    let body = validate_cached_payload(json, checkpoint, "catalog")?;
    crate::catalog::parse_catalog(body.get()).map_err(|_| TrustError::InvalidCatalog)?;
    Ok(())
}

pub(super) fn validate_cached_payload(
    json: &str,
    checkpoint: &MetadataCheckpoint,
    subject: &str,
) -> Result<Box<RawValue>, TrustError> {
    if json.len() > MAX_ENVELOPE_BYTES {
        return Err(TrustError::TooLarge);
    }
    let envelope: Envelope = serde_json::from_str(json).map_err(|_| TrustError::InvalidEnvelope)?;
    if !matches!(envelope.schema_version, 1 | 2) || envelope.signed.len() > MAX_SIGNED_BYTES {
        return Err(TrustError::InvalidEnvelope);
    }
    match envelope.schema_version {
        1 => {
            decode_hex::<64>(&envelope.signature)?;
        }
        2 => {
            super::minisign::decode_signature(&envelope.signature)?;
        }
        _ => return Err(TrustError::InvalidEnvelope),
    }
    let payload: Payload =
        serde_json::from_str(&envelope.signed).map_err(|_| TrustError::InvalidEnvelope)?;
    validate_validity(payload.sequence, payload.issued_at, payload.expires_at)?;
    let hash = encode_hex(digest::digest(&digest::SHA256, envelope.signed.as_bytes()).as_ref());
    if payload.schema_version != envelope.schema_version
        || payload.subject != subject
        || checkpoint.subject != payload.subject
        || checkpoint.metadata_schema_version != envelope.schema_version
        || checkpoint.sequence != payload.sequence
        || checkpoint.payload_sha256 != hash
        || checkpoint.last_verified_at < payload.issued_at
        || checkpoint.last_verified_at >= payload.expires_at
    {
        return Err(TrustError::InvalidEnvelope);
    }
    Ok(payload.body)
}

pub(super) fn verify_payload(
    json: &str,
    store: &TrustStore,
    subject: &str,
    now: u64,
    previous: Option<&MetadataCheckpoint>,
) -> Result<VerifiedPayload, TrustError> {
    if json.len() > MAX_ENVELOPE_BYTES {
        return Err(TrustError::TooLarge);
    }
    let envelope: Envelope = serde_json::from_str(json).map_err(|_| TrustError::InvalidEnvelope)?;
    if !matches!(envelope.schema_version, 1 | 2) || envelope.signed.len() > MAX_SIGNED_BYTES {
        return Err(TrustError::InvalidEnvelope);
    }
    let (status, not_before, expires_at, subjects) = match envelope.schema_version {
        1 => {
            let anchor = store
                .anchors
                .get(&envelope.key_id)
                .ok_or(TrustError::UnknownKey)?;
            (
                anchor.status,
                anchor.not_before,
                anchor.expires_at,
                &anchor.subjects,
            )
        }
        2 => {
            let anchor = store
                .minisign_anchors
                .get(&envelope.key_id)
                .ok_or(TrustError::UnknownKey)?;
            (
                anchor.status,
                anchor.not_before,
                anchor.expires_at,
                &anchor.subjects,
            )
        }
        _ => return Err(TrustError::InvalidEnvelope),
    };
    if status != KeyStatus::Active || now < not_before || now >= expires_at {
        return Err(TrustError::KeyUnavailable);
    }
    if !subjects.iter().any(|allowed| allowed == subject) {
        return Err(TrustError::WrongSubject);
    }
    let domain = if envelope.schema_version == 1 {
        DOMAIN_V1
    } else {
        DOMAIN_V2
    };
    let mut message = Vec::with_capacity(domain.len() + envelope.signed.len());
    message.extend_from_slice(domain);
    message.extend_from_slice(envelope.signed.as_bytes());
    match envelope.schema_version {
        1 => {
            let signature = decode_hex::<64>(&envelope.signature)?;
            let anchor = store
                .anchors
                .get(&envelope.key_id)
                .ok_or(TrustError::UnknownKey)?;
            signature::UnparsedPublicKey::new(&signature::ED25519, anchor.public_key)
                .verify(&message, &signature)
                .map_err(|_| TrustError::InvalidSignature)?;
        }
        2 => {
            let anchor = store
                .minisign_anchors
                .get(&envelope.key_id)
                .ok_or(TrustError::UnknownKey)?;
            super::minisign::verify(&message, &envelope.signature, &anchor.public_key)?;
        }
        _ => return Err(TrustError::InvalidEnvelope),
    }
    let payload: Payload =
        serde_json::from_str(&envelope.signed).map_err(|_| TrustError::InvalidEnvelope)?;
    if payload.schema_version != envelope.schema_version || payload.sequence == 0 {
        return Err(TrustError::InvalidEnvelope);
    }
    if payload.subject != subject {
        return Err(TrustError::WrongSubject);
    }
    validate_validity(payload.sequence, payload.issued_at, payload.expires_at)?;
    if payload.issued_at < not_before || payload.expires_at > expires_at {
        return Err(TrustError::KeyUnavailable);
    }
    if payload.issued_at > now {
        return Err(TrustError::NotYetValid);
    }
    if now >= payload.expires_at {
        return Err(TrustError::Expired);
    }
    let digest = encode_hex(digest::digest(&digest::SHA256, envelope.signed.as_bytes()).as_ref());
    if let Some(previous) = previous {
        if !matches!(previous.metadata_schema_version, 1 | 2) {
            return Err(TrustError::InvalidEnvelope);
        }
        if previous.subject != subject {
            return Err(TrustError::WrongSubject);
        }
        if now < previous.last_verified_at {
            return Err(TrustError::ClockRollback);
        }
        if envelope.schema_version < previous.metadata_schema_version
            || payload.sequence < previous.sequence
        {
            return Err(TrustError::Rollback);
        }
        if payload.sequence == previous.sequence && digest != previous.payload_sha256 {
            return Err(TrustError::ConflictingSequence);
        }
    }
    Ok(VerifiedPayload {
        body: payload.body,
        checkpoint: MetadataCheckpoint {
            metadata_schema_version: envelope.schema_version,
            subject: payload.subject,
            sequence: payload.sequence,
            payload_sha256: digest,
            last_verified_at: now,
        },
        expires_at: payload.expires_at,
    })
}
