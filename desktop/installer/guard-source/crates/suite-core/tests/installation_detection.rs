use fbforge_suite_core::discovery::{
    inspect_windows_installer_image, inspect_windows_x64_image, validate_windows_location,
    DetectionFailure,
};
use fbforge_suite_core::{
    catalog::parse_catalog, discovery::discover_tool, inventory::DetectionIssue,
};
use std::io::{self, Cursor, Read, Seek, SeekFrom};

fn image() -> Vec<u8> {
    let mut bytes = vec![0; 512];
    bytes[..2].copy_from_slice(b"MZ");
    bytes[60..64].copy_from_slice(&128u32.to_le_bytes());
    bytes[128..132].copy_from_slice(b"PE\0\0");
    bytes[132..134].copy_from_slice(&0x8664u16.to_le_bytes());
    bytes[134..136].copy_from_slice(&1u16.to_le_bytes());
    bytes[148..150].copy_from_slice(&240u16.to_le_bytes());
    bytes[150..152].copy_from_slice(&2u16.to_le_bytes());
    bytes[152..154].copy_from_slice(&0x20bu16.to_le_bytes());
    bytes
}

#[test]
fn native_discovery_revalidates_the_tool_contract_before_accessing_metadata() {
    let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let mut tool = catalog.tools()[0].clone();
    tool.registry_names = vec![r"..\Unrelated".into()];
    let report = discover_tool(&tool);
    assert!(report.installations.is_empty());
    assert_eq!(report.issues, vec![DetectionIssue::InvalidManifest]);
}

#[cfg(not(windows))]
#[test]
fn unsupported_platform_is_not_reported_as_confirmed_absence() {
    let catalog = parse_catalog(include_str!("../../../catalog/tools.v1.json")).unwrap();
    let report = discover_tool(&catalog.tools()[0]);
    assert_eq!(report.issues, vec![DetectionIssue::UnsupportedPlatform]);
}

#[test]
fn accepts_nsis_quoted_local_install_locations_and_preserves_spaces() {
    assert_eq!(
        validate_windows_location(r#""C:\Program Files\FBForge Connect""#).unwrap(),
        r"C:\Program Files\FBForge Connect"
    );
    assert_eq!(
        validate_windows_location(r"D:\Tools\FBForge Inspect").unwrap(),
        r"D:\Tools\FBForge Inspect"
    );
}

#[test]
fn rejects_remote_device_relative_and_ambiguous_registry_paths_before_io() {
    for path in [
        r"\\server\share\Tool",
        r"\\?\C:\Tools\Tool",
        r"C:Tools\Tool",
        r"C:\",
        r"C:\Tools\..\Other",
        r"C:\Tools\CON",
        r"C:\Tools\file:stream",
        r"C:\Tools\Tool.",
        r"C:\Tools\Tool ",
        r#"C:\Tools\"quoted"#,
        "C:\\Tools\\Tool\0suffix",
    ] {
        assert!(validate_windows_location(path).is_err(), "{path}");
    }
}

#[test]
fn recognizes_x64_executable_headers_but_rejects_other_architectures_and_dlls() {
    assert!(inspect_windows_x64_image(&mut Cursor::new(image())).is_ok());
    for (offset, value) in [
        (132, 0x14cu16),
        (132, 0xaa64),
        (150, 0x2002),
        (150, 0),
        (152, 0x10b),
        (134, 0),
        (134, 97),
        (148, 0),
    ] {
        let mut bytes = image();
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert!(
            inspect_windows_x64_image(&mut Cursor::new(bytes)).is_err(),
            "offset {offset}"
        );
    }
}

#[test]
fn x64_application_packages_can_use_an_x86_or_x64_installer_stub() {
    assert!(inspect_windows_installer_image(&mut Cursor::new(image())).is_ok());
    let mut x86 = image();
    x86[132..134].copy_from_slice(&0x14cu16.to_le_bytes());
    x86[148..150].copy_from_slice(&224u16.to_le_bytes());
    x86[152..154].copy_from_slice(&0x10bu16.to_le_bytes());
    assert!(inspect_windows_installer_image(&mut Cursor::new(&x86)).is_ok());
    // This is only the bootstrap executable. Installed tools still require x64.
    assert_eq!(
        inspect_windows_x64_image(&mut Cursor::new(x86)),
        Err(DetectionFailure::InvalidImage)
    );
}

#[test]
fn installer_probe_rejects_mixed_pe_formats_unsupported_machines_and_dlls() {
    for (offset, value) in [
        (132, 0x14cu16),
        (132, 0xaa64),
        (150, 0x2002),
        (150, 0),
        (152, 0x10b),
        (134, 0),
        (148, 0),
    ] {
        let mut bytes = image();
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
        assert_eq!(
            inspect_windows_installer_image(&mut Cursor::new(bytes)),
            Err(DetectionFailure::InvalidImage)
        );
    }
    assert_eq!(
        inspect_windows_installer_image(&mut Cursor::new(b"MZ")),
        Err(DetectionFailure::InvalidImage)
    );
}

#[test]
fn bounds_header_reads_and_rejects_corrupt_or_truncated_images() {
    for offset in [0u32, 63, 1_048_577, u32::MAX] {
        let mut bytes = image();
        bytes[60..64].copy_from_slice(&offset.to_le_bytes());
        assert_eq!(
            inspect_windows_x64_image(&mut Cursor::new(bytes)).unwrap_err(),
            DetectionFailure::InvalidImage
        );
    }
    assert_eq!(
        inspect_windows_x64_image(&mut Cursor::new(b"MZ")).unwrap_err(),
        DetectionFailure::InvalidImage
    );
    let mut bytes = image();
    bytes[128] = b'X';
    assert_eq!(
        inspect_windows_x64_image(&mut Cursor::new(bytes)).unwrap_err(),
        DetectionFailure::InvalidImage
    );
}

#[test]
fn distinguishes_inaccessible_images_without_disclosing_paths() {
    struct Inaccessible;
    impl Read for Inaccessible {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "private path",
            ))
        }
    }
    impl Seek for Inaccessible {
        fn seek(&mut self, _: SeekFrom) -> io::Result<u64> {
            Ok(0)
        }
    }
    let error = inspect_windows_x64_image(&mut Inaccessible).unwrap_err();
    assert_eq!(error, DetectionFailure::Unreadable);
    assert!(!error.to_string().contains("private path"));
}
