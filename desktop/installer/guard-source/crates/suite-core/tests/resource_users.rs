use fbforge_suite_core::processes::{inspect_resource_users, ResourceUseIssue, ResourceUsers};

#[test]
fn invalid_or_incomplete_resource_lists_never_report_unused() {
    let invalid = [
        vec![],
        vec!["relative.exe".into()],
        vec![r"\\server\share\tool.exe".into()],
        vec!["C:\\Tools\\a.exe\0ignored".into()],
        vec![r"C:\Tools\..\a.exe".into()],
        vec![r#""C:\Tools\a.exe""#.into()],
        vec![r"C:\Tools\a.exe".into(), r"c:/tools/A.exe".into()],
        vec![r"C:\Tools\a.exe".into(); 8193],
    ];
    for paths in invalid {
        assert_eq!(
            inspect_resource_users(&paths),
            ResourceUsers::Unknown(ResourceUseIssue::InvalidResources)
        );
    }
}

#[cfg(not(windows))]
#[test]
fn missing_native_resource_inspection_is_explicitly_unknown() {
    assert_eq!(
        inspect_resource_users(&[r"C:\Tools\Werkzeug ä\runtime\java.exe".into()]),
        ResourceUsers::Unknown(ResourceUseIssue::UnsupportedPlatform)
    );
}

#[cfg(windows)]
#[test]
fn restart_manager_reports_the_current_executing_image_as_in_use() {
    let image = std::env::current_exe().unwrap();
    let path = image
        .to_str()
        .unwrap()
        .strip_prefix(r"\\?\")
        .unwrap_or(image.to_str().unwrap());
    assert_eq!(inspect_resource_users(&[path.into()]), ResourceUsers::InUse);
}
