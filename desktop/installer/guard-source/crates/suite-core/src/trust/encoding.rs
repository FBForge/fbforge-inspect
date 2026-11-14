use super::TrustError;

pub(crate) fn decode_hex<const N: usize>(value: &str) -> Result<[u8; N], TrustError> {
    if value.len() != N * 2 {
        return Err(TrustError::InvalidEnvelope);
    }
    let mut result = [0; N];
    for (index, pair) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char)
            .to_digit(16)
            .ok_or(TrustError::InvalidEnvelope)?;
        let low = (pair[1] as char)
            .to_digit(16)
            .ok_or(TrustError::InvalidEnvelope)?;
        result[index] = ((high << 4) | low) as u8;
    }
    Ok(result)
}

pub(crate) fn encode_hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
