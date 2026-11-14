//! Read-only admission of a normal installation tree, not private-cache ACLs.
use super::{file_information, final_path, open_read_only, LockedPackageFile};
use crate::authenticode::{
    program_permissions::{
        allows_owner_rights, check_ace, check_temporary_ace, ScanBudget, Scope, MAX_ACES,
        MAX_ANCESTORS,
    },
    ProgramStartFailure as Failure,
};
use std::{
    collections::HashSet,
    ffi::c_void,
    fs::{self, File},
    mem::size_of,
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::{Path, PathBuf},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, HANDLE},
    Security::{
        AllocateAndInitializeSid,
        Authorization::{GetSecurityInfo, SE_FILE_OBJECT},
        CopySid, EqualSid, FreeSid, GetAce, GetLengthSid, GetSecurityDescriptorControl,
        GetSecurityDescriptorDacl, GetSecurityDescriptorLength, GetTokenInformation, IsValidAcl,
        IsValidSecurityDescriptor, IsValidSid, IsWellKnownSid, TokenUser,
        WinBuiltinAdministratorsSid, WinLocalSystemSid, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, SECURITY_NT_AUTHORITY, SE_DACL_PROTECTED, TOKEN_QUERY,
        TOKEN_USER,
    },
    Storage::FileSystem::{
        GetFileType, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_TYPE_DISK,
    },
    System::{
        SystemServices::{
            SECURITY_SERVICE_ID_BASE_RID, SECURITY_SERVICE_ID_RID_COUNT,
            SECURITY_TRUSTED_INSTALLER_RID1, SECURITY_TRUSTED_INSTALLER_RID2,
            SECURITY_TRUSTED_INSTALLER_RID3, SECURITY_TRUSTED_INSTALLER_RID4,
            SECURITY_TRUSTED_INSTALLER_RID5,
        },
        Threading::{GetCurrentProcess, OpenProcessToken},
    },
};

pub(in crate::authenticode) struct LockedProgramTree {
    _entries: Vec<File>,
}

pub(super) fn lock_program_tree(program: &LockedPackageFile) -> Result<LockedProgramTree, Failure> {
    let mut budget = ScanBudget::new();
    let user = TrustedPrincipals::current()?;
    let root = program
        ._directories
        .last()
        .ok_or(Failure::ProgramUnavailable)?;
    let path = held_path(root)?;
    if held_path(&program.file)?.parent() != Some(path.as_path()) {
        return Err(Failure::UnsafeInstallation);
    }
    budget.charge_object(0, path.as_os_str().encode_wide().count())?;
    let mut identities = HashSet::new();
    if !inspect_object(root, &mut identities)? {
        return Err(Failure::UnsafeInstallation);
    }
    inspect_security(root, Scope::ProgramTree, &user, &mut budget)?;
    inspect_ancestor_namespace(program, &user, &mut budget)?;

    let mut entries = Vec::new();
    let mut pending = Vec::new();
    queue_children(&path, 1, &mut pending, &mut budget)?;
    while let Some((path, depth)) = pending.pop() {
        budget.check_time()?;
        let file = open_read_only(&path, FILE_FLAG_BACKUP_SEMANTICS)
            .map_err(|_| Failure::ProgramUnavailable)?;
        let directory = inspect_object(&file, &mut identities)?;
        inspect_security(&file, Scope::ProgramTree, &user, &mut budget)?;
        if directory {
            queue_children(&path, depth + 1, &mut pending, &mut budget)?;
        }
        entries.push(file);
    }
    budget.check_time()?;
    Ok(LockedProgramTree { _entries: entries })
}

fn inspect_ancestor_namespace(
    program: &LockedPackageFile,
    user: &TrustedPrincipals,
    budget: &mut ScanBudget,
) -> Result<(), Failure> {
    if program._directories.len() > MAX_ANCESTORS {
        return Err(Failure::UnsupportedInstallation);
    }
    for ancestor in program._directories.iter().rev().skip(1) {
        // The child can load sidecars later, after our handles are closed. Check
        // every ancestor, including above a protected profile DACL: no foreign
        // rename/delete, reparse/attribute or inherited-security manipulation.
        // Creation of unrelated siblings alone does not violate this condition.
        inspect_security(ancestor, Scope::Ancestor, user, budget)?;
    }
    Ok(())
}

fn queue_children(
    path: &Path,
    depth: usize,
    pending: &mut Vec<(PathBuf, usize)>,
    budget: &mut ScanBudget,
) -> Result<(), Failure> {
    budget.check_time()?;
    let children = fs::read_dir(path).map_err(|_| Failure::ProgramUnavailable)?;
    for child in children {
        budget.check_time()?;
        let path = child.map_err(|_| Failure::ProgramUnavailable)?.path();
        // Charge before queuing: enumeration cannot allocate an unbounded list.
        budget.charge_object(depth, path.as_os_str().encode_wide().count())?;
        pending.push((path, depth));
    }
    budget.check_time()
}

fn inspect_object(file: &File, identities: &mut HashSet<(u32, u32, u32)>) -> Result<bool, Failure> {
    let info = file_information(file).map_err(|_| Failure::ProgramUnavailable)?;
    let directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0;
    // SAFETY: file owns its live disk-object handle throughout this query.
    if unsafe { GetFileType(file.as_raw_handle() as HANDLE) } != FILE_TYPE_DISK
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (!directory && info.nNumberOfLinks != 1)
        || !identities.insert((
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
        ))
    {
        return Err(Failure::UnsafeInstallation);
    }
    Ok(directory)
}

fn held_path(file: &File) -> Result<PathBuf, Failure> {
    let path = final_path(file).map_err(|_| Failure::ProgramUnavailable)?;
    String::from_utf16(&path[..path.len() - 1])
        .map(PathBuf::from)
        .map_err(|_| Failure::UnsupportedInstallation)
}

struct Allocation(*mut c_void);
impl Drop for Allocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: GetSecurityInfo allocates this descriptor with LocalAlloc.
            unsafe { LocalFree(self.0) };
        }
    }
}
struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns a successful OpenProcessToken result.
        unsafe { CloseHandle(self.0) };
    }
}

struct ServiceSid(*mut c_void);
impl ServiceSid {
    fn trusted_installer() -> Result<Self, Failure> {
        let mut sid = ptr::null_mut();
        // SDK-named TrustedInstaller RIDs only. This constructs one exact
        // service SID locally; there is no name lookup or service-wide trust.
        // SAFETY: fixed SDK authority/RIDs and a valid initialized output slot.
        if unsafe {
            AllocateAndInitializeSid(
                &SECURITY_NT_AUTHORITY,
                SECURITY_SERVICE_ID_RID_COUNT as u8,
                SECURITY_SERVICE_ID_BASE_RID as u32,
                SECURITY_TRUSTED_INSTALLER_RID1,
                SECURITY_TRUSTED_INSTALLER_RID2,
                SECURITY_TRUSTED_INSTALLER_RID3,
                SECURITY_TRUSTED_INSTALLER_RID4,
                SECURITY_TRUSTED_INSTALLER_RID5,
                0,
                0,
                &mut sid,
            )
        } == 0
        {
            return Err(Failure::ProgramUnavailable);
        }
        Ok(Self(sid))
    }
}
impl Drop for ServiceSid {
    fn drop(&mut self) {
        // SAFETY: AllocateAndInitializeSid returned this owned SID allocation.
        unsafe { FreeSid(self.0) };
    }
}

struct TrustedPrincipals {
    user_sid: Vec<u32>,
    installer_sid: ServiceSid,
}
impl TrustedPrincipals {
    fn current() -> Result<Self, Failure> {
        let mut token = 0;
        // SAFETY: valid pseudo-process and writable handle output, no privileges enabled.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(Failure::ProgramUnavailable);
        }
        let token = Token(token);
        let mut bytes = 0;
        // SAFETY: null/zero requests the required TokenUser buffer size only.
        unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut bytes) };
        if bytes < size_of::<TOKEN_USER>() as u32 || bytes > 4096 {
            return Err(Failure::UnsupportedInstallation);
        }
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        let capacity = (buffer.len() * size_of::<usize>()) as u32;
        // SAFETY: aligned storage spans the full declared capacity and stays live.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                capacity,
                &mut bytes,
            )
        } == 0
            || bytes > capacity
        {
            return Err(Failure::ProgramUnavailable);
        }
        // SAFETY: the successful bounded OS response contains an aligned TOKEN_USER.
        let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        // SAFETY: OS-owned TokenUser SID is backed by the still-live buffer.
        if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
            return Err(Failure::UnsupportedInstallation);
        }
        // SAFETY: SID validity was established while its buffer remains live.
        let size = unsafe { GetLengthSid(sid) };
        if !(8..=68).contains(&size) {
            return Err(Failure::UnsupportedInstallation);
        }
        let mut copy = vec![0u32; (size as usize).div_ceil(size_of::<u32>())];
        // SAFETY: aligned destination has at least size bytes and source is valid.
        if unsafe { CopySid(size, copy.as_mut_ptr().cast(), sid) } == 0 {
            return Err(Failure::ProgramUnavailable);
        }
        Ok(Self {
            user_sid: copy,
            installer_sid: ServiceSid::trusted_installer()?,
        })
    }

    fn require_trusted_creator_owner(&self) -> Result<(), Failure> {
        use windows_sys::Win32::Security::{TokenOwner, TOKEN_OWNER};
        let mut raw = 0;
        // SAFETY: read the current primary process token used for child creation.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
            return Err(Failure::ProgramUnavailable);
        }
        let token = Token(raw);
        let mut bytes = 0;
        // SAFETY: null/zero requests only the bounded owner-response size.
        unsafe { GetTokenInformation(token.0, TokenOwner, ptr::null_mut(), 0, &mut bytes) };
        if bytes < size_of::<TOKEN_OWNER>() as u32 || bytes > 4096 {
            return Err(Failure::UnsupportedInstallation);
        }
        let mut buffer = vec![0usize; (bytes as usize).div_ceil(size_of::<usize>())];
        let capacity = (buffer.len() * size_of::<usize>()) as u32;
        // SAFETY: live aligned storage spans the full declared output capacity.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenOwner,
                buffer.as_mut_ptr().cast(),
                capacity,
                &mut bytes,
            )
        } == 0
            || bytes > capacity
            || bytes < size_of::<TOKEN_OWNER>() as u32
        {
            return Err(Failure::ProgramUnavailable);
        }
        // SAFETY: the successful bounded response contains TOKEN_OWNER.
        let owner = unsafe { (*buffer.as_ptr().cast::<TOKEN_OWNER>()).Owner };
        let start = buffer.as_ptr() as usize;
        let end = start + bytes as usize;
        if !within(owner, 8, start, end) {
            return Err(Failure::UnsupportedInstallation);
        }
        // SAFETY: SID prefix lies in the live owner response; validate_sid bounds the remainder.
        unsafe { validate_sid(owner, end) }?;
        // SAFETY: SID is complete and valid for the remainder of this call.
        if !unsafe { self.contains(owner) } {
            return Err(Failure::UnsupportedInstallation);
        }
        Ok(())
    }

    // The caller supplies a validated SID within a live security descriptor.
    unsafe fn contains(&self, sid: *mut c_void) -> bool {
        // SAFETY: both SIDs are valid and their backing allocations remain live.
        unsafe {
            EqualSid(sid, self.user_sid.as_ptr().cast_mut().cast()) != 0
                || IsWellKnownSid(sid, WinLocalSystemSid) != 0
                || IsWellKnownSid(sid, WinBuiltinAdministratorsSid) != 0
                || EqualSid(sid, self.installer_sid.0) != 0
        }
    }
}

fn inspect_security(
    file: &File,
    scope: Scope,
    user: &TrustedPrincipals,
    budget: &mut ScanBudget,
) -> Result<bool, Failure> {
    budget.check_time()?;
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut allocation = Allocation(ptr::null_mut());
    // SAFETY: file remains live; all output slots point to initialized storage.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle() as HANDLE,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut allocation.0,
        )
    };
    if status != ERROR_SUCCESS || allocation.0.is_null() {
        return Err(Failure::ProgramUnavailable);
    }
    inspect_descriptor(&allocation, scope, user, budget)
}

fn inspect_descriptor(
    allocation: &Allocation,
    scope: Scope,
    user: &TrustedPrincipals,
    budget: &mut ScanBudget,
) -> Result<bool, Failure> {
    // SAFETY: successful GetSecurityInfo returns an allocated OS descriptor.
    if unsafe { IsValidSecurityDescriptor(allocation.0) } == 0 {
        return Err(Failure::UnsupportedInstallation);
    }
    // SAFETY: descriptor validity was established above.
    let length = unsafe { GetSecurityDescriptorLength(allocation.0) } as usize;
    budget.charge_descriptor(length)?;
    let start = allocation.0 as usize;
    let end = start
        .checked_add(length)
        .ok_or(Failure::UnsupportedInstallation)?;
    let mut control = 0;
    let mut revision = 0;
    let mut present = 0;
    let mut defaulted = 0;
    let mut checked_acl = ptr::null_mut();
    let mut owner = ptr::null_mut();
    let mut owner_defaulted = 0;
    // SAFETY: all outputs have live correctly typed storage and descriptor is valid.
    if unsafe { GetSecurityDescriptorControl(allocation.0, &mut control, &mut revision) } == 0
        || unsafe {
            windows_sys::Win32::Security::GetSecurityDescriptorOwner(
                allocation.0,
                &mut owner,
                &mut owner_defaulted,
            )
        } == 0
        || unsafe {
            GetSecurityDescriptorDacl(allocation.0, &mut present, &mut checked_acl, &mut defaulted)
        } == 0
        || revision != 1
        || present == 0
        || checked_acl.is_null()
        || !within(owner, 8, start, end)
        || !within(checked_acl.cast(), size_of::<ACL>(), start, end)
    {
        return Err(Failure::UnsupportedInstallation);
    }
    let acl = checked_acl;
    // SAFETY: SID begins in the live descriptor and validate_sid checks its extent.
    unsafe { validate_sid(owner, end) }?;
    // SAFETY: the preceding validation established a complete, valid owner SID.
    let owner_is_trusted = unsafe { user.contains(owner) };
    if !owner_is_trusted {
        return Err(match scope {
            Scope::ProgramTree | Scope::InstallerHost | Scope::PrivateTree | Scope::Temporary => {
                Failure::UnsafeInstallation
            }
            Scope::Ancestor | Scope::InstallerAncestor => Failure::UnsupportedInstallation,
        });
    }
    // SAFETY: the fixed ACL header is inside the live descriptor.
    let header = unsafe { &*acl };
    if !within(acl.cast(), usize::from(header.AclSize), start, end)
        || usize::from(header.AceCount) > MAX_ACES
        || !matches!(header.AclRevision, 2 | 4)
        // SAFETY: the entire declared ACL is within its descriptor.
        || unsafe { IsValidAcl(acl) } == 0
    {
        return Err(Failure::UnsupportedInstallation);
    }
    let acl_end = acl as usize + usize::from(header.AclSize);
    let temporary = matches!(scope, Scope::Temporary);
    if temporary {
        user.require_trusted_creator_owner()?;
    }
    let mut child_propagation = false;
    for index in 0..u32::from(header.AceCount) {
        budget.check_time()?;
        let mut ace = ptr::null_mut();
        // SAFETY: index is below the validated ACL's declared ACE count.
        if unsafe { GetAce(acl, index, &mut ace) } == 0
            || !within(ace, size_of::<ACE_HEADER>(), acl as usize, acl_end)
        {
            return Err(Failure::UnsupportedInstallation);
        }
        // SAFETY: a complete ACE_HEADER is within the validated ACL.
        let header = unsafe { &*ace.cast::<ACE_HEADER>() };
        let length = usize::from(header.AceSize);
        if length < 16
            || !within(ace, length, acl as usize, acl_end)
            || !matches!(header.AceType, 0 | 1)
        {
            return Err(Failure::UnsupportedInstallation);
        }
        // Ordinary allow/deny ACE layout is header, u32 mask, then SID.
        // SAFETY: the declared ACE spans the 4-byte header, mask and SID prefix.
        let mask = unsafe { ptr::read_unaligned(ace.cast::<u8>().add(4).cast::<u32>()) };
        // SAFETY: the first eight SID bytes are within the checked ACE.
        let sid = unsafe { ace.cast::<u8>().add(8).cast::<c_void>() };
        // SAFETY: validate_sid verifies the remainder before any OS SID operation.
        unsafe { validate_sid(sid, ace as usize + length) }?;
        // SAFETY: SID is valid and all backing allocations are still live.
        let host_scope = matches!(scope, Scope::InstallerHost | Scope::InstallerAncestor);
        // SAFETY: this SID is complete and remains in the live descriptor. The
        // owner was positively bound above before any OW mapping can apply.
        let owner_rights = host_scope
            && unsafe {
                IsWellKnownSid(sid, windows_sys::Win32::Security::WinCreatorOwnerRightsSid) != 0
            };
        let trusted = unsafe { user.contains(sid) }
            || allows_owner_rights(scope, owner_is_trusted, owner_rights);
        if temporary {
            // SAFETY: this complete validated SID remains in the live descriptor.
            let creator_owner = unsafe {
                IsWellKnownSid(sid, windows_sys::Win32::Security::WinCreatorOwnerSid) != 0
            };
            child_propagation |= check_temporary_ace(
                header.AceType,
                header.AceFlags,
                mask,
                trusted,
                creator_owner,
            )?;
        } else {
            check_ace(scope, header.AceType, header.AceFlags, mask, trusted)?;
        }
    }
    budget.check_time()?;
    if temporary && !child_propagation {
        return Err(Failure::UnsupportedInstallation);
    }
    Ok(control & SE_DACL_PROTECTED != 0)
}

fn within(pointer: *mut c_void, bytes: usize, start: usize, end: usize) -> bool {
    let address = pointer as usize;
    address >= start && address.checked_add(bytes).is_some_and(|limit| limit <= end)
}

// Caller establishes the first 8 SID bytes are within the indicated allocation.
unsafe fn validate_sid(sid: *mut c_void, end: usize) -> Result<(), Failure> {
    // SAFETY: the second byte belongs to the caller-validated SID prefix.
    let count = unsafe { *sid.cast::<u8>().add(1) } as usize;
    if count > 15 || !within(sid, 8 + count * 4, sid as usize, end)
        // SAFETY: the computed complete SID extent is within its live allocation.
        || unsafe { IsValidSid(sid) } == 0
    {
        return Err(Failure::UnsupportedInstallation);
    }
    Ok(())
}

#[cfg(test)]
mod tests;

// Mutation keeps the existing start policy separate: a shared read-only tree
// may be startable, but never becomes a private writable installation target.
pub(crate) struct MutationTree {
    pub(super) root: DirectoryAnchor,
    directories: Vec<File>,
    files: Vec<(PathBuf, File)>,
    file_paths: Vec<PathBuf>,
}
pub(crate) struct DirectoryAnchor {
    file: File,
    _ancestors: Vec<File>,
    path: PathBuf,
}
impl DirectoryAnchor {
    pub(crate) fn is_same_or_descendant_of(&self, root: &Self) -> Result<bool, Failure> {
        let root_volume = object_id(
            root._ancestors
                .first()
                .ok_or(Failure::UnsupportedInstallation)?,
        )?;
        let own_volume = object_id(
            self._ancestors
                .first()
                .ok_or(Failure::UnsupportedInstallation)?,
        )?;
        // A partial SUBST root can omit the actual Windows parent from the
        // lexical ancestor chain. Do not mistake that truncated view for proof.
        if own_volume.volume == root_volume.volume && own_volume != root_volume {
            return Err(Failure::UnsupportedInstallation);
        }
        let expected = root.id()?;
        if self.id()? == expected {
            return Ok(true);
        }
        for ancestor in &self._ancestors {
            if object_id(ancestor)? == expected {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn id(&self) -> Result<crate::installer_targets::journal::ObjectId, Failure> {
        object_id(&self.file)
    }
    pub(crate) fn require_mutable_volume(&self) -> Result<(), Failure> {
        let mut flags = 0;
        let mut volume = 0;
        // SAFETY: the directory handle and scalar output buffers stay live.
        if unsafe {
            windows_sys::Win32::Storage::FileSystem::GetVolumeInformationByHandleW(
                self.file.as_raw_handle() as HANDLE,
                ptr::null_mut(),
                0,
                &mut volume,
                ptr::null_mut(),
                &mut flags,
                ptr::null_mut(),
                0,
            )
        } == 0
            || flags & windows_sys::Win32::System::SystemServices::FILE_PERSISTENT_ACLS == 0
            || flags & windows_sys::Win32::System::SystemServices::FILE_READ_ONLY_VOLUME != 0
            || volume != self.id()?.volume
        {
            return Err(Failure::UnsupportedInstallation);
        }
        Ok(())
    }
    pub(crate) fn recheck_temporary(&self) -> Result<(), Failure> {
        self.require_mutable_volume()?;
        let user = TrustedPrincipals::current()?;
        let mut budget = ScanBudget::new();
        inspect_security(&self.file, Scope::Temporary, &user, &mut budget)?;
        for file in &self._ancestors {
            inspect_security(file, Scope::Ancestor, &user, &mut budget)?;
        }
        Ok(())
    }
    pub(crate) fn recheck_private(&self) -> Result<(), Failure> {
        let user = TrustedPrincipals::current()?;
        let mut budget = ScanBudget::new();
        inspect_security(&self.file, Scope::PrivateTree, &user, &mut budget)?;
        for file in &self._ancestors {
            inspect_security(file, Scope::Ancestor, &user, &mut budget)?;
        }
        Ok(())
    }
}
impl MutationTree {
    pub(crate) fn path(&self) -> &Path {
        self.root.path()
    }
    pub(crate) fn id(&self) -> Result<crate::installer_targets::journal::ObjectId, Failure> {
        self.root.id()
    }
    pub(crate) fn files(&self) -> &[(PathBuf, File)] {
        &self.files
    }
    pub(crate) fn file_paths(&self) -> Vec<PathBuf> {
        self.file_paths.clone()
    }
    pub(crate) fn close_files(&mut self) {
        self.files.clear();
    }
    pub(crate) fn recheck_directories(&self) -> Result<(), Failure> {
        let user = TrustedPrincipals::current()?;
        let mut budget = ScanBudget::new();
        inspect_security(&self.root.file, Scope::PrivateTree, &user, &mut budget)?;
        for file in &self.directories {
            inspect_security(file, Scope::PrivateTree, &user, &mut budget)?;
        }
        for file in &self.root._ancestors {
            inspect_security(file, Scope::Ancestor, &user, &mut budget)?;
        }
        Ok(())
    }
}

pub(super) fn create_private_directory(path: &Path) -> Result<DirectoryAnchor, Failure> {
    use windows_sys::Win32::{
        Security::SECURITY_ATTRIBUTES, Storage::FileSystem::CreateDirectoryW,
    };
    let allocation = private_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: allocation.0,
        bInheritHandle: 0,
    };
    let path_text = mutation_wide(path)?;
    // SAFETY: the terminated path and initialized descriptor outlive this call.
    if unsafe { CreateDirectoryW(path_text.as_ptr(), &attributes) } == 0 {
        return Err(Failure::ProgramUnavailable);
    }
    let user = TrustedPrincipals::current()?;
    let anchor = lock_directory(path, true, false)?;
    if !inspect_security(
        &anchor.file,
        Scope::PrivateTree,
        &user,
        &mut ScanBudget::new(),
    )? {
        return Err(Failure::UnsupportedInstallation);
    }
    Ok(anchor)
}

fn private_descriptor() -> Result<Allocation, Failure> {
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
    };
    let user = TrustedPrincipals::current()?;
    let mut sid_text = ptr::null_mut();
    // SAFETY: current() copied a valid SID; the output receives LocalAlloc memory.
    if unsafe { ConvertSidToStringSidW(user.user_sid.as_ptr().cast_mut().cast(), &mut sid_text) }
        == 0
    {
        return Err(Failure::ProgramUnavailable);
    }
    let sid_allocation = Allocation(sid_text.cast());
    let mut length = 0;
    // SAFETY: the SDK returned a terminated SID string within its documented cap.
    while length < 256 && unsafe { *sid_text.add(length) } != 0 {
        length += 1;
    }
    if length == 256 {
        return Err(Failure::UnsupportedInstallation);
    }
    // SAFETY: the preceding bounded scan established these live code units.
    let sid = String::from_utf16(unsafe { std::slice::from_raw_parts(sid_text, length) })
        .map_err(|_| Failure::UnsupportedInstallation)?;
    let sddl: Vec<u16> = format!("O:{sid}D:P(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut allocation = Allocation(ptr::null_mut());
    // SAFETY: terminated SDDL and live output slot; Allocation owns the result.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut allocation.0,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(Failure::ProgramUnavailable);
    }
    drop(sid_allocation);
    Ok(allocation)
}

pub(super) fn create_private_file(path: &Path) -> Result<File, Failure> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Security::SECURITY_ATTRIBUTES,
        Storage::FileSystem::{
            CreateFileW, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OPEN_REPARSE_POINT,
            FILE_SHARE_READ,
        },
    };
    let allocation = private_descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: allocation.0,
        bInheritHandle: 0,
    };
    let path = mutation_wide(path)?;
    // SAFETY: terminated path and descriptor stay live; CREATE_NEW never follows an existing leaf.
    let raw = unsafe {
        CreateFileW(
            path.as_ptr(),
            0xc000_0000,
            FILE_SHARE_READ,
            &attributes,
            CREATE_NEW,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OPEN_REPARSE_POINT,
            0,
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(Failure::ProgramUnavailable);
    }
    // SAFETY: successful CreateFile returned this uniquely owned handle.
    let file = unsafe { File::from_raw_handle(raw as _) };
    inspect_security(
        &file,
        Scope::PrivateTree,
        &TrustedPrincipals::current()?,
        &mut ScanBudget::new(),
    )?;
    Ok(file)
}

pub(super) fn object_id(
    file: &File,
) -> Result<crate::installer_targets::journal::ObjectId, Failure> {
    let info = file_information(file).map_err(|_| Failure::ProgramUnavailable)?;
    if info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
        return Err(Failure::UnsupportedInstallation);
    }
    Ok(crate::installer_targets::journal::ObjectId {
        volume: info.dwVolumeSerialNumber,
        high: info.nFileIndexHigh,
        low: info.nFileIndexLow,
    })
}

pub(super) fn lock_private_file(path: &Path) -> Result<File, Failure> {
    let owner = LockedPackageFile::open(path).map_err(|_| Failure::ProgramUnavailable)?;
    let user = TrustedPrincipals::current()?;
    let mut budget = ScanBudget::new();
    inspect_security(&owner.file, Scope::PrivateTree, &user, &mut budget)?;
    if owner._directories.len() > MAX_ANCESTORS {
        return Err(Failure::UnsupportedInstallation);
    }
    for ancestor in &owner._directories {
        inspect_security(ancestor, Scope::Ancestor, &user, &mut budget)?;
    }
    Ok(owner.file)
}

pub(super) fn lock_directory(
    path: &Path,
    private: bool,
    renameable: bool,
) -> Result<DirectoryAnchor, Failure> {
    let text = path.to_str().ok_or(Failure::UnsupportedInstallation)?;
    if crate::discovery::validate_windows_location(text).as_deref() != Ok(text) {
        return Err(Failure::UnsupportedInstallation);
    }
    let mut paths: Vec<_> = path.ancestors().collect();
    paths.reverse();
    if paths.is_empty() || paths.len() > MAX_ANCESTORS {
        return Err(Failure::UnsupportedInstallation);
    }
    let root: Vec<u16> = paths[0].as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: the fixed local volume root is terminated and remains live.
    if unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) }
        != windows_sys::Win32::System::WindowsProgramming::DRIVE_FIXED
    {
        return Err(Failure::UnsupportedInstallation);
    }
    let user = TrustedPrincipals::current()?;
    let mut budget = ScanBudget::new();
    let mut identities = HashSet::new();
    let mut held = Vec::new();
    for directory in paths {
        let file = open_read_only(directory, FILE_FLAG_BACKUP_SEMANTICS)
            .map_err(|_| Failure::ProgramUnavailable)?;
        if !inspect_object(&file, &mut identities)? {
            return Err(Failure::UnsafeInstallation);
        }
        inspect_security(&file, Scope::Ancestor, &user, &mut budget)?;
        held.push(file);
    }
    let mut file = held.pop().ok_or(Failure::ProgramUnavailable)?;
    if private {
        inspect_security(&file, Scope::PrivateTree, &user, &mut budget)?;
    }
    let canonical = mutation_path(&file)?;
    if renameable {
        if !private {
            return Err(Failure::UnsafeInstallation);
        }
        file = reopen_directory_for_rename(file, &user, &mut budget)?;
    }
    Ok(DirectoryAnchor {
        file,
        _ancestors: held,
        path: canonical,
    })
}

pub(super) fn lock_mutation_tree(mut program: LockedPackageFile) -> Result<MutationTree, Failure> {
    let user = TrustedPrincipals::current()?;
    let mut budget = ScanBudget::new();
    let root = program
        ._directories
        .pop()
        .ok_or(Failure::ProgramUnavailable)?;
    let path = mutation_path(&root)?;
    if mutation_path(&program.file)?.parent() != Some(path.as_path()) {
        return Err(Failure::UnsafeInstallation);
    }
    budget.charge_object(0, path.as_os_str().encode_wide().count())?;
    let mut identities = HashSet::new();
    if !inspect_object(&root, &mut identities)? {
        return Err(Failure::UnsafeInstallation);
    }
    inspect_security(&root, Scope::PrivateTree, &user, &mut budget)?;
    if program._directories.len() > MAX_ANCESTORS {
        return Err(Failure::UnsupportedInstallation);
    }
    for ancestor in &program._directories {
        inspect_security(ancestor, Scope::Ancestor, &user, &mut budget)?;
    }
    let mut pending = Vec::new();
    queue_children(&path, 1, &mut pending, &mut budget)?;
    let mut files = Vec::new();
    let mut directories = Vec::new();
    while let Some((path, depth)) = pending.pop() {
        let file = open_read_only(&path, FILE_FLAG_BACKUP_SEMANTICS)
            .map_err(|_| Failure::ProgramUnavailable)?;
        let directory = inspect_object(&file, &mut identities)?;
        inspect_security(&file, Scope::PrivateTree, &user, &mut budget)?;
        if directory {
            queue_children(&path, depth + 1, &mut pending, &mut budget)?;
            directories.push(file);
        } else {
            files.push((mutation_path(&file)?, file));
        }
    }
    let root = reopen_directory_for_rename(root, &user, &mut budget)?;
    for directory in &mut directories {
        // ReOpenFile first obtains an independently checked same-object handle;
        // only then is this original no-delete-share directory handle closed.
        let reopened = reopen_directory(directory, &user, &mut budget)?;
        *directory = reopened;
    }
    drop(program.file); // the separately enumerated EXE guard remains in files.
    let file_paths = files.iter().map(|(path, _)| path.clone()).collect();
    Ok(MutationTree {
        root: DirectoryAnchor {
            file: root,
            _ancestors: program._directories,
            path,
        },
        directories,
        files,
        file_paths,
    })
}

fn reopen_directory_for_rename(
    file: File,
    user: &TrustedPrincipals,
    budget: &mut ScanBudget,
) -> Result<File, Failure> {
    let reopened = reopen_directory(&file, user, budget)?;
    drop(file);
    Ok(reopened)
}
fn reopen_directory(
    file: &File,
    user: &TrustedPrincipals,
    budget: &mut ScanBudget,
) -> Result<File, Failure> {
    use std::os::windows::io::FromRawHandle;
    use windows_sys::Win32::{
        Foundation::INVALID_HANDLE_VALUE,
        Storage::FileSystem::{
            ReOpenFile, FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_DELETE, FILE_SHARE_READ,
        },
    };
    let expected = object_id(file)?;
    // SDK ReOpenFile supports FILE_FLAG_BACKUP_SEMANTICS for directory objects.
    // No DELETE access requested while the original no-delete share remains.
    // SAFETY: the original live handle names the object; no pointer parameters.
    let raw = unsafe {
        ReOpenFile(
            file.as_raw_handle() as HANDLE,
            0x8000_0000,
            FILE_SHARE_READ | FILE_SHARE_DELETE,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(Failure::ProgramUnavailable);
    }
    // SAFETY: ReOpenFile returned a new independently owned handle.
    let reopened = unsafe { File::from_raw_handle(raw as _) };
    if object_id(&reopened)? != expected {
        return Err(Failure::UnsafeInstallation);
    }
    inspect_security(&reopened, Scope::PrivateTree, user, budget)?;
    Ok(reopened)
}

// The shared WinTrust helper returns a normalized DOS path with an extended
// prefix. Native target validation and NSIS consume the equivalent plain DOS
// spelling, while the original same-object handles remain authoritative.
fn mutation_path(file: &File) -> Result<PathBuf, Failure> {
    let path = held_path(file)?;
    let text = path.to_str().ok_or(Failure::UnsupportedInstallation)?;
    Ok(PathBuf::from(text.strip_prefix(r"\\?\").unwrap_or(text)))
}

fn mutation_wide(path: &Path) -> Result<Vec<u16>, Failure> {
    let text = path.to_str().ok_or(Failure::UnsupportedInstallation)?;
    if crate::discovery::validate_windows_location(text).as_deref() != Ok(text) {
        return Err(Failure::UnsupportedInstallation);
    }
    Ok(format!(r"\\?\{text}")
        .encode_utf16()
        .chain(Some(0))
        .collect())
}

pub(super) fn lock_installer_temporary_directory(path: &Path) -> Result<DirectoryAnchor, Failure> {
    let directory = lock_directory(path, false, false)?;
    directory.recheck_temporary()?;
    Ok(directory)
}

// Protect the actual native host and its namespace without claiming integrity
// of unrelated sibling assets or creating an installed-program/start proof.
pub(super) fn inspect_installer_host(program: &LockedPackageFile) -> Result<(), Failure> {
    let user = TrustedPrincipals::current()?;
    let mut budget = ScanBudget::new();
    inspect_security(&program.file, Scope::InstallerHost, &user, &mut budget)?;
    if program._directories.len() > MAX_ANCESTORS {
        return Err(Failure::UnsupportedInstallation);
    }
    for ancestor in &program._directories {
        inspect_security(ancestor, Scope::InstallerAncestor, &user, &mut budget)?;
    }
    Ok(())
}
