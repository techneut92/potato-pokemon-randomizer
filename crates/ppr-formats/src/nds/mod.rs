//! Nintendo DS cartridge images: the header that identifies a game, and the whole ROM with its file system,
//! ARM9 binary and overlays ([`NdsRom`]).

mod arm9;
mod fs;
mod rom;

pub use rom::{NdsRom, Overlay};

use crate::io::read_at;
use crate::{FormatError, RomSource, crc16};

const HEADER_LEN: usize = 0x160;
const GAME_CODE: usize = 0x0C;
const VERSION: usize = 0x1E;
const HEADER_CRC: usize = 0x15E;

/// The fields of the cartridge header that identify a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NdsHeader {
    /// 4-character game code, e.g. `CPUE` for Platinum (US).
    pub game_code: [u8; 4],
    /// ROM version.
    pub version: u8,
    /// True when the header CRC-16 at 0x15E matches.
    pub checksum_ok: bool,
}

impl NdsHeader {
    /// Reads the header.
    pub fn read(src: &mut (impl RomSource + ?Sized)) -> Result<Self, FormatError> {
        let h = read_at::<HEADER_LEN>(src, 0)?;
        let mut game_code = [0u8; 4];
        game_code.copy_from_slice(&h[GAME_CODE..GAME_CODE + 4]);
        let stored = u16::from_le_bytes([h[HEADER_CRC], h[HEADER_CRC + 1]]);
        Ok(Self {
            game_code,
            version: h[VERSION],
            checksum_ok: crc16(&h[..HEADER_CRC]) == stored,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reads_code_version_and_checksum() {
        let mut rom = vec![0u8; 0x200];
        rom[0x0C..0x10].copy_from_slice(b"CPUE");
        rom[0x1E] = 1;
        let crc = crc16(&rom[..0x15E]);
        rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        let h = NdsHeader::read(&mut Cursor::new(rom)).expect("valid header");
        assert_eq!((&h.game_code, h.version, h.checksum_ok), (b"CPUE", 1, true));
    }
}
