use super::{manifest::canonical_version, TargetError};
use serde::{Deserialize, Serialize};

pub(super) const MAX_JOURNAL_BYTES: usize = 64 * 1024;
#[cfg(windows)]
pub(super) const RECEIPT: &str = ".fbforge-installation.v1.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ObjectId {
    pub volume: u32,
    pub high: u32,
    pub low: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Phase {
    Staging,
    Verified,
    MoveOldIntent,
    PublishIntent,
    Published,
    Complete,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Journal {
    pub schema_version: u32,
    pub tool_id: String,
    pub app_id: String,
    pub version: String,
    pub source_sha256: [u8; 32],
    pub selection_sha256: [u8; 32],
    pub target: String,
    pub parent: ObjectId,
    pub original: Option<ObjectId>,
    pub original_version: Option<String>,
    pub staging: ObjectId,
    pub phase: Phase,
    pub sequence: u64,
}
impl Journal {
    pub(super) fn decode(bytes: &[u8]) -> Result<Self, TargetError> {
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(TargetError::RecoveryRequired);
        }
        let journal: Self =
            serde_json::from_slice(bytes).map_err(|_| TargetError::RecoveryRequired)?;
        if journal.schema_version != 1
            || journal.sequence == 0
            || journal.target.encode_utf16().count() > 320
            || crate::discovery::validate_windows_location(&journal.target).as_deref()
                != Ok(&journal.target)
            || !crate::catalog::is_identifier(&journal.tool_id)
            || !crate::catalog::is_identifier(&journal.app_id)
            || canonical_version(&journal.version).is_err()
            || journal.original.is_some() != journal.original_version.is_some()
            || journal
                .original_version
                .as_ref()
                .is_some_and(|version| canonical_version(version).is_err())
            || journal.original == Some(journal.staging)
            || journal.parent == journal.staging
            || journal.original == Some(journal.parent)
            || journal.staging.volume != journal.parent.volume
            || journal
                .original
                .is_some_and(|original| original.volume != journal.parent.volume)
        {
            return Err(TargetError::RecoveryRequired);
        }
        Ok(journal)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Layout {
    OriginalIntact,
    OriginalBackedUp,
    Published,
}

/// The durable journal's phase is intent, never evidence a rename took place.
/// Actual current object identities decide the only recoverable layouts.
pub(super) fn inspect_layout(
    journal: &Journal,
    target: Option<ObjectId>,
    backup: Option<ObjectId>,
    stage: Option<ObjectId>,
) -> Result<Layout, TargetError> {
    if target == Some(journal.staging) && stage.is_none() && backup == journal.original {
        return Ok(Layout::Published);
    }
    if stage != Some(journal.staging) {
        return Err(TargetError::RecoveryRequired);
    }
    if target == journal.original && backup.is_none() {
        return Ok(Layout::OriginalIntact);
    }
    if journal.original.is_some() && target.is_none() && backup == journal.original {
        return Ok(Layout::OriginalBackedUp);
    }
    Err(TargetError::RecoveryRequired)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(low: u32) -> ObjectId {
        ObjectId {
            volume: 4,
            high: 0,
            low,
        }
    }
    fn journal() -> Journal {
        Journal {
            schema_version: 1,
            tool_id: "example-tool".into(),
            app_id: "example.app".into(),
            version: "1.0.0".into(),
            source_sha256: [1; 32],
            selection_sha256: [2; 32],
            target: "C:\\private\\app".into(),
            parent: id(1),
            original: Some(id(2)),
            original_version: Some("0.9.0".into()),
            staging: id(3),
            phase: Phase::Staging,
            sequence: 1,
        }
    }
    #[test]
    fn recovery_uses_objects_even_if_the_last_phase_write_lagged_a_rename() {
        let mut journal = journal();
        for phase in [
            Phase::Staging,
            Phase::Verified,
            Phase::MoveOldIntent,
            Phase::PublishIntent,
            Phase::Published,
            Phase::Complete,
        ] {
            journal.phase = phase;
            assert_eq!(
                inspect_layout(&journal, Some(id(2)), None, Some(id(3))),
                Ok(Layout::OriginalIntact)
            );
            assert_eq!(
                inspect_layout(&journal, None, Some(id(2)), Some(id(3))),
                Ok(Layout::OriginalBackedUp)
            );
            assert_eq!(
                inspect_layout(&journal, Some(id(3)), Some(id(2)), None),
                Ok(Layout::Published)
            );
        }
    }
    #[test]
    fn unknown_replaced_or_missing_objects_never_authorize_overwrite_or_rollback() {
        let journal = journal();
        for (target, backup, stage) in [
            (None, None, None),
            (Some(id(8)), Some(id(2)), None),
            (None, Some(id(8)), Some(id(3))),
            (Some(id(2)), Some(id(2)), Some(id(3))),
        ] {
            assert_eq!(
                inspect_layout(&journal, target, backup, stage),
                Err(TargetError::RecoveryRequired)
            );
        }
    }
    #[test]
    fn journal_decode_rejects_unbounded_unknown_or_inconsistent_records() {
        let encode = |journal: &Journal| serde_json::to_vec(journal).unwrap();
        assert!(Journal::decode(&encode(&journal())).is_ok());
        assert!(Journal::decode(&vec![b' '; MAX_JOURNAL_BYTES + 1]).is_err());
        for change in 0..6 {
            let mut record = journal();
            match change {
                0 => record.sequence = 0,
                1 => record.staging = record.parent,
                2 => record.original_version = None,
                3 => record.staging.volume += 1,
                4 => record.target = r"C:\private\..\escape".into(),
                _ => record.version = "v1.0.0".into(),
            }
            assert!(Journal::decode(&encode(&record)).is_err());
        }
        let mut unknown = serde_json::to_value(journal()).unwrap();
        unknown["command"] = "never".into();
        assert!(Journal::decode(&serde_json::to_vec(&unknown).unwrap()).is_err());
    }
    #[test]
    fn fresh_install_does_not_invent_an_old_root() {
        let mut journal = journal();
        journal.original = None;
        journal.original_version = None;
        assert_eq!(
            inspect_layout(&journal, None, None, Some(id(3))),
            Ok(Layout::OriginalIntact)
        );
        assert_eq!(
            inspect_layout(&journal, Some(id(3)), None, None),
            Ok(Layout::Published)
        );
        assert_eq!(
            inspect_layout(&journal, None, Some(id(2)), Some(id(3))),
            Err(TargetError::RecoveryRequired)
        );
    }
}
