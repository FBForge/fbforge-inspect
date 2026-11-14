use super::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

#[test]
fn a_panicking_installer_is_a_terminal_failure_after_lease_release() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let queue = InstallerHandoff::default();
    queue.submit(()).unwrap();
    assert_eq!(
        finish_after_exit(LeaseWitness(Arc::clone(&events)), &queue, 0, |()| panic!(
            "private launch panic fixture"
        )),
        Err(HandoffError::InstallFailed)
    );
    assert_eq!(*events.lock().unwrap(), vec!["lease-dropped"]);
    assert_eq!(queue.submit(()), Err(HandoffError::Closed));
}
struct LeaseWitness(Arc<Mutex<Vec<&'static str>>>);
impl Drop for LeaseWitness {
    fn drop(&mut self) {
        self.0.lock().unwrap().push("lease-dropped");
    }
}
#[test]
fn successful_enqueue_keeps_the_gate_but_failed_or_completed_attempts_release_it() {
    for (result, remains_set) in [
        (Ok(Some(7)), true),
        (Ok(None), false),
        (Err("failure"), false),
    ] {
        let gate = AtomicBool::new(true);
        assert_eq!(finish_attempt(result, &gate), result);
        assert_eq!(gate.load(Ordering::Acquire), remains_set);
    }
}
#[test]
fn queue_accepts_exactly_one_payload_and_closes_even_when_exit_has_no_payload() {
    let queue = InstallerHandoff::default();
    assert_eq!(queue.submit("first"), Ok(()));
    assert_eq!(queue.submit("second"), Err(HandoffError::AlreadyQueued));
    let mut seen = Vec::new();
    assert_eq!(
        finish_after_exit((), &queue, 0, |item| {
            seen.push(item);
            Ok(())
        }),
        Ok(0)
    );
    assert_eq!(seen, vec!["first"]);
    assert_eq!(queue.submit("late"), Err(HandoffError::Closed));
    assert!(finish_after_exit((), &queue, 0, |_| panic!("installer twice")).is_err());
    let empty = InstallerHandoff::default();
    assert_eq!(
        finish_after_exit((), &empty, 0, |_: ()| panic!(
            "ordinary exit must not install"
        )),
        Ok(0)
    );
    assert_eq!(empty.submit(()), Err(HandoffError::Closed));
}
#[test]
fn the_actual_exit_helper_drops_the_lease_before_install_and_never_resumes_after_failure() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let queue = InstallerHandoff::default();
    queue.submit(()).unwrap();
    let result = finish_after_exit(LeaseWitness(Arc::clone(&events)), &queue, 0, |()| {
        events.lock().unwrap().push("installer");
        Err(())
    });
    assert_eq!(result, Err(HandoffError::InstallFailed));
    assert_eq!(*events.lock().unwrap(), vec!["lease-dropped", "installer"]);
    assert_eq!(queue.submit(()), Err(HandoffError::Closed));
}
#[test]
fn abnormal_exit_and_poisoned_queue_never_run_an_installer() {
    let queue = InstallerHandoff::default();
    queue.submit(()).unwrap();
    assert_eq!(
        finish_after_exit((), &queue, 7, |()| panic!("abnormal exit")),
        Ok(7)
    );
    let events = Arc::new(Mutex::new(Vec::new()));
    let queue = InstallerHandoff::default();
    queue.submit(()).unwrap();
    let _ = std::panic::catch_unwind(|| {
        let _lock = queue.state.lock().unwrap();
        panic!("private poison fixture");
    });
    assert_eq!(
        finish_after_exit(LeaseWitness(Arc::clone(&events)), &queue, 0, |()| panic!(
            "poisoned"
        )),
        Err(HandoffError::Unavailable)
    );
    assert_eq!(*events.lock().unwrap(), vec!["lease-dropped"]);
}
#[cfg(windows)]
#[test]
fn the_production_exit_helper_releases_a_real_windows_lease_without_abandoning_it() {
    use fbforge_suite_coordination::{Acquisition, AppIdentity, Lease};
    let identity =
        AppIdentity::new(&format!("example.updater.exit.{}", std::process::id())).unwrap();
    let lease = match Lease::try_acquire(&identity) {
        Acquisition::Acquired(lease) => lease,
        other => panic!("{other:?}"),
    };
    assert!(matches!(Lease::try_acquire(&identity), Acquisition::Busy));
    let owner = std::thread::current().id();
    let queue = InstallerHandoff::default();
    queue.submit(()).unwrap();
    assert_eq!(
        finish_after_exit(lease, &queue, 0, |()| {
            assert_eq!(std::thread::current().id(), owner);
            // Another thread must acquire cleanly: reacquisition on the
            // same thread could hide a forgotten recursive Windows mutex.
            let acquired = std::thread::spawn(move || {
                matches!(Lease::try_acquire(&identity), Acquisition::Acquired(_))
            })
            .join()
            .unwrap();
            assert!(acquired, "other thread must acquire after owner drop");
            Ok(())
        }),
        Ok(0)
    );
}
