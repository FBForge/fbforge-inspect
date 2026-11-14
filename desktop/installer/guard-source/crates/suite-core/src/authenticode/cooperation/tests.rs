use super::*;
use std::io::Cursor;
const APP: &str = "de.fbforge.test";
const PE: usize = 0x80;
const OPT: usize = PE + 24;
const SECT: usize = OPT + 240;
const RAW: usize = 0x200;
fn word(bytes: &mut [u8], at: usize, value: u16) {
    bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
}
fn dword(bytes: &mut [u8], at: usize, value: u32) {
    bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn image() -> Vec<u8> {
    let mut b = vec![0; 0x608];
    b[..2].copy_from_slice(b"MZ");
    dword(&mut b, 60, PE as u32);
    b[PE..PE + 4].copy_from_slice(b"PE\0\0");
    word(&mut b, PE + 4, 0x8664);
    word(&mut b, PE + 6, 1);
    word(&mut b, PE + 20, 240);
    word(&mut b, PE + 22, 2);
    word(&mut b, OPT, 0x20b);
    dword(&mut b, OPT + 32, 0x1000);
    dword(&mut b, OPT + 36, 0x200);
    dword(&mut b, OPT + 56, 0x2000);
    dword(&mut b, OPT + 60, 0x200);
    dword(&mut b, OPT + 108, 16);
    dword(&mut b, OPT + 112 + 16, 0x1000);
    dword(&mut b, OPT + 112 + 20, 0x400);
    dword(&mut b, OPT + 112 + 32, 0x600);
    dword(&mut b, OPT + 112 + 36, 8);
    dword(&mut b, SECT + 8, 0x400);
    dword(&mut b, SECT + 12, 0x1000);
    dword(&mut b, SECT + 16, 0x400);
    dword(&mut b, SECT + 20, 0x200);
    // Type RCDATA(10) -> integer name(0x4642) -> one neutral language -> data.
    word(&mut b, RAW + 14, 1);
    dword(&mut b, RAW + 16, 10);
    dword(&mut b, RAW + 20, 0x80000018);
    word(&mut b, RAW + 24 + 14, 1);
    dword(&mut b, RAW + 40, 0x4642);
    dword(&mut b, RAW + 44, 0x80000030);
    word(&mut b, RAW + 48 + 14, 1);
    dword(&mut b, RAW + 64, 0);
    dword(&mut b, RAW + 68, 72);
    dword(&mut b, RAW + 72, 0x1080);
    dword(&mut b, RAW + 76, 14 + APP.len() as u32);
    b[RAW + 128..RAW + 136].copy_from_slice(b"FBFORGE\0");
    word(&mut b, RAW + 136, 1);
    word(&mut b, RAW + 138, 1);
    word(&mut b, RAW + 140, APP.len() as u16);
    b[RAW + 142..RAW + 142 + APP.len()].copy_from_slice(APP.as_bytes());
    b
}
fn inspect(b: Vec<u8>) -> Result<(), VerificationFailure> {
    inspect_marker(&mut Cursor::new(b), APP)
}
fn bad() -> VerificationFailure {
    VerificationFailure::Invalid(InvalidReason::InvalidImage)
}
#[test]
fn exact_marker_is_structural_evidence_only() {
    assert_eq!(inspect(image()), Ok(()));
}
#[test]
fn wrong_app_and_absent_marker_are_distinct_denials() {
    assert_eq!(
        inspect_marker(&mut Cursor::new(image()), "de.other"),
        Err(VerificationFailure::Invalid(
            InvalidReason::CooperationRejected
        ))
    );
    let mut b = image();
    dword(&mut b, RAW + 40, 99);
    assert_eq!(
        inspect(b),
        Err(VerificationFailure::Invalid(
            InvalidReason::CooperationMissing
        ))
    );
}
#[test]
fn rejects_unknown_format_protocol_trailing_bytes_and_non_ascii_identity() {
    for at in [RAW + 128, RAW + 136, RAW + 138, RAW + 140, RAW + 142] {
        let mut b = image();
        b[at] = 0xff;
        assert_eq!(
            inspect(b),
            Err(VerificationFailure::Invalid(
                InvalidReason::CooperationRejected
            )),
            "{at}"
        );
    }
    let mut b = image();
    dword(&mut b, RAW + 76, 15 + APP.len() as u32);
    assert_eq!(
        inspect(b),
        Err(VerificationFailure::Invalid(
            InvalidReason::CooperationRejected
        ))
    );
}
#[test]
fn rejects_headers_certificates_overlay_and_virtual_only_marker_bytes() {
    for rva in [0x80, 0x600, 0x1400, 0xfffffff0] {
        let mut b = image();
        dword(&mut b, RAW + 72, rva);
        assert_eq!(inspect(b), Err(bad()));
    }
    for cert in [0x200, 0x280, 0x400] {
        let mut b = image();
        dword(&mut b, OPT + 144, cert);
        let certificate_size = b.len() as u32 - cert;
        dword(&mut b, OPT + 148, certificate_size);
        assert_eq!(inspect(b), Err(bad()));
    }
    let mut b = image();
    dword(&mut b, SECT + 8, 0x80);
    assert_eq!(inspect(b), Err(bad()));
}
#[test]
fn rejects_ambiguous_sections_and_raw_virtual_overlaps() {
    for (raw, va) in [(0x200, 0x2000), (0x600, 0x1000)] {
        let mut b = image();
        b.resize(0xa08, 0);
        word(&mut b, PE + 6, 2);
        dword(&mut b, OPT + 56, 0x3000);
        dword(&mut b, OPT + 144, 0xa00);
        let next = SECT + 40;
        dword(&mut b, next + 8, 0x400);
        dword(&mut b, next + 12, va);
        dword(&mut b, next + 16, 0x400);
        dword(&mut b, next + 20, raw);
        assert_eq!(inspect(b), Err(bad()));
    }
}
#[test]
fn rejects_cycles_wrong_depth_duplicate_identity_and_multiple_languages() {
    for (at, value) in [
        (RAW + 20, 0x80000000),
        (RAW + 44, 0x80000000),
        (RAW + 68, 0x80000048),
        (RAW + 20, 24),
        (RAW + 44, 48),
    ] {
        let mut b = image();
        dword(&mut b, at, value);
        assert_eq!(inspect(b), Err(bad()));
    }
    let mut b = image();
    word(&mut b, RAW + 48 + 14, 2);
    assert_eq!(inspect(b), Err(bad()));
    let mut b = image();
    word(&mut b, RAW + 14, 2);
    dword(&mut b, RAW + 24, 10);
    dword(&mut b, RAW + 28, 0x80000018);
    assert_eq!(inspect(b), Err(bad()));
}
#[test]
fn rejects_wrong_architecture_dll_short_headers_and_overflowed_ranges() {
    for (at, value) in [
        (PE + 4, 0x14c),
        (OPT, 0x10b),
        (PE + 22, 0x2002),
        (PE + 6, 97),
    ] {
        let mut b = image();
        word(&mut b, at, value);
        assert_eq!(inspect(b), Err(bad()));
    }
    for length in [0, 63, PE + 25, SECT + 39, RAW + 10, 0x607] {
        let mut b = image();
        b.truncate(length);
        assert_eq!(inspect(b), Err(bad()));
    }
    for at in [
        60,
        SECT + 12,
        SECT + 16,
        SECT + 20,
        OPT + 128,
        OPT + 132,
        OPT + 144,
        OPT + 148,
        RAW + 20,
        RAW + 72,
        RAW + 76,
    ] {
        let mut b = image();
        dword(&mut b, at, u32::MAX);
        assert_eq!(inspect(b), Err(bad()), "{at}");
    }
}
#[test]
fn rejects_unbounded_entry_count_without_reading_large_data() {
    let mut b = image();
    word(&mut b, RAW + 14, u16::MAX);
    assert_eq!(inspect(b), Err(bad()));
}
#[test]
fn rejects_data_aliases_tree_and_truncated_marker_ranges() {
    for rva in [0x1000, 0x1018, 0x1048, 0x13ff] {
        let mut b = image();
        dword(&mut b, RAW + 72, rva);
        assert_eq!(inspect(b), Err(bad()));
    }
}
#[test]
fn permits_other_resource_names_but_not_duplicate_marker_names() {
    // Relocate the second-level directory to permit two non-overlapping entries.
    let mut b = image();
    let directory = b[RAW + 24..RAW + 48].to_vec();
    b[RAW + 0x180..RAW + 0x198].copy_from_slice(&directory);
    dword(&mut b, RAW + 20, 0x80000180);
    word(&mut b, RAW + 0x180 + 14, 2);
    dword(&mut b, RAW + 0x198, 0x4642);
    dword(&mut b, RAW + 0x19c, 0x80000200);
    assert_eq!(inspect(b.clone()), Err(bad()));
    // A different valid RCDATA name and its own payload are harmless.
    dword(&mut b, RAW + 0x198, 0x4643);
    word(&mut b, RAW + 0x200 + 14, 1);
    dword(&mut b, RAW + 0x210, 0);
    dword(&mut b, RAW + 0x214, 0x220);
    dword(&mut b, RAW + 0x220, 0x1240);
    dword(&mut b, RAW + 0x224, 1);
    assert_eq!(inspect(b), Ok(()));
}
#[test]
fn bounded_named_entries_can_coexist_without_impersonating_numeric_marker() {
    let mut b = image();
    let directory = b[RAW + 24..RAW + 48].to_vec();
    b[RAW + 0x180..RAW + 0x198].copy_from_slice(&directory);
    dword(&mut b, RAW + 20, 0x80000180);
    word(&mut b, RAW + 0x180 + 12, 1);
    word(&mut b, RAW + 0x180 + 14, 1);
    dword(&mut b, RAW + 0x190, 0x80000250);
    dword(&mut b, RAW + 0x194, 0x80000200);
    dword(&mut b, RAW + 0x198, 0x4642);
    dword(&mut b, RAW + 0x19c, 0x80000030);
    word(&mut b, RAW + 0x200 + 14, 1);
    dword(&mut b, RAW + 0x210, 0);
    dword(&mut b, RAW + 0x214, 0x220);
    dword(&mut b, RAW + 0x220, 0x1240);
    dword(&mut b, RAW + 0x224, 1);
    word(&mut b, RAW + 0x250, 1);
    word(&mut b, RAW + 0x252, b'x' as u16);
    assert_eq!(inspect(b.clone()), Ok(()));
    word(&mut b, RAW + 0x250, 129);
    assert_eq!(inspect(b.clone()), Err(bad()));
    word(&mut b, RAW + 0x250, 1);
    dword(&mut b, RAW + 0x190, 0x800003ff);
    assert_eq!(inspect(b), Err(bad()));
}
#[test]
fn genuine_io_failure_is_indeterminate_not_corrupt_or_trusted() {
    struct Broken;
    impl std::io::Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        }
    }
    impl std::io::Seek for Broken {
        fn seek(&mut self, _: std::io::SeekFrom) -> std::io::Result<u64> {
            Ok(0x608)
        }
    }
    assert_eq!(
        inspect_marker(&mut Broken, APP),
        Err(VerificationFailure::Indeterminate(
            IndeterminateReason::PackageUnavailable
        ))
    );
}
