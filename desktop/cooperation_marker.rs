//! Build-time assertion only; the Suite still verifies publisher and runtime state.
use std::{fs, io, path::Path};

pub(crate) const APP_ID: &str = "de.fbforge.inspect";

fn marker_bytes() -> Vec<u8> {
    let mut bytes = b"FBFORGE\0\x01\0\x01\0".to_vec();
    bytes.extend_from_slice(&(APP_ID.len() as u16).to_le_bytes());
    bytes.extend_from_slice(APP_ID.as_bytes());
    bytes
}

fn rc_filename(path: &str) -> io::Result<String> {
    if path.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Invalid resource filename",
        ));
    }
    // RC uses doubled quotes and C-style escaped backslashes (tauri-winres too).
    Ok(format!(
        "\"{}\"",
        path.replace('\\', "\\\\").replace('"', "\"\"")
    ))
}

pub(crate) fn resource_for_target(target: &str, out_dir: &Path) -> io::Result<Option<String>> {
    // Build scripts run on the host; the resource belongs to the Cargo target.
    if !target.split('-').any(|part| part == "windows") {
        return Ok(None);
    }
    let path = out_dir.join("fbforge-cooperation.bin");
    let path_text = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Invalid resource filename"))?;
    let filename = rc_filename(path_text)?;
    fs::write(&path, marker_bytes())?;
    // A file resource includes exact bytes: no RC string terminator or padding.
    Ok(Some(format!("LANGUAGE 0, 0\n17986 RCDATA {filename}\n")))
}

#[cfg(test)]
#[path = "cooperation_marker/tests.rs"]
mod tests;
