use super::{
    journal::{self, Journal, Layout, ObjectId, Phase},
    manifest::{self, PayloadManifest},
    TargetError, TargetPhase,
};
use crate::{
    authenticode::installer_targets_bridge as proof,
    catalog::{Capability, ToolManifest},
    discovery::discover_tool,
    inventory::RegistryHive,
    processes::{inspect_resource_users, ResourceUsers},
};
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    os::windows::ffi::OsStrExt,
    path::{Path, PathBuf},
    ptr,
    time::{SystemTime, UNIX_EPOCH},
};
use windows_sys::Win32::{
    Globalization::{CompareStringOrdinal, CSTR_EQUAL, CSTR_GREATER_THAN, CSTR_LESS_THAN},
    System::Com::CoTaskMemFree,
    UI::Shell::{FOLDERID_LocalAppData, SHGetKnownFolderPath, KF_FLAG_DONT_VERIFY},
};
mod health;
mod io;

pub(super) struct NativeTargetPlan {
    tool: ToolManifest,
    payload: PayloadManifest,
    target: PathBuf,
    parent: proof::DirectoryAnchor,
    transaction: proof::DirectoryAnchor,
    backup: proof::DirectoryAnchor,
    stage_anchor: Option<proof::DirectoryAnchor>,
    journal: Journal,
    locator: PathBuf,
    original: Option<proof::MutationTree>,
    stage_health: Option<proof::MutationTree>,
    old_resources: Vec<String>,
    phase: TargetPhase,
    created_at: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Locator {
    schema_version: u32,
    tool_id: String,
    app_id: String,
    transaction: String,
}

impl NativeTargetPlan {
    pub(super) fn prepare(
        tool: ToolManifest,
        payload: PayloadManifest,
        recovery_only: bool,
    ) -> Result<Self, TargetError> {
        proof::require_policy(&tool)?;
        if !tool.capabilities.iter().any(|capability| {
            matches!(
                capability,
                Capability::Install | Capability::Update | Capability::Repair
            )
        }) {
            return Err(TargetError::UnsupportedTarget);
        }
        health::executable(&payload, &tool)?;
        let (target, registered_roots) = discover_target(&tool)?;
        let registered = !registered_roots.is_empty();
        if target.as_os_str().encode_wide().count() > 320 {
            return Err(TargetError::UnsupportedTarget);
        }
        let parent = proof::directory(target.parent().ok_or(TargetError::UnsafeTarget)?, false)?;
        parent
            .require_mutable_volume()
            .map_err(proof::map_permission)?;
        let target = parent
            .path()
            .join(target.file_name().ok_or(TargetError::UnsafeTarget)?);
        // Before creating any transaction or moving the old root, reject raw
        // registry aliases whose equivalence disappears when that root is absent.
        super::binding::bind_registered_target(
            &registered_roots,
            target.to_str().ok_or(TargetError::UnsupportedTarget)?,
            None,
            ordinal_equal,
        )?;
        let prefix = format!(
            ".fbforge-txn-{}-",
            &io::hex(&manifest::sha256(tool.id.as_bytes()))[..16]
        );
        let locator = parent.path().join(format!(
            ".fbforge-active-{}.json",
            &io::hex(&manifest::sha256(tool.id.as_bytes()))[..16]
        ));
        let now = native_time()?;
        if io::exists(&locator)? {
            let raw = io::read(&locator, 4096)?;
            let saved: Locator =
                serde_json::from_slice(&raw).map_err(|_| TargetError::RecoveryRequired)?;
            if saved.schema_version != 1
                || saved.tool_id != tool.id
                || saved.app_id != tool.app_id
                || !valid_transaction_name(&saved.transaction, &prefix)
            {
                return Err(TargetError::RecoveryRequired);
            }
            let transaction = proof::directory(&parent.path().join(saved.transaction), true)?;
            let journal = Journal::decode(&io::read(
                &transaction.path().join("journal.v1.json"),
                journal::MAX_JOURNAL_BYTES,
            )?)?;
            if journal.tool_id != tool.id
                || journal.app_id != tool.app_id
                || journal.version != payload.version().to_string()
                || journal.source_sha256 != payload.source_sha256
                || journal.selection_sha256 != payload.selection_sha256
                || journal.parent != parent.id().map_err(proof::map_permission)?
            {
                return Err(TargetError::RecoveryRequired);
            }
            let target = PathBuf::from(super::binding::bind_registered_target(
                &registered_roots,
                target.to_str().ok_or(TargetError::UnsupportedTarget)?,
                Some(&journal.target),
                ordinal_equal,
            )?);
            let backup = proof::directory(&transaction.path().join("backup"), true)?;
            let stage = transaction.path().join("staging");
            let stage_anchor = if io::exists(&stage)? {
                Some(proof::directory(&stage, true)?)
            } else {
                None
            };
            let mut plan = Self {
                tool,
                payload,
                target,
                parent,
                transaction,
                backup,
                stage_anchor,
                journal,
                locator,
                original: None,
                stage_health: None,
                old_resources: Vec::new(),
                phase: TargetPhase::Staging,
                created_at: now,
            };
            let layout = plan.layout()?;
            if layout == Layout::Published {
                plan.phase = TargetPhase::PublishedPending;
            }
            plan.load_original(layout, now)?;
            return Ok(plan);
        }
        if recovery_only {
            return Err(TargetError::RecoveryRequired);
        }
        if registered && !io::exists(&target)? {
            return Err(TargetError::ManualMigrationRequired);
        }
        io::require_space(parent.path(), payload.total_bytes())?;
        let original = if io::exists(&target)? {
            let executable = existing_executable(&target, &tool)?;
            Some(proof::program_tree(&executable, &tool, None, now)?)
        } else {
            None
        };
        if let Some((tree, version)) = &original {
            check_preserved_namespace(tree.path(), &tool, version)?;
        }
        if original
            .as_ref()
            .is_some_and(|(_, version)| payload.version() < version)
        {
            return Err(TargetError::Downgrade);
        }
        let transaction =
            proof::create_directory(&parent.path().join(format!("{prefix}{}", io::nonce()?)))?;
        let stage_anchor = proof::create_directory(&transaction.path().join("staging"))?;
        let backup = proof::create_directory(&transaction.path().join("backup"))?;
        let journal = Journal {
            schema_version: 1,
            tool_id: tool.id.clone(),
            app_id: tool.app_id.clone(),
            version: payload.version().to_string(),
            source_sha256: payload.source_sha256,
            selection_sha256: payload.selection_sha256,
            target: target
                .to_str()
                .ok_or(TargetError::UnsupportedTarget)?
                .to_string(),
            parent: parent.id().map_err(proof::map_permission)?,
            original: original
                .as_ref()
                .map(|(tree, _)| tree.id())
                .transpose()
                .map_err(proof::map_permission)?,
            original_version: original.as_ref().map(|(_, version)| version.to_string()),
            staging: stage_anchor.id().map_err(proof::map_permission)?,
            phase: Phase::Staging,
            sequence: 1,
        };
        if journal.staging.volume != journal.parent.volume
            || journal
                .original
                .is_some_and(|id| id.volume != journal.parent.volume)
        {
            return Err(TargetError::UnsafeTarget);
        }
        io::create_bytes(
            &stage_anchor
                .path()
                .join(fbforge_suite_coordination::PENDING_INSTALLATION_MARKER),
            b"FBForge installer pending v1\n",
        )?;
        let original = original.map(|(tree, _)| tree);
        let old_resources = resources(original.as_ref())?;
        let mut plan = Self {
            tool,
            payload,
            target,
            parent,
            transaction,
            backup,
            stage_anchor: Some(stage_anchor),
            journal,
            locator,
            original,
            stage_health: None,
            old_resources,
            phase: TargetPhase::Staging,
            created_at: now,
        };
        plan.save(Phase::Staging)?;
        let saved = Locator {
            schema_version: 1,
            tool_id: plan.tool.id.clone(),
            app_id: plan.tool.app_id.clone(),
            transaction: plan
                .transaction
                .path()
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or(TargetError::UnsafeTarget)?
                .to_string(),
        };
        io::create_bytes(
            &plan.locator,
            &serde_json::to_vec(&saved).map_err(|_| TargetError::OperatingSystem)?,
        )?;
        Ok(plan)
    }
    pub(super) fn phase(&self) -> TargetPhase {
        self.phase
    }
    pub(super) fn target_directory(&self) -> &Path {
        &self.target
    }
    pub(super) fn staging_directory(&self) -> Result<&Path, TargetError> {
        if self.phase != TargetPhase::Staging || self.layout()? == Layout::Published {
            return Err(TargetError::InvalidPhase);
        }
        self.stage_anchor
            .as_ref()
            .map(|anchor| anchor.path())
            .or_else(|| self.stage_health.as_ref().map(|tree| tree.path()))
            .ok_or(TargetError::RecoveryRequired)
    }
    pub(super) fn write_payload_file(
        &mut self,
        path: &str,
        reader: &mut impl Read,
    ) -> Result<(), TargetError> {
        let entry = self
            .payload
            .files()
            .iter()
            .find(|entry| entry.path() == path)
            .ok_or(TargetError::InvalidManifest)?;
        let size = entry.size();
        let hash = *entry.sha256();
        let path = health::selected_path(self.staging_directory()?, path);
        self.ensure_stage_parents(&path)?;
        io::remove_private_file(&path)?;
        io::copy_bounded(proof::create_file(&path)?, reader, size, Some(hash))
    }
    pub(super) fn write_uninstaller(&mut self, reader: &mut impl Read) -> Result<(), TargetError> {
        let path = self.staging_directory()?.join("uninstall.exe");
        io::remove_private_file(&path)?;
        io::copy_bounded(proof::create_file(&path)?, reader, 64 * 1024 * 1024, None)
    }
    pub(super) fn publish(&mut self) -> Result<TargetPhase, TargetError> {
        self.check_clock()?;
        if self.phase != TargetPhase::Staging {
            return Err(TargetError::InvalidPhase);
        }
        let mut layout = self.layout()?;
        if layout == Layout::Published {
            self.phase = TargetPhase::PublishedPending;
            return Ok(self.phase);
        }
        let stage = self.staging_directory()?.to_path_buf();
        let inspected_at = native_time()?;
        let tree = health::inspect(&stage, &self.tool, &self.payload, inspected_at, false)?;
        require_current_time(inspected_at)?;
        if tree.id().map_err(proof::map_permission)? != self.journal.staging {
            return Err(TargetError::RecoveryRequired);
        }
        self.stage_health = Some(tree);
        self.save(Phase::Verified)?;
        // All our file proofs, including the separately checked root EXE, are
        // closed before the complete original-tree RM query. No self exclusions.
        if let Some(original) = &mut self.original {
            original.close_files();
            original
                .recheck_directories()
                .map_err(proof::map_permission)?;
            let current_root = if layout == Layout::OriginalIntact {
                self.target.clone()
            } else {
                self.backup.path().join("root")
            };
            self.old_resources = resources_at(original, &current_root)?;
        }
        let stage_health = self
            .stage_health
            .as_mut()
            .ok_or(TargetError::RecoveryRequired)?;
        stage_health.close_files();
        stage_health
            .recheck_directories()
            .map_err(proof::map_permission)?;
        if !self.old_resources.is_empty() {
            match inspect_resource_users(&self.old_resources) {
                ResourceUsers::NoUsersReported => {}
                ResourceUsers::InUse => return Err(TargetError::ResourcesInUse),
                ResourceUsers::Unknown(_) => return Err(TargetError::InspectionUnavailable),
            }
        }
        require_current_time(inspected_at)?;
        if self.layout()? != layout {
            return Err(TargetError::RecoveryRequired);
        }
        self.check_original_namespace(layout)?;
        // The initial creation anchor was no-delete; the freshly checked
        // stage_health directory anchors now deliberately share deletion.
        drop(self.stage_anchor.take());
        if layout == Layout::OriginalIntact && self.journal.original.is_some() {
            self.save(Phase::MoveOldIntent)?;
            require_current_time(inspected_at)?;
            io::rename_directory(&self.target, &self.backup.path().join("root"))?;
            layout = self.layout()?;
            if layout != Layout::OriginalBackedUp {
                return Err(TargetError::RecoveryRequired);
            }
        }
        self.save(Phase::PublishIntent)?;
        // This method is reached only through the live !Send Lease borrow and
        // native owner-thread check. There is no worker handoff between RM/swap.
        require_current_time(inspected_at)?;
        io::rename_directory(&stage, &self.target)?;
        if self.layout()? != Layout::Published {
            return Err(TargetError::RecoveryRequired);
        }
        self.phase = TargetPhase::PublishedPending;
        self.save(Phase::Published)?;
        Ok(self.phase)
    }
    pub(super) fn finalize(&mut self) -> Result<TargetPhase, TargetError> {
        self.check_clock()?;
        if self.phase != TargetPhase::PublishedPending || self.layout()? != Layout::Published {
            return Err(TargetError::InvalidPhase);
        }
        self.check_original_namespace(Layout::Published)?;
        let inspected_at = native_time()?;
        let mut current =
            health::inspect(&self.target, &self.tool, &self.payload, inspected_at, true)?;
        require_current_time(inspected_at)?;
        if current.id().map_err(proof::map_permission)? != self.journal.staging {
            return Err(TargetError::RecoveryRequired);
        }
        let found = discover_tool(&self.tool);
        if !found.issues.is_empty()
            || found.installations.len() != 1
            || normal_root(&found.installations[0].root)? != self.target
            || found.installations[0].reported_version.as_deref()
                != Some(self.journal.version.as_str())
            || found.installations[0].sources.is_empty()
            || found.installations[0]
                .sources
                .iter()
                .any(|source| source.hive != RegistryHive::CurrentUser)
        {
            return Err(TargetError::RecoveryRequired);
        }
        current.close_files();
        let receipt = serde_json::json!({"schemaVersion":1,"toolId":self.tool.id,"appId":self.tool.app_id,"version":self.journal.version,"sourceSha256":io::hex(&self.payload.source_sha256),"selectionSha256":io::hex(&self.payload.selection_sha256),"files":self.payload.files().iter().map(|file|serde_json::json!({"path":file.path(),"size":file.size(),"sha256":io::hex(file.sha256())})).collect::<Vec<_>>()});
        let bytes = serde_json::to_vec(&receipt).map_err(|_| TargetError::OperatingSystem)?;
        let receipt_path = self.target.join(journal::RECEIPT);
        if io::exists(&receipt_path)? {
            if io::read(&receipt_path, manifest::MAX_MANIFEST_BYTES + 65536)? != bytes {
                return Err(TargetError::RecoveryRequired);
            }
        } else {
            io::create_bytes(&receipt_path, &bytes)?;
        }
        require_current_time(inspected_at)?;
        io::remove_private_file(
            &self
                .target
                .join(fbforge_suite_coordination::PENDING_INSTALLATION_MARKER),
        )?;
        self.save(Phase::Complete)?;
        io::remove_private_file(&self.locator)?;
        self.phase = TargetPhase::Complete;
        Ok(self.phase)
    }
    fn load_original(&mut self, layout: Layout, now: u64) -> Result<(), TargetError> {
        if let Some(expected) = self.journal.original {
            let root = if layout == Layout::OriginalIntact {
                self.target.clone()
            } else {
                self.backup.path().join("root")
            };
            let executable = existing_executable(&root, &self.tool)?;
            let (tree, version) = proof::program_tree(&executable, &self.tool, None, now)?;
            if tree.id().map_err(proof::map_permission)? != expected
                || self.journal.original_version.as_deref() != Some(version.to_string().as_str())
            {
                return Err(TargetError::RecoveryRequired);
            }
            if self.payload.version() < &version {
                return Err(TargetError::Downgrade);
            }
            check_preserved_namespace(tree.path(), &self.tool, &version)?;
            self.old_resources = resources(Some(&tree))?;
            self.original = Some(tree);
        }
        Ok(())
    }
    fn check_original_namespace(&self, layout: Layout) -> Result<(), TargetError> {
        if self.original.is_some() {
            let root = if layout == Layout::OriginalIntact {
                self.target.clone()
            } else {
                self.backup.path().join("root")
            };
            let version = manifest::canonical_version(
                self.journal
                    .original_version
                    .as_deref()
                    .ok_or(TargetError::RecoveryRequired)?,
            )?;
            check_preserved_namespace(&root, &self.tool, &version)?;
        }
        Ok(())
    }
    fn layout(&self) -> Result<Layout, TargetError> {
        if self.parent.id().map_err(proof::map_permission)? != self.journal.parent {
            return Err(TargetError::RecoveryRequired);
        }
        journal::inspect_layout(
            &self.journal,
            presence(&self.target)?,
            presence(&self.backup.path().join("root"))?,
            presence(&self.transaction.path().join("staging"))?,
        )
    }
    fn ensure_stage_parents(&self, path: &Path) -> Result<(), TargetError> {
        let root = self.staging_directory()?;
        let relative = path
            .parent()
            .ok_or(TargetError::UnsafeTarget)?
            .strip_prefix(root)
            .map_err(|_| TargetError::UnsafeTarget)?;
        let mut parent = root.to_path_buf();
        for part in relative.components() {
            parent.push(part);
            if io::exists(&parent)? {
                drop(proof::directory(&parent, true)?);
            } else {
                drop(proof::create_directory(&parent)?);
            }
        }
        Ok(())
    }
    fn save(&mut self, phase: Phase) -> Result<(), TargetError> {
        self.transaction
            .recheck_private()
            .map_err(proof::map_permission)?;
        self.journal.phase = phase;
        self.journal.sequence = self
            .journal
            .sequence
            .checked_add(1)
            .ok_or(TargetError::RecoveryRequired)?;
        io::replace_control(
            &self.transaction.path().join("journal.v1.json"),
            &serde_json::to_vec(&self.journal).map_err(|_| TargetError::OperatingSystem)?,
        )
    }
    fn check_clock(&self) -> Result<(), TargetError> {
        if native_time()? < self.created_at {
            Err(TargetError::InspectionUnavailable)
        } else {
            Ok(())
        }
    }
}

fn resources(tree: Option<&proof::MutationTree>) -> Result<Vec<String>, TargetError> {
    tree.map(|tree| {
        tree.file_paths()
            .into_iter()
            .map(|path| {
                path.into_os_string()
                    .into_string()
                    .map_err(|_| TargetError::UnsupportedTarget)
            })
            .collect()
    })
    .unwrap_or(Ok(Vec::new()))
}
fn resources_at(tree: &proof::MutationTree, root: &Path) -> Result<Vec<String>, TargetError> {
    tree.file_paths()
        .into_iter()
        .map(|path| {
            let relative = path
                .strip_prefix(tree.path())
                .map_err(|_| TargetError::UnsafeTarget)?;
            root.join(relative)
                .into_os_string()
                .into_string()
                .map_err(|_| TargetError::UnsupportedTarget)
        })
        .collect()
}
fn presence(path: &Path) -> Result<Option<ObjectId>, TargetError> {
    if !io::exists(path)? {
        return Ok(None);
    }
    Ok(Some(
        proof::directory(path, true)?
            .id()
            .map_err(proof::map_permission)?,
    ))
}
fn existing_executable(root: &Path, tool: &ToolManifest) -> Result<PathBuf, TargetError> {
    let mut found = None;
    for name in &tool.executables {
        let path = root.join(name);
        if io::exists(&path)? {
            if found.is_some() {
                return Err(TargetError::AmbiguousTarget);
            }
            found = Some(path);
        }
    }
    found.ok_or(TargetError::ManualMigrationRequired)
}
fn native_time() -> Result<u64, TargetError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|time| time.as_secs())
        .map_err(|_| TargetError::InspectionUnavailable)
}
fn valid_transaction_name(name: &str, prefix: &str) -> bool {
    name.strip_prefix(prefix).is_some_and(|nonce| {
        nonce.len() == 32
            && nonce
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}
fn discover_target(tool: &ToolManifest) -> Result<(PathBuf, Vec<String>), TargetError> {
    let found = discover_tool(tool);
    if !found.issues.is_empty() || found.installations.len() > 1 {
        return Err(TargetError::AmbiguousTarget);
    }
    if let Some(installation) = found.installations.into_iter().next() {
        if installation.sources.is_empty()
            || installation
                .sources
                .iter()
                .any(|source| source.hive != RegistryHive::CurrentUser)
        {
            return Err(TargetError::UnsupportedTarget);
        }
        let registered_roots = installation
            .sources
            .iter()
            .map(|source| {
                crate::discovery::read_registered_current_user_root(source)
                    .map_err(|_| TargetError::InspectionUnavailable)
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok((normal_root(&installation.root)?, registered_roots));
    }
    let mut raw = ptr::null_mut();
    // SAFETY: fixed known-folder identity, current token, and initialized output.
    let status = unsafe {
        SHGetKnownFolderPath(
            &FOLDERID_LocalAppData,
            KF_FLAG_DONT_VERIFY as u32,
            0,
            &mut raw,
        )
    };
    struct Folder(*mut u16);
    impl Drop for Folder {
        fn drop(&mut self) {
            // SAFETY: SHGetKnownFolderPath allocates this output with the COM allocator.
            unsafe { CoTaskMemFree(self.0.cast()) };
        }
    }
    let allocation = Folder(raw);
    if status != 0 || raw.is_null() {
        return Err(TargetError::UnsupportedTarget);
    }
    // SAFETY: the successful SDK result is terminated; the scan is bounded.
    let mut length = 0;
    while length < 4096 && unsafe { *raw.add(length) } != 0 {
        length += 1;
    }
    if length == 4096 {
        return Err(TargetError::UnsupportedTarget);
    }
    // SAFETY: preceding scan established the live non-NUL UTF16 extent.
    let root = String::from_utf16(unsafe { std::slice::from_raw_parts(raw, length) })
        .map_err(|_| TargetError::UnsupportedTarget)?;
    drop(allocation);
    crate::discovery::validate_windows_location(&root)
        .map_err(|_| TargetError::UnsupportedTarget)?;
    let path = Path::new(&root).join(&tool.product_name);
    crate::discovery::validate_windows_location(
        path.to_str().ok_or(TargetError::UnsupportedTarget)?,
    )
    .map_err(|_| TargetError::UnsupportedTarget)?;
    if path.as_os_str().encode_wide().count() > 320 {
        return Err(TargetError::UnsupportedTarget);
    }
    Ok((path, Vec::new()))
}

fn ordinal_equal(left: &[u16], right: &[u16]) -> Result<bool, TargetError> {
    let left_count = i32::try_from(left.len()).map_err(|_| TargetError::UnsupportedTarget)?;
    let right_count = i32::try_from(right.len()).map_err(|_| TargetError::UnsupportedTarget)?;
    // SAFETY: both bounded UTF16 slices remain live throughout the SDK call.
    // Explicit counts avoid NUL assumptions; TRUE must be exactly 1.
    match unsafe { CompareStringOrdinal(left.as_ptr(), left_count, right.as_ptr(), right_count, 1) }
    {
        CSTR_EQUAL => Ok(true),
        CSTR_LESS_THAN | CSTR_GREATER_THAN => Ok(false),
        _ => Err(TargetError::InspectionUnavailable),
    }
}

fn normal_root(path: &Path) -> Result<PathBuf, TargetError> {
    let text = path.to_str().ok_or(TargetError::UnsupportedTarget)?;
    let text = text.strip_prefix(r"\\?\").unwrap_or(text);
    if crate::discovery::validate_windows_location(text).as_deref() != Ok(text) {
        return Err(TargetError::UnsupportedTarget);
    }
    Ok(PathBuf::from(text))
}

fn require_current_time(verified_at: u64) -> Result<(), TargetError> {
    if native_time()? < verified_at {
        Err(TargetError::InspectionUnavailable)
    } else {
        Ok(())
    }
}

fn check_preserved_namespace(
    root: &Path,
    tool: &ToolManifest,
    version: &semver::Version,
) -> Result<(), TargetError> {
    let receipt = io::read(
        &root.join(journal::RECEIPT),
        super::preservation::MAX_RECEIPT_BYTES,
    )
    .map_err(|_| TargetError::ManualMigrationRequired)?;
    super::preservation::check_existing(root, &receipt, tool, version)
}
