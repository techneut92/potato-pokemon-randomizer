//! A whole DS ROM: its `NitroFS` files, ARM9 binary and ARM9 overlays, readable and replaceable, and saved again.
//!
//! Saving a ROM with no replacements gives back the input byte for byte. Otherwise the ROM is rebuilt in the
//! classic layout:
//!
//! 1. the header area (its size from 0x84), unchanged apart from the offsets below and the header CRC;
//! 2. at the next 0x200 boundary the ARM9 binary with its footer, then straight after it the ARM9 overlay table;
//! 3. at the next boundary the ARM7 binary, straight after it the ARM7 overlay table;
//! 4. at the next boundaries the banner, the file name table (unchanged) and the file allocation table;
//! 5. then every file in file ID order, each at the next 0x200 boundary;
//! 6. the end padded to a multiple of 4. The header gets the new offsets, the used size (0x80), the device
//!    capacity (0x14) and its CRC-16 (0x15E).
//!
//! Gaps are zero-filled. Anything after the last file (on `DSi`-enhanced cartridges, the `DSi` area) is not carried
//! over, so a rebuilt ROM runs in DS mode.
//!
//! Deliberate deviations from the classic layout code:
//!
//! - only replaced overlays are recompressed (reading one does not cause it to be rewritten);
//! - the banner keeps its full size as given by its version (0x840, 0x940, 0xA40 or 0x23C0 bytes) instead of
//!   always 0x840 bytes, so the icon data of later banner versions survives.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashMap};

use super::arm9::{self, Arm9Layout};
use super::fs;
use crate::io::{slice_at, u16_at, usize_at};
use crate::narc::Narc;
use crate::{FormatError, blz, crc16};

const ALIGN: usize = 0x200;
const OVERLAY_ENTRY: usize = 32;

fn align(at: usize) -> usize {
    at.next_multiple_of(ALIGN)
}

/// An entry of the ARM9 overlay table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Overlay {
    /// Where the overlay is loaded in RAM.
    pub ram_address: u32,
    /// How many bytes it takes in RAM (decompressed).
    pub ram_size: u32,
    /// The size of its zero-initialized data after it.
    pub bss_size: u32,
    /// The start of its static initializer table.
    pub static_start: u32,
    /// The end of its static initializer table.
    pub static_end: u32,
    /// The file that holds it.
    pub file_id: u32,
    /// Its compressed size (24 bits).
    pub compressed_size: u32,
    /// Flags: bit 0 is set when it is compressed.
    pub flags: u8,
}

impl Overlay {
    fn read(entry: &[u8]) -> Self {
        let word = |i: usize| u32::from_le_bytes([entry[i], entry[i + 1], entry[i + 2], entry[i + 3]]);
        Self {
            ram_address: word(4),
            ram_size: word(8),
            bss_size: word(12),
            static_start: word(16),
            static_end: word(20),
            file_id: word(24),
            compressed_size: word(28) & 0x00FF_FFFF,
            flags: entry[31],
        }
    }

    fn write(&self, id: usize, compressed_size: u32) -> [u8; OVERLAY_ENTRY] {
        let mut entry = [0u8; OVERLAY_ENTRY];
        let words = [
            u32::try_from(id).unwrap_or_default(),
            self.ram_address,
            self.ram_size,
            self.bss_size,
            self.static_start,
            self.static_end,
            self.file_id,
            compressed_size & 0x00FF_FFFF | u32::from(self.flags) << 24,
        ];
        for (chunk, word) in entry.as_chunks_mut::<4>().0.iter_mut().zip(words) {
            chunk.copy_from_slice(&word.to_le_bytes());
        }
        entry
    }

    /// Whether the stored file is BLZ-compressed: flagged, and as long as the table says the compressed data is.
    fn is_compressed(&self, stored_len: usize) -> bool {
        self.flags != 0 && self.compressed_size != 0 && stored_len == self.compressed_size as usize
    }
}

/// A DS ROM, opened from its bytes.
#[derive(Debug, Clone)]
pub struct NdsRom {
    rom: Vec<u8>,
    /// Start and end of each file, by file ID.
    fat: Vec<(usize, usize)>,
    /// File IDs of named files by path, and paths by file ID.
    ids: HashMap<String, usize>,
    paths: BTreeMap<usize, String>,
    overlays: Vec<Overlay>,
    replaced_files: HashMap<usize, Vec<u8>>,
    /// Replaced overlays, as given (decompressed).
    replaced_overlays: BTreeMap<usize, Vec<u8>>,
    replaced_arm9: Option<(Arm9Layout, Vec<u8>)>,
}

/// `/a/0/0/2` and `a/0/0/2` name the same file.
fn normalize(path: &str) -> &str {
    path.strip_prefix('/').unwrap_or(path)
}

impl NdsRom {
    /// Opens a ROM: reads its file system and overlay table. The ARM9 binary is read when asked for.
    ///
    /// # Errors
    /// When a table points outside the ROM or is malformed.
    pub fn from_bytes(rom: Vec<u8>) -> Result<Self, FormatError> {
        let fnt = usize_at(&rom, 0x40)?;
        let fat_offset = usize_at(&rom, 0x48)?;
        let fat_size = usize_at(&rom, 0x4C)?;
        let fat = slice_at(&rom, fat_offset, fat_size)?
            .as_chunks::<8>()
            .0
            .iter()
            .map(|e| {
                let start = u32::from_le_bytes([e[0], e[1], e[2], e[3]]) as usize;
                let end = u32::from_le_bytes([e[4], e[5], e[6], e[7]]) as usize;
                (start, end)
            })
            .collect::<Vec<_>>();
        let check = |id: usize| -> Result<(), FormatError> {
            let &(start, end) = fat.get(id).ok_or_else(|| FormatError::NotFound {
                what: format!("file ID {id}"),
            })?;
            if end < start || end > rom.len() {
                return Err(FormatError::Truncated { offset: start as u64 });
            }
            Ok(())
        };

        let paths = fs::read_names(&rom, fnt)?;
        let mut ids = HashMap::with_capacity(paths.len());
        for (&id, path) in &paths {
            check(id)?;
            ids.insert(path.clone(), id);
        }

        let table = slice_at(&rom, usize_at(&rom, 0x50)?, usize_at(&rom, 0x54)?)?;
        let overlays = table
            .as_chunks::<OVERLAY_ENTRY>()
            .0
            .iter()
            .map(|e| Overlay::read(e))
            .collect::<Vec<_>>();
        for overlay in &overlays {
            check(overlay.file_id as usize)?;
        }

        Ok(Self {
            rom,
            fat,
            ids,
            paths,
            overlays,
            replaced_files: HashMap::new(),
            replaced_overlays: BTreeMap::new(),
            replaced_arm9: None,
        })
    }

    /// The 4-character game code, e.g. `CPUE`.
    pub fn code(&self) -> String {
        crate::narc::ascii(&self.rom[0x0C..0x10])
    }

    /// The ROM version.
    pub fn version(&self) -> u8 {
        self.rom[0x1E]
    }

    /// The paths of all named files, in file ID order.
    pub fn paths(&self) -> impl Iterator<Item = &str> {
        self.paths.values().map(String::as_str)
    }

    /// The file ID of the file at `path`.
    pub fn file_id(&self, path: &str) -> Option<usize> {
        self.ids.get(normalize(path)).copied()
    }

    fn original(&self, id: usize) -> &[u8] {
        // Checked when opening.
        let (start, end) = self.fat[id];
        &self.rom[start..end]
    }

    /// The contents of the file at `path` (with or without a leading `/`), replaced or not.
    pub fn file(&self, path: &str) -> Option<&[u8]> {
        let id = self.file_id(path)?;
        Some(
            self.replaced_files
                .get(&id)
                .map_or_else(|| self.original(id), Vec::as_slice),
        )
    }

    /// Replaces the file at `path`.
    ///
    /// # Errors
    /// When there is no file at `path`.
    pub fn replace_file(&mut self, path: &str, data: Vec<u8>) -> Result<(), FormatError> {
        let id = self.file_id(path).ok_or_else(|| FormatError::NotFound {
            what: format!("file {path}"),
        })?;
        self.replaced_files.insert(id, data);
        Ok(())
    }

    /// The NARC archive at `path`.
    ///
    /// # Errors
    /// When there is no file at `path` or it is not a NARC.
    pub fn narc(&self, path: &str) -> Result<Narc, FormatError> {
        let data = self.file(path).ok_or_else(|| FormatError::NotFound {
            what: format!("file {path}"),
        })?;
        Narc::parse(data)
    }

    /// Replaces the file at `path` with `narc`.
    ///
    /// # Errors
    /// When there is no file at `path`.
    pub fn replace_narc(&mut self, path: &str, narc: &Narc) -> Result<(), FormatError> {
        self.replace_file(path, narc.to_bytes())
    }

    /// The ARM9 overlay table.
    pub fn overlays(&self) -> &[Overlay] {
        &self.overlays
    }

    /// Overlay `number`, decompressed.
    ///
    /// # Errors
    /// When there is no such overlay or it does not decompress.
    pub fn overlay(&self, number: usize) -> Result<Vec<u8>, FormatError> {
        if let Some(data) = self.replaced_overlays.get(&number) {
            return Ok(data.clone());
        }
        let entry = self.overlays.get(number).ok_or_else(|| missing_overlay(number))?;
        let stored = self.original(entry.file_id as usize);
        if entry.is_compressed(stored.len()) {
            blz::decompress(stored)
        } else {
            Ok(stored.to_vec())
        }
    }

    /// Replaces overlay `number` with `data` (decompressed). It is compressed again on saving if it was compressed.
    ///
    /// # Errors
    /// When there is no such overlay.
    pub fn replace_overlay(&mut self, number: usize, data: Vec<u8>) -> Result<(), FormatError> {
        if number >= self.overlays.len() {
            return Err(missing_overlay(number));
        }
        self.replaced_overlays.insert(number, data);
        Ok(())
    }

    fn arm9_layout(&self) -> Result<Arm9Layout, FormatError> {
        Arm9Layout::read(
            &self.rom,
            usize_at(&self.rom, 0x20)?,
            usize_at(&self.rom, 0x2C)?,
            arm9::ram_address(&self.rom)?,
        )
    }

    /// The ARM9 binary, decompressed and without its footer.
    ///
    /// # Errors
    /// When it lies outside the ROM or does not decompress.
    pub fn arm9(&self) -> Result<Vec<u8>, FormatError> {
        match &self.replaced_arm9 {
            Some((_, data)) => Ok(data.clone()),
            None => self.arm9_layout()?.decode(&self.rom),
        }
    }

    /// Replaces the ARM9 binary with `data` (decompressed, without footer). It is stored like the original:
    /// compressed if that was, with the same footer.
    ///
    /// # Errors
    /// When the original ARM9 cannot be read, so it is unclear how to store the new one.
    pub fn replace_arm9(&mut self, data: Vec<u8>) -> Result<(), FormatError> {
        let layout = match self.replaced_arm9.take() {
            Some((layout, _)) => layout,
            None => self.arm9_layout()?,
        };
        self.replaced_arm9 = Some((layout, data));
        Ok(())
    }

    /// Whether anything was replaced.
    pub fn is_modified(&self) -> bool {
        !self.replaced_files.is_empty() || !self.replaced_overlays.is_empty() || self.replaced_arm9.is_some()
    }

    /// The ROM as bytes: the input itself when nothing was replaced, otherwise rebuilt (see the module docs).
    ///
    /// # Errors
    /// When a region the header points to lies outside the input.
    pub fn save(&self) -> Result<Cow<'_, [u8]>, FormatError> {
        if self.is_modified() {
            self.rebuild().map(Cow::Owned)
        } else {
            Ok(Cow::Borrowed(&self.rom))
        }
    }

    fn rebuild(&self) -> Result<Vec<u8>, FormatError> {
        let rom = &self.rom;
        let header_size = usize_at(rom, 0x84)?.max(0x160);
        let mut out = slice_at(rom, 0, header_size)?.to_vec();

        // ARM9 and its footer.
        let arm9_offset = align(out.len());
        let arm9_size = if let Some((layout, code)) = &self.replaced_arm9 {
            let (stored, len) = layout.encode(code, arm9::ram_address(rom)?);
            put(&mut out, arm9_offset, &stored);
            len
        } else {
            let size = usize_at(rom, 0x2C)?;
            let old = usize_at(rom, 0x20)?;
            // The footer is copied whether there is one or not, as the classic layout does.
            let end = (old + size + 12).min(rom.len());
            put(&mut out, arm9_offset, slice_at(rom, old, end.saturating_sub(old))?);
            size
        };

        // The ARM9 overlay table, filled in below.
        let arm9_ovl_offset = out.len();
        let arm9_ovl_size = self.overlays.len() * OVERLAY_ENTRY;
        out.resize(arm9_ovl_offset + arm9_ovl_size, 0);

        let arm7_offset = align(out.len());
        let arm7_size = usize_at(rom, 0x3C)?;
        put(&mut out, arm7_offset, slice_at(rom, usize_at(rom, 0x30)?, arm7_size)?);

        let arm7_ovl_offset = out.len();
        put(
            &mut out,
            arm7_ovl_offset,
            slice_at(rom, usize_at(rom, 0x58)?, usize_at(rom, 0x5C)?)?,
        );

        let banner_offset = align(out.len());
        let old_banner = usize_at(rom, 0x68)?;
        let banner_size = match u16_at(rom, old_banner)? {
            0x0002 => 0x940,
            0x0003 => 0xA40,
            0x0103 => 0x23C0,
            _ => 0x840,
        };
        put(&mut out, banner_offset, slice_at(rom, old_banner, banner_size)?);

        let fnt_offset = align(out.len());
        put(
            &mut out,
            fnt_offset,
            slice_at(rom, usize_at(rom, 0x40)?, usize_at(rom, 0x44)?)?,
        );

        let fat_offset = align(out.len());
        let fat_size = self.fat.len() * 8;
        out.resize(fat_offset + fat_size, 0);

        let end = self.write_files(&mut out, fat_offset, arm9_ovl_offset);

        let used = end.next_multiple_of(4);
        if out.len() < used {
            out.resize(used, 0);
        }

        let header = [
            (0x20, arm9_offset),
            (0x2C, arm9_size),
            (0x30, arm7_offset),
            (0x3C, arm7_size),
            (0x40, fnt_offset),
            (0x48, fat_offset),
            (0x50, arm9_ovl_offset),
            (0x58, arm7_ovl_offset),
            (0x68, banner_offset),
            (0x80, used),
        ];
        for (at, value) in header {
            out[at..at + 4].copy_from_slice(&to_u32(value).to_le_bytes());
        }
        out[0x14] = device_capacity(used);
        let crc = crc16(&out[..0x15E]);
        out[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        Ok(out)
    }

    /// Writes every file in file ID order after the FAT at `fat_offset`, fills in the FAT and the ARM9 overlay table
    /// at `table_offset`, and returns where the last file ends. Overlays are stored compressed again when they were.
    fn write_files(&self, out: &mut Vec<u8>, fat_offset: usize, table_offset: usize) -> usize {
        let overlays_by_file: HashMap<usize, usize> = self
            .overlays
            .iter()
            .enumerate()
            .map(|(i, o)| (o.file_id as usize, i))
            .collect();
        let fat_size = self.fat.len() * 8;
        let mut fat = Vec::with_capacity(fat_size);
        let mut table = vec![0u8; self.overlays.len() * OVERLAY_ENTRY];
        let mut end = fat_offset + fat_size;
        for id in 0..self.fat.len() {
            let offset = align(end);
            let mut contents = Cow::Borrowed(self.original(id));
            if let Some(data) = self.replaced_files.get(&id) {
                contents = Cow::Borrowed(data.as_slice());
            }
            if let Some(&number) = overlays_by_file.get(&id) {
                let entry = &self.overlays[number];
                let mut compressed_size = entry.compressed_size;
                if let Some(data) = self.replaced_overlays.get(&number) {
                    if entry.is_compressed(self.original(id).len()) {
                        let packed = blz::compress(data, 0, blz::Mode::Normal);
                        compressed_size = u32::try_from(packed.len()).unwrap_or_default();
                        contents = Cow::Owned(packed);
                    } else {
                        contents = Cow::Borrowed(data.as_slice());
                    }
                }
                let at = number * OVERLAY_ENTRY;
                table[at..at + OVERLAY_ENTRY].copy_from_slice(&entry.write(number, compressed_size));
            }
            put(out, offset, &contents);
            end = offset + contents.len();
            fat.extend_from_slice(&to_u32(offset).to_le_bytes());
            fat.extend_from_slice(&to_u32(end).to_le_bytes());
        }
        out[fat_offset..fat_offset + fat_size].copy_from_slice(&fat);
        out[table_offset..table_offset + table.len()].copy_from_slice(&table);
        end
    }
}

fn missing_overlay(number: usize) -> FormatError {
    FormatError::NotFound {
        what: format!("overlay {number}"),
    }
}

fn to_u32(value: usize) -> u32 {
    // DS ROMs are at most 4 GiB (512 MiB in practice).
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Writes `bytes` at `at`, growing `out` with zeros as needed.
fn put(out: &mut Vec<u8>, at: usize, bytes: &[u8]) {
    if out.len() < at + bytes.len() {
        out.resize(at + bytes.len(), 0);
    }
    out[at..at + bytes.len()].copy_from_slice(bytes);
}

/// The header's device capacity for `used` bytes: the cartridge size as 128 KiB shifted left by it, from the
/// power of two above the used size's highest bit.
fn device_capacity(used: usize) -> u8 {
    let mut size = to_u32(used);
    size |= size >> 16;
    size |= size >> 8;
    size |= size >> 4;
    size |= size >> 2;
    size |= size >> 1;
    let size = size.wrapping_add(1).max(128 * 1024);
    let bits = 32 - size.leading_zeros();
    u8::try_from(bits.saturating_sub(18)).unwrap_or(u8::MAX)
}

#[cfg(test)]
mod tests {
    use super::super::fs::tests::build_fnt;
    use super::*;
    use crate::io::u32_at;

    /// A small ROM: header, ARM9 with footer, two overlays (one compressed), ARM7, banner, a file system with
    /// `a/0/0/2`, `data/text.bin` and `x.narc`, laid out with odd gaps so a rebuild moves everything.
    pub(crate) fn sample_rom() -> Vec<u8> {
        let mut rom = vec![0u8; 0x4000];
        rom[0x0C..0x10].copy_from_slice(b"TEST");
        rom[0x1E] = 1;
        rom[0x28..0x2C].copy_from_slice(&0x0200_0000_u32.to_le_bytes());
        rom[0x84..0x88].copy_from_slice(&0x4000_u32.to_le_bytes());
        let set = |rom: &mut Vec<u8>, at: usize, v: usize| {
            rom[at..at + 4].copy_from_slice(&u32::try_from(v).expect("small").to_le_bytes());
        };
        let append = |rom: &mut Vec<u8>, gap: usize, bytes: &[u8]| -> usize {
            rom.resize(rom.len() + gap, 0xFF);
            let at = rom.len();
            rom.extend_from_slice(bytes);
            at
        };

        let arm9: Vec<u8> = (0..0x300u32)
            .map(|i| u8::try_from(i % 200 + 1).expect("small"))
            .collect();
        let at = append(&mut rom, 0x10, &arm9);
        set(&mut rom, 0x20, at);
        set(&mut rom, 0x2C, arm9.len());
        let mut footer = 0xDEC0_0621_u32.to_le_bytes().to_vec();
        footer.extend([1, 2, 3, 4, 5, 6, 7, 8]);
        append(&mut rom, 0, &footer);

        let arm7 = vec![0x77; 0x123];
        let at = append(&mut rom, 0x30, &arm7);
        set(&mut rom, 0x30, at);
        set(&mut rom, 0x3C, arm7.len());
        set(&mut rom, 0x58, at + arm7.len());

        let mut banner = vec![0xBB; 0x840];
        banner[0..2].copy_from_slice(&1u16.to_le_bytes());
        let at = append(&mut rom, 0x7, &banner);
        set(&mut rom, 0x68, at);

        // File IDs 0 and 1 are overlays, 2.. named files.
        let fnt = build_fnt(
            &[
                (0, vec![("a", Some(1)), ("data", Some(4)), ("x.narc", None)]),
                (0, vec![("0", Some(2))]),
                (1, vec![("0", Some(3))]),
                (2, vec![("2", None)]),
                (0, vec![("text.bin", None)]),
            ],
            2,
        );
        let at = append(&mut rom, 0x21, &fnt);
        set(&mut rom, 0x40, at);
        set(&mut rom, 0x44, fnt.len());

        let overlay0: Vec<u8> = (0..0x400u32).map(|i| u8::try_from(i % 16).expect("small")).collect();
        let packed0 = blz::compress(&overlay0, 0, blz::Mode::Normal);
        let overlay1 = vec![0x01; 0x40];
        let narc = Narc::from_files(vec![vec![1, 2, 3], vec![4; 9]]).to_bytes();
        let files: [&[u8]; 5] = [&packed0, &overlay1, &narc, b"file a/0/0/2", b"hello"];

        let mut table = Vec::new();
        for (i, (ram, flags, size)) in [(0x0210_0000_u32, 3u8, packed0.len()), (0x0220_0000, 0, 0)]
            .into_iter()
            .enumerate()
        {
            let mut entry = [0u8; 32];
            entry[0..4].copy_from_slice(&u32::try_from(i).expect("small").to_le_bytes());
            entry[4..8].copy_from_slice(&ram.to_le_bytes());
            entry[24..28].copy_from_slice(&u32::try_from(i).expect("small").to_le_bytes());
            entry[28..31].copy_from_slice(&u32::try_from(size).expect("small").to_le_bytes()[..3]);
            entry[31] = flags;
            table.extend(entry);
        }
        let at = append(&mut rom, 0x5, &table);
        set(&mut rom, 0x50, at);
        set(&mut rom, 0x54, table.len());

        // Files are stored in reverse ID order; the FAT is filled in afterwards.
        let fat_at = append(&mut rom, 0x3, &vec![0; files.len() * 8]);
        set(&mut rom, 0x48, fat_at);
        set(&mut rom, 0x4C, files.len() * 8);
        for (id, file) in files.iter().enumerate().rev() {
            let at = append(&mut rom, 0x11, file);
            set(&mut rom, fat_at + id * 8, at);
            set(&mut rom, fat_at + id * 8 + 4, at + file.len());
        }
        let len = rom.len();
        set(&mut rom, 0x80, len);
        let crc = crc16(&rom[..0x15E]);
        rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
        rom
    }

    fn open(rom: Vec<u8>) -> NdsRom {
        NdsRom::from_bytes(rom).expect("valid")
    }

    #[test]
    fn reads_files_overlays_and_arm9() {
        let rom = open(sample_rom());
        assert_eq!((rom.code().as_str(), rom.version()), ("TEST", 1));
        assert_eq!(rom.paths().collect::<Vec<_>>(), ["x.narc", "a/0/0/2", "data/text.bin"]);
        assert_eq!(rom.file("a/0/0/2"), Some(&b"file a/0/0/2"[..]));
        assert_eq!(rom.file("/data/text.bin"), Some(&b"hello"[..]));
        assert_eq!(rom.file("nope"), None);
        assert_eq!(rom.narc("x.narc").expect("narc").files, [vec![1, 2, 3], vec![4; 9]]);
        let overlay0: Vec<u8> = (0..0x400u32).map(|i| u8::try_from(i % 16).expect("small")).collect();
        assert_eq!(rom.overlay(0).expect("overlay"), overlay0);
        assert_eq!(rom.overlay(1).expect("overlay"), [1; 0x40]);
        assert!(rom.overlay(2).is_err());
        assert_eq!(rom.overlays()[0].ram_address, 0x0210_0000);
        let arm9: Vec<u8> = (0..0x300u32)
            .map(|i| u8::try_from(i % 200 + 1).expect("small"))
            .collect();
        assert_eq!(rom.arm9().expect("arm9"), arm9);
    }

    #[test]
    fn an_unchanged_rom_saves_byte_for_byte() {
        let bytes = sample_rom();
        let rom = open(bytes.clone());
        assert!(matches!(rom.save().expect("saves"), Cow::Borrowed(b) if b == bytes.as_slice()));
    }

    #[test]
    fn a_rebuilt_rom_keeps_its_contents() {
        let mut rom = open(sample_rom());
        let mut narc = rom.narc("x.narc").expect("narc");
        narc.files[1] = vec![5; 100];
        rom.replace_narc("/x.narc", &narc).expect("exists");
        let mut overlay0 = rom.overlay(0).expect("overlay");
        overlay0[0x10] = 0xEE;
        rom.replace_overlay(0, overlay0.clone()).expect("exists");
        rom.replace_overlay(1, vec![2; 0x41]).expect("exists");
        let mut arm9 = rom.arm9().expect("arm9");
        arm9.extend([9; 4]);
        rom.replace_arm9(arm9.clone()).expect("readable");
        assert!(rom.replace_file("missing", vec![]).is_err());
        assert!(rom.replace_overlay(5, vec![]).is_err());

        let saved = rom.save().expect("saves").into_owned();
        assert_eq!(crc16(&saved[..0x15E]), u16::from_le_bytes([saved[0x15E], saved[0x15F]]));
        assert_eq!(saved.len() % 4, 0);
        assert_eq!(u32_at(&saved, 0x80).expect("size") as usize, saved.len());
        assert_eq!(u32_at(&saved, 0x20).expect("arm9"), 0x4000);
        let back = open(saved.clone());
        assert_eq!(back.narc("x.narc").expect("narc"), narc);
        assert_eq!(back.file("a/0/0/2"), Some(&b"file a/0/0/2"[..]));
        assert_eq!(back.overlay(0).expect("overlay"), overlay0);
        assert_eq!(back.overlay(1).expect("overlay"), [2; 0x41]);
        assert_eq!(back.arm9().expect("arm9"), arm9);
        assert_eq!(back.overlays()[0].flags, 3);
        assert_eq!(back.overlays()[1].file_id, 1);
        // Every file starts on a 0x200 boundary.
        let fat = slice_at(&saved, u32_at(&saved, 0x48).expect("fat") as usize, 5 * 8).expect("fat");
        assert!(
            fat.chunks(8)
                .all(|e| u32::from_le_bytes([e[0], e[1], e[2], e[3]]) % 0x200 == 0)
        );
        // Rebuilding the rebuilt ROM with the same replacements gives the same bytes.
        let mut again = open(saved.clone());
        again.replace_file("x.narc", narc.to_bytes()).expect("exists");
        assert_eq!(again.save().expect("saves").as_ref(), saved.as_slice());
    }

    #[test]
    fn rejects_tables_outside_the_rom() {
        let bytes = sample_rom();
        assert!(NdsRom::from_bytes(bytes[..0x100].to_vec()).is_err());
        let mut broken = bytes.clone();
        broken[0x4C..0x50].copy_from_slice(&0xFFFF_FFF0_u32.to_le_bytes());
        assert!(NdsRom::from_bytes(broken).is_err());
        let mut broken = bytes;
        let fat = u32_at(&broken, 0x48).expect("fat") as usize;
        broken[fat + 4..fat + 8].copy_from_slice(&0xFFFF_FFF0_u32.to_le_bytes());
        assert!(NdsRom::from_bytes(broken).is_err());
    }

    #[test]
    fn device_capacity_matches_the_cartridge_size() {
        assert_eq!(device_capacity(1000), 0);
        assert_eq!(device_capacity(0x80_0000), 7);
        assert_eq!(device_capacity(0x7F_FFFC), 6);
        assert_eq!(device_capacity(0x0800_0000), 11);
    }

    /// A real ROM from `PPR_TEST_ROMS` whose file name contains `name`.
    fn real_rom(name: &str) -> Vec<u8> {
        let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a folder of ROM dumps");
        let path = std::fs::read_dir(dir)
            .expect("folder")
            .filter_map(Result::ok)
            .map(|e| e.path())
            .find(|p| p.to_string_lossy().contains(name))
            .unwrap_or_else(|| panic!("no ROM named like {name}"));
        std::fs::read(path).expect("readable")
    }

    /// Checks a real ROM; `compressed` says whether its ARM9 and overlays are compressed.
    fn check_real_rom(name: &str, personal: &str, species: usize, entry_len: usize, compressed: bool) {
        let bytes = real_rom(name);
        let mut rom = open(bytes.clone());
        assert!(matches!(rom.save().expect("saves"), Cow::Borrowed(b) if b == bytes.as_slice()));
        assert!(rom.paths().count() > 100);
        assert!(rom.paths().any(|p| p == personal));

        // ARM9 and a compressed overlay decompress, and compress back to the same contents.
        let layout = rom.arm9_layout().expect("arm9");
        let arm9 = rom.arm9().expect("arm9");
        let (stored, len) = layout.encode(&arm9, arm9::ram_address(&bytes).expect("header"));
        let packed = &stored[..len];
        assert_eq!(layout.compressed(), compressed);
        if compressed {
            assert_eq!(blz::decompress(packed).expect("decompresses"), arm9);
        }
        let compressed_overlay = (0..rom.overlays().len()).find(|&n| {
            let entry = &rom.overlays()[n];
            entry.is_compressed(rom.original(entry.file_id as usize).len())
        });
        assert_eq!(compressed_overlay.is_some(), compressed);
        let number = compressed_overlay.unwrap_or(0);
        let overlay = rom.overlay(number).expect("overlay");
        if compressed {
            assert!(overlay.len() > rom.original(rom.overlays()[number].file_id as usize).len());
            let packed = blz::compress(&overlay, 0, blz::Mode::Normal);
            assert_eq!(blz::decompress(&packed).expect("decompresses"), overlay);
        }

        // The personal data NARC: one entry per species and form.
        let mut narc = rom.narc(personal).expect("narc");
        assert!(narc.files.len() > species);
        assert_eq!(narc.files[1].len(), entry_len);

        // Replace an entry, the ARM9 and the overlay, rebuild and reopen.
        narc.files[1][0] ^= 0xFF;
        rom.replace_narc(personal, &narc).expect("exists");
        rom.replace_arm9(arm9.clone()).expect("readable");
        let mut changed = overlay.clone();
        changed[0] ^= 0xFF;
        rom.replace_overlay(number, changed.clone()).expect("exists");
        let saved = rom.save().expect("saves").into_owned();
        let back = open(saved);
        let original = open(bytes);
        assert_eq!(back.narc(personal).expect("narc"), narc);
        for path in original.paths().filter(|&p| p != personal) {
            assert_eq!(back.file(path), original.file(path), "{path}");
        }
        assert_eq!(back.arm9().expect("arm9"), arm9);
        assert_eq!(back.overlay(number).expect("overlay"), changed);
        for n in (0..original.overlays().len()).filter(|&n| n != number) {
            assert_eq!(
                back.overlay(n).expect("overlay"),
                original.overlay(n).expect("overlay"),
                "overlay {n}"
            );
        }
    }

    #[test]
    #[ignore = "needs a real ROM"]
    fn platinum_opens_saves_and_rebuilds() {
        check_real_rom(
            "Platinum Version (Europe)",
            "poketool/personal/pl_personal.narc",
            493,
            44,
            false,
        );
    }

    #[test]
    #[ignore = "needs a real ROM"]
    fn black_2_opens_saves_and_rebuilds() {
        check_real_rom("Black Version 2", "a/0/1/6", 649, 76, true);
    }
}
