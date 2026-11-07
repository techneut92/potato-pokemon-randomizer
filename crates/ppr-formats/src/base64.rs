//! Base64 with the standard alphabet and padding (RFC 4648).

use crate::FormatError;

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` in Base64.
#[must_use]
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(char::from(ALPHABET[(n >> (18 - 6 * i) & 0x3F) as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn value(c: u8) -> Option<u32> {
    ALPHABET
        .iter()
        .position(|&a| a == c)
        .and_then(|v| u32::try_from(v).ok())
}

/// The bytes of Base64 `text`; padding is optional, but nothing else outside the alphabet is allowed.
pub fn decode(text: &str) -> Result<Vec<u8>, FormatError> {
    let invalid = || FormatError::WrongFormat {
        format: "Base64",
        reason: "a character or the length is not allowed",
    };
    let data = text.trim_end_matches('=').as_bytes();
    if text.len() - data.len() > 2 || data.len() % 4 == 1 {
        return Err(invalid());
    }
    let mut out = Vec::with_capacity(data.len() * 3 / 4);
    for chunk in data.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= value(c).ok_or_else(invalid)? << (18 - 6 * i);
        }
        let bytes = n.to_be_bytes();
        out.extend_from_slice(&bytes[1..chunk.len()]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_rfc_4648_vectors() {
        for (bytes, text) in [
            (&b""[..], ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (&[0xFB, 0xFF, 0x00][..], "+/8A"),
        ] {
            assert_eq!(encode(bytes), text);
            assert_eq!(decode(text).expect("valid"), bytes);
        }
        assert_eq!(decode("Zm8").expect("unpadded"), b"fo");
        assert!(decode("Zm8!").is_err());
        assert!(decode("Z").is_err());
        assert!(decode("Zg===").is_err());
    }
}
