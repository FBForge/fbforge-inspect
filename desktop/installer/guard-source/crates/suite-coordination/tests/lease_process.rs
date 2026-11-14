//! Process creation lives in its own integration-test executable. Other test
//! threads cannot contribute inherited handles around a child launch.
#![cfg(windows)]

use fbforge_suite_coordination::{Acquisition, AppIdentity, Lease};
use std::process::{Command, Stdio};

const MODE: &str = "FBFORGE_COORDINATION_FIXTURE_MODE";
const IDENTITY: &str = "FBFORGE_COORDINATION_FIXTURE_ID";

fn run_child(mode: &str, identity: &AppIdentity) {
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "actual_second_process_observes_exclusion_and_release",
        ])
        .env_clear()
        .env(MODE, mode)
        .env(IDENTITY, identity.as_str())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}{}",
        String::from_utf8_lossy(&child.stdout),
        String::from_utf8_lossy(&child.stderr)
    );
}

#[test]
fn actual_second_process_observes_exclusion_and_release() {
    if let Ok(mode) = std::env::var(MODE) {
        let identity = AppIdentity::new(&std::env::var(IDENTITY).unwrap()).unwrap();
        let outcome = Lease::try_acquire(&identity);
        match mode.as_str() {
            "busy" => assert!(matches!(outcome, Acquisition::Busy), "{outcome:?}"),
            "free" => assert!(matches!(outcome, Acquisition::Acquired(_)), "{outcome:?}"),
            _ => panic!("unknown isolated fixture mode"),
        }
        return;
    }
    let identity = AppIdentity::new(&format!(
        "example.coordination.process-fixture.{}",
        std::process::id()
    ))
    .unwrap();
    let lease = match Lease::try_acquire(&identity) {
        Acquisition::Acquired(lease) => lease,
        outcome => panic!("fixture lease unavailable: {outcome:?}"),
    };
    run_child("busy", &identity);
    drop(lease);
    run_child("free", &identity);
    assert!(matches!(
        Lease::try_acquire(&identity),
        Acquisition::Acquired(_)
    ));
}
