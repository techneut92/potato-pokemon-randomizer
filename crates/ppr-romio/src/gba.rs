//! In-memory GBA ROM with bounds-checked, little-endian access.

use crate::RomError;

/// GBA cartridge ROM is mapped at this address.
const ROM_BASE: u32 = 0x0800_0000;

/// Result of a search that must be unambiguous.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Found {
    /// No match.
    None,
    /// Exactly one match, at this offset.
    Unique(usize),
    /// More than one match.
    Multiple,
}

/// The whole ROM image.
#[derive(Debug, Clone)]
pub(crate) struct GbaRom(Vec<u8>);

fn out_of_range(offset: usize, len: usize) -> RomError {
    RomError::Malformed(format!("read of {len} bytes at {offset:#x} is outside the ROM"))
}

impl GbaRom {
    pub(crate) fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.0
    }

    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }

    pub(crate) fn slice(&self, offset: usize, len: usize) -> Result<&[u8], RomError> {
        offset
            .checked_add(len)
            .and_then(|end| self.0.get(offset..end))
            .ok_or_else(|| out_of_range(offset, len))
    }

    pub(crate) fn u16(&self, offset: usize) -> Result<u16, RomError> {
        let b = self.slice(offset, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn u32(&self, offset: usize) -> Result<u32, RomError> {
        let b = self.slice(offset, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// Reads a pointer and converts it to a file offset. The target may be at most the ROM length (one past the end).
    pub(crate) fn pointer(&self, offset: usize) -> Result<usize, RomError> {
        let raw = self.u32(offset)?;
        raw.checked_sub(ROM_BASE)
            .and_then(|p| usize::try_from(p).ok())
            .filter(|&p| p <= self.0.len())
            .ok_or_else(|| RomError::Malformed(format!("no valid pointer at {offset:#x} ({raw:#010x})")))
    }

    pub(crate) fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), RomError> {
        let end = offset
            .checked_add(data.len())
            .filter(|&e| e <= self.0.len())
            .ok_or_else(|| out_of_range(offset, data.len()))?;
        self.0[offset..end].copy_from_slice(data);
        Ok(())
    }

    pub(crate) fn write_u16(&mut self, offset: usize, value: u16) -> Result<(), RomError> {
        self.write(offset, &value.to_le_bytes())
    }

    /// Writes a pointer to the file offset `target`.
    pub(crate) fn write_pointer(&mut self, offset: usize, target: usize) -> Result<(), RomError> {
        let raw = u32::try_from(target)
            .ok()
            .and_then(|t| t.checked_add(ROM_BASE))
            .ok_or_else(|| RomError::Malformed(format!("{target:#x} cannot be pointed to")))?;
        self.write(offset, &raw.to_le_bytes())
    }

    /// Offsets of the non-overlapping matches of `needle` that lie within
    /// `begin..end`.
    pub(crate) fn search(&self, begin: usize, end: usize, needle: &[u8]) -> Vec<usize> {
        let end = end.min(self.0.len());
        let mut found = Vec::new();
        if needle.is_empty() {
            return found;
        }
        let mut at = begin;
        while at + needle.len() <= end {
            if self.0[at..].starts_with(needle) {
                found.push(at);
                at += needle.len();
            } else {
                at += 1;
            }
        }
        found
    }

    /// Searches the whole ROM for `needle`, counting overlapping matches.
    pub(crate) fn find(&self, needle: &[u8]) -> Found {
        let Some(&first) = needle.first() else {
            return Found::None;
        };
        let mut found = Found::None;
        let mut start = 0;
        while let Some(pos) = self.0[start..].iter().position(|&b| b == first) {
            let at = start + pos;
            if self.0[at..].starts_with(needle) {
                if found != Found::None {
                    return Found::Multiple;
                }
                found = Found::Unique(at);
            }
            start = at + 1;
        }
        found
    }
}

/// Parses a hex string such as `"CD214900"` into bytes.
pub(crate) fn hex(s: &str) -> Result<Vec<u8>, RomError> {
    if !s.len().is_multiple_of(2) {
        return Err(RomError::Malformed(format!("odd-length hex string {s}")));
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| RomError::Malformed(format!("invalid hex string {s}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn little_endian_access_and_bounds() {
        let mut rom = GbaRom::new(vec![0x34, 0x12, 0x00, 0x08, 0xFF]);
        assert_eq!(rom.u16(0).expect("in range"), 0x1234);
        assert_eq!(rom.u32(0).expect("in range"), 0x0800_1234);
        assert!(rom.u32(2).is_err());
        rom.write_u16(3, 0xBEEF).expect("in range");
        assert_eq!(rom.bytes()[3..], [0xEF, 0xBE]);
        assert!(rom.write_u16(4, 1).is_err());
    }

    #[test]
    fn pointers_must_point_into_the_rom() {
        let rom = GbaRom::new([0x04, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x09, 0, 0, 0, 0].to_vec());
        assert_eq!(rom.pointer(0).expect("valid"), 4);
        assert!(rom.pointer(4).is_err(), "beyond the end");
        assert!(rom.pointer(8).is_err(), "below the ROM base");
    }

    #[test]
    fn pointers_round_trip_and_search_skips_overlaps() {
        let mut rom = GbaRom::new(vec![0; 16]);
        rom.write_pointer(4, 12).expect("in range");
        assert_eq!(rom.bytes()[4..8], [12, 0, 0, 8]);
        assert_eq!(rom.pointer(4).expect("valid"), 12);
        let rom = GbaRom::new(vec![1, 1, 1, 1, 1, 2, 1, 1]);
        assert_eq!(rom.search(0, 8, &[1, 1]), [0, 2, 6]);
        assert_eq!(rom.search(1, 7, &[1, 1]), [1, 3]);
    }

    #[test]
    fn find_requires_a_unique_match() {
        let rom = GbaRom::new(vec![1, 2, 3, 1, 2, 4, 2, 2, 2]);
        assert_eq!(rom.find(&[1, 2, 3]), Found::Unique(0));
        assert_eq!(rom.find(&[1, 2]), Found::Multiple);
        assert_eq!(rom.find(&[2, 2]), Found::Multiple, "overlapping matches count");
        assert_eq!(rom.find(&[9]), Found::None);
        assert_eq!(hex("CD2149").expect("valid"), [0xCD, 0x21, 0x49]);
        assert!(hex("ABC").is_err());
    }
}
