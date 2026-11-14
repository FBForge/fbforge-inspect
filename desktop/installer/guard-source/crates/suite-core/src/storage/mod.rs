//! Private per-user state. Native callers supply the Suite-owned directory,
//! never a directory or cached evidence chosen by the renderer or a manifest.
mod files;
mod format;
mod packages;
pub use packages::{
    CachedPackage, PackageCache, PackageCacheError, PackageCacheLimits, PackageStaging,
};
mod releases;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
use windows as platform;

use crate::{
    catalog::Catalog,
    inventory::{Inventory, InventoryError},
    settings::{SettingsError, UserSettings},
    trust::{verify_catalog, MetadataCheckpoint, TrustError, TrustStore, VerifiedCatalog},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt,
    fs::File,
    path::{Path, PathBuf},
    sync::Arc,
};

pub(crate) const MAX_STATE_BYTES: usize = 16_777_216;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CachedMetadata {
    envelope: String,
    checkpoint: MetadataCheckpoint,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SavedState {
    schema_version: u32,
    generation: u64,
    catalog: Option<CachedMetadata>,
    inventory: Option<String>,
    #[serde(default)]
    settings: Option<String>,
    #[serde(
        default,
        deserialize_with = "releases::deserialize_releases",
        skip_serializing_if = "BTreeMap::is_empty"
    )]
    releases: BTreeMap<String, CachedMetadata>,
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            generation: 0,
            catalog: None,
            inventory: None,
            settings: None,
            releases: BTreeMap::new(),
        }
    }
}

pub struct StateStore {
    root: PathBuf,
    _lock: Arc<File>,
    state: SavedState,
    requires_reopen: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CatalogCacheStatus {
    Missing,
    Current,
    Unavailable(TrustError),
}

pub struct RecoveredInventory {
    pub inventory: Inventory,
    pub catalog_status: CatalogCacheStatus,
}

impl StateStore {
    pub fn open(root: &Path) -> Result<Self, StoreError> {
        files::prepare_directory(root)?;
        let lock = files::open_lock(root)?;
        lock.try_lock().map_err(|error| match error {
            std::fs::TryLockError::WouldBlock => StoreError::Busy,
            std::fs::TryLockError::Error(_) => StoreError::Io,
        })?;
        let state = match files::read_state(root)? {
            Some(json) => format::decode(&json)?,
            None => SavedState::default(),
        };
        files::clean_temporary_files(root)?;
        Ok(Self {
            root: root.to_owned(),
            _lock: Arc::new(lock),
            state,
            requires_reopen: false,
        })
    }

    pub fn load_catalog(
        &mut self,
        trust: &TrustStore,
        now: u64,
    ) -> Result<VerifiedCatalog, StoreError> {
        self.ensure_ready()?;
        let verified = self.verify_saved_catalog(trust, now)?;
        let cached = self
            .state
            .catalog
            .as_ref()
            .ok_or(StoreError::MissingCatalog)?;
        // A successful cache check advances the persisted clock checkpoint too.
        // Returning evidence without committing that time would permit rollback
        // to an older clock after a restart.
        if cached.checkpoint != *verified.checkpoint() {
            let updated = CachedMetadata {
                envelope: cached.envelope.clone(),
                checkpoint: verified.checkpoint().clone(),
            };
            let mut next = self.state.clone();
            next.catalog = Some(updated);
            self.persist(next)?;
        }
        Ok(verified)
    }

    pub fn load_inventory(
        &mut self,
        builtin: &Catalog,
        trust: &TrustStore,
        now: u64,
    ) -> Result<RecoveredInventory, StoreError> {
        self.ensure_ready()?;
        match self.load_catalog(trust, now) {
            Ok(verified) => Ok(RecoveredInventory {
                inventory: self.restore_inventory(verified.catalog())?,
                catalog_status: CatalogCacheStatus::Current,
            }),
            Err(StoreError::MissingCatalog) => Ok(RecoveredInventory {
                inventory: self.restore_inventory(builtin)?,
                catalog_status: CatalogCacheStatus::Missing,
            }),
            Err(StoreError::Trust(error)) => Ok(RecoveredInventory {
                inventory: self.restore_inventory(builtin)?,
                catalog_status: CatalogCacheStatus::Unavailable(error),
            }),
            Err(error) => Err(error),
        }
    }

    pub fn accept_catalog(
        &mut self,
        envelope: &str,
        trust: &TrustStore,
        now: u64,
    ) -> Result<VerifiedCatalog, StoreError> {
        self.ensure_ready()?;
        let checkpoint = self.state.catalog.as_ref().map(|cached| &cached.checkpoint);
        let verified =
            verify_catalog(envelope, trust, now, checkpoint).map_err(StoreError::Trust)?;
        let inventory = self.restore_inventory(verified.catalog())?;
        let mut next = self.state.clone();
        next.catalog = Some(CachedMetadata {
            envelope: envelope.into(),
            checkpoint: verified.checkpoint().clone(),
        });
        next.inventory = Some(inventory.snapshot_json().map_err(StoreError::Inventory)?);
        self.persist(next)?;
        Ok(verified)
    }

    pub fn save_inventory(&mut self, inventory: &Inventory) -> Result<(), StoreError> {
        self.ensure_ready()?;
        let mut next = self.state.clone();
        next.inventory = Some(inventory.snapshot_json().map_err(StoreError::Inventory)?);
        self.persist(next)
    }

    pub fn load_settings(&self) -> Result<UserSettings, StoreError> {
        self.ensure_ready()?;
        match &self.state.settings {
            Some(json) => UserSettings::from_json(json).map_err(StoreError::Settings),
            None => Ok(UserSettings::default()),
        }
    }

    pub fn save_settings(&mut self, settings: &UserSettings) -> Result<(), StoreError> {
        self.ensure_ready()?;
        let mut next = self.state.clone();
        next.settings = Some(settings.to_json().map_err(StoreError::Settings)?);
        self.persist(next)
    }

    fn restore_inventory(&self, current: &Catalog) -> Result<Inventory, StoreError> {
        match &self.state.inventory {
            Some(snapshot) => Inventory::restore_snapshot(snapshot, current),
            None => Inventory::from_catalog(current),
        }
        .map_err(StoreError::Inventory)
    }

    fn verify_saved_catalog(
        &self,
        trust: &TrustStore,
        now: u64,
    ) -> Result<VerifiedCatalog, StoreError> {
        let cached = self
            .state
            .catalog
            .as_ref()
            .ok_or(StoreError::MissingCatalog)?;
        verify_catalog(&cached.envelope, trust, now, Some(&cached.checkpoint))
            .map_err(StoreError::Trust)
    }

    fn ensure_ready(&self) -> Result<(), StoreError> {
        if self.requires_reopen {
            Err(StoreError::Indeterminate)
        } else {
            Ok(())
        }
    }

    fn persist(&mut self, mut next: SavedState) -> Result<(), StoreError> {
        next.generation = self
            .state
            .generation
            .checked_add(1)
            .ok_or(StoreError::CorruptState)?;
        let json = format::encode(&next)?;
        if let Err(error) = files::write_state(&self.root, &json) {
            if error == StoreError::Indeterminate {
                self.requires_reopen = true;
            }
            return Err(error);
        }
        self.state = next;
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum StoreError {
    Busy,
    UnsafePath,
    CorruptState,
    TooLarge,
    Io,
    Indeterminate,
    MissingCatalog,
    MissingRelease,
    Trust(TrustError),
    Inventory(InventoryError),
    Settings(SettingsError),
}
impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy=>"Die lokale Datenablage wird bereits verwendet.",
            Self::UnsafePath=>"Die lokale Datenablage hat keinen geeigneten geschützten Dateipfad.",
            Self::CorruptState=>"Die lokale Datenablage ist beschädigt oder wird nicht unterstützt.",
            Self::TooLarge=>"Die lokale Datenablage überschreitet die zulässige Größe.",
            Self::Io=>"Die lokale Datenablage konnte nicht vollständig gelesen oder geschrieben werden.",
            Self::Indeterminate=>"Der Speicherstand ist unklar. Öffnen Sie die Datenablage erneut, bevor Sie fortfahren.",
            Self::MissingCatalog=>"Es wurde noch kein verifizierter Katalog gespeichert.",
            Self::MissingRelease=>"Für dieses Werkzeug wurden noch keine verifizierten Release-Metadaten gespeichert.",
            Self::Trust(error)=>return error.fmt(f),
            Self::Inventory(error)=>return error.fmt(f),
            Self::Settings(error)=>return error.fmt(f),
        })
    }
}
impl std::error::Error for StoreError {}
