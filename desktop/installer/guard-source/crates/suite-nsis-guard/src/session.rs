use fbforge_suite_coordination::{Acquisition, AppIdentity, Lease};
use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc::{sync_channel, Receiver, SyncSender},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};

pub(crate) const STARTING: u8 = 0;
pub(crate) const GRANTED: u8 = 1;
pub(crate) const DENIED: u8 = 2;
pub(crate) const RELEASED: u8 = 3;
const POSTGRANT_FAILED: u8 = 4;
const RELEASE_COMMITTED: u8 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Reply {
    Held,
    Released,
    Busy,
    Abandoned,
    Unavailable,
}
impl Reply {
    #[cfg(windows)]
    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::Held => "v1:held",
            Self::Released => "v1:released",
            Self::Busy => "v1:busy",
            Self::Abandoned => "v1:abandoned",
            Self::Unavailable => "v1:unavailable",
        }
    }
}
enum Resource<T> {
    Acquired(T),
    Abandoned(T),
    Denied(Reply),
}
enum Command {
    Status(SyncSender<Reply>),
    Release(SyncSender<Reply>),
    #[cfg(test)]
    Panic,
    #[cfg(test)]
    Stall(SyncSender<()>, Receiver<()>),
}

pub(crate) struct Session {
    used: bool,
    // A missing sender does not prove that a pending/abandoned owner dropped.
    // Only an explicit no-resource startup reply or Released retires this fact.
    needs_release: bool,
    phase: Arc<AtomicU8>,
    commands: Option<SyncSender<Command>>,
    startup: Option<Receiver<Reply>>,
    join: Option<JoinHandle<()>>,
}
impl Session {
    #[cfg(test)]
    pub(crate) fn new() -> Self {
        Self::with_phase(initial_phase())
    }
    pub(crate) fn with_phase(phase: Arc<AtomicU8>) -> Self {
        Self {
            used: false,
            needs_release: false,
            phase,
            commands: None,
            startup: None,
            join: None,
        }
    }
    pub(crate) fn acquire(&mut self, identity: &str, timeout: Duration) -> Reply {
        if self.used || self.phase.load(Ordering::Acquire) != STARTING {
            return Reply::Unavailable;
        }
        self.used = true;
        let Ok(identity) = AppIdentity::new(identity) else {
            return Reply::Unavailable;
        };
        self.start(move || match Lease::try_acquire(&identity) {
            Acquisition::Acquired(lease) => Resource::Acquired(lease),
            Acquisition::AbandonedNeedsRescan(lease) => Resource::Abandoned(lease),
            Acquisition::Busy => Resource::Denied(Reply::Busy),
            Acquisition::Unavailable(_) => Resource::Denied(Reply::Unavailable),
        });
        self.accept(timeout)
    }
    fn start<T: 'static>(&mut self, acquire: impl FnOnce() -> Resource<T> + Send + 'static) {
        let (commands, incoming) = sync_channel(1);
        let (ready, startup) = sync_channel(1);
        let phase = Arc::clone(&self.phase);
        match std::thread::Builder::new()
            .name("nsis-guard-owner".into())
            .spawn(move || own(acquire, incoming, ready, phase))
        {
            Ok(join) => {
                self.needs_release = true;
                self.commands = Some(commands);
                self.startup = Some(startup);
                self.join = Some(join);
            }
            Err(_) => {
                self.phase.store(DENIED, Ordering::Release);
            }
        }
    }
    fn accept(&mut self, timeout: Duration) -> Reply {
        let Some(startup) = self.startup.take() else {
            return Reply::Unavailable;
        };
        match startup.recv_timeout(timeout) {
            Ok(Reply::Held) => {
                // Only this atomic transition authorizes a grant. A timeout or
                // owner failure that wins first permanently prevents late grants.
                if self
                    .phase
                    .compare_exchange(STARTING, GRANTED, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    Reply::Held
                } else {
                    self.poison()
                }
            }
            Ok(Reply::Abandoned) => {
                self.phase.store(DENIED, Ordering::Release);
                Reply::Abandoned
            }
            Ok(reply @ (Reply::Busy | Reply::Unavailable)) => {
                self.needs_release = false;
                self.commands.take();
                self.phase.store(DENIED, Ordering::Release);
                reply
            }
            _ => self.poison(),
        }
    }
    pub(crate) fn status(&mut self, timeout: Duration) -> Reply {
        if self.phase.load(Ordering::Acquire) != GRANTED {
            return Reply::Unavailable;
        }
        if self.join.as_ref().is_some_and(|join| join.is_finished()) {
            return self.poison();
        }
        self.request(false, timeout)
    }
    pub(crate) fn release(&mut self, timeout: Duration) -> Reply {
        let phase = self.phase.load(Ordering::Acquire);
        if phase == RELEASED || !self.needs_release {
            return Reply::Released;
        }
        if phase == RELEASE_COMMITTED || self.commands.is_none() {
            return Reply::Unavailable;
        }
        self.request(true, timeout)
    }
    fn request(&mut self, release: bool, timeout: Duration) -> Reply {
        let (reply, receive) = sync_channel(1);
        let command = if release {
            Command::Release(reply)
        } else {
            Command::Status(reply)
        };
        let sent = self
            .commands
            .as_ref()
            .is_some_and(|sender| sender.try_send(command).is_ok());
        if !sent {
            return self.poison();
        }
        match receive.recv_timeout(timeout) {
            Ok(Reply::Held) if !release && self.phase.load(Ordering::Acquire) == GRANTED => {
                Reply::Held
            }
            Ok(Reply::Released) if release && self.phase.load(Ordering::Acquire) == RELEASED => {
                self.needs_release = false;
                self.commands.take();
                Reply::Released
            }
            _ => self.poison(),
        }
    }
    fn poison(&mut self) -> Reply {
        if deny_future_grants(&self.phase) {
            std::process::exit(70);
        }
        self.commands.take();
        Reply::Unavailable
    }
    #[cfg(test)]
    fn start_for_test<T: 'static>(acquire: impl FnOnce() -> Resource<T> + Send + 'static) -> Self {
        let mut session = Self::new();
        session.used = true;
        session.start(acquire);
        session
    }
    #[cfg(test)]
    fn finish_for_test(&mut self) {
        if let Some(join) = self.join.take() {
            join.join().unwrap();
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // Disappearing control after a grant must not leave NSIS mutating while
        // the owner releases its lease. Production sessions live in the pinned DLL.
        if deny_future_grants(&self.phase) {
            std::process::exit(70);
        }
        self.commands.take();
    }
}

pub(crate) fn initial_phase() -> Arc<AtomicU8> {
    Arc::new(AtomicU8::new(STARTING))
}

// ABI rejection and accept() decide against the very same atomic, even before
// a Session/worker exists. A failure cannot be overwritten by a late grant.
pub(crate) fn deny_acquire(phase: &AtomicU8) {
    let _ = phase.compare_exchange(STARTING, DENIED, Ordering::AcqRel, Ordering::Acquire);
}

// Winning this transition consumes the caller's promise that mutations ended.
// Later ACK timeout cannot retroactively turn a committed release into failure.
// If failure wins first, every owner must fail-stop before dropping the resource.
pub(crate) fn commit_release(phase: &AtomicU8) -> bool {
    phase
        .fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |current| match current {
                STARTING | GRANTED | DENIED => Some(RELEASE_COMMITTED),
                _ => None,
            },
        )
        .is_ok()
}

pub(crate) fn deny_future_grants(phase: &AtomicU8) -> bool {
    // Keep the fail-stop obligation visible to every competing failure path:
    // another thread may be preempted between this transition and process exit.
    let previous = phase
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
            Some(match current {
                GRANTED | POSTGRANT_FAILED => POSTGRANT_FAILED,
                RELEASED | RELEASE_COMMITTED => current,
                _ => DENIED,
            })
        })
        .expect("phase update always returns a value");
    matches!(previous, GRANTED | POSTGRANT_FAILED)
}

fn own<T>(
    acquire: impl FnOnce() -> Resource<T>,
    incoming: Receiver<Command>,
    ready: SyncSender<Reply>,
    phase: Arc<AtomicU8>,
) {
    let (resource, initial) = match catch_unwind(AssertUnwindSafe(acquire)) {
        Ok(Resource::Acquired(resource)) => (resource, Reply::Held),
        Ok(Resource::Abandoned(resource)) => (resource, Reply::Abandoned),
        Ok(Resource::Denied(reply)) => {
            phase.store(DENIED, Ordering::Release);
            let _ = ready.try_send(reply);
            return;
        }
        Err(_) => {
            phase.store(DENIED, Ordering::Release);
            return;
        }
    };
    // Keep the actual resource OUTSIDE the unwinding scope. A fatal owner error
    // after a committed grant aborts this installer before Rust can drop the lease.
    let outcome = catch_unwind(AssertUnwindSafe(|| -> Result<SyncSender<Reply>, ()> {
        ready.try_send(initial).map_err(|_| ())?;
        loop {
            match incoming.recv().map_err(|_| ())? {
                Command::Status(reply) => {
                    let answer =
                        if initial == Reply::Held && phase.load(Ordering::Acquire) == GRANTED {
                            Reply::Held
                        } else {
                            Reply::Unavailable
                        };
                    reply.try_send(answer).map_err(|_| ())?;
                }
                Command::Release(reply) => return Ok(reply),
                #[cfg(test)]
                Command::Panic => panic!("postgrant owner fixture"),
                #[cfg(test)]
                Command::Stall(entered, resume) => {
                    entered.send(()).unwrap();
                    resume.recv().unwrap();
                }
            }
        }
    }));
    match outcome {
        Ok(Ok(reply)) => {
            if !commit_release(&phase) {
                // In particular POSTGRANT_FAILED is never overwritten. This
                // thread must enforce the obligation if its caller was paused.
                std::process::exit(70);
            }
            drop(resource);
            phase.store(RELEASED, Ordering::Release);
            let _ = reply.try_send(Reply::Released);
        }
        _ => {
            if deny_future_grants(&phase) {
                std::process::exit(70);
            }
            drop(resource);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct Witness(Arc<Mutex<Vec<std::thread::ThreadId>>>);
    impl Drop for Witness {
        fn drop(&mut self) {
            self.0.lock().unwrap().push(std::thread::current().id());
        }
    }

    #[test]
    fn postgrant_denial_never_loses_its_failstop_obligation() {
        let phase = AtomicU8::new(GRANTED);
        assert!(deny_future_grants(&phase));
        assert!(
            deny_future_grants(&phase),
            "a second failure must still precede lease drop with fail-stop"
        );
        assert!(phase
            .compare_exchange(STARTING, GRANTED, Ordering::AcqRel, Ordering::Acquire)
            .is_err());
        let phase = AtomicU8::new(STARTING);
        assert!(!deny_future_grants(&phase));
        assert!(!deny_future_grants(&phase));
    }

    #[test]
    fn holds_the_resource_on_its_owner_thread_until_explicit_release() {
        let drops = Arc::new(Mutex::new(Vec::new()));
        let owner = Arc::new(Mutex::new(None));
        let record = Arc::clone(&drops);
        let thread = Arc::clone(&owner);
        let mut session = Session::start_for_test(move || {
            *thread.lock().unwrap() = Some(std::thread::current().id());
            Resource::Acquired(Witness(record))
        });
        assert_eq!(session.accept(Duration::from_secs(1)), Reply::Held);
        assert_eq!(session.status(Duration::from_secs(1)), Reply::Held);
        assert!(drops.lock().unwrap().is_empty());
        assert_eq!(session.release(Duration::from_secs(1)), Reply::Released);
        assert_eq!(*drops.lock().unwrap(), vec![owner.lock().unwrap().unwrap()]);
        assert_eq!(session.status(Duration::from_secs(1)), Reply::Unavailable);
    }

    #[test]
    fn denied_and_abandoned_acquisitions_never_become_a_write_grant() {
        let drops = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&drops);
        let mut session = Session::start_for_test(move || Resource::Abandoned(Witness(record)));
        assert_eq!(session.accept(Duration::from_secs(1)), Reply::Abandoned);
        assert_eq!(session.status(Duration::from_secs(1)), Reply::Unavailable);
        assert!(drops.lock().unwrap().is_empty());
        assert_eq!(session.release(Duration::from_secs(1)), Reply::Released);
        assert_eq!(drops.lock().unwrap().len(), 1);
        let mut busy = Session::start_for_test(|| Resource::<()>::Denied(Reply::Busy));
        assert_eq!(busy.accept(Duration::from_secs(1)), Reply::Busy);
        assert_eq!(busy.status(Duration::from_secs(1)), Reply::Unavailable);
    }

    #[test]
    fn a_pregrant_timeout_is_sticky_even_when_a_late_acquisition_succeeds() {
        let (proceed, wait) = std::sync::mpsc::sync_channel(1);
        let drops = Arc::new(Mutex::new(Vec::new()));
        let record = Arc::clone(&drops);
        let mut session = Session::start_for_test(move || {
            wait.recv().unwrap();
            Resource::Acquired(Witness(record))
        });
        assert_eq!(
            session.accept(Duration::from_millis(10)),
            Reply::Unavailable
        );
        proceed.send(()).unwrap();
        session.finish_for_test();
        assert_eq!(session.accept(Duration::from_secs(1)), Reply::Unavailable);
        assert_eq!(session.status(Duration::from_secs(1)), Reply::Unavailable);
        assert_eq!(drops.lock().unwrap().len(), 1);
    }

    #[test]
    fn a_pregrant_worker_panic_never_publishes_a_grant() {
        let mut session = Session::start_for_test(|| -> Resource<()> {
            panic!("pregrant fixture");
        });
        assert_eq!(session.accept(Duration::from_secs(1)), Reply::Unavailable);
        assert_eq!(session.status(Duration::from_secs(1)), Reply::Unavailable);
        session.finish_for_test();
    }

    #[test]
    fn postgrant_failures_end_the_own_process_before_resource_drop() {
        const MODE: &str = "FBFORGE_GUARD_LIFETIME_FIXTURE";
        const MARKER: &str = "FBFORGE_GUARD_DROP_MARKER";
        if let Ok(mode) = std::env::var(MODE) {
            struct DropFile(std::path::PathBuf);
            impl Drop for DropFile {
                fn drop(&mut self) {
                    std::fs::write(&self.0, b"dropped").unwrap();
                }
            }
            let marker = std::path::PathBuf::from(std::env::var_os(MARKER).unwrap());
            let mut session = Session::start_for_test(move || Resource::Acquired(DropFile(marker)));
            assert_eq!(session.accept(Duration::from_secs(1)), Reply::Held);
            match mode.as_str() {
                "panic" => {
                    session
                        .commands
                        .as_ref()
                        .unwrap()
                        .send(Command::Panic)
                        .unwrap();
                    session.finish_for_test();
                }
                "timeout" => {
                    let (entered, ready) = sync_channel(1);
                    let (_resume, wait) = sync_channel(1);
                    session
                        .commands
                        .as_ref()
                        .unwrap()
                        .send(Command::Stall(entered, wait))
                        .unwrap();
                    ready.recv().unwrap();
                    session.status(Duration::from_millis(10));
                }
                "lost-control" => drop(session),
                _ => panic!("unknown lifetime fixture"),
            }
            panic!("postgrant failure returned instead of stopping the fixture process");
        }
        for mode in ["panic", "timeout", "lost-control"] {
            let marker =
                std::env::temp_dir().join(format!("guard-drop-{}-{mode}", std::process::id()));
            assert!(!marker.exists());
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "session::tests::postgrant_failures_end_the_own_process_before_resource_drop",
                ])
                .env_clear()
                .env(MODE, mode)
                .env(MARKER, &marker)
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert_eq!(
                child.status.code(),
                Some(70),
                "{mode}: {}",
                String::from_utf8_lossy(&child.stderr)
            );
            assert!(
                !marker.exists(),
                "resource dropped before fail-stop: {mode}"
            );
        }
    }

    #[test]
    fn release_and_failure_have_one_ordered_decision_before_drop() {
        const MODE: &str = "FBFORGE_RELEASE_ORDER_FIXTURE";
        const MARKER: &str = "FBFORGE_RELEASE_DROP_MARKER";
        if let Ok(mode) = std::env::var(MODE) {
            struct ReleaseWitness {
                marker: std::path::PathBuf,
                entered: Option<SyncSender<()>>,
                resume: Option<Receiver<()>>,
            }
            impl Drop for ReleaseWitness {
                fn drop(&mut self) {
                    if let Some(entered) = &self.entered {
                        entered.send(()).unwrap();
                    }
                    if let Some(resume) = &self.resume {
                        resume.recv().unwrap();
                    }
                    std::fs::write(&self.marker, b"dropped").unwrap();
                }
            }
            let marker = std::path::PathBuf::from(std::env::var_os(MARKER).unwrap());
            let (drop_entered, dropping) = sync_channel(1);
            let (resume_drop, may_drop) = sync_channel(1);
            let blocks_drop = mode == "release-wins";
            let mut session = Session::start_for_test(move || {
                Resource::Acquired(ReleaseWitness {
                    marker,
                    entered: blocks_drop.then_some(drop_entered),
                    resume: blocks_drop.then_some(may_drop),
                })
            });
            assert_eq!(session.accept(Duration::from_secs(1)), Reply::Held);
            let (reply, released) = sync_channel(1);
            if mode == "failure-wins" {
                let (entered, stalled) = sync_channel(1);
                let (resume, wait) = sync_channel(1);
                session
                    .commands
                    .as_ref()
                    .unwrap()
                    .send(Command::Stall(entered, wait))
                    .unwrap();
                stalled.recv().unwrap();
                session
                    .commands
                    .as_ref()
                    .unwrap()
                    .send(Command::Release(reply))
                    .unwrap();
                // The failing caller is preempted after recording the obligation,
                // before its process::exit. The real worker must honor it too.
                assert!(deny_future_grants(&session.phase));
                resume.send(()).unwrap();
                session.finish_for_test();
                panic!("release worker returned instead of fail-stopping before drop");
            }
            session
                .commands
                .as_ref()
                .unwrap()
                .send(Command::Release(reply))
                .unwrap();
            dropping.recv_timeout(Duration::from_secs(1)).unwrap();
            assert!(
                !deny_future_grants(&session.phase),
                "release committed first, so its completed-mutation permission must win"
            );
            // Actual ACK timeout while the owner's resource Drop is blocked.
            assert!(released.recv_timeout(Duration::from_millis(10)).is_err());
            assert_eq!(session.poison(), Reply::Unavailable);
            assert_eq!(
                session.release(Duration::from_millis(10)),
                Reply::Unavailable,
                "a lost command sender is not an ACK before the resource dropped"
            );
            resume_drop.send(()).unwrap();
            assert_eq!(
                released.recv_timeout(Duration::from_secs(1)).unwrap(),
                Reply::Released
            );
            session.finish_for_test();
            assert_eq!(session.phase.load(Ordering::Acquire), RELEASED);
            assert_eq!(session.release(Duration::from_millis(10)), Reply::Released);
            return;
        }
        for (mode, expected) in [("failure-wins", 70), ("release-wins", 0)] {
            let marker = std::env::temp_dir()
                .join(format!("guard-release-order-{}-{mode}", std::process::id()));
            assert!(!marker.exists());
            let child = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "session::tests::release_and_failure_have_one_ordered_decision_before_drop",
                ])
                .env_clear()
                .env(MODE, mode)
                .env(MARKER, &marker)
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert_eq!(
                child.status.code(),
                Some(expected),
                "{mode}: {}",
                String::from_utf8_lossy(&child.stderr)
            );
            assert_eq!(
                marker.exists(),
                mode == "release-wins",
                "unexpected resource drop: {mode}"
            );
            if marker.exists() {
                std::fs::remove_file(marker).unwrap();
            }
        }
    }

    #[test]
    fn abi_denial_and_worker_grant_use_the_same_atomic_decision() {
        let shared = initial_phase();
        let mut denied = Session::with_phase(Arc::clone(&shared));
        let (resume, wait) = sync_channel(1);
        denied.used = true;
        denied.start(move || {
            wait.recv().unwrap();
            Resource::Acquired(())
        });
        deny_acquire(&shared);
        resume.send(()).unwrap();
        assert_eq!(denied.accept(Duration::from_secs(1)), Reply::Unavailable);
        denied.finish_for_test();
        assert_eq!(
            denied.acquire("example.guard.denied", Duration::from_secs(1)),
            Reply::Unavailable
        );
        let mut granted = Session::start_for_test(|| Resource::Acquired(()));
        assert_eq!(granted.accept(Duration::from_secs(1)), Reply::Held);
        // A rejection after the earlier grant does not retroactively free it.
        deny_acquire(&granted.phase);
        assert_eq!(granted.status(Duration::from_secs(1)), Reply::Held);
        assert_eq!(granted.release(Duration::from_secs(1)), Reply::Released);
    }

    #[test]
    fn invalid_identity_never_starts_an_owner_or_grants() {
        let mut session = Session::new();
        assert_eq!(
            session.acquire("../bad", Duration::from_secs(1)),
            Reply::Unavailable
        );
        assert_eq!(session.status(Duration::from_secs(1)), Reply::Unavailable);
        assert_eq!(session.release(Duration::from_secs(1)), Reply::Released);
    }
}
