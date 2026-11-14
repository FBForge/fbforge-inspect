//! A deliberately narrow PE resource reader. Parsing is not publisher proof.
use super::{IndeterminateReason, InvalidReason, VerificationFailure};
use crate::catalog::is_identifier;
use std::io::{self, Read, Seek, SeekFrom};

pub(super) const MAX_PROGRAM_BYTES: u64 = 1 << 30;
const MAX_RESOURCE_BYTES: u64 = 8 << 20;
const MAX_ENTRIES: usize = 4096;
fn invalid() -> VerificationFailure {
    VerificationFailure::Invalid(InvalidReason::InvalidImage)
}
fn rejected() -> VerificationFailure {
    VerificationFailure::Invalid(InvalidReason::CooperationRejected)
}
fn missing() -> VerificationFailure {
    VerificationFailure::Invalid(InvalidReason::CooperationMissing)
}
fn io_error(error: io::Error) -> VerificationFailure {
    if error.kind() == io::ErrorKind::UnexpectedEof {
        invalid()
    } else {
        VerificationFailure::Indeterminate(IndeterminateReason::PackageUnavailable)
    }
}
fn read_at<const N: usize>(
    file: &mut (impl Read + Seek),
    offset: u64,
) -> Result<[u8; N], VerificationFailure> {
    file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
    let mut bytes = [0; N];
    file.read_exact(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}
fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().expect("fixed field"))
}
fn u32_at(bytes: &[u8], offset: usize) -> u64 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("fixed field")).into()
}
fn end(start: u64, length: u64) -> Result<u64, VerificationFailure> {
    start.checked_add(length).ok_or_else(invalid)
}
fn overlaps(a: (u64, u64), b: (u64, u64)) -> bool {
    a.0 < b.1 && b.0 < a.1
}
#[derive(Clone, Copy)]
struct Section {
    raw: (u64, u64),
    virtual_range: (u64, u64),
    backed_end: u64,
}

pub(super) fn inspect_marker(
    file: &mut (impl Read + Seek),
    app_id: &str,
) -> Result<(), VerificationFailure> {
    inspect_resource(file, app_id, ResourceKind::Program).map(|_| ())
}

#[derive(Clone, Copy)]
enum ResourceKind {
    Program,
    Installer,
    ProgramVersion,
}

enum ResourceData {
    ProgramMarker,
    InstallerIdentity(super::DeclaredInstallerIdentity),
    ProgramVersion(semver::Version),
}

#[cfg(test)]
pub(super) fn inspect_installer_protocol(
    file: &mut (impl Read + Seek),
    app_id: &str,
    version: &str,
) -> Result<(), VerificationFailure> {
    let data = read_installer_identity(file)?;
    super::installer_protocol::require_identity(&data, app_id, version)
}

pub(super) fn read_installer_identity(
    file: &mut (impl Read + Seek),
) -> Result<super::DeclaredInstallerIdentity, VerificationFailure> {
    match inspect_resource(file, "", ResourceKind::Installer)? {
        ResourceData::InstallerIdentity(data) => Ok(data),
        _ => Err(invalid()),
    }
}

pub(super) fn read_program_product_version(
    file: &mut (impl Read + Seek),
) -> Result<semver::Version, VerificationFailure> {
    match inspect_resource(file, "", ResourceKind::ProgramVersion)? {
        ResourceData::ProgramVersion(version) => Ok(version),
        _ => Err(invalid()),
    }
}

fn inspect_resource(
    file: &mut (impl Read + Seek),
    app_id: &str,
    kind: ResourceKind,
) -> Result<ResourceData, VerificationFailure> {
    let length = file.seek(SeekFrom::End(0)).map_err(io_error)?;
    let maximum = match kind {
        ResourceKind::Program | ResourceKind::ProgramVersion => MAX_PROGRAM_BYTES,
        ResourceKind::Installer => 32 << 30,
    };
    if !(64..=maximum).contains(&length) {
        return Err(invalid());
    }
    let dos = read_at::<64>(file, 0)?;
    let pe_offset = u32_at(&dos, 60);
    if &dos[..2] != b"MZ" || !(64..=1_048_576).contains(&pe_offset) {
        return Err(invalid());
    }
    let pe = read_at::<24>(file, pe_offset)?;
    let section_count = u16_at(&pe, 6) as u64;
    let optional_size = u16_at(&pe, 20) as u64;
    let minimum_optional = match kind {
        ResourceKind::Installer => 136,
        _ => 152,
    };
    let supported_machine = match kind {
        ResourceKind::Installer => matches!(u16_at(&pe, 4), 0x14c | 0x8664),
        _ => u16_at(&pe, 4) == 0x8664,
    };
    if &pe[..4] != b"PE\0\0"
        || !supported_machine
        || !(1..=96).contains(&section_count)
        || !(minimum_optional..=4096).contains(&optional_size)
        || u16_at(&pe, 22) & 0x2002 != 2
    {
        return Err(invalid());
    }
    let optional_start = end(pe_offset, 24)?;
    let optional = read_at::<96>(file, optional_start)?;
    let directories_start = match (u16_at(&pe, 4), u16_at(&optional, 0), kind) {
        (0x8664, 0x20b, _) if optional_size >= 152 => 112,
        (0x14c, 0x10b, ResourceKind::Installer) => 96,
        _ => return Err(invalid()),
    };
    let alignment = u32_at(&optional, 32);
    let file_alignment = u32_at(&optional, 36);
    let image_size = u32_at(&optional, 56);
    let headers = u32_at(&optional, 60);
    let directory_count = u32_at(
        &read_at::<4>(file, optional_start + directories_start - 4)?,
        0,
    );
    let table = end(optional_start, optional_size)?;
    let table_end = end(table, section_count * 40)?;
    if !alignment.is_power_of_two()
        || alignment < 4096
        || !file_alignment.is_power_of_two()
        || !(512..=65536).contains(&file_alignment)
        || file_alignment > alignment
        || image_size == 0
        || !image_size.is_multiple_of(alignment)
        || headers < table_end
        || headers > length
        || !headers.is_multiple_of(file_alignment)
        || !(5..=16).contains(&directory_count)
        || directories_start + directory_count * 8 > optional_size
    {
        return Err(invalid());
    }
    let directories = read_at::<40>(file, optional_start + directories_start)?;
    let resource_rva = u32_at(&directories, 16);
    let resource_size = u32_at(&directories, 20);
    let certificate = u32_at(&directories, 32);
    let certificate_size = u32_at(&directories, 36);
    if certificate == 0
        || !certificate.is_multiple_of(8)
        || certificate_size < 8
        || end(certificate, certificate_size)? != length
        || certificate < headers
    {
        return Err(invalid());
    }
    let mut sections: Vec<Section> = Vec::with_capacity(section_count as usize);
    for index in 0..section_count {
        let bytes = read_at::<40>(file, table + index * 40)?;
        let virtual_size = u32_at(&bytes, 8);
        let va = u32_at(&bytes, 12);
        let raw_size = u32_at(&bytes, 16);
        let raw_start = u32_at(&bytes, 20);
        let virtual_end = end(va, virtual_size.max(raw_size))?;
        let raw_end = end(raw_start, raw_size)?;
        if va < headers
            || !va.is_multiple_of(alignment)
            || virtual_end > image_size
            || virtual_size.max(raw_size) == 0
            || (raw_size != 0
                && (raw_start < headers
                    || !raw_start.is_multiple_of(file_alignment)
                    || !raw_size.is_multiple_of(file_alignment)
                    || raw_end > certificate))
        {
            return Err(invalid());
        }
        let section = Section {
            raw: (raw_start, raw_end),
            virtual_range: (va, virtual_end),
            backed_end: end(va, virtual_size.min(raw_size))?,
        };
        if sections.iter().any(|other| {
            (raw_size != 0 && other.raw.0 != other.raw.1 && overlaps(section.raw, other.raw))
                || overlaps(section.virtual_range, other.virtual_range)
        }) {
            return Err(invalid());
        }
        sections.push(section);
    }
    if resource_rva == 0 && resource_size == 0 {
        return Err(missing());
    }
    if !(16..=MAX_RESOURCE_BYTES).contains(&resource_size) {
        return Err(invalid());
    }
    let resource_end = end(resource_rva, resource_size)?;
    let section = sections
        .iter()
        .find(|section| {
            resource_rva >= section.virtual_range.0 && resource_end <= section.backed_end
        })
        .ok_or_else(invalid)?;
    let base = end(section.raw.0, resource_rva - section.virtual_range.0)?;
    let mut tree = ResourceTree {
        file,
        base,
        rva: resource_rva,
        size: resource_size,
        entries: 0,
        occupied: Vec::new(),
        found: None,
        app_id,
        kind,
    };
    tree.directory(0, 0, false)?;
    tree.found.ok_or_else(missing)
}

struct ResourceTree<'a, R> {
    file: &'a mut R,
    base: u64,
    rva: u64,
    size: u64,
    entries: usize,
    occupied: Vec<(u64, u64)>,
    found: Option<ResourceData>,
    app_id: &'a str,
    kind: ResourceKind,
}
impl<R: Read + Seek> ResourceTree<'_, R> {
    fn range(&self, offset: u64, size: u64) -> Result<(), VerificationFailure> {
        if end(offset, size)? > self.size {
            return Err(invalid());
        }
        Ok(())
    }
    fn claim(&mut self, offset: u64, size: u64) -> Result<(), VerificationFailure> {
        self.range(offset, size)?;
        let range = (offset, end(offset, size)?);
        if self.occupied.iter().any(|other| overlaps(range, *other)) {
            return Err(invalid());
        }
        self.occupied.push(range);
        Ok(())
    }
    fn directory(
        &mut self,
        offset: u64,
        depth: u8,
        target: bool,
    ) -> Result<(), VerificationFailure> {
        self.range(offset, 16)?;
        if !offset.is_multiple_of(4) {
            return Err(invalid());
        }
        let header = read_at::<16>(self.file, self.base + offset)?;
        let named = u16_at(&header, 12) as usize;
        let count = named + u16_at(&header, 14) as usize;
        self.entries = self.entries.checked_add(count).ok_or_else(invalid)?;
        if count == 0 || self.entries > MAX_ENTRIES || (depth == 2 && target && count != 1) {
            return Err(invalid());
        }
        self.claim(offset, 16 + count as u64 * 8)?;
        let mut keys = std::collections::HashSet::new();
        for index in 0..count {
            let entry = read_at::<8>(self.file, self.base + offset + 16 + index as u64 * 8)?;
            let name = u32_at(&entry, 0);
            let value = u32_at(&entry, 4);
            let is_named = name & 0x80000000 != 0;
            if is_named != (index < named) {
                return Err(invalid());
            }
            let key = if is_named {
                let string_offset = name & 0x7fffffff;
                self.range(string_offset, 2)?;
                let length = u16_at(&read_at::<2>(self.file, self.base + string_offset)?, 0) as u64;
                if length == 0 || length > 128 {
                    return Err(invalid());
                }
                self.range(string_offset, 2 + length * 2)?;
                let mut text = Vec::with_capacity(length as usize + 1);
                text.push(1u32);
                let mut bytes = [0; 256];
                self.file
                    .read_exact(&mut bytes[..length as usize * 2])
                    .map_err(io_error)?;
                text.extend(
                    bytes[..length as usize * 2]
                        .chunks_exact(2)
                        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]) as u32),
                );
                text
            } else {
                if name > 0xffff {
                    return Err(invalid());
                }
                vec![0, name as u32]
            };
            if !keys.insert(key) {
                return Err(invalid());
            }
            let matches = !is_named
                && match depth {
                    0 => {
                        name == match self.kind {
                            ResourceKind::Program => 10,
                            ResourceKind::Installer | ResourceKind::ProgramVersion => 16,
                        }
                    }
                    1 => {
                        target
                            && name
                                == match self.kind {
                                    ResourceKind::Program => 0x4642,
                                    ResourceKind::Installer | ResourceKind::ProgramVersion => 1,
                                }
                    }
                    _ => target,
                };
            let child = value & 0x7fffffff;
            if depth < 2 {
                if value & 0x80000000 == 0 {
                    return Err(invalid());
                }
                self.directory(child, depth + 1, matches)?;
            } else {
                if value & 0x80000000 != 0 || (target && is_named) || !child.is_multiple_of(4) {
                    return Err(invalid());
                }
                self.claim(child, 16)?;
                let data = read_at::<16>(self.file, self.base + child)?;
                let rva = u32_at(&data, 0);
                let size = u32_at(&data, 4);
                if rva < self.rva || size == 0 || u32_at(&data, 12) != 0 {
                    return Err(invalid());
                }
                let payload = rva - self.rva;
                self.claim(payload, size)?;
                if matches {
                    if self.found.is_some() {
                        return Err(invalid());
                    }
                    let data = match self.kind {
                        ResourceKind::Program => {
                            self.marker(payload, size)?;
                            ResourceData::ProgramMarker
                        }
                        ResourceKind::Installer | ResourceKind::ProgramVersion => {
                            if size > u16::MAX as u64 {
                                return Err(rejected());
                            }
                            self.file
                                .seek(SeekFrom::Start(self.base + payload))
                                .map_err(io_error)?;
                            let mut bytes = vec![0; size as usize];
                            self.file.read_exact(&mut bytes).map_err(io_error)?;
                            match self.kind {
                                ResourceKind::Installer => ResourceData::InstallerIdentity(
                                    super::installer_protocol::identity(&bytes)?,
                                ),
                                ResourceKind::ProgramVersion => ResourceData::ProgramVersion(
                                    super::installer_protocol::product_version(&bytes)?,
                                ),
                                ResourceKind::Program => unreachable!("handled above"),
                            }
                        }
                    };
                    self.found = Some(data);
                }
            }
        }
        Ok(())
    }
    fn marker(&mut self, offset: u64, size: u64) -> Result<(), VerificationFailure> {
        if !(15..=142).contains(&size) {
            return Err(rejected());
        }
        let header = read_at::<14>(self.file, self.base + offset)?;
        let length = u16_at(&header, 12) as usize;
        if &header[..8] != b"FBFORGE\0"
            || u16_at(&header, 8) != 1
            || u16_at(&header, 10) != 1
            || !(1..=128).contains(&length)
            || size != 14 + length as u64
        {
            return Err(rejected());
        }
        let mut bytes = [0; 128];
        self.file
            .read_exact(&mut bytes[..length])
            .map_err(io_error)?;
        let app_id = std::str::from_utf8(&bytes[..length]).map_err(|_| rejected())?;
        if !is_identifier(app_id) || app_id != self.app_id {
            return Err(rejected());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
