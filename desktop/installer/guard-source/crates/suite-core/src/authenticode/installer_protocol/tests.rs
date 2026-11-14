use super::*;
const APP: &str = "de.fbforge.test";
fn pad(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}
fn block(key: &str, kind: u16, value: &[u8], words: u16, children: &[Vec<u8>]) -> Vec<u8> {
    let mut bytes = vec![0; 6];
    for unit in key.encode_utf16().chain(Some(0)) {
        bytes.extend(unit.to_le_bytes());
    }
    pad(&mut bytes);
    bytes.extend(value);
    for child in children {
        pad(&mut bytes);
        bytes.extend(child);
    }
    let len = bytes.len() as u16;
    bytes[..2].copy_from_slice(&len.to_le_bytes());
    bytes[2..4].copy_from_slice(&words.to_le_bytes());
    bytes[4..6].copy_from_slice(&kind.to_le_bytes());
    bytes
}
fn string(key: &str, value: &str) -> Vec<u8> {
    let units: Vec<_> = value.encode_utf16().chain(Some(0)).collect();
    let bytes: Vec<_> = units.iter().flat_map(|n| n.to_le_bytes()).collect();
    block(key, 1, &bytes, units.len() as u16, &[])
}
fn resource(app: &str, protocol: &str, extra: Vec<Vec<u8>>) -> Vec<u8> {
    let mut strings = vec![
        string("FBForgeAppId", app),
        string("FBForgeInstallerProtocol", protocol),
        string("ProductVersion", "0.3.0"),
    ];
    strings.extend(extra);
    version_resource(strings, "040904b0", 0)
}
fn version_resource(strings: Vec<Vec<u8>>, table_key: &str, kind: u16) -> Vec<u8> {
    let table = block(table_key, kind, &[], 0, &strings);
    let sfi = block("StringFileInfo", kind, &[], 0, &[table]);
    let mut fixed = vec![0; 52];
    fixed[..4].copy_from_slice(&0xfeef04bdu32.to_le_bytes());
    fixed[4..8].copy_from_slice(&0x10000u32.to_le_bytes());
    block("VS_VERSION_INFO", 0, &fixed, 52, &[sfi])
}
#[test]
fn installer_protocol_exact_strings_are_only_structural_assertions() {
    assert_eq!(
        inspect(
            &resource(APP, "1", vec![string("ProductName", "Dummy")]),
            APP,
            "0.3.0"
        ),
        Ok(())
    );
}
#[test]
fn installer_protocol_missing_wrong_identity_and_future_version_are_denied() {
    for bytes in [
        resource("de.other", "1", vec![]),
        resource(APP, "2", vec![]),
        resource(APP, "01", vec![]),
        resource(APP, "1 ", vec![]),
        resource(APP, "1", vec![string("FBForgeAppId", APP)]),
    ] {
        assert!(inspect(&bytes, APP, "0.3.0").is_err());
    }
    assert!(inspect(&[], APP, "0.3.0").is_err());
}
#[test]
fn installer_protocol_rejects_every_truncation_and_corrupt_lengths() {
    let bytes = resource(APP, "1", vec![]);
    for length in 0..bytes.len() {
        assert!(
            inspect(&bytes[..length], APP, "0.3.0").is_err(),
            "length {length}"
        );
    }
    for offset in [0, 2, 4, 6, 38, 40] {
        let mut corrupt = bytes.clone();
        corrupt[offset] = 0xff;
        assert!(inspect(&corrupt, APP, "0.3.0").is_err(), "offset {offset}");
    }
    let mut trailing = bytes;
    trailing.extend([0; 4]);
    assert!(inspect(&trailing, APP, "0.3.0").is_err());
}
#[test]
fn installer_protocol_rejects_case_aliases_and_oversized_blobs() {
    assert!(inspect(
        &resource(APP, "1", vec![string("fbforgeappid", APP)]),
        APP,
        "0.3.0"
    )
    .is_err());
    assert!(inspect(&vec![0; 65536], APP, "0.3.0").is_err());
}

fn put16(bytes: &mut [u8], at: usize, n: u16) {
    bytes[at..at + 2].copy_from_slice(&n.to_le_bytes());
}
fn put32(bytes: &mut [u8], at: usize, n: u32) {
    bytes[at..at + 4].copy_from_slice(&n.to_le_bytes());
}
fn pe(installer32: bool) -> Vec<u8> {
    let mut bytes = vec![0; 0x1208];
    bytes[..2].copy_from_slice(b"MZ");
    put32(&mut bytes, 60, 0x80);
    bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
    put16(&mut bytes, 0x84, if installer32 { 0x14c } else { 0x8664 });
    put16(&mut bytes, 0x86, 1);
    let optional = 0x98;
    let optional_size = if installer32 { 224 } else { 240 };
    put16(&mut bytes, 0x94, optional_size);
    put16(&mut bytes, 0x96, 2);
    put16(
        &mut bytes,
        optional,
        if installer32 { 0x10b } else { 0x20b },
    );
    put32(&mut bytes, optional + 32, 4096);
    put32(&mut bytes, optional + 36, 512);
    put32(&mut bytes, optional + 56, 8192);
    put32(&mut bytes, optional + 60, 512);
    let directories = optional + if installer32 { 96 } else { 112 };
    put32(&mut bytes, directories - 4, 16);
    put32(&mut bytes, directories + 16, 4096);
    put32(&mut bytes, directories + 20, 4096);
    put32(&mut bytes, directories + 32, 0x1200);
    put32(&mut bytes, directories + 36, 8);
    let section = optional + optional_size as usize;
    put32(&mut bytes, section + 8, 4096);
    put32(&mut bytes, section + 12, 4096);
    put32(&mut bytes, section + 16, 4096);
    put32(&mut bytes, section + 20, 512);
    put16(&mut bytes, 512 + 14, 1);
    put32(&mut bytes, 512 + 16, 16);
    put32(&mut bytes, 512 + 20, 0x80000018);
    put16(&mut bytes, 512 + 24 + 14, 1);
    put32(&mut bytes, 512 + 40, 1);
    put32(&mut bytes, 512 + 44, 0x80000030);
    put16(&mut bytes, 512 + 48 + 14, 1);
    put32(&mut bytes, 512 + 64, 1033);
    put32(&mut bytes, 512 + 68, 72);
    let payload = resource(APP, "1", vec![]);
    put32(&mut bytes, 512 + 72, 4096 + 128);
    put32(&mut bytes, 512 + 76, payload.len() as u32);
    bytes[640..640 + payload.len()].copy_from_slice(&payload);
    bytes
}
#[test]
fn installer_protocol_resource_is_supported_in_pe32_and_pe32plus() {
    for x86 in [true, false] {
        assert_eq!(
            super::super::cooperation::inspect_installer_protocol(
                &mut std::io::Cursor::new(pe(x86)),
                APP,
                "0.3.0"
            ),
            Ok(())
        );
        let data = crate::authenticode::read_installer_identity(&mut std::io::Cursor::new(pe(x86)))
            .unwrap();
        assert_eq!(data.app_id(), APP);
        assert_eq!(data.version().to_string(), "0.3.0");
        assert_eq!(data.protocol(), 1);
    }
}
#[test]
fn installer_protocol_does_not_weaken_application_x64_or_resource_ranges() {
    assert!(
        super::super::cooperation::inspect_marker(&mut std::io::Cursor::new(pe(true)), APP)
            .is_err()
    );
    for resource in [0x80u32, 0x1200, 0xfffffff0] {
        let mut bytes = pe(true);
        put32(&mut bytes, 512 + 72, resource);
        assert!(super::super::cooperation::inspect_installer_protocol(
            &mut std::io::Cursor::new(bytes),
            APP,
            "0.3.0"
        )
        .is_err());
    }
    let mut bytes = pe(true);
    put16(&mut bytes, 0x96, 0x2002);
    assert!(super::super::cooperation::inspect_installer_protocol(
        &mut std::io::Cursor::new(bytes),
        APP,
        "0.3.0"
    )
    .is_err());
    for (machine, magic) in [(0x14c, 0x20b), (0x8664, 0x10b), (0xaa64, 0x20b)] {
        let mut bytes = pe(true);
        put16(&mut bytes, 0x84, machine);
        put16(&mut bytes, 0x98, magic);
        assert!(super::super::cooperation::inspect_installer_protocol(
            &mut std::io::Cursor::new(bytes),
            APP,
            "0.3.0"
        )
        .is_err());
    }
}

#[test]
fn installer_protocol_product_version_is_exact_canonical_and_unique() {
    let bytes = resource(APP, "1", vec![]);
    for version in ["0.4.0", "v0.3.0", "0.3.0.0", "0.3.0+other"] {
        assert!(inspect(&bytes, APP, version).is_err());
    }
    assert!(inspect(
        &resource(APP, "1", vec![string("ProductVersion", "0.3.0")]),
        APP,
        "0.3.0"
    )
    .is_err());
    assert!(inspect(
        &resource(APP, "1", vec![string("productversion", "0.3.0")]),
        APP,
        "0.3.0"
    )
    .is_err());
}

#[test]
fn installer_protocol_string_lengths_are_words_and_utf16_must_be_valid() {
    let mut leaf = string("FBForgeInstallerProtocol", "1");
    leaf[2..4].copy_from_slice(&4u16.to_le_bytes());
    assert!(super::block(&leaf).is_err());
    let mut bytes = resource(APP, "1", vec![]);
    let needle: Vec<_> = APP.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let start = bytes
        .windows(needle.len())
        .position(|n| n == needle.as_slice())
        .unwrap();
    bytes[start..start + 2].copy_from_slice(&0xd800u16.to_le_bytes());
    assert!(inspect(&bytes, APP, "0.3.0").is_err());
}

#[test]
fn sdk_text_containers_supply_version_data_without_an_installer_claim() {
    let bytes = version_resource(vec![string("ProductVersion", "0.3.0")], "000004b0", 1);
    assert_eq!(super::product_version(&bytes).unwrap().to_string(), "0.3.0");
    assert!(inspect(&bytes, APP, "0.3.0").is_err());
}

#[test]
fn canonical_legacy_version_info_is_not_an_installer_protocol_assertion() {
    for strings in [
        vec![string("ProductVersion", "0.3.0")],
        vec![
            string("ProductVersion", "0.3.0"),
            string("FBForgeAppId", APP),
        ],
        vec![
            string("ProductVersion", "0.3.0"),
            string("FBForgeInstallerProtocol", "1"),
        ],
    ] {
        let bytes = version_resource(strings, "040904b0", 0);
        assert_eq!(super::product_version(&bytes).unwrap().to_string(), "0.3.0");
        assert!(inspect(&bytes, APP, "0.3.0").is_err());
    }
}

#[test]
fn a_valid_different_product_version_does_not_match_the_release() {
    let bytes = version_resource(
        vec![
            string("FBForgeAppId", APP),
            string("FBForgeInstallerProtocol", "1"),
            string("ProductVersion", "0.4.0"),
        ],
        "040904b0",
        0,
    );
    assert_eq!(super::product_version(&bytes).unwrap().to_string(), "0.4.0");
    assert!(inspect(&bytes, APP, "0.3.0").is_err());
}

#[test]
fn held_program_version_reader_is_x64_only_and_structural() {
    assert_eq!(
        crate::authenticode::read_program_product_version(&mut std::io::Cursor::new(pe(false)))
            .unwrap()
            .to_string(),
        "0.3.0"
    );
    assert!(
        crate::authenticode::read_program_product_version(&mut std::io::Cursor::new(pe(true)))
            .is_err()
    );
}

#[test]
fn real_nsis_313_compiler_version_info_has_the_exact_protocol_and_release_version() {
    // Unsigned public fixture: compiler layout only, never publisher evidence.
    let bytes = include_bytes!("fixtures/nsis-3.13-version-info.bin");
    assert_eq!(inspect(bytes, APP, "0.3.0"), Ok(()));
    let mut unknown_structure = bytes.to_vec();
    unknown_structure[44..48].copy_from_slice(&2u32.to_le_bytes());
    assert!(inspect(&unknown_structure, APP, "0.3.0").is_err());
    assert!(inspect(bytes, "de.other", "0.3.0").is_err());
    assert!(inspect(bytes, APP, "0.4.0").is_err());
}

#[test]
fn real_sdk_neutral_version_info_is_program_data_without_an_installer_claim() {
    let bytes = include_bytes!("fixtures/sdk-neutral-version-info.bin");
    assert_eq!(product_version(bytes).unwrap().to_string(), "0.3.0");
    assert!(identity(bytes).is_err());
}

#[test]
fn host_manifest_digest_must_be_exact_when_present() {
    for value in [
        "AB".repeat(32),
        "ab".repeat(31),
        "x".repeat(64),
        format!("{} ", "ab".repeat(32)),
    ] {
        assert!(identity(&resource(
            APP,
            "1",
            vec![string("FBForgePayloadManifestSha256", &value)]
        ))
        .is_err());
    }
    assert!(identity(&resource(
        APP,
        "1",
        vec![string("fbforgepayloadmanifestsha256", &"ab".repeat(32))]
    ))
    .is_err());
}

#[test]
fn host_manifest_digest_is_optional_for_v1_but_exact_when_present() {
    assert!(identity(&resource(APP, "1", vec![]))
        .unwrap()
        .payload_manifest_sha256()
        .is_none());
    let bytes = resource(
        APP,
        "1",
        vec![string("FBForgePayloadManifestSha256", &"ab".repeat(32))],
    );
    assert_eq!(
        identity(&bytes).unwrap().payload_manifest_sha256(),
        Some(&[0xab; 32])
    );
    for value in [
        "AB".repeat(32),
        "ab".repeat(31),
        "x".repeat(64),
        format!("{} ", "ab".repeat(32)),
    ] {
        assert!(identity(&resource(
            APP,
            "1",
            vec![string("FBForgePayloadManifestSha256", &value)]
        ))
        .is_err());
    }
    assert!(identity(&resource(
        APP,
        "1",
        vec![string("fbforgepayloadmanifestsha256", &"ab".repeat(32))]
    ))
    .is_err());
    assert!(identity(&resource(
        APP,
        "1",
        vec![
            string("FBForgePayloadManifestSha256", &"ab".repeat(32)),
            string("FBForgePayloadManifestSha256", &"ab".repeat(32))
        ]
    ))
    .is_err());
}
