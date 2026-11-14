use super::{SavedState, StoreError, MAX_STATE_BYTES};
use crate::{
    catalog::parse_catalog,
    inventory::Inventory,
    settings::UserSettings,
    trust::{encoding::encode_hex, validate_cached_catalog, validate_cached_release},
};
use ring::digest;
use serde::{Deserialize, Serialize};

const MAX_PAYLOAD_BYTES: usize = 8_388_608;

// Detects accidental state corruption, including otherwise valid JSON edits.
// This checksum is not authentication. The private directory is the trust boundary.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StateEnvelope {
    schema_version: u32,
    payload: String,
    sha256: String,
}

fn checksum(payload: &str) -> String {
    encode_hex(digest::digest(&digest::SHA256, payload.as_bytes()).as_ref())
}

pub(super) fn encode(state: &SavedState) -> Result<Vec<u8>, StoreError> {
    let payload = serde_json::to_string(state).map_err(|_| StoreError::CorruptState)?;
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(StoreError::TooLarge);
    }
    let envelope = StateEnvelope {
        schema_version: 1,
        sha256: checksum(&payload),
        payload,
    };
    let bytes = serde_json::to_vec(&envelope).map_err(|_| StoreError::CorruptState)?;
    if bytes.len() > MAX_STATE_BYTES {
        return Err(StoreError::TooLarge);
    }
    Ok(bytes)
}

pub(super) fn decode(json: &str) -> Result<SavedState, StoreError> {
    if json.len() > MAX_STATE_BYTES {
        return Err(StoreError::TooLarge);
    }
    let envelope: StateEnvelope =
        serde_json::from_str(json).map_err(|_| StoreError::CorruptState)?;
    if envelope.schema_version != 1
        || envelope.payload.len() > MAX_PAYLOAD_BYTES
        || envelope.sha256 != checksum(&envelope.payload)
    {
        return Err(StoreError::CorruptState);
    }
    let state: SavedState =
        serde_json::from_str(&envelope.payload).map_err(|_| StoreError::CorruptState)?;
    if state.schema_version != 1 || state.generation == 0 {
        return Err(StoreError::CorruptState);
    }
    if let Some(cache) = &state.catalog {
        validate_cached_catalog(&cache.envelope, &cache.checkpoint)
            .map_err(|_| StoreError::CorruptState)?;
    }
    for (tool_id, cache) in &state.releases {
        validate_cached_release(&cache.envelope, &cache.checkpoint, tool_id)
            .map_err(|_| StoreError::CorruptState)?;
    }
    if let Some(snapshot) = &state.inventory {
        let empty = parse_catalog(r#"{"schemaVersion":1,"tools":[]}"#)
            .map_err(|_| StoreError::CorruptState)?;
        Inventory::restore_snapshot(snapshot, &empty).map_err(|_| StoreError::CorruptState)?;
    }
    if let Some(settings) = &state.settings {
        UserSettings::from_json(settings).map_err(|_| StoreError::CorruptState)?;
    }
    Ok(state)
}
