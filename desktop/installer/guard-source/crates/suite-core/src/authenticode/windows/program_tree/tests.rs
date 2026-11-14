use super::*;
use std::{
    fs::OpenOptions,
    os::windows::fs::OpenOptionsExt,
    sync::atomic::{AtomicU64, Ordering},
};
use windows_sys::Win32::Security::{
    Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SetNamedSecurityInfoW,
    },
    InitializeSecurityDescriptor, MakeSelfRelativeSD, SetSecurityDescriptorDacl,
    SetSecurityDescriptorOwner, PROTECTED_DACL_SECURITY_INFORMATION, SECURITY_DESCRIPTOR,
    UNPROTECTED_DACL_SECURITY_INFORMATION,
};

const OTHER: &str = "S-1-5-21-101-102-103-1001";
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn descriptor(sddl: &str) -> Allocation {
    let text: Vec<u16> = sddl.encode_utf16().chain(Some(0)).collect();
    let mut descriptor = Allocation(ptr::null_mut());
    // SAFETY: fixed test SDDL is terminated and output has initialized storage.
    assert_ne!(
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                text.as_ptr(),
                1,
                &mut descriptor.0,
                ptr::null_mut(),
            )
        },
        0
    );
    descriptor
}

fn current_sid(user: &TrustedPrincipals) -> String {
    sid_string(user.user_sid.as_ptr().cast_mut().cast())
}

fn sid_string(sid: *mut c_void) -> String {
    let mut text = ptr::null_mut();
    // SAFETY: current() copied this valid SID; output receives a LocalAlloc buffer.
    assert_ne!(unsafe { ConvertSidToStringSidW(sid, &mut text) }, 0);
    let allocation = Allocation(text.cast());
    let mut length = 0;
    // SAFETY: OS produces a terminated SID string shorter than the fixed cap.
    while length < 256 && unsafe { *text.add(length) } != 0 {
        length += 1;
    }
    assert!(length < 256);
    // SAFETY: the validated terminated string spans these length code units.
    let result = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) }).unwrap();
    drop(allocation);
    result
}

fn evaluate(sddl: &str, scope: Scope) -> Result<bool, Failure> {
    let user = TrustedPrincipals::current().unwrap();
    inspect_descriptor(&descriptor(sddl), scope, &user, &mut ScanBudget::new())
}

fn null_dacl(user: &TrustedPrincipals) -> Vec<usize> {
    // Build an actual NULL DACL through SDK functions; SDDL parsers in emulators
    // can represent NO_ACCESS_CONTROL as an empty ACL, which is a different case.
    // SAFETY: initialized POD storage is large/aligned enough for an absolute SD.
    let mut absolute: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    let raw = ptr::addr_of_mut!(absolute).cast();
    // SAFETY: each call operates on the initialized stack descriptor, with live SID.
    assert_ne!(unsafe { InitializeSecurityDescriptor(raw, 1) }, 0);
    assert_ne!(
        unsafe { SetSecurityDescriptorOwner(raw, user.user_sid.as_ptr().cast_mut().cast(), 0) },
        0
    );
    assert_ne!(
        unsafe { SetSecurityDescriptorDacl(raw, 1, ptr::null(), 0) },
        0
    );
    let mut size = 0;
    // SAFETY: null/zero asks for the required self-relative output size only.
    unsafe { MakeSelfRelativeSD(raw, ptr::null_mut(), &mut size) };
    assert!(size > 0 && size <= 4096);
    let mut result = vec![0usize; (size as usize).div_ceil(size_of::<usize>())];
    // SAFETY: destination allocation has the exact requested capacity.
    assert_ne!(
        unsafe { MakeSelfRelativeSD(raw, result.as_mut_ptr().cast(), &mut size) },
        0
    );
    result
}

#[test]
fn os_descriptors_allow_foreign_readers_but_not_owners_or_writers() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FRFX;;;BU)"),
            Scope::ProgramTree
        ),
        Ok(true)
    );
    assert_eq!(
        evaluate(&format!("O:{OTHER}D:P(A;;FA;;;{sid})"), Scope::ProgramTree),
        Err(Failure::UnsafeInstallation)
    );
    for rights in [
        "FW",
        "GA",
        "WD",
        "WO",
        "0x00000010",
        "0x00000100",
        "0x00000040",
    ] {
        assert_eq!(
            evaluate(
                &format!("O:{sid}D:P(A;;FA;;;{sid})(A;;{rights};;;{OTHER})"),
                Scope::ProgramTree
            ),
            Err(Failure::UnsafeInstallation),
            "{rights}"
        );
    }
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;SY)(A;;FA;;;BA)"),
            Scope::ProgramTree
        ),
        Ok(true)
    );
    let installer = sid_string(user.installer_sid.0);
    assert_eq!(
        evaluate(
            &format!("O:{installer}D:P(A;;FA;;;{installer})(A;;FRFX;;;BU)"),
            Scope::ProgramTree
        ),
        Ok(true)
    );
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;S-1-5-80-1-2-3-4-5)"),
            Scope::ProgramTree
        ),
        Err(Failure::UnsafeInstallation)
    );
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;S-1-5-80-0)"),
            Scope::ProgramTree
        ),
        Err(Failure::UnsafeInstallation)
    );
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;{sid})(A;OIIO;GA;;;{OTHER})"),
            Scope::ProgramTree
        ),
        Ok(true)
    );
}

#[test]
fn null_and_absent_dacls_are_rejected_and_control_is_read_exactly() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    assert_eq!(
        evaluate(&format!("O:{sid}"), Scope::ProgramTree),
        Err(Failure::UnsupportedInstallation)
    );
    let mut null = null_dacl(&user);
    // Borrow this SDK-produced self-relative descriptor without LocalFree: its
    // backing allocation belongs to the live Vec above, not to LocalAlloc.
    let null = std::mem::ManuallyDrop::new(Allocation(null.as_mut_ptr().cast()));
    assert_eq!(
        inspect_descriptor(&null, Scope::ProgramTree, &user, &mut ScanBudget::new()),
        Err(Failure::UnsupportedInstallation)
    );
    assert_eq!(
        evaluate(&format!("O:{sid}D:(A;;FA;;;{sid})"), Scope::ProgramTree),
        Ok(false)
    );
    // Deny evaluation cannot turn a different unsafe allow into admission.
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(D;;FW;;;{OTHER})(A;;FA;;;{sid})(A;;FW;;;{OTHER})"),
            Scope::ProgramTree
        ),
        Err(Failure::UnsafeInstallation)
    );
}

#[test]
fn unsupported_object_ace_from_the_os_does_not_grant_admission() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(OA;;FR;00112233-4455-6677-8899-aabbccddeeff;;{sid})"),
            Scope::ProgramTree
        ),
        Err(Failure::UnsupportedInstallation)
    );
}

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let base = std::env::var_os("FBFORGE_ACL_FIXTURE_ROOT")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        fs::create_dir_all(&base).unwrap();
        let path = base.join(format!(
            "fbforge-program-tree-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        let fixture = Self(path);
        // The production contract checks every ancestor and does not require a
        // protected root. Keep this general fixture independent of that option.
        fixture.set_acl(&fixture.0, false, "");
        fixture
    }
    fn set_acl(&self, path: &Path, protected: bool, extra: &str) {
        let user = TrustedPrincipals::current().unwrap();
        let sid = current_sid(&user);
        let descriptor = descriptor(&format!("O:{sid}D:{}(A;OICI;FA;;;{sid})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FRFX;;;BU){extra}", if protected { "P" } else { "" }));
        let mut path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let flags = DACL_SECURITY_INFORMATION
            | if protected {
                PROTECTED_DACL_SECURITY_INFORMATION
            } else {
                UNPROTECTED_DACL_SECURITY_INFORMATION
            };
        let mut acl = ptr::null_mut();
        let mut present = 0;
        let mut defaulted = 0;
        // SAFETY: descriptor is OS-produced and all outputs are correctly typed.
        assert_ne!(
            unsafe {
                GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted)
            },
            0
        );
        assert_ne!(present, 0);
        // SAFETY: scratch fixture only, live descriptor and terminated mutable path.
        assert_eq!(
            unsafe {
                SetNamedSecurityInfoW(
                    path.as_mut_ptr(),
                    SE_FILE_OBJECT,
                    flags,
                    ptr::null_mut(),
                    ptr::null_mut(),
                    acl,
                    ptr::null(),
                )
            },
            ERROR_SUCCESS
        );
        let path = String::from_utf16(&path[..path.len() - 1]).unwrap();
        let file = open_read_only(Path::new(&path), FILE_FLAG_BACKUP_SEMANTICS).unwrap();
        // Verify the fixture's real native control flag rather than assuming a
        // successful ACL setter also established inheritance protection.
        let result = inspect_security(&file, Scope::Ancestor, &user, &mut ScanBudget::new());
        if extra.is_empty() {
            assert_eq!(result.unwrap(), protected);
        }
    }
    fn program(&self, root: &Path) -> LockedPackageFile {
        let executable = root.join("fixture.exe");
        fs::write(&executable, b"unsigned permission fixture; never executed").unwrap();
        LockedPackageFile::open(&executable).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn real_tree_retains_every_file_with_an_inherited_program_acl() {
    let fixture = Fixture::new();
    let root = fixture.0.join("program");
    fs::create_dir(&root).unwrap();
    fixture.set_acl(&root, false, "");
    let runtime = root.join("runtime");
    fs::create_dir(&runtime).unwrap();
    let jar = runtime.join("engine.jar");
    fs::write(&jar, b"unsigned test asset").unwrap();
    let program = fixture.program(&root);
    let tree = lock_program_tree(&program).unwrap();
    assert!(OpenOptions::new().write(true).open(&jar).is_err());
    assert!(fs::remove_file(&jar).is_err());
    drop(tree);
    assert!(OpenOptions::new().write(true).open(&jar).is_ok());
}

#[test]
fn real_tree_rejects_a_dependency_writable_by_builtin_users() {
    let fixture = Fixture::new();
    let jar = fixture.0.join("engine.jar");
    fs::write(&jar, b"unsigned test asset").unwrap();
    fixture.set_acl(&jar, false, "(A;;FW;;;BU)");
    let program = fixture.program(&fixture.0);
    assert!(matches!(
        lock_program_tree(&program),
        Err(Failure::UnsafeInstallation)
    ));
}

#[test]
fn real_tree_rejects_a_hardlinked_dependency() {
    let fixture = Fixture::new();
    let jar = fixture.0.join("engine.jar");
    fs::write(&jar, b"unsigned test asset").unwrap();
    let link = fixture.0.join("alias.jar");
    fs::hard_link(&jar, &link).unwrap();
    let program = fixture.program(&fixture.0);
    assert!(matches!(
        lock_program_tree(&program),
        Err(Failure::UnsafeInstallation)
    ));
}

#[test]
fn existing_data_writer_prevents_tree_admission() {
    let fixture = Fixture::new();
    let jar = fixture.0.join("engine.jar");
    fs::write(&jar, b"unsigned test asset").unwrap();
    let writer = OpenOptions::new()
        .write(true)
        .share_mode(7)
        .open(&jar)
        .unwrap();
    let program = fixture.program(&fixture.0);
    assert!(matches!(
        lock_program_tree(&program),
        Err(Failure::ProgramUnavailable)
    ));
    drop(writer);
    assert!(lock_program_tree(&program).is_ok());
}

fn assert_unsafe_parent_namespace(protected: bool) {
    let fixture = Fixture::new();
    let root = fixture.0.join("program");
    fs::create_dir(&root).unwrap();
    fixture.set_acl(&root, protected, "");
    let program = fixture.program(&root);
    for rights in ["0x00000040", "SD", "WD", "WO", "0x00000100", "FW", "GA"] {
        fixture.set_acl(&fixture.0, false, &format!("(A;;{rights};;;BU)"));
        assert!(
            matches!(
                lock_program_tree(&program),
                Err(Failure::UnsafeInstallation)
            ),
            "{rights}"
        );
    }
}

#[test]
fn program_acl_does_not_hide_an_unsafe_parent_namespace() {
    assert_unsafe_parent_namespace(false);
}

#[test]
fn protected_program_acl_does_not_hide_an_unsafe_parent_namespace() {
    assert_unsafe_parent_namespace(true);
}

#[test]
fn complete_tree_is_required_even_when_the_executable_itself_is_safe() {
    let fixture = Fixture::new();
    let program = fixture.program(&fixture.0);
    let mut directory = fixture.0.clone();
    for _ in 0..=crate::authenticode::program_permissions::MAX_DEPTH {
        directory = directory.join("d");
        fs::create_dir(&directory).unwrap();
    }
    assert!(matches!(
        lock_program_tree(&program),
        Err(Failure::UnsupportedInstallation)
    ));
}

#[test]
fn descriptor_ace_budget_is_checked_before_granting_admission() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    let mut sddl = format!("O:{sid}D:P(A;;FA;;;{sid})");
    for _ in 0..MAX_ACES {
        sddl.push_str("(A;;FR;;;BU)");
    }
    assert_eq!(
        evaluate(&sddl, Scope::ProgramTree),
        Err(Failure::UnsupportedInstallation)
    );
}

#[test]
fn private_mutation_descriptor_rejects_foreign_read_access() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;{sid})(A;;FRFX;;;BU)"),
            Scope::PrivateTree
        ),
        Err(Failure::UnsafeInstallation)
    );
}

#[test]
fn unsigned_filesystem_fixture_closes_files_then_swaps_whole_directories() {
    use std::io::Write;
    use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_WRITE_THROUGH};
    // Only permission/handle/rename behavior is exercised. These unsigned bytes
    // never pass a publisher verifier and no execution/installer proof is faked.
    let fixture = Fixture::new();
    let old = fixture.0.join("target");
    let stage = fixture.0.join("staging");
    let backup = fixture.0.join("backup");
    for path in [&old, &stage] {
        drop(create_private_directory(path).unwrap());
        let mut file = create_private_file(&path.join("fixture.exe")).unwrap();
        file.write_all(b"unsigned fixture, never executed").unwrap();
    }
    let mut unknown = create_private_file(&old.join("unknown-user-file.bin")).unwrap();
    unknown
        .write_all(b"retain the entire original tree")
        .unwrap();
    drop(unknown);
    let mut old_tree =
        lock_mutation_tree(LockedPackageFile::open(&old.join("fixture.exe")).unwrap()).unwrap();
    let mut stage_tree =
        lock_mutation_tree(LockedPackageFile::open(&stage.join("fixture.exe")).unwrap()).unwrap();
    assert_eq!(old_tree.file_paths().len(), 2);
    let original = old_tree.id().unwrap();
    let staged = stage_tree.id().unwrap();
    let rename = |from: &Path, to: &Path| {
        let from: Vec<_> = from.as_os_str().encode_wide().chain(Some(0)).collect();
        let to: Vec<_> = to.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: both terminated fixture-only paths remain live for the call.
        unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), MOVEFILE_WRITE_THROUGH) }
    };
    // Our file proofs retain a real no-write share until deliberately released.
    assert!(OpenOptions::new()
        .write(true)
        .open(old.join("fixture.exe"))
        .is_err());
    old_tree.close_files();
    stage_tree.close_files();
    old_tree.recheck_directories().unwrap();
    stage_tree.recheck_directories().unwrap();
    assert_ne!(rename(&old, &backup), 0);
    assert!(!old.exists());
    assert_ne!(rename(&stage, &old), 0);
    assert_eq!(
        lock_directory(&old, true, false).unwrap().id().unwrap(),
        staged
    );
    assert_eq!(
        lock_directory(&backup, true, false).unwrap().id().unwrap(),
        original
    );
    assert_eq!(
        fs::read(backup.join("unknown-user-file.bin")).unwrap(),
        b"retain the entire original tree"
    );
    // A second publication cannot replace an existing destination.
    assert_eq!(rename(&backup, &old), 0);
}

#[test]
fn temporary_child_permissions_are_bound_to_the_real_primary_token_owner() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;{sid})(A;OICIIO;GA;;;CO)"),
            Scope::Temporary,
        ),
        Ok(true)
    );
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;OICI;FA;;;{sid})(A;OICIIO;FW;;;BU)"),
            Scope::Temporary,
        ),
        Err(Failure::UnsafeInstallation)
    );
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;{sid})(A;OICINPIO;GA;;;CO)"),
            Scope::Temporary,
        ),
        Err(Failure::UnsupportedInstallation)
    );
    assert_eq!(
        evaluate(
            &format!("O:{sid}D:P(A;;FA;;;{sid})(A;OICI;FRFX;;;BU)"),
            Scope::Temporary,
        ),
        Err(Failure::UnsupportedInstallation)
    );
}

#[test]
fn temporary_directory_guard_allows_private_child_creation_and_file_write() {
    let fixture = Fixture::new();
    let temporary = fixture.0.join("temporary");
    drop(create_private_directory(&temporary).unwrap());
    let guard = lock_installer_temporary_directory(&temporary).unwrap();
    guard.recheck_temporary().unwrap();
    let child = temporary.join("plugins");
    fs::create_dir(&child).unwrap();
    fs::write(
        child.join("fixture.bin"),
        b"unsigned private NSIS child fixture",
    )
    .unwrap();
    assert_eq!(
        fs::read(child.join("fixture.bin")).unwrap(),
        b"unsigned private NSIS child fixture"
    );
    guard.recheck_temporary().unwrap();
}

#[test]
fn installer_host_owner_rights_are_bound_and_do_not_widen_program_or_temp_policy() {
    let user = TrustedPrincipals::current().unwrap();
    let sid = current_sid(&user);
    let own = format!("O:{sid}D:P(A;;FA;;;OW)(A;;FA;;;SY)");
    assert_eq!(evaluate(&own, Scope::InstallerHost), Ok(true));
    assert_eq!(evaluate(&own, Scope::InstallerAncestor), Ok(true));
    let foreign = format!("O:{OTHER}D:P(A;;FA;;;OW)(A;;FRFX;;;{sid})");
    assert!(evaluate(&foreign, Scope::InstallerHost).is_err());
    assert!(evaluate(&foreign, Scope::InstallerAncestor).is_err());
    for scope in [
        Scope::ProgramTree,
        Scope::PrivateTree,
        Scope::Ancestor,
        Scope::Temporary,
    ] {
        assert!(evaluate(&own, scope).is_err());
    }
    assert!(evaluate(&format!("O:{sid}D:P(A;;FA;;;CO)"), Scope::InstallerHost).is_err());
}
