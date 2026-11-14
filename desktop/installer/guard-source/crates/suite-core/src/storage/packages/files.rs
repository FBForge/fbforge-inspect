use super::{platform, PackageCacheError, PackageCacheLimits, MAX_BYTES};
use crate::storage::{files, StoreError};
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
};

#[derive(PartialEq, Eq)]
pub(super) struct Stamp(pub [u64; 6]);
pub(super) struct Entry {
    pub name: String,
    pub size: u64,
}
pub(super) struct Directory {
    pub path: PathBuf,
    pub handle: File,
    _store_root: File,
}
fn map_store(error: StoreError) -> PackageCacheError {
    match error {
        StoreError::UnsafePath => PackageCacheError::UnsafePath,
        _ => PackageCacheError::Store(error),
    }
}
impl Directory {
    pub fn open(store_root: &Path) -> Result<Self, PackageCacheError> {
        let store_handle = platform::open_directory(store_root)?;
        let path = store_root.join("packages-v1");
        files::prepare_directory(&path).map_err(map_store)?;
        let handle = platform::open_directory(&path)?;
        Ok(Self {
            path,
            handle,
            _store_root: store_handle,
        })
    }
    pub fn validate(&self, file: &File) -> Result<(), PackageCacheError> {
        platform::validate_file(file)
    }
    pub fn open_lock(&self) -> Result<File, PackageCacheError> {
        platform::open_lock(self)
    }
    pub fn open_file(&self, name: &str, create: bool) -> Result<File, PackageCacheError> {
        platform::open_file(self, name, create)
    }
    pub fn remove(&self, file: &File, name: &str) -> Result<(), PackageCacheError> {
        platform::remove(self, file, name)
    }
    pub fn publish(
        &self,
        file: &File,
        source: &str,
        destination: &str,
    ) -> Result<(), PackageCacheError> {
        platform::publish(self, file, source, destination)
    }
    pub fn create_staging(&self) -> Result<(String, File), PackageCacheError> {
        let mut nonce = [0u8; 16];
        SystemRandom::new()
            .fill(&mut nonce)
            .map_err(|_| PackageCacheError::Io)?;
        self.create_staging_with_nonce(&nonce)
    }
    fn create_staging_with_nonce(
        &self,
        nonce: &[u8; 16],
    ) -> Result<(String, File), PackageCacheError> {
        let name = format!(
            ".package-{}.part",
            crate::trust::encoding::encode_hex(nonce)
        );
        // CREATE_NEW/O_EXCL: a collision is never eligible for our cleanup.
        let file = self.open_file(&name, true)?;
        Ok((name, file))
    }
    pub fn clean_and_list(&mut self) -> Result<Vec<Entry>, PackageCacheError> {
        let mut entries = Vec::new();
        for (index, entry) in fs::read_dir(&self.path)
            .map_err(|_| PackageCacheError::Io)?
            .enumerate()
        {
            if index >= 128 {
                return Err(PackageCacheError::Full);
            }
            let entry = entry.map_err(|_| PackageCacheError::Io)?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| PackageCacheError::UnsafePath)?;
            if name == "state.lock" {
                continue;
            }
            let temporary = name
                .strip_prefix(".package-")
                .and_then(|s| s.strip_suffix(".part"))
                .is_some_and(|s| hex_name(s, 32));
            let completed = name.strip_suffix(".pkg").is_some_and(|s| hex_name(s, 64));
            if !temporary && !completed {
                return Err(PackageCacheError::UnsafePath);
            }
            let file = self.open_file(&name, false)?;
            let size = file.metadata().map_err(|_| PackageCacheError::Io)?.len();
            if size > MAX_BYTES {
                return Err(PackageCacheError::Full);
            }
            if temporary {
                self.remove(&file, &name)?;
            } else {
                entries.push(Entry { name, size });
            }
        }
        entries.sort_unstable_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
    pub fn reserve(
        &self,
        entries: Vec<Entry>,
        incoming: u64,
        incoming_files: usize,
        limits: PackageCacheLimits,
    ) -> Result<(), PackageCacheError> {
        let mut bytes = entries
            .iter()
            .try_fold(0u64, |sum, e| sum.checked_add(e.size))
            .ok_or(PackageCacheError::Full)?;
        let mut count = entries.len();
        for entry in entries {
            if bytes
                .checked_add(incoming)
                .is_some_and(|n| n <= limits.bytes)
                && count
                    .checked_add(incoming_files)
                    .is_some_and(|n| n <= limits.files)
            {
                return Ok(());
            }
            let file = self.open_file(&entry.name, false)?;
            self.remove(&file, &entry.name)?;
            bytes -= entry.size;
            count -= 1;
        }
        if incoming <= limits.bytes
            && count
                .checked_add(incoming_files)
                .is_some_and(|n| n <= limits.files)
        {
            Ok(())
        } else {
            Err(PackageCacheError::Full)
        }
    }
}
fn hex_name(text: &str, length: usize) -> bool {
    text.len() == length
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn staging_collision_never_removes_or_truncates_an_existing_file() {
        let root =
            std::env::temp_dir().join(format!("suite-package-collision-{}", std::process::id()));
        crate::storage::files::prepare_directory(&root).unwrap();
        let directory = Directory::open(&root).unwrap();
        let name = ".package-00000000000000000000000000000000.part";
        let mut existing = directory.open_file(name, true).unwrap();
        existing.write_all(b"existing bytes").unwrap();
        drop(existing);
        assert!(directory.create_staging_with_nonce(&[0; 16]).is_err());
        let mut existing = directory.open_file(name, false).unwrap();
        let mut bytes = Vec::new();
        existing.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"existing bytes");
        drop(existing);
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }
}
