//! Native private staging and directory publication. Parsing grants no trust;
//! callers must originate the manifest from authenticated NSIS/private PLUGINSDIR.
#[cfg(any(windows, test))]
mod binding;
mod host;
#[cfg(any(windows, test))]
pub(crate) mod journal;
mod manifest;
#[cfg(any(windows, test))]
mod preservation;
use crate::catalog::{validate_tool, ToolManifest};
use fbforge_suite_coordination::Lease;
pub use host::{VerifiedInstallerHost, PAYLOAD_MANIFEST_LEAF};
pub use manifest::{parse_payload_manifest, PayloadFile, PayloadManifest};
use std::fmt;
use std::{io::Read, path::Path};
#[cfg(windows)]
mod windows;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetPhase {
    Staging,
    PublishedPending,
    Complete,
}

/// A private target preparation, borrowing the actual native app lease. Neither
/// identity nor phase can be supplied by a renderer or set by a public boolean.
///
/// ```compile_fail
/// use fbforge_suite_core::installer_targets::InstallerTargetPlan;
/// fn require_send<T:Send>() {}
/// require_send::<InstallerTargetPlan<'static>>();
/// ```
pub struct InstallerTargetPlan<'lease> {
    lease: &'lease Lease,
    #[cfg(windows)]
    native: windows::NativeTargetPlan,
}
impl fmt::Debug for InstallerTargetPlan<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InstallerTargetPlan")
            .finish_non_exhaustive()
    }
}
impl<'lease> InstallerTargetPlan<'lease> {
    pub fn prepare(
        lease: &'lease Lease,
        tool: &ToolManifest,
        payload: PayloadManifest,
    ) -> Result<Self, TargetError> {
        validate_tool(tool).map_err(|_| TargetError::InvalidManifest)?;
        if !lease.is_current_thread_owner()
            || !lease.app_id().eq_ignore_ascii_case(&tool.app_id)
            || payload.tool_id() != tool.id
            || payload.app_id() != tool.app_id
        {
            return Err(TargetError::WrongLease);
        }
        #[cfg(windows)]
        {
            Ok(Self {
                lease,
                native: windows::NativeTargetPlan::prepare(
                    tool.clone(),
                    payload,
                    lease.requires_rescan(),
                )?,
            })
        }
        #[cfg(not(windows))]
        {
            let _ = payload;
            Err(TargetError::UnsupportedPlatform)
        }
    }
    fn check_owner(&self) -> Result<(), TargetError> {
        if self.lease.is_current_thread_owner() {
            Ok(())
        } else {
            Err(TargetError::WrongLease)
        }
    }
    pub fn phase(&self) -> TargetPhase {
        #[cfg(windows)]
        {
            self.native.phase()
        }
        #[cfg(not(windows))]
        {
            TargetPhase::Staging
        }
    }
    /// Native NSIS extraction destination only, never an IPC filesystem grant.
    pub fn staging_directory(&self) -> Result<&Path, TargetError> {
        self.check_owner()?;
        #[cfg(windows)]
        {
            self.native.staging_directory()
        }
        #[cfg(not(windows))]
        {
            Err(TargetError::UnsupportedPlatform)
        }
    }
    pub fn target_directory(&self) -> Result<&Path, TargetError> {
        self.check_owner()?;
        #[cfg(windows)]
        {
            Ok(self.native.target_directory())
        }
        #[cfg(not(windows))]
        {
            Err(TargetError::UnsupportedPlatform)
        }
    }
    pub fn write_payload_file(
        &mut self,
        path: &str,
        reader: &mut impl Read,
    ) -> Result<(), TargetError> {
        self.check_owner()?;
        #[cfg(windows)]
        {
            self.native.write_payload_file(path, reader)
        }
        #[cfg(not(windows))]
        {
            let _ = (path, reader);
            Err(TargetError::UnsupportedPlatform)
        }
    }
    pub fn write_uninstaller(&mut self, reader: &mut impl Read) -> Result<(), TargetError> {
        self.check_owner()?;
        #[cfg(windows)]
        {
            self.native.write_uninstaller(reader)
        }
        #[cfg(not(windows))]
        {
            let _ = reader;
            Err(TargetError::UnsupportedPlatform)
        }
    }
    /// Full native health and all-original-resource RM query occur immediately
    /// before same-volume directory publication on this lease's owner thread.
    pub fn publish(&mut self) -> Result<TargetPhase, TargetError> {
        self.check_owner()?;
        #[cfg(windows)]
        {
            self.native.publish()
        }
        #[cfg(not(windows))]
        {
            Err(TargetError::UnsupportedPlatform)
        }
    }
    /// Recheck the actual published tree and current native registry binding,
    /// durably emit its receipt, then remove only its reserved pending marker.
    pub fn finalize(&mut self) -> Result<TargetPhase, TargetError> {
        self.check_owner()?;
        #[cfg(windows)]
        {
            self.native.finalize()
        }
        #[cfg(not(windows))]
        {
            Err(TargetError::UnsupportedPlatform)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetError {
    InvalidManifest,
    WrongLease,
    UnsupportedPlatform,
    UnsupportedTarget,
    AmbiguousTarget,
    ManualMigrationRequired,
    UnsafeTarget,
    Downgrade,
    NotEnoughSpace,
    PayloadMismatch,
    PublisherRejected,
    ResourcesInUse,
    InspectionUnavailable,
    InvalidPhase,
    RecoveryRequired,
    OperatingSystem,
}
impl fmt::Display for TargetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Das native Installationsziel konnte nicht freigegeben werden.")
    }
}
impl std::error::Error for TargetError {}
