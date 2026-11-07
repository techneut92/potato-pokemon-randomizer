//! Mini packs: small archives inside GARC files, e.g. the Gen 6 and Gen 7 evolution and level-up tables of a
//! species form.
//!
//! A pack is a 2-character identifier, a 16-bit file count, `count + 1` 32-bit offsets (the last one is the end)
//! and the files, each padded to 4 bytes.

use crate::FormatError;

/// The files of the pack `data` with the 2-character `identifier`, or `None` when `data` is no such pack.
///
/// # Errors
/// When the offsets point outside the pack.
pub fn unpack(data: &[u8], identifier: [u8; 2]) -> Result<Option<Vec<Vec<u8>>>, FormatError> {
    if data.len() < 4 || data[..2] != identifier {
        return Ok(None);
    }
    let count = usize::from(u16::from_le_bytes([data[2], data[3]]));
    let offset = |i: usize| -> Result<usize, FormatError> {
        let at = 4 + i * 4;
        data.get(at..at + 4)
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]) as usize)
            .ok_or(FormatError::Truncated { offset: at as u64 })
    };
    let mut files = Vec::with_capacity(count);
    let mut start = offset(0)?;
    for i in 0..count {
        let end = offset(i + 1)?;
        let file = data.get(start..end).ok_or(FormatError::WrongFormat {
            format: "mini pack",
            reason: "file out of bounds",
        })?;
        files.push(file.to_vec());
        start = end;
    }
    Ok(Some(files))
}

/// Packs `files` under the 2-character `identifier`.
///
/// # Errors
/// When there are more than 65535 files or the pack would exceed 4 GiB.
pub fn pack(files: &[Vec<u8>], identifier: [u8; 2]) -> Result<Vec<u8>, FormatError> {
    let too_large = FormatError::Unencodable {
        format: "mini pack",
        reason: "too many files or too much data",
    };
    let count = u16::try_from(files.len()).map_err(|_| too_large)?;
    let data_offset = 4 + 4 + files.len() * 4;
    let mut out = Vec::with_capacity(data_offset + files.iter().map(|f| f.len() + 3).sum::<usize>());
    out.extend_from_slice(&identifier);
    out.extend_from_slice(&count.to_le_bytes());
    let mut data = Vec::new();
    for file in files {
        let at = u32::try_from(data_offset + data.len()).map_err(|_| FormatError::Unencodable {
            format: "mini pack",
            reason: "too much data",
        })?;
        out.extend_from_slice(&at.to_le_bytes());
        data.extend_from_slice(file);
        data.resize(data.len().next_multiple_of(4), 0);
    }
    let end = u32::try_from(data_offset + data.len()).map_err(|_| FormatError::Unencodable {
        format: "mini pack",
        reason: "too much data",
    })?;
    out.extend_from_slice(&end.to_le_bytes());
    out.extend_from_slice(&data);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let files = vec![b"abc".to_vec(), Vec::new(), b"defgh".to_vec()];
        let packed = pack(&files, *b"WD").expect("fits");
        assert_eq!(&packed[..4], b"WD\x03\x00");
        assert_eq!(u32::from_le_bytes([packed[4], packed[5], packed[6], packed[7]]), 20);
        let unpacked = unpack(&packed, *b"WD").expect("valid").expect("a pack");
        // Files come back with their padding, as the offsets include it.
        assert_eq!(unpacked, [b"abc\0".to_vec(), Vec::new(), b"defgh\0\0\0".to_vec()]);
    }

    #[test]
    fn other_identifiers_are_not_a_pack() {
        let packed = pack(&[b"abcd".to_vec()], *b"WD").expect("fits");
        assert_eq!(unpack(&packed, *b"BL").expect("valid"), None);
        assert_eq!(unpack(b"W", *b"WD").expect("valid"), None);
    }

    #[test]
    fn rejects_offsets_outside_the_pack() {
        assert!(unpack(b"WD\x01\x00\x0c\x00\x00\x00\xff\x00\x00\x00", *b"WD").is_err());
    }
}
