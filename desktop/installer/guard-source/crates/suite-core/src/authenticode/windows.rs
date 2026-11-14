use super::{
    AuthenticodePolicy, IndeterminateReason as Unknown, InvalidReason as Invalid,
    OwnedVerifiedCooperatingProgram, PreparedProgramStart, ProgramStartFailure,
    ProgramStartOutcome, VerificationFailure as Failure, VerifiedCachedWindowsPackage,
    VerifiedWindowsPackage,
};

pub(crate) mod installer_targets_bridge;
mod program_tree;
use crate::{
    catalog::ToolManifest,
    discovery::{
        inspect_windows_installer_image, inspect_windows_x64_image, validate_windows_location,
        DetectionFailure,
    },
    storage::CachedPackage,
    trust::{TrustError, VerifiedRelease},
};
pub(super) use program_tree::LockedProgramTree;
use ring::digest;
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    io::{Seek, SeekFrom},
    mem::{size_of, zeroed},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
    ptr,
};
use windows_sys::{
    core::GUID,
    Win32::{
        Foundation::*,
        Security::{Cryptography::*, WinTrust::*},
        Storage::FileSystem::*,
        System::{
            LibraryLoader::{GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_SYSTEM32},
            Threading::{CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW},
            WindowsProgramming::DRIVE_FIXED,
        },
    },
};

pub(super) struct LockedPackageFile {
    file: File,
    // Keeping every directory open without delete sharing prevents ancestor
    // replacement while the final, no-follow file handle is being checked.
    _directories: Vec<File>,
}

/// Crate-private lifetime guard only; never exposes a directory or OS handle.
pub(crate) struct InstallerTemporaryDirectory {
    temporary: program_tree::DirectoryAnchor,
    _windows_root: program_tree::DirectoryAnchor,
}

impl InstallerTemporaryDirectory {
    pub(crate) fn lock(path: &Path, windows: &Path) -> Result<Self, Failure> {
        let temporary = program_tree::lock_installer_temporary_directory(path)
            .map_err(|_| invalid(Invalid::UnsafeFile))?;
        let windows_root = program_tree::lock_directory(windows, false, false)
            .map_err(|_| invalid(Invalid::UnsafeFile))?;
        if temporary
            .is_same_or_descendant_of(&windows_root)
            .map_err(|_| invalid(Invalid::UnsafeFile))?
        {
            return Err(invalid(Invalid::UnsafeFile));
        }
        Ok(Self {
            temporary,
            _windows_root: windows_root,
        })
    }

    pub(crate) fn recheck(&self) -> Result<(), Failure> {
        self.temporary
            .recheck_temporary()
            .map_err(|_| invalid(Invalid::UnsafeFile))
    }
}

fn unknown(reason: Unknown) -> Failure {
    Failure::Indeterminate(reason)
}

fn invalid(reason: Invalid) -> Failure {
    Failure::Invalid(reason)
}

impl LockedPackageFile {
    fn open(path: &Path) -> Result<Self, Failure> {
        let text = path.to_str().ok_or(invalid(Invalid::UnsafeFile))?;
        if validate_windows_location(text).as_deref() != Ok(text) {
            return Err(invalid(Invalid::UnsafeFile));
        }
        let mut ancestors: Vec<_> = path.ancestors().skip(1).collect();
        ancestors.reverse();
        let root = ancestors.first().ok_or(invalid(Invalid::UnsafeFile))?;
        let root = wide(root.to_str().ok_or(invalid(Invalid::UnsafeFile))?);
        // SAFETY: root is a validated, terminated drive-root string.
        if unsafe { GetDriveTypeW(root.as_ptr()) } != DRIVE_FIXED {
            return Err(invalid(Invalid::UnsafeFile));
        }
        let mut directories = Vec::new();
        for ancestor in ancestors {
            let directory = open_read_only(ancestor, FILE_FLAG_BACKUP_SEMANTICS)?;
            let information = file_information(&directory)?;
            if information.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
                || information.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
            {
                return Err(invalid(Invalid::UnsafeFile));
            }
            directories.push(directory);
        }
        let file = open_read_only(path, 0)?;
        let information = file_information(&file)?;
        // SAFETY: file owns a live OS handle throughout the call.
        if unsafe { GetFileType(file.as_raw_handle() as HANDLE) } != FILE_TYPE_DISK
            || information.dwFileAttributes
                & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
                != 0
            || information.nNumberOfLinks != 1
        {
            return Err(invalid(Invalid::UnsafeFile));
        }
        Ok(Self {
            file,
            _directories: directories,
        })
    }
}

fn open_read_only(path: &Path, flags: u32) -> Result<File, Failure> {
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(flags | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .map_err(|_| unknown(Unknown::PackageUnavailable))
}

fn file_information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION, Failure> {
    // SAFETY: the output contains only plain integer fields and starts zeroed.
    let mut information = unsafe { zeroed() };
    // SAFETY: live file handle and full writable output structure.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle() as HANDLE, &mut information) } == 0
    {
        return Err(unknown(Unknown::PackageUnavailable));
    }
    Ok(information)
}

fn final_path(file: &File) -> Result<Vec<u16>, Failure> {
    let mut buffer = vec![0; 4097];
    // SAFETY: live file handle and output storage of the declared WCHAR count.
    let count = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle() as HANDLE,
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
        )
    } as usize;
    if count == 0 || count >= buffer.len() {
        return Err(unknown(Unknown::PackageUnavailable));
    }
    let path = String::from_utf16(&buffer[..count]).map_err(|_| invalid(Invalid::UnsafeFile))?;
    validate_windows_location(path.strip_prefix(r"\\?\").unwrap_or(&path))
        .map_err(|_| invalid(Invalid::UnsafeFile))?;
    buffer.truncate(count + 1);
    Ok(buffer)
}

pub(super) fn verify_package(
    path: &Path,
    release: &VerifiedRelease,
    policy: &AuthenticodePolicy,
    now: u64,
) -> Result<VerifiedWindowsPackage, Failure> {
    let mut package = LockedPackageFile::open(path)?;
    let evidence = verify_held_file(&mut package.file, release, policy, now)?;
    Ok(VerifiedWindowsPackage {
        _package: package,
        sha256: evidence.sha256,
        byte_count: evidence.byte_count,
        signer_certificate_sha256: evidence.signer_certificate_sha256,
    })
}

pub(super) fn verify_cached_package<'cache>(
    mut package: CachedPackage<'cache>,
    release: &VerifiedRelease,
    policy: &AuthenticodePolicy,
    now: u64,
) -> Result<VerifiedCachedWindowsPackage<'cache>, Failure> {
    let file = package.file_mut();
    validate_held_file(file)?;
    let evidence = verify_held_file(file, release, policy, now)?;
    Ok(VerifiedCachedWindowsPackage {
        _package: package,
        sha256: evidence.sha256,
        byte_count: evidence.byte_count,
        signer_certificate_sha256: evidence.signer_certificate_sha256,
    })
}

// Reuse the exact locked path owner and the existing provider-state RAII. No
// package release is invented: this is an installed-image publisher assertion.
pub(super) fn verify_cooperating_program(
    path: &Path,
    tool: &ToolManifest,
    policy: &AuthenticodePolicy,
    now: u64,
) -> Result<OwnedVerifiedCooperatingProgram, Failure> {
    let mut program = LockedPackageFile::open(path)?;
    inspect_windows_x64_image(&mut program.file).map_err(|error| match error {
        DetectionFailure::Unreadable => unknown(Unknown::PackageUnavailable),
        _ => invalid(Invalid::InvalidImage),
    })?;
    super::cooperation::inspect_marker(&mut program.file, &tool.app_id)?;
    program
        .file
        .seek(SeekFrom::Start(0))
        .map_err(|_| unknown(Unknown::PackageUnavailable))?;
    let certificate_sha256 = verify_publisher(&program.file, policy, now)?;
    Ok(OwnedVerifiedCooperatingProgram {
        _program: program,
        app_id: tool.app_id.clone(),
        binding: super::start::ProgramBinding {
            tool_id: tool.id.clone(),
            policy_id: policy.id.clone(),
            certificate_sha256,
            verified_at: now,
        },
    })
}

// The owner stays live through the exact CreateProcessW call. Nothing reopens
// its path in application code; held read/no-write/no-delete and ancestor handles
// keep the named executable stable while Windows opens the new image.
pub(super) fn request_program_start(
    prepared: PreparedProgramStart,
    tool: &ToolManifest,
) -> Result<ProgramStartOutcome, ProgramStartFailure> {
    let result = create_held_program(
        &prepared.program._program.file,
        tool,
        prepared.program.binding.verified_at,
    );
    drop(prepared);
    result
}

pub(super) fn prepare_program_start(
    program: OwnedVerifiedCooperatingProgram,
) -> Result<PreparedProgramStart, ProgramStartFailure> {
    let tree = program_tree::lock_program_tree(&program._program)?;
    Ok(PreparedProgramStart {
        program,
        _tree: tree,
    })
}

// Private OS boundary. The public entry above always consumes a verified owner;
// the isolated unsigned fixture exercises only this call, not publisher trust.
fn create_held_program(
    file: &File,
    tool: &ToolManifest,
    verified_at: u64,
) -> Result<ProgramStartOutcome, ProgramStartFailure> {
    let application = final_path(file).map_err(|_| ProgramStartFailure::ProgramUnavailable)?;
    let text = String::from_utf16(&application[..application.len() - 1])
        .map_err(|_| ProgramStartFailure::ProgramUnavailable)?;
    let path = Path::new(&text);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ProgramStartFailure::ProgramUnavailable)?;
    if !tool
        .executables
        .iter()
        .any(|allowed| allowed.eq_ignore_ascii_case(name))
    {
        return Err(ProgramStartFailure::ManifestRejected);
    }
    let directory = path
        .parent()
        .and_then(|parent| parent.to_str())
        .ok_or(ProgramStartFailure::ProgramUnavailable)?;
    let directory = wide(directory);
    // argv[0] only. lpApplicationName is always absolute and independently set;
    // the quotes prevent native path spaces from becoming extra child arguments.
    let mut command = wide(&format!("\"{text}\""));
    // SAFETY: unspecified startup fields permit zero defaults; cb is set below.
    let mut startup: STARTUPINFOW = unsafe { zeroed() };
    startup.cb = size_of::<STARTUPINFOW>() as u32;
    // SAFETY: POD output starts zeroed and is read only after successful creation.
    let mut information: PROCESS_INFORMATION = unsafe { zeroed() };
    super::start::check_time(verified_at, std::time::SystemTime::now())?;
    // SAFETY: buffers are bounded, terminated, retained for this synchronous call;
    // the command buffer is writable. No handle inheritance, shell, elevation,
    // suspension, job breakaway or renderer-controlled argument/environment.
    if unsafe {
        CreateProcessW(
            application.as_ptr(),
            command.as_mut_ptr(),
            ptr::null(),
            ptr::null(),
            0,
            0,
            ptr::null(),
            directory.as_ptr(),
            &startup,
            &mut information,
        )
    } == 0
    {
        return Err(ProgramStartFailure::OperatingSystem);
    }
    // These handles belong only to this newly created process. Closing them does
    // not terminate it. Do not wait for readiness or kill on drop/Suite shutdown.
    // SAFETY: successful creation returned these live, owned, non-inherited handles.
    unsafe {
        CloseHandle(information.hThread);
        CloseHandle(information.hProcess);
    }
    Ok(ProgramStartOutcome::StartRequested)
}

struct HeldFileEvidence {
    sha256: String,
    byte_count: u64,
    signer_certificate_sha256: [u8; 32],
}

pub(super) fn prepare_installer_package<'cache>(
    mut package: CachedPackage<'cache>,
    release: &VerifiedRelease,
    policy: &AuthenticodePolicy,
    now: u64,
) -> Result<VerifiedCachedWindowsPackage<'cache>, Failure> {
    validate_held_file(package.file_mut())?;
    let identity = super::read_installer_identity(package.file_mut())?;
    super::installer_protocol::require_identity(
        &identity,
        release.app_id(),
        &release.version().to_string(),
    )?;
    // The existing complete verifier owns the PE/hash/publisher checks and
    // WinTrust VERIFY/CLOSE lifetimes. No previous evidence is forwarded.
    verify_cached_package(package, release, policy, now)
}

pub(super) fn checked_installer_path(file: &File) -> Result<Vec<u16>, Failure> {
    validate_held_file(file)?;
    final_path(file)
}

fn validate_held_file(file: &File) -> Result<(), Failure> {
    let information = file_information(file)?;
    // SAFETY: caller retains the already open package handle throughout.
    if unsafe { GetFileType(file.as_raw_handle() as HANDLE) } != FILE_TYPE_DISK
        || information.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)
            != 0
        || information.nNumberOfLinks != 1
    {
        return Err(invalid(Invalid::UnsafeFile));
    }
    // Cache roots are independently private/held. Preserve the path adapter's
    // fixed-local-drive policy too, based on this same handle, without reopening.
    let final_name = final_path(file)?;
    let text = String::from_utf16(&final_name[..final_name.len() - 1])
        .map_err(|_| invalid(Invalid::UnsafeFile))?;
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    let drive = wide(&text[..3]); // final_path already validated ASCII drive syntax.
                                  // SAFETY: bounded, terminated drive-root string derived from the held file.
    if unsafe { GetDriveTypeW(drive.as_ptr()) } != DRIVE_FIXED {
        return Err(invalid(Invalid::UnsafeFile));
    }
    Ok(())
}

// Both adapters use exactly this PE/hash/publisher path on their existing File.
// WinTrust's VERIFY/CLOSE frame remains inside verify_publisher: CLOSE completes
// before its path/settings backing storage and this borrowed file can disappear.
fn verify_held_file(
    file: &mut File,
    release: &VerifiedRelease,
    policy: &AuthenticodePolicy,
    now: u64,
) -> Result<HeldFileEvidence, Failure> {
    inspect_windows_installer_image(file).map_err(|error| match error {
        DetectionFailure::Unreadable => unknown(Unknown::PackageUnavailable),
        _ => invalid(Invalid::InvalidImage),
    })?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| unknown(Unknown::PackageUnavailable))?;
    let integrity = release
        .verify_package(&mut *file)
        .map_err(|error| match error {
            TrustError::PackageRead => unknown(Unknown::PackageUnavailable),
            TrustError::WithdrawnRelease => invalid(Invalid::WithdrawnRelease),
            _ => invalid(Invalid::PackageMismatch),
        })?;
    file.seek(SeekFrom::Start(0))
        .map_err(|_| unknown(Unknown::PackageUnavailable))?;
    let signer_certificate_sha256 = verify_publisher(file, policy, now)?;
    Ok(HeldFileEvidence {
        sha256: integrity.sha256().to_owned(),
        byte_count: integrity.byte_count(),
        signer_certificate_sha256,
    })
}

struct TrustState {
    data: WINTRUST_DATA,
    action: GUID,
}

impl Drop for TrustState {
    fn drop(&mut self) {
        self.data.dwStateAction = WTD_STATEACTION_CLOSE;
        // SAFETY: this guard owns provider state from its VERIFY attempt. All
        // referenced file/settings storage outlives the guard, even on failure.
        unsafe {
            WinVerifyTrust(
                INVALID_HANDLE_VALUE,
                &mut self.action,
                (&mut self.data as *mut WINTRUST_DATA).cast(),
            );
        }
    }
}

type ProviderData = unsafe extern "system" fn(HANDLE) -> *mut CRYPT_PROVIDER_DATA;
type ProviderSigner =
    unsafe extern "system" fn(*mut CRYPT_PROVIDER_DATA, u32, BOOL, u32) -> *mut CRYPT_PROVIDER_SGNR;

struct ProviderHelpers {
    module: HMODULE,
    data: ProviderData,
    signer: ProviderSigner,
}

impl ProviderHelpers {
    fn load() -> Result<Self, Failure> {
        // SAFETY: fixed system DLL name, no caller-controlled search path.
        let module = unsafe {
            LoadLibraryExW(
                wide("wintrust.dll").as_ptr(),
                0,
                LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        if module == 0 {
            return Err(unknown(Unknown::ProviderUnavailable));
        }
        // SAFETY: names and signatures are the documented Wintrust exports.
        let exports = unsafe {
            (
                GetProcAddress(module, c"WTHelperProvDataFromStateData".as_ptr().cast()),
                GetProcAddress(module, c"WTHelperGetProvSignerFromChain".as_ptr().cast()),
            )
        };
        if let (Some(data), Some(signer)) = exports {
            // SAFETY: the exports have the exact documented system calling
            // convention and parameter types represented by these aliases.
            return Ok(unsafe {
                Self {
                    module,
                    data: std::mem::transmute::<unsafe extern "system" fn() -> isize, ProviderData>(
                        data,
                    ),
                    signer: std::mem::transmute::<
                        unsafe extern "system" fn() -> isize,
                        ProviderSigner,
                    >(signer),
                }
            });
        }
        // SAFETY: release the successfully loaded module on partial failure.
        unsafe { FreeLibrary(module) };
        Err(unknown(Unknown::ProviderUnavailable))
    }
}

impl Drop for ProviderHelpers {
    fn drop(&mut self) {
        // SAFETY: helpers and provider pointers are no longer used after drop.
        unsafe { FreeLibrary(self.module) };
    }
}

fn verify_publisher(
    file: &File,
    policy: &AuthenticodePolicy,
    now: u64,
) -> Result<[u8; 32], Failure> {
    let path = final_path(file)?;
    let helpers = ProviderHelpers::load()?;
    let mut file_info = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: path.as_ptr(),
        hFile: file.as_raw_handle() as HANDLE,
        pgKnownSubject: ptr::null_mut(),
    };
    let mut strong = CERT_STRONG_SIGN_PARA {
        cbSize: size_of::<CERT_STRONG_SIGN_PARA>() as u32,
        dwInfoChoice: CERT_STRONG_SIGN_OID_INFO_CHOICE,
        Anonymous: CERT_STRONG_SIGN_PARA_0 {
            pszOID: szOID_CERT_STRONG_SIGN_OS_1.cast_mut(),
        },
    };
    let mut signature_settings = WINTRUST_SIGNATURE_SETTINGS {
        cbStruct: size_of::<WINTRUST_SIGNATURE_SETTINGS>() as u32,
        dwIndex: 0,
        dwFlags: WSS_VERIFY_SPECIFIC,
        cSecondarySigs: 0,
        dwVerifiedSigIndex: 0,
        pCryptoPolicy: &mut strong,
    };
    // SAFETY: all unspecified WINTRUST_DATA members permit zero/null defaults.
    let mut data: WINTRUST_DATA = unsafe { zeroed() };
    data.cbStruct = size_of::<WINTRUST_DATA>() as u32;
    data.dwUIChoice = WTD_UI_NONE;
    data.fdwRevocationChecks = WTD_REVOKE_WHOLECHAIN;
    data.dwUnionChoice = WTD_CHOICE_FILE;
    data.Anonymous.pFile = &mut file_info;
    data.dwStateAction = WTD_STATEACTION_VERIFY;
    data.dwProvFlags = WTD_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT
        | WTD_DISABLE_MD2_MD4
        | WTD_CACHE_ONLY_URL_RETRIEVAL;
    data.dwUIContext = WTD_UICONTEXT_EXECUTE;
    data.pSignatureSettings = &mut signature_settings;
    let mut state = TrustState {
        data,
        action: WINTRUST_ACTION_GENERIC_VERIFY_V2,
    };
    // SAFETY: all pointer targets stay alive through VERIFY and guard cleanup.
    let status = unsafe {
        WinVerifyTrust(
            INVALID_HANDLE_VALUE,
            &mut state.action,
            (&mut state.data as *mut WINTRUST_DATA).cast::<c_void>(),
        )
    };
    check_status(status)?;
    if state.data.hWVTStateData == 0 || signature_settings.dwVerifiedSigIndex != 0 {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: successful VERIFY state remains owned and live until function exit.
    let provider = unsafe { (helpers.data)(state.data.hWVTStateData) };
    if provider.is_null() {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: OS helper returned a live provider record for this state.
    let signer = unsafe { (helpers.signer)(provider, 0, 0, 0) };
    let publisher = checked_signer(signer, &state)?;
    if publisher.dwSignerType & SGNR_TYPE_TIMESTAMP != 0 {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    let leaf = signer_certificate(publisher)?;
    // SAFETY: certificate context remains live under provider state ownership.
    let certificate = unsafe { &*leaf };
    if certificate.pbCertEncoded.is_null()
        || certificate.cbCertEncoded == 0
        || certificate.cbCertEncoded > 1_048_576
    {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: OS certificate context describes this complete, bounded DER buffer.
    let der = unsafe {
        std::slice::from_raw_parts(
            certificate.pbCertEncoded,
            certificate.cbCertEncoded as usize,
        )
    };
    let fingerprint: [u8; 32] = digest::digest(&digest::SHA256, der)
        .as_ref()
        .try_into()
        .map_err(|_| unknown(Unknown::ProviderStateUnavailable))?;
    if !policy.certificate_sha256.contains(&fingerprint) {
        return Err(invalid(Invalid::SignerMismatch));
    }
    if publisher.csCounterSigners == 0 {
        return Err(invalid(Invalid::TimestampRejected));
    }
    if publisher.csCounterSigners > 1 {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: fetch only the sole countersigner belonging to this publisher.
    let timestamp = checked_signer(unsafe { (helpers.signer)(provider, 0, 1, 0) }, &state)?;
    if timestamp.dwSignerType != SGNR_TYPE_TIMESTAMP {
        return Err(invalid(Invalid::TimestampRejected));
    }
    let signing_time = filetime(publisher.sftVerifyAsOf);
    if signing_time != filetime(timestamp.sftVerifyAsOf) {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    let unix_time = signing_time
        .checked_sub(116_444_736_000_000_000)
        .ok_or(invalid(Invalid::TimestampRejected))?
        / 10_000_000;
    if unix_time > now {
        return Err(invalid(Invalid::TimestampRejected));
    }
    // No unsigned signingTime attributes are consumed. The timestamp is from
    // this successfully verified signature, with an unambiguous provider time.
    verify_chain(leaf, publisher, false)?;
    verify_chain(signer_certificate(timestamp)?, timestamp, true)?;
    Ok(fingerprint)
}

fn checked_signer(
    pointer: *mut CRYPT_PROVIDER_SGNR,
    _state: &TrustState,
) -> Result<&CRYPT_PROVIDER_SGNR, Failure> {
    if pointer.is_null() {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: caller obtains this pointer only from a live Windows provider.
    let signer = unsafe { &*pointer };
    if signer.cbStruct < size_of::<CRYPT_PROVIDER_SGNR>() as u32 || signer.psSigner.is_null() {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    check_status(signer.dwError as i32)?;
    // SAFETY: signer record is live and owns the message algorithm OID string.
    let oid = unsafe { (*signer.psSigner).HashAlgorithm.pszObjId };
    let mut bytes = Vec::new();
    if oid.is_null() {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: Windows message OIDs are terminated strings. Limit inspection to
    // 64 bytes rather than accepting an unbounded algorithm identifier.
    for index in 0..64 {
        let byte = unsafe { *oid.add(index) };
        if byte == 0 {
            return if matches!(
                bytes.as_slice(),
                b"2.16.840.1.101.3.4.2.1" | b"2.16.840.1.101.3.4.2.2" | b"2.16.840.1.101.3.4.2.3"
            ) {
                Ok(signer)
            } else {
                Err(invalid(Invalid::SignatureRejected))
            };
        }
        bytes.push(byte);
    }
    Err(unknown(Unknown::ProviderStateUnavailable))
}

fn signer_certificate(signer: &CRYPT_PROVIDER_SGNR) -> Result<*const CERT_CONTEXT, Failure> {
    if signer.csCertChain == 0 || signer.csCertChain > 128 || signer.pasCertChain.is_null() {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    // SAFETY: the checked nonempty provider array owns its leaf record.
    let leaf = unsafe { &*signer.pasCertChain };
    if leaf.pCert.is_null() {
        return Err(unknown(Unknown::ProviderStateUnavailable));
    }
    check_status(leaf.dwError as i32)?;
    Ok(leaf.pCert)
}

struct Chain(*mut CERT_CHAIN_CONTEXT);
impl Drop for Chain {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: only chains allocated by CertGetCertificateChain are owned.
            unsafe { CertFreeCertificateChain(self.0) };
        }
    }
}

fn verify_chain(
    certificate: *const CERT_CONTEXT,
    signer: &CRYPT_PROVIDER_SGNR,
    timestamp: bool,
) -> Result<(), Failure> {
    let mut usage = if timestamp {
        szOID_KP_TIME_STAMP_SIGNING
    } else {
        szOID_PKIX_KP_CODE_SIGNING
    }
    .cast_mut();
    let parameters = CERT_CHAIN_PARA {
        cbSize: size_of::<CERT_CHAIN_PARA>() as u32,
        RequestedUsage: CERT_USAGE_MATCH {
            dwType: USAGE_MATCH_TYPE_AND,
            Usage: CTL_USAGE {
                cUsageIdentifier: 1,
                rgpszUsageIdentifier: &mut usage,
            },
        },
    };
    let mut chain = Chain(ptr::null_mut());
    // SAFETY: live provider certificate/time and complete output/usage structs.
    // Cache-only applies separately to issuer retrieval and revocation retrieval.
    let result = unsafe {
        CertGetCertificateChain(
            0,
            certificate,
            &signer.sftVerifyAsOf,
            (*certificate).hCertStore,
            &parameters,
            CERT_CHAIN_REVOCATION_CHECK_CHAIN_EXCLUDE_ROOT
                | CERT_CHAIN_REVOCATION_CHECK_CACHE_ONLY
                | CERT_CHAIN_CACHE_ONLY_URL_RETRIEVAL
                | CERT_CHAIN_DISABLE_AUTH_ROOT_AUTO_UPDATE,
            ptr::null(),
            &mut chain.0,
        )
    };
    if result == 0 || chain.0.is_null() {
        return Err(unknown(Unknown::ChainUnavailable));
    }
    // SAFETY: successful chain result owns the complete summary status.
    let errors = unsafe { (*chain.0).TrustStatus.dwErrorStatus };
    if errors & CERT_TRUST_IS_REVOKED != 0 {
        return Err(invalid(Invalid::Revoked));
    }
    if errors & (CERT_TRUST_REVOCATION_STATUS_UNKNOWN | CERT_TRUST_IS_OFFLINE_REVOCATION) != 0 {
        return Err(unknown(Unknown::RevocationUnavailable));
    }
    if errors & CERT_TRUST_IS_PARTIAL_CHAIN != 0 {
        return Err(unknown(Unknown::ChainUnavailable));
    }
    let rejected_errors = CERT_TRUST_IS_NOT_TIME_VALID
        | CERT_TRUST_IS_NOT_TIME_NESTED
        | CERT_TRUST_IS_NOT_SIGNATURE_VALID
        | CERT_TRUST_IS_NOT_VALID_FOR_USAGE
        | CERT_TRUST_IS_UNTRUSTED_ROOT
        | CERT_TRUST_IS_CYCLIC
        | CERT_TRUST_INVALID_EXTENSION
        | CERT_TRUST_INVALID_POLICY_CONSTRAINTS
        | CERT_TRUST_INVALID_BASIC_CONSTRAINTS
        | CERT_TRUST_INVALID_NAME_CONSTRAINTS
        | CERT_TRUST_IS_EXPLICIT_DISTRUST
        | CERT_TRUST_HAS_WEAK_SIGNATURE;
    if errors & !rejected_errors != 0 {
        return Err(unknown(Unknown::UnknownWindowsStatus));
    }
    if errors != 0 {
        return Err(invalid(Invalid::ChainRejected));
    }
    let mut publisher_policy = AUTHENTICODE_EXTRA_CERT_CHAIN_POLICY_PARA {
        cbSize: size_of::<AUTHENTICODE_EXTRA_CERT_CHAIN_POLICY_PARA>() as u32,
        dwRegPolicySettings: 0,
        pSignerInfo: signer.psSigner,
    };
    let mut timestamp_policy = AUTHENTICODE_TS_EXTRA_CERT_CHAIN_POLICY_PARA {
        cbSize: size_of::<AUTHENTICODE_TS_EXTRA_CERT_CHAIN_POLICY_PARA>() as u32,
        dwRegPolicySettings: 0,
        fCommercial: 0,
    };
    let parameters = CERT_CHAIN_POLICY_PARA {
        cbSize: size_of::<CERT_CHAIN_POLICY_PARA>() as u32,
        dwFlags: 0,
        pvExtraPolicyPara: if timestamp {
            (&mut timestamp_policy as *mut AUTHENTICODE_TS_EXTRA_CERT_CHAIN_POLICY_PARA).cast()
        } else {
            (&mut publisher_policy as *mut AUTHENTICODE_EXTRA_CERT_CHAIN_POLICY_PARA).cast()
        },
    };
    // SAFETY: zero/null defaults are valid for this policy output structure.
    let mut status: CERT_CHAIN_POLICY_STATUS = unsafe { zeroed() };
    status.cbSize = size_of::<CERT_CHAIN_POLICY_STATUS>() as u32;
    // SAFETY: chain remains owned through policy evaluation, all buffers live.
    let evaluated = unsafe {
        CertVerifyCertificateChainPolicy(
            if timestamp {
                CERT_CHAIN_POLICY_AUTHENTICODE_TS
            } else {
                CERT_CHAIN_POLICY_AUTHENTICODE
            },
            chain.0,
            &parameters,
            &mut status,
        )
    };
    if evaluated == 0 {
        return Err(unknown(Unknown::ChainUnavailable));
    }
    check_status(status.dwError as i32)
}

fn check_status(status: i32) -> Result<(), Failure> {
    match status {
        0 => Ok(()),
        TRUST_E_NOSIGNATURE => Err(invalid(Invalid::Unsigned)),
        CERT_E_REVOKED | CRYPT_E_REVOKED | TRUST_E_EXPLICIT_DISTRUST => {
            Err(invalid(Invalid::Revoked))
        }
        CRYPT_E_REVOCATION_OFFLINE
        | CRYPT_E_NO_REVOCATION_CHECK
        | CRYPT_E_NO_REVOCATION_DLL
        | CERT_E_REVOCATION_FAILURE => Err(unknown(Unknown::RevocationUnavailable)),
        TRUST_E_TIME_STAMP | TRUST_E_COUNTER_SIGNER => Err(invalid(Invalid::TimestampRejected)),
        TRUST_E_BAD_DIGEST
        | TRUST_E_CERT_SIGNATURE
        | TRUST_E_MALFORMED_SIGNATURE
        | TRUST_E_SUBJECT_NOT_TRUSTED
        | NTE_BAD_ALGID => Err(invalid(Invalid::SignatureRejected)),
        CERT_E_EXPIRED
        | CERT_E_WRONG_USAGE
        | CERT_E_UNTRUSTEDROOT
        | CERT_E_UNTRUSTEDCA
        | CERT_E_UNTRUSTEDTESTROOT
        | CERT_E_INVALID_POLICY
        | CERT_E_MALFORMED
        | TRUST_E_BASIC_CONSTRAINTS => Err(invalid(Invalid::ChainRejected)),
        TRUST_E_PROVIDER_UNKNOWN | TRUST_E_ACTION_UNKNOWN => {
            Err(unknown(Unknown::ProviderUnavailable))
        }
        _ => Err(unknown(Unknown::UnknownWindowsStatus)),
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

fn filetime(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}
