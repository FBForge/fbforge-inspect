//! NSIS 3.13 API 1.0, Unicode. All NSIS-owned pointers are call-local.
mod v2;
use crate::session::{deny_acquire, initial_phase, Reply, Session};
use std::{
    mem::offset_of,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr,
    sync::{
        atomic::{AtomicBool, AtomicU8, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::{GlobalFree, HMODULE, HWND},
    System::{
        LibraryLoader::{
            GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_PIN,
        },
        Memory::{GlobalAlloc, GPTR},
    },
};

const TIMEOUT: Duration = Duration::from_secs(2);
const API_VERSION: i32 = 0x0001_0000;
const API_VERSION_SLOT: usize = 7;
const ERROR_SLOT: usize = 2;

#[repr(C)]
pub(crate) struct Stack {
    next: *mut Stack,
    text: [u16; 1],
}
#[repr(C)]
struct ExecFlags {
    values: [i32; 14],
}
type Callback = unsafe extern "C" fn(i32) -> usize;
#[repr(C)]
pub(crate) struct Extra {
    flags: *mut ExecFlags,
    execute: Option<unsafe extern "system" fn(i32, HWND) -> i32>,
    validate: Option<unsafe extern "system" fn(*mut u16)>,
    register: Option<unsafe extern "system" fn(HMODULE, Callback) -> i32>,
}
static SESSION: OnceLock<Mutex<Session>> = OnceLock::new();
static PHASE: OnceLock<Arc<std::sync::atomic::AtomicU8>> = OnceLock::new();
static PINNED: OnceLock<HMODULE> = OnceLock::new();
static REGISTERED: OnceLock<bool> = OnceLock::new();
static FAILED: AtomicBool = AtomicBool::new(false);
static PROTOCOL: AtomicU8 = AtomicU8::new(0);
fn claim_protocol(version: u8) -> bool {
    PROTOCOL
        .compare_exchange(0, version, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
        || PROTOCOL.load(Ordering::Acquire) == version
}

// The SDK declares export calls as cdecl with five arguments. The NSISCALL
// helpers inside Extra are stdcall on x86; the registered callback is cdecl.
/// # Safety
/// NSIS must provide live Unicode API 1.0 call structures and one stack argument.
#[no_mangle]
pub unsafe extern "C" fn AcquireV1(
    _: HWND,
    string_size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    extra: *mut Extra,
) {
    // SAFETY: callers must supply the NSIS SDK's live, writable call structures.
    unsafe { entry(string_size, stack, extra, Operation::Acquire) };
}
/// # Safety
/// NSIS must provide live Unicode API 1.0 call structures, without arguments.
#[no_mangle]
pub unsafe extern "C" fn StatusV1(
    _: HWND,
    string_size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    extra: *mut Extra,
) {
    // SAFETY: the same SDK precondition as AcquireV1; no stack arguments consumed.
    unsafe { entry(string_size, stack, extra, Operation::Status) };
}
/// # Safety
/// Call structures must be live and all protected mutations must have finished.
#[no_mangle]
pub unsafe extern "C" fn ReleaseV1(
    _: HWND,
    string_size: i32,
    _: *mut u16,
    stack: *mut *mut Stack,
    extra: *mut Extra,
) {
    // SAFETY: the caller promises all protected mutations have finished.
    unsafe { entry(string_size, stack, extra, Operation::Release) };
}
enum Operation {
    Acquire,
    Status,
    Release,
}

unsafe fn error(extra: *mut Extra) {
    // SAFETY: non-null Extra/flags belong to the live SDK call, not the worker.
    if !extra.is_null() && !unsafe { (*extra).flags }.is_null() {
        unsafe { (*(*extra).flags).values[ERROR_SLOT] = 1 };
    }
}

struct Output {
    node: *mut Stack,
}
impl Drop for Output {
    fn drop(&mut self) {
        if !self.node.is_null() {
            // SAFETY: this private GPTR allocation has not been handed to NSIS.
            unsafe { GlobalFree(self.node.cast()) };
        }
    }
}
impl Output {
    fn allocate(string_size: usize) -> Option<Self> {
        let bytes = offset_of!(Stack, text).checked_add(string_size.checked_mul(2)?)?;
        // SAFETY: bounded size, zero-initialized fixed allocation required by NSIS.
        let node = unsafe { GlobalAlloc(GPTR, bytes) }.cast::<Stack>();
        (!node.is_null()).then_some(Self { node })
    }
    unsafe fn publish(self, stack: *mut *mut Stack, reply: Reply) {
        // SAFETY: fixed V1 text and unchanged SDK preconditions.
        unsafe { self.publish_text(stack, reply.text()) };
    }
    unsafe fn publish_text(mut self, stack: *mut *mut Stack, value: &str) {
        // SAFETY: the fixed status fits the validated SDK capacity. GlobalAlloc
        // provides proper alignment; NSIS owns and GlobalFrees the node afterward.
        let text = unsafe { ptr::addr_of_mut!((*self.node).text).cast::<u16>() };
        for (index, character) in value.encode_utf16().chain(Some(0)).enumerate() {
            unsafe { *text.add(index) = character };
        }
        unsafe {
            (*self.node).next = *stack;
            *stack = self.node
        };
        self.node = ptr::null_mut();
    }
}

unsafe fn argument(stack: *mut *mut Stack, size: usize) -> Option<String> {
    // SAFETY: unchanged NSIS stack preconditions and V1 argument cap.
    unsafe { argument_bounded(stack, size, 129) }
}
unsafe fn argument_bounded(stack: *mut *mut Stack, size: usize, bound: usize) -> Option<String> {
    // SAFETY: Stack nodes and text capacity follow the live Unicode SDK contract.
    let node = unsafe { *stack };
    if node.is_null() {
        return None;
    }
    unsafe { *stack = (*node).next };
    let allocation = Output { node };
    let text = unsafe { ptr::addr_of!((*allocation.node).text).cast::<u16>() };
    let mut value = Vec::new();
    for index in 0..size.min(bound) {
        let character = unsafe { *text.add(index) };
        if character == 0 {
            return String::from_utf16(&value).ok();
        }
        value.push(character);
    }
    None
}

fn phase() -> &'static Arc<std::sync::atomic::AtomicU8> {
    PHASE.get_or_init(initial_phase)
}

fn pin_self() -> Option<HMODULE> {
    let module = *PINNED.get_or_init(|| {
        let mut module = 0;
        // SAFETY: only our fixed code address is used, before argument/Extra
        // validation. PIN preserves early sticky denials across NSIS FreeLibrary.
        if unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_PIN,
                AcquireV1 as *const () as *const u16,
                &mut module,
            )
        } == 0
        {
            return 0;
        }
        module
    });
    (module != 0).then_some(module)
}

unsafe fn resident(extra: *mut Extra) -> bool {
    *REGISTERED.get_or_init(|| {
        let Some(module) = pin_self() else {
            return false;
        };
        // SAFETY: callback registration still requires API-validated live Extra;
        // no NSIS pointer is retained by the worker or by this static result.
        let register = unsafe { (*extra).register };
        register.is_some_and(|register| unsafe { register(module, on_unload) } >= 0)
    })
}

unsafe fn reject(operation: &Operation, extra: *mut Extra) {
    if matches!(operation, Operation::Acquire) {
        deny_acquire(phase());
    }
    // SAFETY: this uses only null-checked, call-local SDK flags.
    unsafe { error(extra) };
}

unsafe fn entry(size: i32, stack: *mut *mut Stack, extra: *mut Extra, operation: Operation) {
    // SAFETY: the unchanged SDK preconditions are forwarded to the implementation.
    unsafe { entry_with_allocator(size, stack, extra, operation, Output::allocate) };
}

unsafe fn entry_with_allocator(
    size: i32,
    stack: *mut *mut Stack,
    extra: *mut Extra,
    operation: Operation,
    allocate: fn(usize) -> Option<Output>,
) {
    // A module that rejected its first Acquire must not forget that decision on
    // NSIS's ordinary FreeLibrary path. Callback registration remains later.
    if matches!(operation, Operation::Acquire) && pin_self().is_none() {
        unsafe { reject(&operation, extra) };
        return;
    }
    if matches!(operation, Operation::Acquire) && !claim_protocol(1) {
        unsafe { reject(&operation, extra) };
        return;
    }
    if PROTOCOL.load(Ordering::Acquire) == 2 {
        unsafe { reject(&operation, extra) };
        return;
    }
    if !(129..=32768).contains(&size) || stack.is_null() {
        unsafe { reject(&operation, extra) };
        return;
    }
    // Allocate the reply before any worker can grant ownership. Allocation
    // failure cannot produce a successful status or leave a stale reply.
    let Some(output) = allocate(size as usize) else {
        unsafe { reject(&operation, extra) };
        return;
    };
    let reply = catch_unwind(AssertUnwindSafe(|| {
        // SAFETY: call-local SDK pointers; no reference escapes this function.
        let identity = if matches!(operation, Operation::Acquire) {
            let Some(value) = (unsafe { argument(stack, size as usize) }) else {
                return Reply::Unavailable;
            };
            Some(value)
        } else {
            None
        };
        if extra.is_null() || unsafe { (*extra).flags }.is_null() {
            return Reply::Unavailable;
        }
        let version = unsafe { (*(*extra).flags).values[API_VERSION_SLOT] };
        if version < API_VERSION || version >> 16 != 1 {
            return Reply::Unavailable;
        }
        if FAILED.load(Ordering::Acquire) && !matches!(operation, Operation::Release) {
            return Reply::Unavailable;
        }
        if matches!(operation, Operation::Acquire) && !unsafe { resident(extra) } {
            return Reply::Unavailable;
        }
        let Ok(mut session) = SESSION
            .get_or_init(|| Mutex::new(Session::with_phase(Arc::clone(phase()))))
            .try_lock()
        else {
            return Reply::Unavailable;
        };
        match operation {
            Operation::Acquire => session.acquire(identity.as_deref().unwrap_or(""), TIMEOUT),
            Operation::Status => session.status(TIMEOUT),
            Operation::Release => session.release(TIMEOUT),
        }
    }))
    .unwrap_or_else(|_| {
        FAILED.store(true, Ordering::Release);
        if matches!(operation, Operation::Acquire) {
            deny_acquire(phase());
        }
        Reply::Unavailable
    });
    if !matches!(reply, Reply::Held | Reply::Released) {
        unsafe { reject(&operation, extra) };
    }
    unsafe { output.publish(stack, reply) };
}

unsafe extern "C" fn on_unload(_: i32) -> usize {
    // Unload notifications prove neither mutation completion nor a safe release
    // point. PIN retains the code; only explicit ReleaseV1 or process exit ends
    // ownership. Never retain or access NSIS call pointers from this callback.
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;
    #[test]
    fn output_allocation_failure_denies_a_later_valid_session() {
        let mut flags = ExecFlags { values: [0; 14] };
        flags.values[API_VERSION_SLOT] = API_VERSION;
        let mut extra = Extra {
            flags: &mut flags,
            execute: None,
            validate: None,
            register: None,
        };
        let mut top = ptr::null_mut();
        // Fault injection is private to this unit test, not an ABI or env switch.
        unsafe { entry_with_allocator(1024, &mut top, &mut extra, Operation::Acquire, |_| None) };
        assert!(top.is_null());
        assert_eq!(flags.values[ERROR_SLOT], 1);
        let mut session = Session::with_phase(Arc::clone(phase()));
        assert_eq!(
            session.acquire("example.guard.allocation", TIMEOUT),
            Reply::Unavailable
        );
        assert_eq!(session.status(TIMEOUT), Reply::Unavailable);
    }

    #[test]
    fn structures_match_the_pinned_unicode_nsis_api() {
        assert_eq!(size_of::<ExecFlags>(), 56);
        assert_eq!(offset_of!(Stack, text), size_of::<usize>());
        assert_eq!(size_of::<Extra>(), 4 * size_of::<usize>());
        assert_eq!(offset_of!(Extra, register), 3 * size_of::<usize>());
        assert!(Reply::Unavailable.text().encode_utf16().count() + 1 < 129);
    }
}
