use super::StoreError;
use std::{
    ffi::c_void,
    fs::{File, Metadata, OpenOptions},
    os::windows::{
        ffi::OsStrExt,
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_ALREADY_EXISTS, ERROR_SUCCESS, HANDLE,
    },
    Security::{
        Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, GetNamedSecurityInfoW,
            GetSecurityInfo, SE_FILE_OBJECT,
        },
        EqualSid, GetAce, GetSecurityDescriptorDacl, GetTokenInformation, IsValidSid,
        SetFileSecurityW, TokenUser, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION,
        OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_ATTRIBUTES,
        TOKEN_QUERY, TOKEN_USER,
    },
    Storage::FileSystem::{
        CreateDirectoryW, GetFileInformationByHandle, MoveFileExW, BY_HANDLE_FILE_INFORMATION,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_OPEN_REPARSE_POINT, MOVEFILE_REPLACE_EXISTING,
        MOVEFILE_WRITE_THROUGH,
    },
    System::Threading::{GetCurrentProcess, OpenProcessToken},
};

struct Allocation(*mut c_void);
impl Drop for Allocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: pointers originate only from the documented LocalAlloc-based APIs.
            unsafe {
                LocalFree(self.0 as _);
            }
        }
    }
}
struct Token(HANDLE);
impl Drop for Token {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns the successfully opened token handle.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub(super) fn validate_root_syntax(root: &Path) -> Result<(), StoreError> {
    let root = root.to_str().ok_or(StoreError::UnsafePath)?;
    crate::discovery::validate_windows_location(root.strip_prefix(r"\\?\").unwrap_or(root))
        .map_err(|_| StoreError::UnsafePath)?;
    Ok(())
}

pub(super) fn validate_ancestor(metadata: &Metadata) -> Result<(), StoreError> {
    if !metadata.is_dir() || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(StoreError::UnsafePath);
    }
    Ok(())
}

fn descriptor() -> Result<Allocation, StoreError> {
    let text: Vec<u16> = "D:P(A;OICI;FA;;;OW)(A;OICI;FA;;;SY)"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor = Allocation(ptr::null_mut());
    // SAFETY: fixed terminated SDDL, valid output pointer and supported revision 1.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            text.as_ptr(),
            1,
            &mut descriptor.0,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(StoreError::Io);
    }
    Ok(descriptor)
}

pub(super) fn create_directory(root: &Path) -> Result<(), StoreError> {
    let root = wide(root)?;
    let descriptor = descriptor()?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    // SAFETY: the pathname and security descriptor stay alive for the entire call.
    if unsafe { CreateDirectoryW(root.as_ptr(), &attributes) } == 0 {
        // SAFETY: GetLastError reads only this thread's immediately preceding error.
        if unsafe { GetLastError() } != ERROR_ALREADY_EXISTS {
            return Err(StoreError::Io);
        }
    }
    Ok(())
}

fn require_private_access(path: &Path) -> Result<(), StoreError> {
    let path = wide(path)?;
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut security = Allocation(ptr::null_mut());
    // SAFETY: terminated path and initialized owner/DACL output pointers.
    let result = unsafe {
        GetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut security.0,
        )
    };
    if result != ERROR_SUCCESS || owner.is_null() {
        return Err(StoreError::UnsafePath);
    }
    validate_private_security(owner, acl)
}

/// Validate the actual held cache object, never a pathname reopened after hashing.
pub(super) fn require_private_handle(file: &File) -> Result<(), StoreError> {
    let mut owner = ptr::null_mut();
    let mut acl = ptr::null_mut();
    let mut security = Allocation(ptr::null_mut());
    // SAFETY: the borrowed File remains open and all outputs have live storage.
    let result = unsafe {
        GetSecurityInfo(
            file.as_raw_handle() as HANDLE,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            ptr::null_mut(),
            &mut acl,
            ptr::null_mut(),
            &mut security.0,
        )
    };
    if result != ERROR_SUCCESS || owner.is_null() {
        return Err(StoreError::UnsafePath);
    }
    validate_private_security(owner, acl)
}

// Only OS-returned owner/DACL pointers backed by a live descriptor reach here.
fn validate_private_security(owner: *mut c_void, acl: *mut ACL) -> Result<(), StoreError> {
    let mut token = 0;
    // SAFETY: valid pseudo-process handle and writable token output.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(StoreError::Io);
    }
    let token = Token(token);
    let mut size = 0;
    // SAFETY: null buffer with zero length requests the required size only.
    unsafe {
        GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut size);
    }
    if size < std::mem::size_of::<TOKEN_USER>() as u32 || size > 4096 {
        return Err(StoreError::UnsafePath);
    }
    let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
    let capacity = (buffer.len() * std::mem::size_of::<usize>()) as u32;
    // SAFETY: usize storage provides pointer alignment and the full declared capacity.
    if unsafe {
        GetTokenInformation(
            token.0,
            TokenUser,
            buffer.as_mut_ptr().cast(),
            capacity,
            &mut size,
        )
    } == 0
        || size > capacity
    {
        return Err(StoreError::Io);
    }
    // SAFETY: successful TokenUser result includes a full aligned TOKEN_USER.
    let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
    if user.User.Sid.is_null() {
        return Err(StoreError::UnsafePath);
    }
    // SAFETY: both SIDs originate from successful OS queries whose backing storage is live.
    if unsafe { EqualSid(owner, user.User.Sid) } == 0 {
        return Err(StoreError::UnsafePath);
    }
    let expected = descriptor()?;
    let mut expected_acl = ptr::null_mut();
    let mut present = 0;
    let mut defaulted = 0;
    // SAFETY: descriptor is produced by the fixed SDDL constructor and stays live.
    if unsafe {
        GetSecurityDescriptorDacl(expected.0, &mut present, &mut expected_acl, &mut defaulted)
    } == 0
        || present == 0
        || expected_acl.is_null()
        || acl.is_null()
    {
        return Err(StoreError::UnsafePath);
    }
    // Never adopt state that another principal could previously have changed.
    // Only the two fixed owner-rights/SYSTEM grants are accepted, including
    // their inherited file forms. Narrower/unrecognized ACLs fail closed too.
    // SAFETY: both ACLs belong to successful OS queries with live descriptors.
    unsafe { validate_private_acl(acl, expected_acl) }?;
    Ok(())
}

unsafe fn validate_private_acl(acl: *const ACL, expected: *const ACL) -> Result<(), StoreError> {
    // SAFETY: caller supplies OS-returned ACLs backed by live descriptors.
    let count = unsafe { (*acl).AceCount };
    if count == 0 || count > 8 || unsafe { (*expected).AceCount } != 2 {
        return Err(StoreError::UnsafePath);
    }
    for index in 0..u32::from(count) {
        // SAFETY: index is bounded by the OS-returned ACE count.
        let sid = unsafe { allowed_ace_sid(acl, index) }?;
        let mut permitted = false;
        for expected_index in 0..2 {
            // SAFETY: the fixed expected ACL has exactly two allow ACEs.
            let expected_sid = unsafe { allowed_ace_sid(expected, expected_index) }?;
            // SAFETY: helpers validated SID extents and both descriptors stay alive.
            permitted |= unsafe { EqualSid(sid, expected_sid) } != 0;
        }
        if !permitted {
            return Err(StoreError::UnsafePath);
        }
    }
    Ok(())
}

unsafe fn allowed_ace_sid(acl: *const ACL, index: u32) -> Result<*mut c_void, StoreError> {
    let mut ace: *mut c_void = ptr::null_mut();
    // SAFETY: the caller supplies a valid ACL and a bounded index.
    if unsafe { GetAce(acl, index, &mut ace) } == 0 || ace.is_null() {
        return Err(StoreError::UnsafePath);
    }
    // SAFETY: GetAce provides a valid ACE_HEADER. Only ordinary allow ACEs
    // contain the SID at offset 8. SID length is checked before EqualSid.
    let header = unsafe { &*ace.cast::<ACE_HEADER>() };
    if header.AceType != 0 || header.AceSize < 16 {
        return Err(StoreError::UnsafePath);
    }
    let sid = unsafe { ace.cast::<u8>().add(8) };
    let sid_length = 8 + 4 * usize::from(unsafe { *sid.add(1) });
    if sid_length > usize::from(header.AceSize) - 8 || unsafe { IsValidSid(sid.cast()) } == 0 {
        return Err(StoreError::UnsafePath);
    }
    Ok(sid.cast())
}

fn protect(path: &Path) -> Result<(), StoreError> {
    require_private_access(path)?;
    let path = wide(path)?;
    let descriptor = descriptor()?;
    // SAFETY: only the owned Suite path is changed, after an explicit owner check.
    if unsafe {
        SetFileSecurityW(
            path.as_ptr(),
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            descriptor.0,
        )
    } == 0
    {
        return Err(StoreError::Io);
    }
    Ok(())
}

pub(super) fn validate_directory(root: &Path) -> Result<(), StoreError> {
    protect(root)
}
pub(super) fn configure_open(options: &mut OpenOptions) {
    options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}
pub(super) fn validate_file(file: &File, path: &Path) -> Result<(), StoreError> {
    // SAFETY: a zeroed POD output is valid and the borrowed File handle stays open.
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: output covers the entire native structure, handle is borrowed only.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
        return Err(StoreError::Io);
    }
    if info.nNumberOfLinks != 1
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file.metadata().map_err(|_| StoreError::Io)?.is_file()
    {
        return Err(StoreError::UnsafePath);
    }
    protect(path)
}

pub(super) fn replace_file(source: &Path, destination: &Path) -> Result<(), StoreError> {
    let source = wide(source)?;
    let destination = wide(destination)?;
    // SAFETY: both owned paths remain live. No cross-volume copy or delayed move.
    if unsafe {
        MoveFileExW(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(StoreError::Indeterminate);
    }
    Ok(())
}
pub(super) fn sync_directory(_root: &Path) -> Result<(), StoreError> {
    Ok(())
} // MoveFileEx uses WRITE_THROUGH.

fn wide(path: &Path) -> Result<Vec<u16>, StoreError> {
    let mut value: Vec<u16> = path.as_os_str().encode_wide().collect();
    if value.contains(&0) {
        return Err(StoreError::UnsafePath);
    }
    value.push(0);
    Ok(value)
}
