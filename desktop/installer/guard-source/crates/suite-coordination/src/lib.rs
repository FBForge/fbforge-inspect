//! Shared coordination for cooperating native applications, installers and
//! updaters. A lease is neither process-state evidence nor execution authority.
//! Consumers must keep ownership on one OS thread and recheck other prerequisites.

mod installation;
#[cfg(windows)]
mod windows;
pub use installation::{
    check_pending_installation, PendingInstallation, PENDING_INSTALLATION_MARKER,
};

use std::{fmt, marker::PhantomData, rc::Rc};

/// Stable native app identity. Matching is ASCII case-insensitive, independently
/// of installation paths, executable names and renderer-provided state.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AppIdentity(String);

impl AppIdentity {
    pub fn new(value: &str) -> Result<Self, InvalidAppIdentity> {
        if value.is_empty()
            || value.len() > 128
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
            || !value.as_bytes()[0].is_ascii_alphanumeric()
            || !value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
        {
            return Err(InvalidAppIdentity);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidAppIdentity;
impl fmt::Display for InvalidAppIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Die App-Identität ist ungültig.")
    }
}
impl std::error::Error for InvalidAppIdentity {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnavailableReason {
    UnsupportedPlatform,
    IdentityUnavailable,
    UntrustedObject,
    OperatingSystem,
}
impl fmt::Display for UnavailableReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedPlatform => "Die gemeinsame App-Koordination benötigt Windows.",
            Self::IdentityUnavailable => {
                "Die Windows-Benutzeridentität konnte nicht geprüft werden."
            }
            Self::UntrustedObject => "Die gemeinsame App-Sperre ist nicht ausreichend geschützt.",
            Self::OperatingSystem => "Die gemeinsame App-Sperre ist nicht verfügbar.",
        })
    }
}
impl std::error::Error for UnavailableReason {}

/// All outcomes are nonblocking. Abandoned ownership carries a held lease but
/// requires fresh resource/process/health checks, never automatic continuation.
#[derive(Debug)]
#[must_use]
pub enum Acquisition {
    Acquired(Lease),
    AbandonedNeedsRescan(Lease),
    Busy,
    Unavailable(UnavailableReason),
}

/// Thread-owned guard, deliberately neither Send nor Sync nor Clone.
/// Do not retain it across asynchronous task migration or transfer it to another
/// process. Drop releases ownership and closes the underlying non-inherited handle.
///
/// ```compile_fail
/// use fbforge_suite_coordination::Lease;
/// fn require_send<T: Send>() {}
/// require_send::<Lease>();
/// ```
///
/// ```compile_fail
/// use fbforge_suite_coordination::Lease;
/// fn require_sync<T: Sync>() {}
/// require_sync::<Lease>();
/// ```
#[must_use]
pub struct Lease {
    #[cfg(windows)]
    _native: windows::NativeLease,
    requires_rescan: bool,
    app_id: AppIdentity,
    _thread_bound: PhantomData<Rc<()>>,
}

impl Lease {
    /// Uses a global SID-scoped named mutex to include other Windows sessions.
    /// Object ownership and the explicit private DACL are checked before waiting.
    /// Reentrant acquisition on the same thread returns Busy instead of taking
    /// advantage of the OS mutex's recursive ownership semantics.
    pub fn try_acquire(identity: &AppIdentity) -> Acquisition {
        #[cfg(windows)]
        {
            windows::try_acquire(identity)
        }
        #[cfg(not(windows))]
        {
            let _ = identity;
            Acquisition::Unavailable(UnavailableReason::UnsupportedPlatform)
        }
    }

    pub fn requires_rescan(&self) -> bool {
        self.requires_rescan
    }

    /// Native identity of the held lease, without a caller-controlled override.
    pub fn app_id(&self) -> &str {
        self.app_id.as_str()
    }

    /// Positive native owner-thread check; callers cannot set this state.
    pub fn is_current_thread_owner(&self) -> bool {
        #[cfg(windows)]
        {
            self._native.is_current_thread_owner()
        }
        #[cfg(not(windows))]
        {
            false
        }
    }
}

impl fmt::Debug for Lease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Lease")
            .field("requires_rescan", &self.requires_rescan)
            .finish_non_exhaustive()
    }
}
