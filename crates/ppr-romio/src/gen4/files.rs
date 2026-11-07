//! Working copies of what the handler changes in a DS ROM: NARCs, plain files, overlays, the ARM9 binary and the
//! text archive. They are loaded on first use and written back by [`DsFiles::flush`] only when they differ from what
//! the ROM holds, so an unchanged game saves byte for byte.
//!
//! The ARM9 is extended once when loaded, as the games' patches and repointed data need room in it ([`super::arm9`]);
//! the ROM entries' ARM9 offsets count with that. It is only written back when something else in it changed.

use std::collections::{BTreeMap, BTreeSet};

use ppr_formats::narc::Narc;
use ppr_formats::nds::NdsRom;

use crate::RomError;
use crate::nds_text::gen4::{self, Gen4Text};
use crate::textfmt::{self, Breaks};

/// The line breaks of Gen 4 text.
const BREAKS: Breaks<'static> = Breaks {
    newline: "\\n",
    extraline: "\\l",
    newpara: "\\p",
};

/// A decoded text file and whether it was packed.
#[derive(Debug, Clone)]
struct Text {
    original: Gen4Text,
    current: Vec<String>,
    compressed: bool,
}

/// Loaded DS ROM with working copies.
#[derive(Debug)]
pub(super) struct DsFiles {
    rom: NdsRom,
    narcs: BTreeMap<String, Narc>,
    files: BTreeMap<String, Vec<u8>>,
    overlays: BTreeMap<usize, Vec<u8>>,
    /// The ARM9 as loaded and extended, and the working copy.
    arm9_base: Vec<u8>,
    arm9: Vec<u8>,
    text_path: String,
    texts: BTreeMap<usize, Text>,
    /// Overlays whose working copy may differ from the ROM's.
    touched_overlays: BTreeSet<usize>,
}

#[expect(clippy::needless_pass_by_value, reason = "made for map_err")]
fn format(e: ppr_formats::FormatError) -> RomError {
    RomError::Malformed(e.to_string())
}

impl DsFiles {
    /// Opens `bytes`; `text_path` is the text archive. The ARM9 is read and passed to `prepare_arm9` (which extends
    /// it) before anything else sees it.
    pub(super) fn open(
        bytes: Vec<u8>,
        text_path: String,
        prepare_arm9: impl FnOnce(&mut Vec<u8>) -> Result<(), RomError>,
    ) -> Result<Self, RomError> {
        let rom = NdsRom::from_bytes(bytes).map_err(format)?;
        let mut arm9 = rom.arm9().map_err(format)?;
        prepare_arm9(&mut arm9)?;
        Ok(Self {
            rom,
            narcs: BTreeMap::new(),
            files: BTreeMap::new(),
            overlays: BTreeMap::new(),
            arm9_base: arm9.clone(),
            arm9,
            text_path,
            texts: BTreeMap::new(),
            touched_overlays: BTreeSet::new(),
        })
    }

    /// The NARC at `path`, loaded on first use.
    pub(super) fn narc(&mut self, path: &str) -> Result<&mut Narc, RomError> {
        if !self.narcs.contains_key(path) {
            let narc = self.rom.narc(path).map_err(format)?;
            self.narcs.insert(path.to_owned(), narc);
        }
        self.narcs
            .get_mut(path)
            .ok_or_else(|| RomError::Malformed(format!("no NARC {path}")))
    }

    /// File `index` of the NARC at `path`.
    pub(super) fn narc_file(&mut self, path: &str, index: usize) -> Result<&mut Vec<u8>, RomError> {
        let narc = self.narc(path)?;
        let count = narc.files.len();
        narc.files
            .get_mut(index)
            .ok_or_else(|| RomError::Malformed(format!("{path} has {count} files, not {}", index + 1)))
    }

    /// The plain file at `path`.
    pub(super) fn file(&mut self, path: &str) -> Result<&mut Vec<u8>, RomError> {
        if !self.files.contains_key(path) {
            let data = self
                .rom
                .file(path)
                .ok_or_else(|| RomError::Malformed(format!("no file {path}")))?
                .to_vec();
            self.files.insert(path.to_owned(), data);
        }
        self.files
            .get_mut(path)
            .ok_or_else(|| RomError::Malformed(format!("no file {path}")))
    }

    /// Overlay `number`, decompressed.
    pub(super) fn overlay(&mut self, number: usize) -> Result<&mut Vec<u8>, RomError> {
        if !self.overlays.contains_key(&number) {
            let data = self.rom.overlay(number).map_err(format)?;
            self.overlays.insert(number, data);
        }
        self.touched_overlays.insert(number);
        self.overlays
            .get_mut(&number)
            .ok_or_else(|| RomError::Malformed(format!("no overlay {number}")))
    }

    /// The ARM9 binary (extended).
    pub(super) fn arm9(&mut self) -> &mut Vec<u8> {
        &mut self.arm9
    }

    /// The ARM9 binary, to read.
    pub(super) fn arm9_ref(&self) -> &[u8] {
        &self.arm9
    }

    fn decode(&mut self, index: usize) -> Result<&mut Text, RomError> {
        if !self.texts.contains_key(&index) {
            let path = self.text_path.clone();
            let data = self.narc_file(&path, index)?;
            let decoded = gen4::decode(data)?;
            self.texts.insert(
                index,
                Text {
                    current: decoded.strings.clone(),
                    compressed: decoded.compressed,
                    original: decoded,
                },
            );
        }
        self.texts
            .get_mut(&index)
            .ok_or_else(|| RomError::Malformed(format!("no text file {index}")))
    }

    /// The strings of text file `index`.
    pub(super) fn strings(&mut self, index: usize) -> Result<Vec<String>, RomError> {
        Ok(self.decode(index)?.current.clone())
    }

    /// Replaces the strings of text file `index`; it is saved unpacked.
    pub(super) fn set_strings(&mut self, index: usize, strings: Vec<String>) -> Result<(), RomError> {
        self.set_strings_packed(index, strings, false)
    }

    /// Replaces the strings of text file `index`, packed 9 bits per code if `compressed`.
    pub(super) fn set_strings_packed(
        &mut self,
        index: usize,
        strings: Vec<String>,
        compressed: bool,
    ) -> Result<(), RomError> {
        let text = self.decode(index)?;
        text.current = strings;
        text.compressed = compressed;
        Ok(())
    }

    /// Replaces names in text file `index`: each string holding a key of `replacements` gets the values for the
    /// keys and is wrapped again at `line_length` characters. A name keeps its spaces on one line: they are
    /// underscores while wrapping, and every underscore of the string is a space afterwards.
    pub(super) fn replace_names(
        &mut self,
        index: usize,
        replacements: &BTreeMap<String, String>,
        line_length: usize,
    ) -> Result<(), RomError> {
        let pairs: Vec<(String, String)> = replacements
            .iter()
            .map(|(from, to)| (from.clone(), to.replace(' ', "_")))
            .collect();
        let pairs: Vec<(&str, &str)> = pairs.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut strings = self.strings(index)?;
        let mut changed = false;
        for s in &mut strings {
            if pairs.iter().any(|(from, _)| s.contains(from)) {
                *s = textfmt::format_with_replacements(s, &pairs, BREAKS, line_length, |w| w.chars().count())
                    .replace('_', " ");
                changed = true;
            }
        }
        if changed {
            self.set_strings(index, strings)?;
        }
        Ok(())
    }

    /// Writes the working copies that differ from the ROM's into it.
    pub(super) fn flush(&mut self) -> Result<(), RomError> {
        let text_path = self.text_path.clone();
        let mut changed_texts = Vec::new();
        for (&index, t) in &self.texts {
            if t.current != t.original.strings {
                changed_texts.push((index, gen4::encode(&t.current, t.compressed)?));
            }
        }
        for (index, data) in changed_texts {
            *self.narc_file(&text_path, index)? = data;
        }
        for t in self.texts.values_mut() {
            t.original.strings.clone_from(&t.current);
            t.original.compressed = t.compressed;
        }
        for (path, narc) in &self.narcs {
            let original = self.rom.narc(path).map_err(format)?;
            if original != *narc {
                self.rom.replace_narc(path, narc).map_err(format)?;
            }
        }
        for (path, data) in &self.files {
            if self.rom.file(path) != Some(data.as_slice()) {
                self.rom.replace_file(path, data.clone()).map_err(format)?;
            }
        }
        for &number in &self.touched_overlays {
            let Some(data) = self.overlays.get(&number) else {
                continue;
            };
            if self.rom.overlay(number).map_err(format)? != *data {
                self.rom.replace_overlay(number, data.clone()).map_err(format)?;
            }
        }
        self.touched_overlays.clear();
        if self.arm9 != self.arm9_base {
            self.rom.replace_arm9(self.arm9.clone()).map_err(format)?;
            self.arm9_base.clone_from(&self.arm9);
        }
        Ok(())
    }

    /// The ROM as saved.
    pub(super) fn save(&self) -> Result<std::borrow::Cow<'_, [u8]>, RomError> {
        self.rom.save().map_err(format)
    }
}

/// A 16-bit little-endian value.
pub(super) fn word(data: &[u8], at: usize) -> Result<u16, RomError> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| RomError::Malformed(format!("no 16-bit value at {at:#x}")))
}

/// A 32-bit little-endian value.
pub(super) fn long(data: &[u8], at: usize) -> Result<u32, RomError> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| RomError::Malformed(format!("no 32-bit value at {at:#x}")))
}

/// A byte.
pub(super) fn byte(data: &[u8], at: usize) -> Result<u8, RomError> {
    data.get(at)
        .copied()
        .ok_or_else(|| RomError::Malformed(format!("no byte at {at:#x}")))
}

/// Writes a 16-bit little-endian value.
pub(super) fn put_word(data: &mut [u8], at: usize, value: u16) -> Result<(), RomError> {
    put(data, at, &value.to_le_bytes())
}

/// Writes a 32-bit little-endian value.
pub(super) fn put_long(data: &mut [u8], at: usize, value: u32) -> Result<(), RomError> {
    put(data, at, &value.to_le_bytes())
}

/// Writes `bytes` at `at`.
pub(super) fn put(data: &mut [u8], at: usize, bytes: &[u8]) -> Result<(), RomError> {
    data.get_mut(at..at + bytes.len())
        .ok_or_else(|| RomError::Malformed(format!("no room for {} bytes at {at:#x}", bytes.len())))?
        .copy_from_slice(bytes);
    Ok(())
}

/// Every place `needle` occurs in `data`.
pub(super) fn search(data: &[u8], needle: &[u8]) -> Vec<usize> {
    if needle.is_empty() || needle.len() > data.len() {
        return Vec::new();
    }
    data.windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle)
        .map(|(i, _)| i)
        .collect()
}

/// Where the bytes of a hex string occur in `data`, if exactly once and not at the start.
pub(super) fn find(data: &[u8], hex: &str) -> Option<usize> {
    let needle = hex_bytes(hex)?;
    match search(data, &needle).as_slice() {
        [at] if *at > 0 => Some(*at),
        _ => None,
    }
}

/// Where the bytes after a hex prefix start, if the prefix occurs exactly once (and not at the start).
pub(super) fn find_after(data: &[u8], hex: &str) -> Option<usize> {
    find(data, hex).map(|at| at + hex.len() / 2)
}

/// The bytes of a string of hex digits.
pub(super) fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) || s.is_empty() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}
