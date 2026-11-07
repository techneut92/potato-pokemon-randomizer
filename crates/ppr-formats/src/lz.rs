//! LZ-compressed data of either BIOS variant, told apart by its type byte.

use crate::FormatError;
use crate::lz10;
use crate::lz11::{self, LZ11};

/// Decompresses LZ10 or LZ11 data starting at `offset` of `data`, whichever its type byte names.
///
/// # Errors
/// When the data at `offset` is neither, ends early or refers back before the start of the output.
pub fn decompress(data: &[u8], offset: usize) -> Result<Vec<u8>, FormatError> {
    match data.get(offset) {
        Some(&LZ11) => lz11::decompress(data, offset),
        Some(_) => lz10::decompress(data, offset),
        None => Err(FormatError::Truncated { offset: offset as u64 }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_variant_by_type_byte() {
        let packed = lz10::compress(b"hello hello hello");
        assert_eq!(decompress(&packed, 0).expect("LZ10"), b"hello hello hello");
        assert_eq!(decompress(&[LZ11, 1, 0, 0, 0, b'x'], 0).expect("LZ11"), b"x");
        assert!(decompress(&[0x30, 1, 0, 0], 0).is_err());
        assert!(decompress(&[], 0).is_err());
    }
}
