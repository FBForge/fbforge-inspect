use super::super::{journal::RECEIPT, manifest::PayloadManifest, TargetError};
use crate::{authenticode::installer_targets_bridge as proof, catalog::ToolManifest};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

pub(super) fn executable(
    payload: &PayloadManifest,
    tool: &ToolManifest,
) -> Result<String, TargetError> {
    let names: Vec<_> = payload
        .files()
        .iter()
        .filter(|file| {
            !file.path().contains('\\')
                && tool
                    .executables
                    .iter()
                    .any(|name| name.eq_ignore_ascii_case(file.path()))
        })
        .collect();
    if names.len() != 1 {
        return Err(TargetError::InvalidManifest);
    }
    Ok(names[0].path().to_string())
}
pub(super) fn inspect(
    root: &Path,
    tool: &ToolManifest,
    payload: &PayloadManifest,
    now: u64,
    receipt_allowed: bool,
) -> Result<proof::MutationTree, TargetError> {
    let executable = root.join(executable(payload, tool)?);
    let (tree, _) = proof::program_tree(&executable, tool, Some(payload.version()), now)?;
    let mut observed = BTreeSet::new();
    let mut uninstaller = false;
    let mut marker = false;
    for (path, file) in tree.files() {
        let relative = path
            .strip_prefix(tree.path())
            .map_err(|_| TargetError::UnsafeTarget)?;
        let relative = relative
            .to_str()
            .ok_or(TargetError::UnsupportedTarget)?
            .to_string();
        let folded = relative.to_ascii_lowercase();
        if !observed.insert(folded.clone()) {
            return Err(TargetError::PayloadMismatch);
        }
        match folded.as_str() {
            ".fbforge-install.pending" => {
                if file
                    .metadata()
                    .map_err(|_| TargetError::OperatingSystem)?
                    .len()
                    > 4096
                {
                    return Err(TargetError::PayloadMismatch);
                }
                marker = true;
            }
            "uninstall.exe" => {
                let mut file = file.try_clone().map_err(|_| TargetError::OperatingSystem)?;
                hash_file(&file, 64 * 1024 * 1024)?;
                proof::uninstaller(&mut file, tool, payload.version(), now)?;
                uninstaller = true;
            }
            RECEIPT if receipt_allowed => {}
            _ => {
                let expected = payload
                    .files()
                    .iter()
                    .find(|entry| entry.path().eq_ignore_ascii_case(&relative))
                    .ok_or(TargetError::PayloadMismatch)?;
                let (size, hash) = hash_file(file, expected.size())?;
                if size != expected.size() || &hash != expected.sha256() {
                    return Err(TargetError::PayloadMismatch);
                }
            }
        }
    }
    if !uninstaller
        || (!receipt_allowed && !marker)
        || payload
            .files()
            .iter()
            .any(|entry| !observed.contains(&entry.path().to_ascii_lowercase()))
    {
        return Err(TargetError::PayloadMismatch);
    }
    Ok(tree)
}
pub(super) fn hash_file(file: &File, limit: u64) -> Result<(u64, [u8; 32]), TargetError> {
    let length = file
        .metadata()
        .map_err(|_| TargetError::OperatingSystem)?
        .len();
    if length > limit {
        return Err(TargetError::PayloadMismatch);
    }
    let mut reader = file.try_clone().map_err(|_| TargetError::OperatingSystem)?;
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| TargetError::OperatingSystem)?;
    let mut hash = ring::digest::Context::new(&ring::digest::SHA256);
    let mut bytes = 0u64;
    let mut buffer = [0; 64 * 1024];
    loop {
        let size = reader
            .read(&mut buffer)
            .map_err(|_| TargetError::OperatingSystem)?;
        if size == 0 {
            break;
        }
        bytes = bytes
            .checked_add(size as u64)
            .ok_or(TargetError::PayloadMismatch)?;
        if bytes > limit {
            return Err(TargetError::PayloadMismatch);
        }
        hash.update(&buffer[..size]);
    }
    if bytes != length {
        return Err(TargetError::PayloadMismatch);
    }
    Ok((
        bytes,
        hash.finish().as_ref().try_into().expect("SHA256 size"),
    ))
}
pub(super) fn selected_path(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('\\')
        .fold(root.to_path_buf(), |path, component| path.join(component))
}
