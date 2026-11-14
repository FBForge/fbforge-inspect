//! V2 owner-thread transport. Native proofs never cross this channel.
#[cfg(all(windows, test))]
use crate::session::initial_phase;
#[cfg(windows)]
use crate::session::{
    commit_release, deny_acquire, deny_future_grants, GRANTED, RELEASED, STARTING,
};
use std::{collections::BTreeSet, path::PathBuf};
#[cfg(windows)]
use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::Duration,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Code {
    Preparing,
    Staging,
    RecoveryStaging,
    Publishing,
    PublishedPending,
    RecoveryPublishedPending,
    Finalizing,
    Complete,
    Busy,
    Unavailable,
    Released,
}
impl Code {
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Preparing => "v2:preparing",
            Self::Staging => "v2:staging",
            Self::RecoveryStaging => "v2:recoveryStaging",
            Self::Publishing => "v2:publishing",
            Self::PublishedPending => "v2:publishedPending",
            Self::RecoveryPublishedPending => "v2:recoveryPublishedPending",
            Self::Finalizing => "v2:finalizing",
            Self::Complete => "v2:complete",
            Self::Busy => "v2:busy",
            Self::Unavailable => "v2:unavailable",
            Self::Released => "v2:released",
        }
    }
    pub(crate) fn success(self) -> bool {
        !matches!(self, Self::Busy | Self::Unavailable)
    }
    #[cfg(windows)]
    fn stable(self) -> bool {
        matches!(
            self,
            Self::Staging
                | Self::RecoveryStaging
                | Self::PublishedPending
                | Self::RecoveryPublishedPending
                | Self::Complete
                | Self::Unavailable
        )
    }
}
#[derive(Debug)]
pub(crate) struct Input {
    pub tool_id: String,
    pub plugin_directory: PathBuf,
    pub components: Vec<String>,
}
impl Input {
    pub(crate) fn parse(tool_id: String, directory: String, components: String) -> Option<Self> {
        if !identifier(&tool_id, 128)
            || directory.encode_utf16().count() > 960
            || fbforge_suite_core::discovery::validate_windows_location(&directory).as_deref()
                != Ok(directory.as_str())
            || components.len() > 8 * 64 + 7
        {
            return None;
        }
        let components = if components.is_empty() {
            Vec::new()
        } else {
            components
                .split(',')
                .map(str::to_string)
                .collect::<Vec<_>>()
        };
        let mut seen = BTreeSet::new();
        if components.len() > 8
            || components
                .iter()
                .any(|id| !identifier(id, 64) || !seen.insert(id.clone()))
        {
            return None;
        }
        Some(Self {
            tool_id,
            plugin_directory: directory.into(),
            components,
        })
    }
}
fn identifier(value: &str, maximum: usize) -> bool {
    !value.is_empty()
        && value.len() <= maximum
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.as_bytes()[value.len() - 1].is_ascii_alphanumeric()
}
#[cfg(windows)]
#[derive(Clone, Default)]
struct Paths {
    target: String,
    stage: Option<String>,
}
#[cfg(windows)]
struct Snapshot {
    code: Code,
    paths: Option<Paths>,
}
#[cfg(windows)]
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            code: Code::Unavailable,
            paths: None,
        }
    }
}
#[cfg(windows)]
#[derive(Clone, Copy)]
pub(crate) enum Operation {
    Publish,
    Finalize,
}
#[cfg(windows)]
enum Command {
    Operation(Operation),
    Release(SyncSender<Code>),
}
#[cfg(windows)]
pub(crate) struct Session {
    used: bool,
    lifecycle: Arc<AtomicU8>,
    snapshot: Arc<Mutex<Snapshot>>,
    commands: Option<SyncSender<Command>>,
    owner: Option<JoinHandle<()>>,
}
#[cfg(windows)]
impl Session {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_lifecycle(initial_phase())
    }
    pub(crate) fn with_lifecycle(lifecycle: Arc<AtomicU8>) -> Self {
        Self {
            used: false,
            lifecycle,
            snapshot: Arc::new(Mutex::new(Snapshot::default())),
            commands: None,
            owner: None,
        }
    }
    pub(crate) fn deny_prepare(&self) {
        deny_acquire(&self.lifecycle);
    }
    pub(crate) fn prepare(&mut self, input: Input) -> Code {
        if self.used || self.lifecycle.load(Ordering::Acquire) != STARTING {
            return Code::Unavailable;
        }
        self.used = true;
        let (commands, incoming) = sync_channel(1);
        let lifecycle = Arc::clone(&self.lifecycle);
        let snapshot = Arc::clone(&self.snapshot);
        self.set(Code::Preparing, None);
        match std::thread::Builder::new()
            .name("nsis-mutation-owner-v2".into())
            .spawn(move || own(input, incoming, lifecycle, snapshot))
        {
            Ok(owner) => {
                self.commands = Some(commands);
                self.owner = Some(owner);
                Code::Preparing
            }
            Err(_) => {
                self.deny_prepare();
                self.set(Code::Unavailable, None);
                Code::Unavailable
            }
        }
    }
    pub(crate) fn poll(&mut self) -> Code {
        let code = match self.snapshot.try_lock() {
            Ok(snapshot) => snapshot.code,
            Err(_) => return Code::Unavailable,
        };
        if self.lifecycle.load(Ordering::Acquire) == GRANTED {
            if self.owner.as_ref().is_none_or(JoinHandle::is_finished) {
                return self.poison();
            }
            return code;
        }
        if matches!(
            code,
            Code::Staging
                | Code::RecoveryStaging
                | Code::PublishedPending
                | Code::RecoveryPublishedPending
                | Code::Complete
        ) {
            return Code::Unavailable;
        }
        code
    }
    pub(crate) fn path(&mut self, kind: &str) -> Option<String> {
        let code = self.poll();
        if !matches!(
            code,
            Code::Staging
                | Code::RecoveryStaging
                | Code::PublishedPending
                | Code::RecoveryPublishedPending
                | Code::Complete
        ) {
            return None;
        }
        let snapshot = self.snapshot.try_lock().ok()?;
        if snapshot.code != code {
            return None;
        }
        let paths = snapshot.paths.as_ref()?;
        match kind {
            "target" => Some(format!("v2:targetPath:{}", paths.target)),
            "staging" if matches!(code, Code::Staging | Code::RecoveryStaging) => paths
                .stage
                .as_ref()
                .map(|path| format!("v2:stagingPath:{path}")),
            _ => None,
        }
    }
    pub(crate) fn operation(&mut self, operation: Operation) -> Code {
        let current = self.poll();
        let next = match (operation, current) {
            (Operation::Publish, Code::Staging | Code::RecoveryStaging) => Code::Publishing,
            (Operation::Finalize, Code::PublishedPending | Code::RecoveryPublishedPending) => {
                Code::Finalizing
            }
            _ => return Code::Unavailable,
        };
        self.set(next, None);
        if self
            .commands
            .as_ref()
            .is_some_and(|sender| sender.try_send(Command::Operation(operation)).is_ok())
        {
            next
        } else {
            self.poison()
        }
    }
    pub(crate) fn release(&mut self, timeout: Duration) -> Code {
        if !self.used || self.lifecycle.load(Ordering::Acquire) == RELEASED {
            return Code::Released;
        }
        if self.owner.as_ref().is_some_and(JoinHandle::is_finished) {
            if let Some(owner) = self.owner.take() {
                if owner.join().is_err() {
                    return self.poison();
                }
            }
            // Positive owner completion, never merely a missing command sender.
            if commit_release(&self.lifecycle) {
                self.lifecycle.store(RELEASED, Ordering::Release);
                self.set(Code::Released, None);
                return Code::Released;
            }
            return Code::Unavailable;
        }
        if self.lifecycle.load(Ordering::Acquire) != GRANTED || !self.poll().stable() {
            self.deny_prepare();
            return Code::Unavailable;
        }
        let (reply, receive) = sync_channel(1);
        if self
            .commands
            .as_ref()
            .is_none_or(|sender| sender.try_send(Command::Release(reply)).is_err())
        {
            return self.poison();
        }
        match receive.recv_timeout(timeout) {
            Ok(Code::Released) if self.lifecycle.load(Ordering::Acquire) == RELEASED => {
                self.commands.take();
                Code::Released
            }
            _ => self.poison(),
        }
    }
    fn poison(&mut self) -> Code {
        if deny_future_grants(&self.lifecycle) {
            std::process::exit(70);
        }
        self.commands.take();
        self.set(Code::Unavailable, None);
        Code::Unavailable
    }
    fn set(&self, code: Code, paths: Option<Paths>) {
        store(&self.snapshot, code, paths);
    }
}
#[cfg(windows)]
impl Drop for Session {
    fn drop(&mut self) {
        if deny_future_grants(&self.lifecycle) {
            std::process::exit(70);
        }
        self.commands.take();
    }
}
#[cfg(windows)]
fn store(snapshot: &Mutex<Snapshot>, code: Code, paths: Option<Paths>) {
    if let Ok(mut snapshot) = snapshot.lock() {
        snapshot.code = code;
        if let Some(paths) = paths {
            snapshot.paths = Some(paths);
        }
    }
}

#[cfg(windows)]
fn own(
    input: Input,
    incoming: Receiver<Command>,
    lifecycle: Arc<AtomicU8>,
    snapshot: Arc<Mutex<Snapshot>>,
) {
    use fbforge_suite_coordination::{Acquisition, AppIdentity, Lease};
    use fbforge_suite_core::installer_targets::{TargetPhase, VerifiedInstallerHost};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    let host = match catch_unwind(AssertUnwindSafe(|| {
        VerifiedInstallerHost::verify_current(&input.tool_id)
    })) {
        Ok(Ok(host)) => host,
        _ => {
            deny_acquire(&lifecycle);
            store(&snapshot, Code::Unavailable, None);
            return;
        }
    };
    if lifecycle.load(Ordering::Acquire) != STARTING {
        return;
    }
    let identity = match AppIdentity::new(host.app_id()) {
        Ok(identity) => identity,
        Err(_) => {
            deny_acquire(&lifecycle);
            store(&snapshot, Code::Unavailable, None);
            return;
        }
    };
    // Actual host/file and actual Lease remain OUTSIDE the caught plan scope.
    let lease = match Lease::try_acquire(&identity) {
        Acquisition::Acquired(lease) | Acquisition::AbandonedNeedsRescan(lease) => lease,
        Acquisition::Busy => {
            deny_acquire(&lifecycle);
            store(&snapshot, Code::Busy, None);
            return;
        }
        Acquisition::Unavailable(_) => {
            deny_acquire(&lifecycle);
            store(&snapshot, Code::Unavailable, None);
            return;
        }
    };
    let recovery = lease.requires_rescan();
    let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<SyncSender<Code>, ()> {
        if lifecycle.load(Ordering::Acquire) != STARTING {
            return Err(());
        }
        // This ordinary lexical borrow cannot outlive or migrate its owner.
        let mut plan = host
            .prepare_target(&lease, &input.plugin_directory, &input.components)
            .map_err(|_| ())?;
        let target = plan
            .target_directory()
            .map_err(|_| ())?
            .to_str()
            .ok_or(())?
            .to_string();
        let stage = if plan.phase() == TargetPhase::Staging {
            Some(
                plan.staging_directory()
                    .map_err(|_| ())?
                    .to_str()
                    .ok_or(())?
                    .to_string(),
            )
        } else {
            None
        };
        if lifecycle
            .compare_exchange(STARTING, GRANTED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(());
        }
        let ready = match (plan.phase(), recovery) {
            (TargetPhase::Staging, false) => Code::Staging,
            (TargetPhase::Staging, true) => Code::RecoveryStaging,
            (TargetPhase::PublishedPending, false) => Code::PublishedPending,
            (TargetPhase::PublishedPending, true) => Code::RecoveryPublishedPending,
            _ => return Err(()),
        };
        store(&snapshot, ready, Some(Paths { target, stage }));
        loop {
            match incoming.recv().map_err(|_| ())? {
                Command::Release(reply) => return Ok(reply),
                Command::Operation(operation) => {
                    let result = match operation {
                        Operation::Publish => plan.publish(),
                        Operation::Finalize => plan.finalize(),
                    };
                    let code = match result {
                        Ok(TargetPhase::PublishedPending) if recovery => {
                            Code::RecoveryPublishedPending
                        }
                        Ok(TargetPhase::PublishedPending) => Code::PublishedPending,
                        Ok(TargetPhase::Complete) => Code::Complete,
                        _ => Code::Unavailable,
                    };
                    store(&snapshot, code, None);
                }
            }
        }
    }));
    match outcome {
        Ok(Ok(reply)) => {
            if !commit_release(&lifecycle) {
                std::process::exit(70);
            }
            drop(lease);
            lifecycle.store(RELEASED, Ordering::Release);
            store(&snapshot, Code::Released, None);
            let _ = reply.try_send(Code::Released);
        }
        _ => {
            if deny_future_grants(&lifecycle) {
                std::process::exit(70);
            }
            drop(lease);
            store(&snapshot, Code::Unavailable, None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_native_inputs_check_syntax_without_authorizing_tool_ids() {
        let parse =
            |tool: &str, path: &str, csv: &str| Input::parse(tool.into(), path.into(), csv.into());
        let input = parse("fbforge-connect", r"C:\Users\A\Temp\plugins", "scanner").unwrap();
        assert_eq!(input.tool_id, "fbforge-connect");
        assert_eq!(
            input.plugin_directory,
            PathBuf::from(r"C:\Users\A\Temp\plugins")
        );
        assert_eq!(input.components, vec!["scanner"]);
        assert!(parse("fbforge-inspect", r"C:\Users\A\Temp\plugins", "").is_some());
        assert!(parse("future-same-kind-tool", r"C:\private", "").is_some());
        assert!(parse("fbforge-suite", r"C:\private", "").is_some());
        for (tool, path, csv) in [
            ("../tool", r"C:\private", ""),
            ("", r"C:\private", ""),
            ("fbforge-connect", r"\\server\share", ""),
            ("fbforge-connect", r"C:\private\..\other", ""),
            ("fbforge-connect", r"C:\private", "scanner,scanner"),
            ("fbforge-connect", r"C:\private", "scanner,"),
            ("fbforge-connect", r"C:\private", ",scanner"),
            ("fbforge-connect", r"C:\private", "scanner;exec"),
        ] {
            assert!(parse(tool, path, csv).is_none());
        }
        assert!(parse("fbforge-connect", r"C:\private", &"x".repeat(65)).is_none());
        assert!(parse(
            "fbforge-connect",
            r"C:\private",
            &(0..9)
                .map(|n| format!("c{n}"))
                .collect::<Vec<_>>()
                .join(",")
        )
        .is_none());
    }
    #[test]
    fn versioned_reply_codes_have_fixed_bounded_text_and_explicit_denials() {
        for (code, expected) in [
            (Code::Preparing, "v2:preparing"),
            (Code::Staging, "v2:staging"),
            (Code::RecoveryStaging, "v2:recoveryStaging"),
            (Code::Publishing, "v2:publishing"),
            (Code::PublishedPending, "v2:publishedPending"),
            (
                Code::RecoveryPublishedPending,
                "v2:recoveryPublishedPending",
            ),
            (Code::Finalizing, "v2:finalizing"),
            (Code::Complete, "v2:complete"),
            (Code::Busy, "v2:busy"),
            (Code::Unavailable, "v2:unavailable"),
            (Code::Released, "v2:released"),
        ] {
            assert_eq!(code.text(), expected);
            assert!(expected.len() < 64);
            assert_eq!(
                code.success(),
                !matches!(expected, "v2:busy" | "v2:unavailable")
            );
        }
    }
    #[cfg(windows)]
    #[test]
    fn a_preparation_denial_is_sticky_and_never_starts_a_worker() {
        let mut session = Session::new();
        session.deny_prepare();
        let input = Input::parse(
            "fbforge-inspect".into(),
            r"C:\private".into(),
            String::new(),
        )
        .unwrap();
        assert_eq!(session.prepare(input), Code::Unavailable);
        assert!(session.owner.is_none());
        assert_eq!(session.poll(), Code::Unavailable);
        assert_eq!(session.path("target"), None);
    }
    #[cfg(windows)]
    #[test]
    fn path_and_mutation_requests_do_not_create_a_grant_before_preparation() {
        let mut session = Session::new();
        assert_eq!(session.poll(), Code::Unavailable);
        assert_eq!(session.path("staging"), None);
        assert_eq!(session.operation(Operation::Publish), Code::Unavailable);
        assert_eq!(session.operation(Operation::Finalize), Code::Unavailable);
        assert_eq!(session.release(Duration::from_millis(1)), Code::Released);
    }
}
