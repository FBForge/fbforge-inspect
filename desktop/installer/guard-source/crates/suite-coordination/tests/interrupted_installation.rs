use fbforge_suite_coordination::{
    check_pending_installation, PendingInstallation, PENDING_INSTALLATION_MARKER,
};
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn pending_installations_block_start_until_explicitly_completed() {
    let root = std::env::temp_dir().join(format!(
        "suite-install-pending-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    assert_eq!(check_pending_installation(&root), Ok(()));
    let marker = root.join(PENDING_INSTALLATION_MARKER);
    fs::write(&marker, b"incomplete installation").unwrap();
    assert_eq!(
        check_pending_installation(&root),
        Err(PendingInstallation::Detected)
    );
    fs::remove_file(&marker).unwrap();
    assert_eq!(check_pending_installation(&root), Ok(()));
    // A directory or a malformed marker cannot turn pending work into success.
    fs::create_dir(&marker).unwrap();
    assert_eq!(
        check_pending_installation(&root),
        Err(PendingInstallation::Detected)
    );
    fs::remove_dir(&marker).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(root.join("missing"), &marker).unwrap();
        assert_eq!(
            check_pending_installation(&root),
            Err(PendingInstallation::Detected)
        );
        fs::remove_file(&marker).unwrap();
    }
    fs::write(&marker, b"still pending").unwrap();
    assert_eq!(
        check_pending_installation(&marker),
        Err(PendingInstallation::Unavailable)
    );
    fs::remove_dir_all(root).unwrap();
}
