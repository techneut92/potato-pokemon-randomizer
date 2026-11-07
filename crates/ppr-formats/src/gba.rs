//! Game Boy Advance cartridge header.

use crate::io::{read_at, source_len};
use crate::{FormatError, RomSource};

/// ROM sizes the Gen 3 games ship with: 8, 16 or 32 MiB.
pub const SIZES: [u64; 3] = [0x80_0000, 0x100_0000, 0x200_0000];

const START: u64 = 0xA0;
const GAME_CODE: usize = 0xAC - 0xA0;
const VERSION: usize = 0xBC - 0xA0;
const COMPLEMENT: usize = 0xBD - 0xA0;

/// The fields of the cartridge header that identify a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GbaHeader {
    /// 12-byte internal title.
    pub title: [u8; 12],
    /// 4-character game code, e.g. `BPEE` for Emerald (US).
    pub game_code: [u8; 4],
    /// Software version.
    pub version: u8,
    /// True when the complement check at 0xBD matches, which the BIOS requires of every real cartridge.
    pub checksum_ok: bool,
}

impl GbaHeader {
    /// Reads the header. Fails unless the size is one of [`SIZES`].
    pub fn read(src: &mut (impl RomSource + ?Sized)) -> Result<Self, FormatError> {
        let size = source_len(src)?;
        if !SIZES.contains(&size) {
            return Err(FormatError::WrongFormat {
                format: "Game Boy Advance",
                reason: "unexpected ROM size",
            });
        }
        let h = read_at::<0x1E>(src, START)?;
        let complement = h[..COMPLEMENT]
            .iter()
            .fold(0u8, |c, &b| c.wrapping_sub(b))
            .wrapping_sub(0x19);
        let mut title = [0u8; 12];
        title.copy_from_slice(&h[..12]);
        let mut game_code = [0u8; 4];
        game_code.copy_from_slice(&h[GAME_CODE..GAME_CODE + 4]);
        Ok(Self {
            title,
            game_code,
            version: h[VERSION],
            checksum_ok: complement == h[COMPLEMENT],
        })
    }

    /// True when the internal title starts with `name`.
    pub fn title_starts_with(&self, name: &str) -> bool {
        self.title.starts_with(name.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn reads_code_version_and_checksum() {
        let mut rom = vec![0u8; 0x80_0000];
        rom[0xA0..0xAC].copy_from_slice(b"POKEMON EMER");
        rom[0xAC..0xB0].copy_from_slice(b"BPEE");
        rom[0xBD] = rom[0xA0..0xBD]
            .iter()
            .fold(0u8, |c, &b| c.wrapping_sub(b))
            .wrapping_sub(0x19);
        let h = GbaHeader::read(&mut Cursor::new(rom)).expect("valid header");
        assert_eq!(&h.game_code, b"BPEE");
        assert!(h.title_starts_with("POKEMON EMER"));
        assert!(h.checksum_ok);
    }
}
