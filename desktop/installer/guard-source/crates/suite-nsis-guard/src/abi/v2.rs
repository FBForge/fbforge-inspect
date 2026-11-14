//! Additive Unicode NSIS protocol2. Output allocation precedes every request.
use super::{
    argument_bounded, claim_protocol, error, pin_self, resident, Extra, Output, Stack, API_VERSION,
    API_VERSION_SLOT, FAILED, TIMEOUT,
};
use crate::{
    mutation::{Code, Input, Operation, Session},
    session::{deny_acquire, initial_phase},
};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc, Mutex, OnceLock,
    },
};
use windows_sys::Win32::Foundation::HWND;
static SESSION: OnceLock<Mutex<Session>> = OnceLock::new();
static PHASE: OnceLock<Arc<AtomicU8>> = OnceLock::new();
fn phase() -> &'static Arc<AtomicU8> {
    PHASE.get_or_init(initial_phase)
}
#[derive(Clone, Copy)]
enum Request {
    Prepare,
    Poll,
    Path,
    Publish,
    Finalize,
    Release,
}

macro_rules! export {
    ($name:ident,$request:ident) => {
        /// # Safety
        /// Live Unicode NSIS3.13 API1 structures and the documented V2 arguments
        /// are required. No pointer is retained after this call.
        #[no_mangle]
        pub unsafe extern "C" fn $name(
            _: HWND,
            size: i32,
            _: *mut u16,
            stack: *mut *mut Stack,
            extra: *mut Extra,
        ) {
            // SAFETY: the export forwards the documented NSIS pointer contract.
            unsafe { entry(size, stack, extra, Request::$request) };
        }
    };
}
export!(PrepareV2, Prepare);
export!(PollV2, Poll);
export!(PathV2, Path);
export!(PublishV2, Publish);
export!(FinalizeV2, Finalize);
export!(ReleaseV2, Release);

unsafe fn reject(request: Request, extra: *mut Extra) {
    if matches!(request, Request::Prepare) {
        deny_acquire(phase());
    }
    // SAFETY: error uses only null-checked call-local SDK flags.
    unsafe { error(extra) };
}
unsafe fn entry(size: i32, stack: *mut *mut Stack, extra: *mut Extra, request: Request) {
    // The same fixed DLL address is pinned BEFORE V2 parsing or owner creation.
    if pin_self().is_none() {
        unsafe { reject(request, extra) };
        return;
    }
    if matches!(request, Request::Prepare) && !claim_protocol(2) {
        unsafe { reject(request, extra) };
        return;
    }
    if !(1024..=32768).contains(&size) || stack.is_null() {
        unsafe { reject(request, extra) };
        return;
    }
    let Some(output) = Output::allocate(size as usize) else {
        unsafe { reject(request, extra) };
        return;
    };
    let result = catch_unwind(AssertUnwindSafe(|| -> Option<(String, bool)> {
        // Consume exactly this call's own arguments; native pointers never enter
        // the worker. Prepare parameters follow NSIS's left-to-right pop order.
        let input = if matches!(request, Request::Prepare) {
            let tool = unsafe { argument_bounded(stack, size as usize, 129) }?;
            let directory = unsafe { argument_bounded(stack, size as usize, 961) }?;
            let components = unsafe { argument_bounded(stack, size as usize, 520) }?;
            Some(Input::parse(tool, directory, components)?)
        } else {
            None
        };
        let path_kind = if matches!(request, Request::Path) {
            Some(unsafe { argument_bounded(stack, size as usize, 8) }?)
        } else {
            None
        };
        if extra.is_null() || unsafe { (*extra).flags }.is_null() {
            return None;
        }
        let version = unsafe { (*(*extra).flags).values[API_VERSION_SLOT] };
        if version < API_VERSION || version >> 16 != 1 || !claim_protocol(2) {
            return None;
        }
        if FAILED.load(Ordering::Acquire) && !matches!(request, Request::Release) {
            return None;
        }
        if matches!(request, Request::Prepare) && !unsafe { resident(extra) } {
            return None;
        }
        let Ok(mut session) = SESSION
            .get_or_init(|| Mutex::new(Session::with_lifecycle(Arc::clone(phase()))))
            .try_lock()
        else {
            return None;
        };
        let code = match request {
            Request::Prepare => session.prepare(input?),
            Request::Poll => session.poll(),
            Request::Path => {
                return session
                    .path(path_kind.as_deref()?)
                    .map(|value| (value, true))
            }
            Request::Publish => session.operation(Operation::Publish),
            Request::Finalize => session.operation(Operation::Finalize),
            Request::Release => session.release(TIMEOUT),
        };
        Some((code.text().to_string(), code.success()))
    }));
    let (text, success) = match result {
        Ok(Some((text, success))) if text.encode_utf16().count() < size as usize => (text, success),
        Ok(_) => (Code::Unavailable.text().to_string(), false),
        Err(_) => {
            FAILED.store(true, Ordering::Release);
            deny_acquire(phase());
            (Code::Unavailable.text().to_string(), false)
        }
    };
    if !success {
        unsafe { reject(request, extra) };
    }
    // SAFETY: reply capacity was checked, allocation is private until handed
    // back to this live SDK stack. NSIS owns/GlobalFrees the result afterward.
    unsafe { output.publish_text(stack, &text) };
}
