//! Full Tauri Minisign boxes, not stripped raw Ed25519 signatures. Crypto,
//! prehash and global-signature checks belong to minisign-verify 0.3.0.
use super::TrustError;
use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};

fn decode_text(encoded: &str, max_bytes: usize) -> Result<String, TrustError> {
    if encoded.len() > max_bytes {
        return Err(TrustError::InvalidEnvelope);
    }
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| TrustError::InvalidEnvelope)?;
    String::from_utf8(decoded).map_err(|_| TrustError::InvalidEnvelope)
}

pub(super) fn decode_public_key(encoded: &str) -> Result<PublicKey, TrustError> {
    let text = decode_text(encoded, 8192)?;
    let lines: Vec<_> = text.lines().collect();
    if lines.len() != 2 || !lines[0].starts_with("untrusted comment: ") {
        return Err(TrustError::InvalidEnvelope);
    }
    PublicKey::decode(&text).map_err(|_| TrustError::InvalidEnvelope)
}

pub(super) fn decode_signature(encoded: &str) -> Result<Signature, TrustError> {
    let text = decode_text(encoded, 16_384)?;
    let lines: Vec<_> = text.lines().collect();
    if lines.len() != 4 || !lines[0].starts_with("untrusted comment: ") {
        return Err(TrustError::InvalidEnvelope);
    }
    Signature::decode(&text).map_err(|_| TrustError::InvalidEnvelope)
}

pub(super) fn verify(message: &[u8], encoded: &str, public_key: &str) -> Result<(), TrustError> {
    let key = decode_public_key(public_key)?;
    let signature = decode_signature(encoded)?;
    // `false` rejects legacy non-prehashed signatures. The library also checks
    // the embedded key ID and the global signature over the trusted comment.
    key.verify(message, &signature, false)
        .map_err(|_| TrustError::InvalidSignature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_a_genuine_public_legacy_signature_even_when_its_message_matches() {
        // Public upstream minisign-verify 0.3.0 test vector, src/lib.rs verify().
        // No private key. Upstream MIT copyright Frank Denis.
        let key = "untrusted comment: minisign public key E7620F1842B4E81F\nRWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
        let signature = "untrusted comment: signature from minisign secret key\nRWQf6LRCGA9i59SLOFxz6NxvASXDJeRtuZykwQepbDEGt87ig1BNpWaVWuNrm73YiIiJbq71Wi+dP9eKL8OC351vwIasSSbXxwA=\ntrusted comment: timestamp:1555779966\tfile:test\nQtKMXWyYcwdpZAlPF7tE2ENJkRd1ujvKjlj1m9RtHTBnZPa5WKU5uWRs5GoP5M/VqE81QFuMKI5k/SfNQUaOAA==";
        PublicKey::decode(key)
            .unwrap()
            .verify(b"test", &Signature::decode(signature).unwrap(), true)
            .unwrap();
        assert_eq!(
            verify(b"test", &STANDARD.encode(signature), &STANDARD.encode(key)),
            Err(TrustError::InvalidSignature)
        );
    }
}
