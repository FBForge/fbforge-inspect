use base64::Engine;

/// The caller still obtains the destination exclusively from the native save dialog.
pub fn bytes(content: &str, format: &str) -> Result<Vec<u8>, &'static str> {
    if content.len() > 32 * 1024 * 1024 {
        return Err("report_size_limit");
    }
    match format {
        "json" | "html" | "txt" | "ics" => Ok(content.as_bytes().to_vec()),
        "zip" => {
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(content)
                .map_err(|_| "archive_encoding_invalid")?;
            if bytes.len() > 20 * 1024 * 1024 || !bytes.starts_with(b"PK\x03\x04") {
                return Err("archive_format_invalid");
            }
            Ok(bytes)
        }
        _ => Err("report_format_invalid"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn report_text_is_unchanged_and_archive_is_binary() {
        for format in ["json", "html", "txt", "ics"] {
            assert_eq!(
                bytes("Umlaut ä\n", format).unwrap(),
                "Umlaut ä\n".as_bytes()
            );
        }
        let fixture = b"PK\x03\x04\x00\xff\x00\x80";
        let encoded = base64::engine::general_purpose::STANDARD.encode(fixture);
        assert_eq!(bytes(&encoded, "zip").unwrap(), fixture);
    }
    #[test]
    fn archive_export_rejects_unknown_format_encoding_and_size() {
        assert!(bytes("x", "exe").is_err());
        assert!(bytes("not base64", "zip").is_err());
        assert!(bytes("eA==", "zip").is_err());
        assert!(bytes(&" ".repeat(32 * 1024 * 1024 + 1), "json").is_err());
        let oversized = [b"PK\x03\x04".to_vec(), vec![0; 20 * 1024 * 1024]].concat();
        let encoded = base64::engine::general_purpose::STANDARD.encode(oversized);
        assert!(bytes(&encoded, "zip").is_err());
    }
}
