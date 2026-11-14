//! Keep process creation in its own test executable. During fork/exec, a child
//! can briefly inherit another thread's close-on-exec lock descriptor. That
//! would legitimately keep the unrelated test's lock alive after parent drop.
//! Cargo runs integration-test executables serially without reducing the number
//! of threads inside any executable. The cross-process lock contract stays real.
use fbforge_suite_core::storage::{StateStore, StoreError};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "suite-state-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn store(&self) -> PathBuf {
        self.0.join("FBForgeSuite")
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn file_lock_blocks_other_processes_and_releases_with_the_store() {
    const CHILD_ROOT: &str = "FBFORGE_STORE_LOCK_TEST_ROOT";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        assert!(matches!(
            StateStore::open(&PathBuf::from(root)),
            Err(StoreError::Busy)
        ));
        return;
    }
    let directory = Directory::new();
    let store = StateStore::open(&directory.store()).unwrap();
    let child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "file_lock_blocks_other_processes_and_releases_with_the_store",
        ])
        .env_clear()
        .env(CHILD_ROOT, directory.store())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "{}",
        String::from_utf8_lossy(&child.stderr)
    );
    drop(store);
    assert!(StateStore::open(&directory.store()).is_ok());
}
