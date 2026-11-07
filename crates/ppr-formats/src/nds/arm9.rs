//! The ARM9 binary: where its code ends, its footer, and whether it is BLZ-compressed.

use crate::FormatError;
use crate::blz;
use crate::io::{slice_at, u32_at};

/// The word that starts the 12-byte footer after the ARM9 code.
const NITRO_CODE: u32 = 0xDEC0_0621;

fn word(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

/// How the ARM9 binary is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Arm9Layout {
    /// The code: its offset in the ROM and its length, without footer blocks.
    pub(super) offset: usize,
    pub(super) len: usize,
    /// Written after the code; trailing all-zero or footer blocks of the code are moved here.
    pub(super) footer: Option<Vec<u8>>,
    /// Where the code holds its own end address (RAM address + compressed length), when it is compressed.
    pub(super) size_offset: Option<usize>,
}

impl Arm9Layout {
    /// Reads the layout from the header fields: the ARM9 at `offset`, `size` bytes long, loaded at `ram_address`.
    pub(super) fn read(rom: &[u8], offset: usize, size: usize, ram_address: u32) -> Result<Self, FormatError> {
        let code = slice_at(rom, offset, size)?;
        let mut footer = match slice_at(rom, offset + size, 12) {
            Ok(f) if word(f, 0) == NITRO_CODE => Some(f.to_vec()),
            _ => None,
        };
        let mut len = size;
        while len >= 12 {
            let tail = &code[len - 12..len];
            if word(tail, 0) != NITRO_CODE && tail.iter().any(|&b| b != 0) {
                break;
            }
            let mut moved = tail.to_vec();
            moved.extend(footer.take().unwrap_or_default());
            footer = Some(moved);
            len -= 12;
        }
        let code = &code[..len];

        let mut size_offset = None;
        if len >= 8 && (0x08..=0x0B).contains(&code[len - 5]) {
            let packed =
                usize::from(code[len - 8]) | usize::from(code[len - 7]) << 8 | usize::from(code[len - 6]) << 16;
            if packed > len * 9 / 10 && packed < len * 11 / 10 {
                // The code stores its end address somewhere in the uncompressed head; it must be found exactly once.
                let end = u32::try_from(len)
                    .unwrap_or_default()
                    .wrapping_add(ram_address)
                    .to_le_bytes();
                let mut found = code.windows(4).enumerate().filter(|(_, w)| *w == end).map(|(i, _)| i);
                match (found.next(), found.next()) {
                    (Some(at), None) => size_offset = Some(at),
                    _ => {
                        return Err(FormatError::WrongFormat {
                            format: "NDS",
                            reason: "the compressed ARM9 does not store its end address exactly once",
                        });
                    }
                }
            }
        }
        Ok(Self {
            offset,
            len,
            footer,
            size_offset,
        })
    }

    /// Whether the code is BLZ-compressed.
    pub(super) fn compressed(&self) -> bool {
        self.size_offset.is_some()
    }

    /// The code, decompressed.
    pub(super) fn decode(&self, rom: &[u8]) -> Result<Vec<u8>, FormatError> {
        let code = slice_at(rom, self.offset, self.len)?;
        if self.compressed() {
            blz::decompress(code)
        } else {
            Ok(code.to_vec())
        }
    }

    /// `code` stored the way the original was: compressed (with its end address updated) if that was, followed by
    /// the footer.
    pub(super) fn encode(&self, code: &[u8], ram_address: u32) -> (Vec<u8>, usize) {
        let mut out = match self.size_offset {
            Some(at) => {
                let mut packed = blz::compress(code, blz::ARM9_HEAD, blz::Mode::Normal);
                let end = u32::try_from(packed.len())
                    .unwrap_or_default()
                    .wrapping_add(ram_address)
                    .to_le_bytes();
                // The classic layout code skips an end address found at offset 0.
                if at > 0 && at + 4 <= packed.len() {
                    packed[at..at + 4].copy_from_slice(&end);
                }
                packed
            }
            None => code.to_vec(),
        };
        let len = out.len();
        if let Some(footer) = &self.footer {
            out.extend_from_slice(footer);
        }
        (out, len)
    }
}

/// The ARM9's RAM address, from the header.
pub(super) fn ram_address(rom: &[u8]) -> Result<u32, FormatError> {
    u32_at(rom, 0x28)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_footer_and_moves_zero_blocks_into_it() {
        let mut rom = vec![0x11u8; 0x40];
        // Code at 0x10, 0x20 bytes, its last 12 bytes zero; the footer follows.
        rom[0x24..0x30].fill(0);
        rom[0x30..0x34].copy_from_slice(&NITRO_CODE.to_le_bytes());
        rom[0x34..0x3C].fill(0x22);
        let layout = Arm9Layout::read(&rom, 0x10, 0x20, 0x0200_0000).expect("valid");
        assert_eq!(layout.len, 0x14);
        let footer = layout.footer.clone().expect("has a footer");
        assert_eq!(footer.len(), 24);
        assert_eq!(footer[..12], [0; 12]);
        assert!(!layout.compressed());
        let (stored, len) = layout.encode(&layout.decode(&rom).expect("valid"), 0);
        assert_eq!(len, 0x14);
        assert_eq!(stored, rom[0x10..0x3C]);
    }

    #[test]
    fn a_compressed_arm9_round_trips_with_its_end_address() {
        let ram = 0x0200_0000_u32;
        // An uncompressed head with a placeholder for the end address, then code that compresses a little, so the
        // compressed part is most of the binary, as in real ROMs.
        let mut code: Vec<u8> = (0..blz::ARM9_HEAD)
            .map(|i| u8::try_from(i % 251).unwrap_or(0))
            .collect();
        let mut x = 7_u32;
        code.extend((0..0x40000).map(|_| {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            u8::try_from((x >> 16) % 9).unwrap_or(0) * 0x11
        }));
        let mut packed = blz::compress(&code, blz::ARM9_HEAD, blz::Mode::Normal);
        let end = (u32::try_from(packed.len()).expect("small") + ram).to_le_bytes();
        packed[0x800..0x804].copy_from_slice(&end);
        code[0x800..0x804].copy_from_slice(&end);
        let mut rom = vec![0u8; 0x100];
        rom.extend(&packed);
        rom.extend([0xFF; 12]);
        let layout = Arm9Layout::read(&rom, 0x100, packed.len(), ram).expect("valid");
        assert!(layout.compressed());
        assert_eq!(layout.size_offset, Some(0x800));
        assert_eq!(layout.footer, None);
        assert_eq!(layout.decode(&rom).expect("valid"), code);
        let (stored, len) = layout.encode(&code, ram);
        assert_eq!((stored.len(), len), (packed.len(), packed.len()));
        assert_eq!(stored, packed);
    }
}
