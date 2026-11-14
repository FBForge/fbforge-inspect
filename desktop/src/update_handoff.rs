//! Native one-shot updater handoff. No installer runs before the main owner
//! has returned from the event loop and dropped its coordination lease.
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) fn finish_attempt<T>(
    result: Result<Option<T>, &'static str>,
    gate: &AtomicBool,
) -> Result<Option<T>, &'static str> {
    gate.store(matches!(result, Ok(Some(_))), Ordering::Release);
    result
}

#[cfg(any(windows, test))]
use std::sync::Mutex;
#[cfg(any(windows, test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandoffError {
    AlreadyQueued,
    Closed,
    Unavailable,
    InstallFailed,
}
#[cfg(any(windows, test))]
enum Slot<T> {
    Empty,
    Pending(T),
    Closed,
}
#[cfg(any(windows, test))]
pub(crate) struct InstallerHandoff<T> {
    state: Mutex<Slot<T>>,
}
#[cfg(any(windows, test))]
impl<T> Default for InstallerHandoff<T> {
    fn default() -> Self {
        Self {
            state: Mutex::new(Slot::Empty),
        }
    }
}
#[cfg(any(windows, test))]
impl<T> InstallerHandoff<T> {
    pub(crate) fn submit(&self, pending: T) -> Result<(), HandoffError> {
        let mut state = self.state.lock().map_err(|_| HandoffError::Unavailable)?;
        match &*state {
            Slot::Empty => {
                *state = Slot::Pending(pending);
                Ok(())
            }
            Slot::Pending(_) => Err(HandoffError::AlreadyQueued),
            Slot::Closed => Err(HandoffError::Closed),
        }
    }
    fn take_after_exit(&self) -> Result<Option<T>, HandoffError> {
        let mut state = self.state.lock().map_err(|_| HandoffError::Unavailable)?;
        match std::mem::replace(&mut *state, Slot::Closed) {
            Slot::Empty => Ok(None),
            Slot::Pending(pending) => Ok(Some(pending)),
            Slot::Closed => Err(HandoffError::Closed),
        }
    }
}
#[cfg(any(windows, test))]
pub(crate) fn finish_after_exit<L, T>(
    lease: L,
    queue: &InstallerHandoff<T>,
    exit_code: i32,
    install: impl FnOnce(T) -> Result<(), ()>,
) -> Result<i32, HandoffError> {
    // L is the actual thread-bound Lease in production. It never crosses threads.
    drop(lease);
    let pending = queue.take_after_exit()?;
    if exit_code == 0 {
        if let Some(pending) = pending {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| install(pending)));
            if !matches!(result, Ok(Ok(()))) {
                return Err(HandoffError::InstallFailed);
            }
        }
    }
    Ok(exit_code)
}

#[cfg(test)]
#[path = "update_handoff/tests.rs"]
mod tests;
