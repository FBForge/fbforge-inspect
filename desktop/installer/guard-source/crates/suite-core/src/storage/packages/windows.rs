use super::{
    files::{Directory, Stamp},
    PackageCacheError,
};
use std::{
    fs::File,
    os::windows::{
        ffi::OsStrExt,
        io::{AsRawHandle, FromRawHandle},
    },
    path::Path,
    ptr,
};
use windows_sys::Win32::{
    Foundation::{GetLastError, GENERIC_READ, GENERIC_WRITE, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, FileDispositionInfo, FileRenameInfo, GetFileInformationByHandle, ReOpenFile,
        SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, CREATE_NEW, DELETE,
        FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO, FILE_FLAG_BACKUP_SEMANTICS,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_RENAME_INFO, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL,
    },
};

fn wide(path: &Path) -> Result<Vec<u16>, PackageCacheError> {
    let mut v: Vec<u16> = path.as_os_str().encode_wide().collect();
    if v.contains(&0) || v.len() > 32767 {
        return Err(PackageCacheError::UnsafePath);
    }
    v.push(0);
    Ok(v)
}
fn open(
    path: &Path,
    access: u32,
    sharing: u32,
    disposition: u32,
    flags: u32,
) -> Result<File, PackageCacheError> {
    let path = wide(path)?;
    // SAFETY: terminated native-only path, fixed flags and no inheritable handle.
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            sharing,
            ptr::null(),
            disposition,
            flags,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(PackageCacheError::Io);
    }
    // SAFETY: successful CreateFileW transfers one uniquely owned handle.
    Ok(unsafe { File::from_raw_handle(handle as _) })
}
pub(super) fn open_directory(path: &Path) -> Result<File, PackageCacheError> {
    let file = open(
        path,
        GENERIC_READ | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        OPEN_EXISTING,
        FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    crate::storage::windows::require_private_handle(&file)
        .map_err(|_| PackageCacheError::UnsafePath)?;
    let info = information(&file)?;
    if !file.metadata().map_err(|_| PackageCacheError::Io)?.is_dir()
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(PackageCacheError::UnsafePath);
    }
    Ok(file)
}
fn leaf(directory: &Directory, name: &str) -> Result<std::path::PathBuf, PackageCacheError> {
    if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
        return Err(PackageCacheError::UnsafePath);
    }
    Ok(directory.path.join(name))
}
pub(super) fn open_file(
    directory: &Directory,
    name: &str,
    create: bool,
) -> Result<File, PackageCacheError> {
    let file = open(
        &leaf(directory, name)?,
        GENERIC_READ | READ_CONTROL | DELETE | if create { GENERIC_WRITE } else { 0 },
        0,
        if create { CREATE_NEW } else { OPEN_EXISTING },
        FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    validate_file(&file)?;
    Ok(file)
}
pub(super) fn open_lock(directory: &Directory) -> Result<File, PackageCacheError> {
    // Sharing permits another owner to reach the nonblocking OS lock check. Cache
    // package handles themselves deny all data/delete sharing until guard drop.
    let file = open(
        &leaf(directory, "state.lock")?,
        GENERIC_READ | GENERIC_WRITE | READ_CONTROL,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
        OPEN_ALWAYS,
        FILE_FLAG_OPEN_REPARSE_POINT,
    )?;
    validate_file(&file)?;
    Ok(file)
}
fn information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION, PackageCacheError> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // SAFETY: borrowed file handle and full writable native POD output.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut info) } == 0 {
        return Err(PackageCacheError::Io);
    }
    Ok(info)
}
pub(super) fn validate_file(file: &File) -> Result<(), PackageCacheError> {
    let info = information(file)?;
    if info.nNumberOfLinks != 1
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file
            .metadata()
            .map_err(|_| PackageCacheError::Io)?
            .is_file()
    {
        return Err(PackageCacheError::UnsafePath);
    }
    crate::storage::windows::require_private_handle(file).map_err(|_| PackageCacheError::UnsafePath)
}
pub(super) fn stamp(file: &File) -> Result<Stamp, PackageCacheError> {
    let i = information(file)?;
    Ok(Stamp([
        i.dwVolumeSerialNumber as u64,
        ((i.nFileIndexHigh as u64) << 32) | i.nFileIndexLow as u64,
        ((i.nFileSizeHigh as u64) << 32) | i.nFileSizeLow as u64,
        ((i.ftLastWriteTime.dwHighDateTime as u64) << 32) | i.ftLastWriteTime.dwLowDateTime as u64,
        i.nNumberOfLinks as u64,
        i.dwFileAttributes as u64,
    ]))
}

pub(super) fn prepare_installer_read(file: &mut File) -> Result<(), PackageCacheError> {
    fn reopen(file: &File, access: u32, sharing: u32) -> Result<File, PackageCacheError> {
        // SAFETY: this is an existing CreateFileW object, never a name reopen.
        // Each success transfers one independently owned non-inherited handle.
        let handle = unsafe {
            ReOpenFile(
                file.as_raw_handle() as HANDLE,
                access,
                sharing,
                FILE_FLAG_OPEN_REPARSE_POINT,
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(PackageCacheError::Io);
        }
        Ok(unsafe { File::from_raw_handle(handle as _) })
    }
    let identity = information(file)?;
    // Zero access does not conflict with the original share0 handle. Its share
    // mask accommodates that handle's READ/WRITE/DELETE access until we close it.
    let anchor = reopen(
        file,
        0,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
    )?;
    drop(std::mem::replace(file, anchor));
    let reader = reopen(file, GENERIC_READ | READ_CONTROL, FILE_SHARE_READ)?;
    validate_file(&reader)?;
    let current = information(&reader)?;
    if identity.dwVolumeSerialNumber != current.dwVolumeSerialNumber
        || identity.nFileIndexHigh != current.nFileIndexHigh
        || identity.nFileIndexLow != current.nFileIndexLow
    {
        return Err(PackageCacheError::Corrupt);
    }
    // A writer in the transition window prevents this read/shareREAD open.
    // Bytes that changed and were closed are caught by the mandatory new hash.
    *file = reader;
    Ok(())
}
pub(super) fn remove(
    _directory: &Directory,
    file: &File,
    _name: &str,
) -> Result<(), PackageCacheError> {
    validate_file(file)?;
    let info = FILE_DISPOSITION_INFO { DeleteFile: 1 };
    // SAFETY: live DELETE-capable handle and exact native structure size.
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            FileDispositionInfo,
            (&info as *const FILE_DISPOSITION_INFO).cast(),
            std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32,
        )
    } == 0
    {
        return Err(PackageCacheError::Io);
    }
    Ok(())
}
pub(super) fn publish(
    directory: &Directory,
    file: &File,
    _source: &str,
    destination: &str,
) -> Result<(), PackageCacheError> {
    validate_file(file)?;
    leaf(directory, destination)?;
    let name: Vec<u16> = destination.encode_utf16().collect();
    let size = std::mem::offset_of!(FILE_RENAME_INFO, FileName) + name.len() * 2;
    let mut buffer = vec![0usize; size.div_ceil(std::mem::size_of::<usize>())];
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    // SAFETY: usize storage aligns FILE_RENAME_INFO, the bounded allocation
    // includes its variable UTF-16 tail. The directory handle anchors the leaf.
    unsafe {
        (*info).Anonymous.ReplaceIfExists = 0;
        (*info).RootDirectory = directory.handle.as_raw_handle() as HANDLE;
        (*info).FileNameLength = (name.len() * 2) as u32;
        ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
    }
    // SAFETY: live DELETE-capable source handle and fully initialized buffer.
    let result = unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle() as HANDLE,
            FileRenameInfo,
            buffer.as_ptr().cast(),
            size as u32,
        )
    };
    if result == 0 {
        // SAFETY: preserve the immediately preceding OS failure before Drop/IO.
        let _error = unsafe { GetLastError() };
        return Err(PackageCacheError::Indeterminate);
    }
    // The cache is disposable. This is atomic publication, not a promise of
    // durable directory metadata after sudden power loss on every filesystem.
    Ok(())
}
