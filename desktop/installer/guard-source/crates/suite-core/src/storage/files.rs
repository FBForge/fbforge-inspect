use super::{platform, StoreError, MAX_STATE_BYTES};
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path},
};

pub(super) fn prepare_directory(root: &Path) -> Result<(), StoreError> {
    platform::validate_root_syntax(root)?;
    validate_ancestry(root, true)?;
    platform::create_directory(root)?;
    validate_ancestry(root, false)?;
    platform::validate_directory(root)
}

fn validate_ancestry(root: &Path, allow_missing_leaf: bool) -> Result<(), StoreError> {
    if !root.is_absolute()
        || root.file_name().is_none()
        || root
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(StoreError::UnsafePath);
    }
    for path in root.ancestors().collect::<Vec<_>>().into_iter().rev() {
        match fs::symlink_metadata(path) {
            Ok(metadata) => platform::validate_ancestor(&metadata)?,
            Err(error)
                if error.kind() == std::io::ErrorKind::NotFound
                    && path == root
                    && allow_missing_leaf => {}
            Err(_) => return Err(StoreError::UnsafePath),
        }
    }
    Ok(())
}

fn open_file(path: &Path, write: bool, create: bool, exclusive: bool) -> Result<File, StoreError> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(write)
        .create(create)
        .create_new(exclusive);
    platform::configure_open(&mut options);
    let file = options.open(path).map_err(|_| StoreError::Io)?;
    platform::validate_file(&file, path)?;
    Ok(file)
}

pub(super) fn open_lock(root: &Path) -> Result<File, StoreError> {
    open_file(&root.join("state.lock"), true, true, false)
}

pub(super) fn read_state(root: &Path) -> Result<Option<String>, StoreError> {
    let path = root.join("state-v1.json");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(StoreError::Io),
        Ok(metadata) if !metadata.is_file() || metadata.file_type().is_symlink() => {
            return Err(StoreError::UnsafePath)
        }
        Ok(_) => {}
    }
    let file = open_file(&path, false, false, false)?;
    if file.metadata().map_err(|_| StoreError::Io)?.len() > MAX_STATE_BYTES as u64 {
        return Err(StoreError::TooLarge);
    }
    let mut json = String::new();
    file.take(MAX_STATE_BYTES as u64 + 1)
        .read_to_string(&mut json)
        .map_err(|_| StoreError::CorruptState)?;
    if json.len() > MAX_STATE_BYTES {
        return Err(StoreError::TooLarge);
    }
    Ok(Some(json))
}

pub(super) fn write_state(root: &Path, data: &[u8]) -> Result<(), StoreError> {
    let mut nonce = [0u8; 16];
    SystemRandom::new()
        .fill(&mut nonce)
        .map_err(|_| StoreError::Io)?;
    write_state_with_nonce(root, data, &nonce)
}

fn write_state_with_nonce(root: &Path, data: &[u8], nonce: &[u8; 16]) -> Result<(), StoreError> {
    validate_ancestry(root, false)?;
    platform::validate_directory(root)?;
    let destination = root.join("state-v1.json");
    if fs::symlink_metadata(&destination).is_ok() {
        open_file(&destination, false, false, false)?;
    }
    let name = format!(".state-{}.tmp", crate::trust::encoding::encode_hex(nonce));
    let temporary = root.join(name);
    let mut was_created = false;
    let result = (|| {
        let mut file = open_file(&temporary, true, false, true)?;
        was_created = true;
        file.write_all(data).map_err(|_| StoreError::Io)?;
        file.sync_all().map_err(|_| StoreError::Io)?;
        drop(file);
        platform::replace_file(&temporary, &destination)?;
        platform::sync_directory(root)?;
        Ok(())
    })();
    // Only the exclusively created random name is eligible for cleanup.
    // There is no recursive deletion or deletion of a caller-provided filename.
    if result.is_err() && was_created {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub(super) fn clean_temporary_files(root: &Path) -> Result<(), StoreError> {
    for entry in fs::read_dir(root).map_err(|_| StoreError::Io)? {
        let entry = entry.map_err(|_| StoreError::Io)?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(nonce) = name
            .strip_prefix(".state-")
            .and_then(|value| value.strip_suffix(".tmp"))
        else {
            continue;
        };
        if nonce.len() == 32 && nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            let file = open_file(&entry.path(), false, false, false)?;
            if file.metadata().map_err(|_| StoreError::Io)?.len() > MAX_STATE_BYTES as u64 {
                return Err(StoreError::TooLarge);
            }
            drop(file);
            fs::remove_file(entry.path()).map_err(|_| StoreError::Io)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_name_collision_never_deletes_an_existing_file() {
        let root = std::env::temp_dir().join(format!("suite-collision-{}", std::process::id()));
        prepare_directory(&root).unwrap();
        let file = root.join(".state-00000000000000000000000000000000.tmp");
        let mut existing = open_file(&file, true, false, true).unwrap();
        existing.write_all(b"existing data").unwrap();
        drop(existing);
        assert!(write_state_with_nonce(&root, b"new data", &[0; 16]).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"existing data");
        fs::remove_dir_all(&root).unwrap();
    }
}
