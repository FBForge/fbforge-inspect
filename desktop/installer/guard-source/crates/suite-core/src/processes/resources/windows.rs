use super::{classify_query, ResourceUseIssue, ResourceUsers};
use std::ptr;
use windows_sys::Win32::{
    Foundation::ERROR_SUCCESS,
    System::RestartManager::{
        RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY,
    },
};

struct Session(Option<u32>);
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            // SAFETY: only our successful RmStartSession creates this handle.
            unsafe { RmEndSession(handle) };
        }
    }
}

pub(super) fn inspect(paths: &[Vec<u16>]) -> ResourceUsers {
    let mut handle = 0;
    let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
    // SAFETY: writable handle and the documented terminated-key buffer, flags=0.
    if unsafe { RmStartSession(&mut handle, 0, key.as_mut_ptr()) } != ERROR_SUCCESS {
        return unavailable();
    }
    let mut session = Session(Some(handle));
    let pointers: Vec<_> = paths.iter().map(|path| path.as_ptr()).collect();
    // SAFETY: bounded, nonempty file-only list. All terminated strings and their
    // pointer array stay alive through registration. No services or PIDs supplied.
    if unsafe {
        RmRegisterResources(
            handle,
            pointers.len() as u32,
            pointers.as_ptr(),
            0,
            ptr::null(),
            0,
            ptr::null(),
        )
    } != ERROR_SUCCESS
    {
        return unavailable();
    }
    let mut needed = 0;
    let mut returned = 0;
    let mut reboot = 0;
    // SAFETY: zero-capacity query deliberately supplies no RM_PROCESS_INFO buffer.
    // A positive required count is enough to report InUse without exposing PIDs.
    let status = unsafe {
        RmGetList(
            handle,
            &mut needed,
            &mut returned,
            ptr::null_mut(),
            &mut reboot,
        )
    };
    let result = classify_query(status, needed, returned, reboot);
    // Consume ownership before ending: an error can mean the session no longer
    // exists. Never retry a possibly invalid/reused handle in Drop.
    session.0 = None;
    // SAFETY: our successful start has not yet been followed by an end attempt.
    if unsafe { RmEndSession(handle) } != ERROR_SUCCESS {
        return unavailable();
    }
    result
}

fn unavailable() -> ResourceUsers {
    ResourceUsers::Unknown(ResourceUseIssue::InspectionUnavailable)
}
