use crate::{Acquisition, AppIdentity, Lease, UnavailableReason as Unavailable};
use ring::digest;
use std::{
    cell::RefCell, collections::HashSet, ffi::c_void, marker::PhantomData, mem::size_of, ptr,
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, LocalFree, ERROR_ACCESS_DENIED, ERROR_INSUFFICIENT_BUFFER,
        ERROR_INVALID_HANDLE, ERROR_NO_TOKEN, ERROR_SUCCESS, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0,
        WAIT_TIMEOUT,
    },
    Security::{
        Authorization::{
            ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
            GetSecurityInfo, SE_KERNEL_OBJECT,
        },
        EqualSid, GetAce, GetLengthSid, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
        GetSecurityDescriptorOwner, GetTokenInformation, IsValidAcl, IsValidSid, TokenUser,
        ACCESS_ALLOWED_ACE, ACE_HEADER, ACL, DACL_SECURITY_INFORMATION, OWNER_SECURITY_INFORMATION,
        SECURITY_ATTRIBUTES, SE_DACL_PRESENT, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER,
    },
    System::Threading::{
        CreateMutexExW, GetCurrentProcess, GetCurrentThread, GetCurrentThreadId, OpenProcessToken,
        OpenThreadToken, ReleaseMutex, WaitForSingleObject, MUTEX_MODIFY_STATE,
        SYNCHRONIZATION_READ_CONTROL, SYNCHRONIZATION_SYNCHRONIZE,
    },
};

const ACCESS: u32 = MUTEX_MODIFY_STATE | SYNCHRONIZATION_READ_CONTROL | SYNCHRONIZATION_SYNCHRONIZE;
const MAX_TOKEN_BYTES: usize = 4096;
const MAX_SID_CHARACTERS: usize = 256;

thread_local! {
    static HELD_IDENTITIES: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: only successfully opened, uniquely owned native handles enter here.
        unsafe { CloseHandle(self.0) };
    }
}

struct Allocation(*mut c_void);
impl Drop for Allocation {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: these allocations originate from LocalAlloc-based Win32 APIs.
            unsafe { LocalFree(self.0) };
        }
    }
}

struct UserIdentity {
    token_information: Vec<usize>,
    sid_text: String,
}

impl UserIdentity {
    fn sid(&self) -> *mut c_void {
        // SAFETY: successfully queried TokenUser data is aligned, bounded, and
        // retained in this allocation for the lifetime of the returned SID.
        unsafe {
            (*self.token_information.as_ptr().cast::<TOKEN_USER>())
                .User
                .Sid
        }
    }

    fn current() -> Result<Self, Unavailable> {
        let mut raw = 0;
        // SAFETY: query the effective thread token, using self access for the query.
        if unsafe { OpenThreadToken(GetCurrentThread(), TOKEN_QUERY, 1, &mut raw) } == 0 {
            // SAFETY: consume the failed token query's error before another OS call.
            if unsafe { GetLastError() } != ERROR_NO_TOKEN {
                return Err(Unavailable::IdentityUnavailable);
            }
            // SAFETY: threads without impersonation use the process's primary token.
            if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut raw) } == 0 {
                return Err(Unavailable::IdentityUnavailable);
            }
        }
        let token = Handle(raw);
        let mut length = 0;
        // SAFETY: a zero-length query requests the required TokenUser buffer size.
        let probed =
            unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut length) };
        // SAFETY: inspect the immediately preceding failed size probe.
        if probed != 0
            || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER
            || (length as usize) < size_of::<TOKEN_USER>()
            || length as usize > MAX_TOKEN_BYTES
        {
            return Err(Unavailable::IdentityUnavailable);
        }
        let mut token_information = vec![0usize; (length as usize).div_ceil(size_of::<usize>())];
        let capacity = (token_information.len() * size_of::<usize>()) as u32;
        // SAFETY: aligned storage of at least the requested bounded size.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                token_information.as_mut_ptr().cast(),
                capacity,
                &mut length,
            )
        } == 0
            || length > capacity
            || (length as usize) < size_of::<TOKEN_USER>()
        {
            return Err(Unavailable::IdentityUnavailable);
        }
        let mut user = Self {
            token_information,
            sid_text: String::new(),
        };
        // SAFETY: successful TokenUser results contain a SID in the retained buffer.
        if user.sid().is_null()
            || unsafe { IsValidSid(user.sid()) } == 0
            || !(8..=68).contains(&unsafe { GetLengthSid(user.sid()) })
        {
            return Err(Unavailable::IdentityUnavailable);
        }
        let mut text = ptr::null_mut();
        // SAFETY: a valid retained SID and a writable allocation output.
        if unsafe { ConvertSidToStringSidW(user.sid(), &mut text) } == 0 || text.is_null() {
            return Err(Unavailable::IdentityUnavailable);
        }
        let allocation = Allocation(text.cast());
        let mut length = 0;
        // SAFETY: ConvertSidToStringSidW returns a terminated SID string. The
        // extra bound also rejects any unexpectedly long OS representation.
        while length < MAX_SID_CHARACTERS && unsafe { *text.add(length) } != 0 {
            length += 1;
        }
        if length == MAX_SID_CHARACTERS {
            return Err(Unavailable::IdentityUnavailable);
        }
        // SAFETY: the allocation remains live and length precedes its terminator.
        user.sid_text = String::from_utf16(unsafe { std::slice::from_raw_parts(text, length) })
            .map_err(|_| Unavailable::IdentityUnavailable)?;
        drop(allocation);
        Ok(user)
    }
}

fn object_name(identity: &AppIdentity, sid: &str) -> String {
    let mut hash = digest::Context::new(&digest::SHA256);
    hash.update(b"FBForge coordination v1\0");
    hash.update(identity.as_str().as_bytes());
    hash.update(&[0]);
    hash.update(sid.as_bytes());
    let digest = hash.finish();
    let mut name = String::from("Global\\FBForge.Coordination.v1.");
    for byte in digest.as_ref() {
        use std::fmt::Write;
        // String formatting cannot fail.
        write!(name, "{byte:02x}").expect("writing into a String cannot fail");
    }
    name
}

fn descriptor_text(user: &UserIdentity) -> String {
    let mut grants = format!("(A;;0x00120001;;;{})", user.sid_text);
    if user.sid_text != "S-1-5-18" {
        grants.push_str("(A;;0x00120001;;;SY)");
    }
    format!("O:{}D:P{grants}", user.sid_text)
}

fn descriptor(user: &UserIdentity) -> Result<Allocation, Unavailable> {
    let sddl: Vec<u16> = descriptor_text(user)
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let mut descriptor = Allocation(ptr::null_mut());
    // SAFETY: only the OS-derived SID is interpolated into fixed owner/DACL SDDL.
    if unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            sddl.as_ptr(),
            1,
            &mut descriptor.0,
            ptr::null_mut(),
        )
    } == 0
    {
        return Err(Unavailable::OperatingSystem);
    }
    Ok(descriptor)
}

fn require_private_object(
    handle: &Handle,
    user: &UserIdentity,
    expected: &Allocation,
) -> Result<(), Unavailable> {
    let mut actual = Allocation(ptr::null_mut());
    // SAFETY: query the exact opened kernel object, with READ_CONTROL access.
    if unsafe {
        GetSecurityInfo(
            handle.0,
            SE_KERNEL_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
            &mut actual.0,
        )
    } != ERROR_SUCCESS
        || actual.0.is_null()
    {
        return Err(Unavailable::UntrustedObject);
    }
    require_private_descriptor(&actual, user, expected)
}

fn require_private_descriptor(
    actual: &Allocation,
    user: &UserIdentity,
    expected: &Allocation,
) -> Result<(), Unavailable> {
    let mut owner = ptr::null_mut();
    let mut defaulted = 0;
    // SAFETY: actual is an OS-created descriptor retained through this check.
    if unsafe { GetSecurityDescriptorOwner(actual.0, &mut owner, &mut defaulted) } == 0
        || owner.is_null()
        || unsafe { EqualSid(owner, user.sid()) } == 0
    {
        return Err(Unavailable::UntrustedObject);
    }
    let mut control = 0;
    let mut revision = 0;
    // SAFETY: descriptor is OS-returned and both scalar outputs are writable.
    if unsafe { GetSecurityDescriptorControl(actual.0, &mut control, &mut revision) } == 0
        || control & (SE_DACL_PRESENT | SE_DACL_PROTECTED) != SE_DACL_PRESENT | SE_DACL_PROTECTED
    {
        return Err(Unavailable::UntrustedObject);
    }
    let mut acl = ptr::null_mut();
    let mut present = 0;
    // SAFETY: actual descriptor remains live through ACL validation below.
    if unsafe { GetSecurityDescriptorDacl(actual.0, &mut present, &mut acl, &mut defaulted) } == 0
        || present == 0
        || acl.is_null()
    {
        return Err(Unavailable::UntrustedObject);
    }
    let mut expected_acl = ptr::null_mut();
    // SAFETY: this descriptor comes from the fixed SDDL constructor above.
    if unsafe {
        GetSecurityDescriptorDacl(expected.0, &mut present, &mut expected_acl, &mut defaulted)
    } == 0
        || present == 0
        || expected_acl.is_null()
    {
        return Err(Unavailable::OperatingSystem);
    }
    // SAFETY: live descriptors back both ACLs throughout validation.
    unsafe { require_exact_acl(acl, expected_acl) }
}

unsafe fn require_exact_acl(actual: *const ACL, expected: *const ACL) -> Result<(), Unavailable> {
    // SAFETY: caller supplies OS-returned ACLs backed by retained descriptors.
    if unsafe { IsValidAcl(actual) } == 0 || unsafe { IsValidAcl(expected) } == 0 {
        return Err(Unavailable::UntrustedObject);
    }
    // SAFETY: validated ACL headers are readable in their retained buffers.
    let count = unsafe { (*expected).AceCount };
    if !(1..=2).contains(&count) || unsafe { (*actual).AceCount } != count {
        return Err(Unavailable::UntrustedObject);
    }
    let mut seen = 0u8;
    for index in 0..u32::from(count) {
        // SAFETY: indices are bounded by their validated ACL counts.
        let sid = unsafe { allowed_sid(actual, index) }?;
        let mut matched = false;
        for expected_index in 0..u32::from(count) {
            // SAFETY: same validated expected ACL and bounded index.
            let expected_sid = unsafe { allowed_sid(expected, expected_index) }?;
            // SAFETY: allowed_sid validates SID structure and ACE bounds.
            if unsafe { EqualSid(sid, expected_sid) } != 0 {
                let bit = 1 << expected_index;
                if seen & bit != 0 {
                    return Err(Unavailable::UntrustedObject);
                }
                seen |= bit;
                matched = true;
                break;
            }
        }
        if !matched {
            return Err(Unavailable::UntrustedObject);
        }
    }
    Ok(())
}

unsafe fn allowed_sid(acl: *const ACL, index: u32) -> Result<*mut c_void, Unavailable> {
    let mut pointer = ptr::null_mut();
    // SAFETY: caller supplies a validated ACL and a bounded ACE index.
    if unsafe { GetAce(acl, index, &mut pointer) } == 0 || pointer.is_null() {
        return Err(Unavailable::UntrustedObject);
    }
    // SAFETY: GetAce provides at least an ACE_HEADER in the retained ACL.
    let header = unsafe { &*pointer.cast::<ACE_HEADER>() };
    if header.AceType != 0 || header.AceFlags != 0 || header.AceSize < 16 {
        return Err(Unavailable::UntrustedObject);
    }
    // SAFETY: only ordinary allow ACEs have this fixed mask and SID layout.
    let ace = unsafe { &*pointer.cast::<ACCESS_ALLOWED_ACE>() };
    let sid = unsafe { pointer.cast::<u8>().add(8) };
    let sid_length = 8 + 4 * usize::from(unsafe { *sid.add(1) });
    if ace.Mask != ACCESS
        || sid_length != usize::from(header.AceSize) - 8
        || unsafe { IsValidSid(sid.cast()) } == 0
    {
        return Err(Unavailable::UntrustedObject);
    }
    Ok(sid.cast())
}

pub(super) struct NativeLease {
    handle: Handle,
    name: String,
    owner_thread: u32,
}

impl NativeLease {
    pub(super) fn is_current_thread_owner(&self) -> bool {
        // SAFETY: thread identity lookup has no side effects.
        self.owner_thread == unsafe { GetCurrentThreadId() }
    }
}
impl Drop for NativeLease {
    fn drop(&mut self) {
        // SAFETY: the public !Send/!Sync guard prevents safe thread transfer.
        if unsafe { GetCurrentThreadId() } == self.owner_thread {
            // SAFETY: this guard owns one successful wait on this exact thread.
            unsafe { ReleaseMutex(self.handle.0) };
            let _ = HELD_IDENTITIES.try_with(|held| held.borrow_mut().remove(&self.name));
        }
        // Handle's Drop runs after this method, never before ReleaseMutex.
    }
}

pub(super) fn try_acquire(identity: &AppIdentity) -> Acquisition {
    match acquire(identity) {
        Ok(outcome) => outcome,
        Err(reason) => Acquisition::Unavailable(reason),
    }
}

fn acquire(identity: &AppIdentity) -> Result<Acquisition, Unavailable> {
    let user = UserIdentity::current()?;
    let name = object_name(identity, &user.sid_text);
    if HELD_IDENTITIES
        .try_with(|held| held.borrow().contains(&name))
        .map_err(|_| Unavailable::OperatingSystem)?
    {
        return Ok(Acquisition::Busy);
    }
    let descriptor = descriptor(&user)?;
    let attributes = SECURITY_ATTRIBUTES {
        nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: 0,
    };
    let name_wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    // SAFETY: retained descriptor/name, minimum rights, and no initial ownership.
    let raw = unsafe { CreateMutexExW(&attributes, name_wide.as_ptr(), 0, ACCESS) };
    if raw == 0 {
        // SAFETY: consume the failed create/open call's error immediately.
        return Err(match unsafe { GetLastError() } {
            ERROR_ACCESS_DENIED | ERROR_INVALID_HANDLE => Unavailable::UntrustedObject,
            _ => Unavailable::OperatingSystem,
        });
    }
    let handle = Handle(raw);
    require_private_object(&handle, &user, &descriptor)?;
    // SAFETY: a protected query/synchronize-capable mutex handle, nonblocking wait.
    let result = unsafe { WaitForSingleObject(handle.0, 0) };
    let requires_rescan = match result {
        WAIT_OBJECT_0 => false,
        WAIT_ABANDONED => true,
        WAIT_TIMEOUT => return Ok(Acquisition::Busy),
        _ => return Err(Unavailable::OperatingSystem),
    };
    // Construct the releasing guard before tracking ownership in thread-local state.
    let native = NativeLease {
        handle,
        name,
        // SAFETY: reads the current OS thread's identity without side effects.
        owner_thread: unsafe { GetCurrentThreadId() },
    };
    HELD_IDENTITIES
        .try_with(|held| held.borrow_mut().insert(native.name.clone()))
        .map_err(|_| Unavailable::OperatingSystem)?;
    let lease = Lease {
        _native: native,
        requires_rescan,
        app_id: identity.clone(),
        _thread_bound: PhantomData,
    };
    Ok(if requires_rescan {
        Acquisition::AbandonedNeedsRescan(lease)
    } else {
        Acquisition::Acquired(lease)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        mem::ManuallyDrop,
        sync::atomic::{AtomicU64, Ordering},
        thread,
    };
    use windows_sys::Win32::{
        Foundation::ERROR_FILE_NOT_FOUND,
        System::Threading::{CreateEventW, OpenMutexW},
    };

    fn identity() -> AppIdentity {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        AppIdentity::new(&format!(
            "example.coordination.kernel-fixture.{}.{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ))
        .unwrap()
    }

    fn parse_sddl(text: &str) -> Allocation {
        let text: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        let mut descriptor = Allocation(ptr::null_mut());
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

    fn precreate(identity: &AppIdentity, user: &UserIdentity, descriptor: &Allocation) -> Handle {
        let name: Vec<u16> = object_name(identity, &user.sid_text)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let handle = Handle(unsafe { CreateMutexExW(&attributes, name.as_ptr(), 0, ACCESS) });
        assert_ne!(handle.0, 0);
        handle
    }

    #[test]
    fn namespace_is_global_stable_and_separates_identity_and_user() {
        let app = AppIdentity::new("EXAMPLE.Tool").unwrap();
        let name = object_name(&app, "S-1-5-21-1-2-3-1001");
        assert_eq!(
            name,
            "Global\\FBForge.Coordination.v1.2339d93c361242b18293f44491c28d4faf794a615d96b2479c51ba1e5fe57f23"
        );
        assert!(name.starts_with("Global\\FBForge.Coordination.v1."));
        assert_eq!(name.len(), "Global\\FBForge.Coordination.v1.".len() + 64);
        assert_eq!(
            name,
            object_name(
                &AppIdentity::new("example.tool").unwrap(),
                "S-1-5-21-1-2-3-1001"
            )
        );
        assert_ne!(name, object_name(&app, "S-1-5-21-1-2-3-1002"));
        assert_ne!(
            name,
            object_name(
                &AppIdentity::new("example.other").unwrap(),
                "S-1-5-21-1-2-3-1001"
            )
        );
    }

    #[test]
    fn actual_mutex_blocks_recursive_and_other_thread_ownership_then_releases() {
        let id = identity();
        let user = UserIdentity::current().unwrap();
        let descriptor = descriptor(&user).unwrap();
        // Keep the same kernel object alive across Drop so recreation cannot
        // conceal a missing ReleaseMutex call.
        let _observer = precreate(&id, &user, &descriptor);
        let lease = match Lease::try_acquire(&id) {
            Acquisition::Acquired(lease) => lease,
            result => panic!("{result:?}"),
        };
        assert!(!lease.requires_rescan());
        assert!(matches!(Lease::try_acquire(&id), Acquisition::Busy));
        let second = id.clone();
        assert!(
            thread::spawn(move || matches!(Lease::try_acquire(&second), Acquisition::Busy))
                .join()
                .unwrap()
        );
        drop(lease);
        let third = id.clone();
        assert!(thread::spawn(move || matches!(
            Lease::try_acquire(&third),
            Acquisition::Acquired(_)
        ))
        .join()
        .unwrap());
        assert!(matches!(Lease::try_acquire(&id), Acquisition::Acquired(_)));
    }

    #[test]
    fn dropping_the_last_guard_closes_the_named_object_handle() {
        let id = identity();
        let user = UserIdentity::current().unwrap();
        let name: Vec<u16> = object_name(&id, &user.sid_text)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let lease = match Lease::try_acquire(&id) {
            Acquisition::Acquired(lease) => lease,
            result => panic!("{result:?}"),
        };
        drop(lease);
        let reopened = unsafe { OpenMutexW(ACCESS, 0, name.as_ptr()) };
        let error = unsafe { GetLastError() };
        if reopened != 0 {
            drop(Handle(reopened));
        }
        assert_eq!(reopened, 0, "the final handle was not closed");
        assert_eq!(error, ERROR_FILE_NOT_FOUND);
    }

    #[test]
    fn terminated_owner_thread_produces_a_held_abandoned_lease() {
        let id = identity();
        let second = id.clone();
        let raw = thread::spawn(move || {
            let lease = match Lease::try_acquire(&second) {
                Acquisition::Acquired(lease) => lease,
                result => panic!("{result:?}"),
            };
            // Deliberately skip Drop while this real OS owner thread exits.
            // Return its handle so the test closes it after abandoned recovery.
            let lease = ManuallyDrop::new(lease);
            lease._native.handle.0
        })
        .join()
        .unwrap();
        let _retained = Handle(raw);
        let lease = match Lease::try_acquire(&id) {
            Acquisition::AbandonedNeedsRescan(lease) => lease,
            result => panic!("{result:?}"),
        };
        assert!(lease.requires_rescan());
        assert!(matches!(Lease::try_acquire(&id), Acquisition::Busy));
        drop(lease);
        assert!(matches!(Lease::try_acquire(&id), Acquisition::Acquired(_)));
    }

    #[test]
    fn broader_precreated_dacl_is_rejected_without_taking_ownership() {
        let id = identity();
        let user = UserIdentity::current().unwrap();
        let broad = parse_sddl(&format!("O:{}D:P(A;;0x00120001;;;WD)", user.sid_text));
        let handle = precreate(&id, &user, &broad);
        assert!(matches!(
            Lease::try_acquire(&id),
            Acquisition::Unavailable(Unavailable::UntrustedObject)
        ));
        // No ownership was taken before rejecting the untrusted object.
        assert_eq!(unsafe { WaitForSingleObject(handle.0, 0) }, WAIT_OBJECT_0);
        assert_ne!(unsafe { ReleaseMutex(handle.0) }, 0);
    }

    #[test]
    fn foreign_owner_is_rejected_even_with_the_expected_private_acl() {
        let user = UserIdentity::current().unwrap();
        let expected = descriptor(&user).unwrap();
        let actual = parse_sddl(&descriptor_text(&user).replacen(
            &format!("O:{}", user.sid_text),
            "O:S-1-5-21-101-202-303-404",
            1,
        ));
        assert_ne!(user.sid_text, "S-1-5-21-101-202-303-404");
        // This is the same OS descriptor policy used after GetSecurityInfo.
        // It does not require privileges to assign a genuinely foreign owner.
        assert_eq!(
            require_private_descriptor(&actual, &user, &expected),
            Err(Unavailable::UntrustedObject)
        );
    }

    #[test]
    fn precreated_object_of_another_kernel_type_is_rejected() {
        let id = identity();
        let user = UserIdentity::current().unwrap();
        let descriptor = descriptor(&user).unwrap();
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        let name: Vec<u16> = object_name(&id, &user.sid_text)
            .encode_utf16()
            .chain(Some(0))
            .collect();
        let event = Handle(unsafe { CreateEventW(&attributes, 1, 0, name.as_ptr()) });
        assert_ne!(event.0, 0);
        assert!(matches!(
            Lease::try_acquire(&id),
            Acquisition::Unavailable(Unavailable::UntrustedObject)
        ));
    }
}
