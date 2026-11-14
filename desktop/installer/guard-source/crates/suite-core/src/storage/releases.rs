use super::{CachedMetadata, SavedState, StateStore, StoreError};
use crate::{
    catalog::is_identifier,
    trust::{
        verify_release, verify_release_candidate, TrustStore, VerifiedCatalog, VerifiedRelease,
    },
};
use semver::Version;
use std::{collections::BTreeMap, fmt};

const MAX_RELEASE_TOOLS: usize = 500;

impl StateStore {
    /// Stored identifiers only, never evidence of current key/time/catalog trust.
    /// Call load_saved_release to authenticate any corresponding display data.
    pub fn saved_release_tool_ids(&self) -> Result<impl Iterator<Item = &str>, StoreError> {
        self.ensure_ready()?;
        Ok(self.state.releases.keys().map(String::as_str))
    }

    /// Persist the exact signed envelope and its monotone per-tool checkpoint
    /// together with the current catalog's revalidation time before returning.
    /// Withdrawn releases are retained and never fall back to an older cache.
    pub fn accept_release(
        &mut self,
        envelope: &str,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
        expected_version: &Version,
    ) -> Result<VerifiedRelease, StoreError> {
        self.accept_release_for_version(envelope, trust, now, tool_id, Some(expected_version))
    }

    /// Authenticate and retain the offered version without guessing it from an
    /// untrusted body. Uses the same per-tool checkpoint as exact-version reads.
    pub fn accept_release_candidate(
        &mut self,
        envelope: &str,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
    ) -> Result<VerifiedRelease, StoreError> {
        self.accept_release_for_version(envelope, trust, now, tool_id, None)
    }

    fn accept_release_for_version(
        &mut self,
        envelope: &str,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
        expected_version: Option<&Version>,
    ) -> Result<VerifiedRelease, StoreError> {
        self.ensure_ready()?;
        if self.state.releases.len() >= MAX_RELEASE_TOOLS
            && !self.state.releases.contains_key(tool_id)
        {
            return Err(StoreError::TooLarge);
        }
        let catalog = self.verify_saved_catalog(trust, now)?;
        let previous = self
            .state
            .releases
            .get(tool_id)
            .map(|cache| &cache.checkpoint);
        let verified = match expected_version {
            Some(version) => {
                verify_release(envelope, trust, now, previous, &catalog, tool_id, version)
            }
            None => verify_release_candidate(envelope, trust, now, previous, &catalog, tool_id),
        }
        .map_err(StoreError::Trust)?;
        let mut next = self.state.clone();
        update_catalog_checkpoint(&mut next, &catalog)?;
        next.releases.insert(
            tool_id.to_owned(),
            CachedMetadata {
                envelope: envelope.to_owned(),
                checkpoint: verified.checkpoint().clone(),
            },
        );
        self.persist(next)?;
        Ok(verified)
    }

    /// Re-evaluate saved release metadata against current native trust, time and
    /// the latest persisted catalog. The caller cannot supply a stale catalog.
    pub fn load_release(
        &mut self,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
        expected_version: &Version,
    ) -> Result<VerifiedRelease, StoreError> {
        self.load_release_for_version(trust, now, tool_id, Some(expected_version))
    }

    /// Revalidate the saved offered version using current catalog/key/time and
    /// persisted rollback rules. Missing/withdrawn/expired metadata never falls
    /// back to a different saved release or a guessed version.
    pub fn load_saved_release(
        &mut self,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
    ) -> Result<VerifiedRelease, StoreError> {
        self.load_release_for_version(trust, now, tool_id, None)
    }

    fn load_release_for_version(
        &mut self,
        trust: &TrustStore,
        now: u64,
        tool_id: &str,
        expected_version: Option<&Version>,
    ) -> Result<VerifiedRelease, StoreError> {
        self.ensure_ready()?;
        let catalog = self.verify_saved_catalog(trust, now)?;
        let cached = self
            .state
            .releases
            .get(tool_id)
            .ok_or(StoreError::MissingRelease)?;
        let checkpoint = Some(&cached.checkpoint);
        let verified = match expected_version {
            Some(version) => verify_release(
                &cached.envelope,
                trust,
                now,
                checkpoint,
                &catalog,
                tool_id,
                version,
            ),
            None => verify_release_candidate(
                &cached.envelope,
                trust,
                now,
                checkpoint,
                &catalog,
                tool_id,
            ),
        }
        .map_err(StoreError::Trust)?;
        if cached.checkpoint != *verified.checkpoint()
            || self
                .state
                .catalog
                .as_ref()
                .is_some_and(|cache| cache.checkpoint != *catalog.checkpoint())
        {
            let mut next = self.state.clone();
            update_catalog_checkpoint(&mut next, &catalog)?;
            next.releases
                .get_mut(tool_id)
                .ok_or(StoreError::MissingRelease)?
                .checkpoint = verified.checkpoint().clone();
            self.persist(next)?;
        }
        Ok(verified)
    }
}

fn update_catalog_checkpoint(
    state: &mut SavedState,
    catalog: &VerifiedCatalog,
) -> Result<(), StoreError> {
    state
        .catalog
        .as_mut()
        .ok_or(StoreError::MissingCatalog)?
        .checkpoint = catalog.checkpoint().clone();
    Ok(())
}

pub(super) fn deserialize_releases<'de, D>(
    deserializer: D,
) -> Result<BTreeMap<String, CachedMetadata>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Releases;
    impl<'de> serde::de::Visitor<'de> for Releases {
        type Value = BTreeMap<String, CachedMetadata>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("eindeutige begrenzte Release-Metadaten je Werkzeug")
        }
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: serde::de::MapAccess<'de>,
        {
            let mut releases = BTreeMap::new();
            while let Some(id) = map.next_key::<String>()? {
                if !is_identifier(&id)
                    || releases.len() >= MAX_RELEASE_TOOLS
                    || releases.contains_key(&id)
                {
                    return Err(serde::de::Error::custom(
                        "Ungültige gespeicherte Release-Metadaten.",
                    ));
                }
                releases.insert(id, map.next_value()?);
            }
            Ok(releases)
        }
    }
    deserializer.deserialize_map(Releases)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_empty_release_id_enumeration_rejects_an_indeterminate_store() {
        let root = std::env::temp_dir().join(format!("suite-release-ids-{}", std::process::id()));
        std::fs::create_dir(&root).unwrap();
        let mut store = StateStore::open(&root.join("state")).unwrap();
        assert_eq!(store.saved_release_tool_ids().unwrap().count(), 0);
        // Direct private unit state, never a runtime hook or environment override.
        store.requires_reopen = true;
        assert!(matches!(
            store.saved_release_tool_ids(),
            Err(StoreError::Indeterminate)
        ));
        drop(store);
        std::fs::remove_dir_all(root).unwrap();
    }
}
