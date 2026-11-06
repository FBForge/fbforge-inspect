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

/// A job is inert public JSON, never executed during export.
pub fn validate_job(content: &str) -> io::Result<()> {
    if content.len() > 2 * 1024 * 1024 {
        return Err(io::Error::other("job_size_limit"));
    }
    let value: serde_json::Value = serde_json::from_str(content)?;
    if value["format"] != "fbforge-inspect-job"
        || value["formatVersion"] != 1
        || value["checks"]
            .as_array()
            .is_none_or(|checks| checks.is_empty() || checks.len() > 32)
    {
        return Err(io::Error::other("job_format_invalid"));
    }
    Ok(())
}
pub fn write_job(content: &str, target: &Path) -> io::Result<()> {
    validate_job(content)?;
    use io::Write;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    file.write_all(content.as_bytes())
}
#[cfg(test)]
mod job_tests {
    use super::*;
    #[test]
    fn job_export_preserves_an_existing_file() {
        let root = std::env::temp_dir().join(format!(
            "inspect-job-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let target = root.join("auftrag.json");
        fs::write(&target, "existing").unwrap();
        assert!(write_job(
            r#"{"format":"fbforge-inspect-job","formatVersion":1,"checks":[{}]}"#,
            &target
        )
        .is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "existing");
        fs::remove_file(&target).unwrap();
        let content = "{\"format\":\"fbforge-inspect-job\",\"formatVersion\":1,\"checks\":[{}]}\n";
        write_job(content, &target).unwrap();
        assert_eq!(fs::read(&target).unwrap(), content.as_bytes());
        fs::remove_file(target).unwrap();
        fs::remove_dir(root).unwrap();
    }
    #[test]
    fn exported_job_requires_bounded_versioned_nonempty_envelope() {
        assert!(validate_job(
            r#"{"format":"fbforge-inspect-job","formatVersion":1,"checks":[{}]}"#
        )
        .is_ok());
        for text in [
            "null",
            "[]",
            r#"{"format":"other","formatVersion":1,"checks":[{}]}"#,
            r#"{"format":"fbforge-inspect-job","formatVersion":1,"checks":[]}"#,
        ] {
            assert!(validate_job(text).is_err());
        }
        assert!(validate_job(&" ".repeat(2 * 1024 * 1024 + 1)).is_err());
    }
}
