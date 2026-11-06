use std::{fs, io, path::Path};

/// Only bundled public license material is copied. Links and special files are rejected.
pub fn licenses(source: &Path, target: &Path) -> io::Result<()> {
    fn copy(
        source: &Path,
        target: &Path,
        count: &mut usize,
        bytes: &mut u64,
        depth: usize,
    ) -> io::Result<()> {
        let metadata = fs::symlink_metadata(source)?;
        *count += 1;
        if *count > 5000 || depth > 16 || metadata.file_type().is_symlink() {
            return Err(io::Error::other("license_export_limit_or_link"));
        }
        if metadata.is_dir() {
            fs::create_dir(target)?;
            for entry in fs::read_dir(source)? {
                let entry = entry?;
                copy(
                    &entry.path(),
                    &target.join(entry.file_name()),
                    count,
                    bytes,
                    depth + 1,
                )?;
            }
        } else if metadata.is_file() {
            *bytes = bytes.saturating_add(metadata.len());
            if *bytes > 32 * 1024 * 1024 {
                return Err(io::Error::other("license_export_size_limit"));
            }
            let mut input = fs::File::open(source)?;
            let mut output = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(target)?;
            io::copy(&mut input, &mut output)?;
        } else {
            return Err(io::Error::other("license_export_special_file"));
        }
        Ok(())
    }
    copy(source, target, &mut 0, &mut 0, 0)
}
