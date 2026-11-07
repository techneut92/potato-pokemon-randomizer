//! Game Boy and Game Boy Color cartridge header.

use crate::io::{read_at, source_len};
use crate::{FormatError, RomSource};

/// Smallest and largest ROM sizes the Gen 1 and Gen 2 games ship with.
pub const SIZE_RANGE: std::ops::RangeInclusive<u64> = 0x8_0000..=0x20_0000;

const START: u64 = 0x134;
const TITLE: usize = 0;
const CGB_FLAG: usize = 0x143 - 0x134;
const GAME_CODE: usize = 0x13F - 0x134;
const DESTINATION: usize = 0x14A - 0x134;
const VERSION: usize = 0x14C - 0x134;
const HEADER_CHECKSUM: usize = 0x14D - 0x134;
const GLOBAL_CHECKSUM: usize = 0x14E - 0x134;

/// The fields of the cartridge header that identify a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbHeader {
    /// Raw title area (0x134..0x144). Older games use all 16 bytes for the title; newer ones end it with a
    /// 4-character game code at 0x13F and the CGB flag at 0x143.
    pub title_area: [u8; 16],
    /// Destination code: 0 for Japan, 1 elsewhere.
    pub destination: u8,
    /// Mask ROM version number.
    pub version: u8,
    /// Big-endian global checksum stored in the header.
    pub global_checksum: u16,
    /// ROM size in bytes.
    pub size: u64,
    /// True when the header checksum at 0x14D matches, which the boot ROM requires of every real cartridge.
    pub checksum_ok: bool,
}

impl GbHeader {
    /// Reads the header. Fails if the size is outside [`SIZE_RANGE`].
    pub fn read(src: &mut (impl RomSource + ?Sized)) -> Result<Self, FormatError> {
        let size = source_len(src)?;
        if !SIZE_RANGE.contains(&size) {
            return Err(FormatError::WrongFormat {
                format: "Game Boy",
                reason: "unexpected ROM size",
            });
        }
        let h = read_at::<0x1C>(src, START)?;
        let checksum = h[..HEADER_CHECKSUM]
            .iter()
            .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));
        let mut title_area = [0u8; 16];
        title_area.copy_from_slice(&h[TITLE..TITLE + 16]);
        Ok(Self {
            title_area,
            destination: h[DESTINATION],
            version: h[VERSION],
            global_checksum: u16::from_be_bytes([h[GLOBAL_CHECKSUM], h[GLOBAL_CHECKSUM + 1]]),
            size,
            checksum_ok: checksum == h[HEADER_CHECKSUM],
        })
    }

    /// True when the title area starts with `sig`.
    pub fn title_starts_with(&self, sig: &str) -> bool {
        self.title_area.starts_with(sig.as_bytes())
    }

    /// The 4-character game code at 0x13F (Game Boy Color games).
    pub fn game_code(&self) -> &[u8] {
        &self.title_area[GAME_CODE..GAME_CODE + 4]
    }

    /// True when the cartridge declares Game Boy Color support.
    pub fn is_cgb(&self) -> bool {
        self.title_area[CGB_FLAG] & 0x80 != 0
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn rom(title: &[u8], destination: u8, version: u8) -> Vec<u8> {
        let mut rom = vec![0u8; 0x10_0000];
        rom[0x134..0x134 + title.len()].copy_from_slice(title);
        rom[0x14A] = destination;
        rom[0x14C] = version;
        rom[0x14E..0x150].copy_from_slice(&0xF57E_u16.to_be_bytes());
        rom[0x14D] = rom[0x134..0x14D]
            .iter()
            .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));
        rom
    }

    #[test]
    fn reads_identity_fields() {
        let h = GbHeader::read(&mut Cursor::new(rom(b"POKEMON RED", 1, 0))).expect("valid header");
        assert!(h.title_starts_with("POKEMON RED"));
        assert!(!h.title_starts_with("POKEMON BLUE"));
        assert_eq!((h.destination, h.version, h.global_checksum), (1, 0, 0xF57E));
        assert!(h.checksum_ok);
    }

    #[test]
    fn reads_cgb_game_code() {
        let title = *b"PM_CRYSTAL\0BYTE\x80";
        let h = GbHeader::read(&mut Cursor::new(rom(&title, 1, 1))).expect("valid header");
        assert_eq!(h.game_code(), b"BYTE");
        assert!(h.is_cgb());
    }

    #[test]
    fn detects_bad_checksum() {
        let mut r = rom(b"POKEMON RED", 1, 0);
        r[0x14D] ^= 0xFF;
        assert!(!GbHeader::read(&mut Cursor::new(r)).expect("readable").checksum_ok);
    }

    #[test]
    fn rejects_wrong_size() {
        let err = GbHeader::read(&mut Cursor::new(vec![0u8; 0x1000])).expect_err("too small");
        assert!(matches!(err, FormatError::WrongFormat { .. }));
    }
}
