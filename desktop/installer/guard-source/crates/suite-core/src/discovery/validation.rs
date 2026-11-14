use std::{
    fmt,
    io::{self, Read, Seek, SeekFrom},
};

/// Validate syntax before any filesystem access. This is not ownership,
/// reparse-point, publisher or execution validation.
pub fn validate_windows_location(raw: &str) -> Result<String, DetectionFailure> {
    let value = if raw.starts_with('"') && raw.ends_with('"') && raw.len() >= 2 {
        &raw[1..raw.len() - 1]
    } else {
        raw
    };
    let bytes = value.as_bytes();
    if bytes.len() < 4
        || bytes.len() > 4096
        || !bytes[0].is_ascii_alphabetic()
        || bytes[1] != b':'
        || !matches!(bytes[2], b'\\' | b'/')
        || !value[3..].split(['\\', '/']).all(valid_component)
    {
        return Err(DetectionFailure::UnsafeLocation);
    }
    Ok(value.to_owned())
}

fn valid_component(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let device = lower.split('.').next().unwrap_or_default();
    !value.is_empty()
        && value.len() <= 255
        && value.trim() == value
        && !value.ends_with('.')
        && !value
            .chars()
            .any(|c| c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        && !matches!(device, "con" | "prn" | "aux" | "nul")
        && !(device.len() == 4
            && (device.starts_with("com") || device.starts_with("lpt"))
            && matches!(device.as_bytes()[3], b'1'..=b'9'))
}

/// Bounded architecture/header probe only, not a complete PE loader, signature
/// verification or installation-health check. No data is executed.
pub fn inspect_windows_installer_image(
    reader: &mut (impl Read + Seek),
) -> Result<(), DetectionFailure> {
    inspect_windows_image(reader, ImageKind::Installer)
}

/// Installed applications must be x64 even when their installer stub is x86.
pub fn inspect_windows_x64_image(reader: &mut (impl Read + Seek)) -> Result<(), DetectionFailure> {
    inspect_windows_image(reader, ImageKind::ApplicationX64)
}

enum ImageKind {
    Installer,
    ApplicationX64,
}

fn inspect_windows_image(
    reader: &mut (impl Read + Seek),
    kind: ImageKind,
) -> Result<(), DetectionFailure> {
    reader
        .seek(SeekFrom::Start(0))
        .map_err(|_| DetectionFailure::Unreadable)?;
    let mut dos = [0u8; 64];
    read_header(reader, &mut dos)?;
    if &dos[..2] != b"MZ" {
        return Err(DetectionFailure::InvalidImage);
    }
    let offset = u32::from_le_bytes(dos[60..64].try_into().expect("fixed DOS offset"));
    if !(64..=1_048_576).contains(&offset) {
        return Err(DetectionFailure::InvalidImage);
    }
    reader
        .seek(SeekFrom::Start(offset.into()))
        .map_err(|_| DetectionFailure::Unreadable)?;
    let mut pe = [0u8; 26];
    read_header(reader, &mut pe)?;
    let machine = u16::from_le_bytes([pe[4], pe[5]]);
    let sections = u16::from_le_bytes([pe[6], pe[7]]);
    let optional_size = u16::from_le_bytes([pe[20], pe[21]]);
    let flags = u16::from_le_bytes([pe[22], pe[23]]);
    let magic = u16::from_le_bytes([pe[24], pe[25]]);
    let is_x64 = machine == 0x8664 && magic == 0x20b && optional_size >= 112;
    let is_x86 = machine == 0x14c && magic == 0x10b && optional_size >= 96;
    let has_supported_architecture = match kind {
        ImageKind::ApplicationX64 => is_x64,
        ImageKind::Installer => is_x64 || is_x86,
    };
    if &pe[..4] != b"PE\0\0"
        || !has_supported_architecture
        || !(1..=96).contains(&sections)
        || flags & 0x0002 == 0
        || flags & 0x2000 != 0
    {
        return Err(DetectionFailure::InvalidImage);
    }
    Ok(())
}

fn read_header(reader: &mut impl Read, target: &mut [u8]) -> Result<(), DetectionFailure> {
    reader
        .read_exact(target)
        .map_err(|error| match error.kind() {
            io::ErrorKind::UnexpectedEof => DetectionFailure::InvalidImage,
            _ => DetectionFailure::Unreadable,
        })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetectionFailure {
    UnsafeLocation,
    InvalidImage,
    Unreadable,
}

impl fmt::Display for DetectionFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::UnsafeLocation => {
                "Der Installationspfad ist nicht als lokaler Windows-Pfad geeignet."
            }
            Self::InvalidImage => "Die Programmdatei hat keinen unterstützten Windows-Dateikopf.",
            Self::Unreadable => "Die Programmdatei konnte nicht geprüft werden.",
        })
    }
}
impl std::error::Error for DetectionFailure {}
