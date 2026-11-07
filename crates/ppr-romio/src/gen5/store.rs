//! The ROM's parts as the handler reads and changes them: NARCs, overlays, the ARM9, plain files and text, each
//! read once and kept, and written back by [`Store::flush`] only where it differs from the ROM.
//!
//! Also the ARM9 extension: the part of the ARM9 copied to ITCM grows, so patches and repointed data fit there.

use std::collections::{BTreeMap, BTreeSet};

use ppr_formats::narc::Narc;
use ppr_formats::nds::NdsRom;

use crate::RomError;
use crate::freespace::FreedSpace;
use crate::nds_text::gen5 as text;

/// Where the ARM9 is loaded in RAM.
pub(super) const ARM9_ADDRESS: usize = 0x0200_4000;
/// Where ITCM is mapped in RAM, and the end of its mirrors.
const ITCM_ADDRESS: usize = 0x0100_0000;
const ITCM_END: usize = 0x01FF_FFFF;
const ITCM_LENGTH: usize = 0x8000;

/// One of the two text NARCs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Text {
    /// System text: names, descriptions, battle messages.
    Strings,
    /// Story text: what people say.
    Story,
}

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// Bytes of `data` at `at`, or an error naming what was read.
pub(super) fn slice(data: &[u8], at: usize, len: usize) -> Result<&[u8], RomError> {
    data.get(at..at + len)
        .ok_or_else(|| malformed(format!("{len} bytes at {at:#x} are past the end ({:#x})", data.len())))
}

pub(super) fn byte(data: &[u8], at: usize) -> Result<u8, RomError> {
    Ok(slice(data, at, 1)?[0])
}

pub(super) fn word(data: &[u8], at: usize) -> Result<u16, RomError> {
    let b = slice(data, at, 2)?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

pub(super) fn long(data: &[u8], at: usize) -> Result<u32, RomError> {
    let b = slice(data, at, 4)?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

pub(super) fn put(data: &mut [u8], at: usize, bytes: &[u8]) -> Result<(), RomError> {
    let len = data.len();
    data.get_mut(at..at + bytes.len())
        .ok_or_else(|| malformed(format!("{} bytes at {at:#x} are past the end ({len:#x})", bytes.len())))?
        .copy_from_slice(bytes);
    Ok(())
}

pub(super) fn put_byte(data: &mut [u8], at: usize, value: u8) -> Result<(), RomError> {
    put(data, at, &[value])
}

pub(super) fn put_word(data: &mut [u8], at: usize, value: u16) -> Result<(), RomError> {
    put(data, at, &value.to_le_bytes())
}

pub(super) fn put_long(data: &mut [u8], at: usize, value: u32) -> Result<(), RomError> {
    put(data, at, &value.to_le_bytes())
}

/// A relative pointer at `at`: the target is counted from the end of the pointer.
pub(super) fn relative_pointer(data: &[u8], at: usize) -> Result<usize, RomError> {
    let delta = i64::from(long(data, at)?.cast_signed());
    usize::try_from(delta + i64::try_from(at + 4).unwrap_or(i64::MAX))
        .map_err(|_| malformed(format!("relative pointer at {at:#x} points before the file")))
}

pub(super) fn put_relative_pointer(data: &mut [u8], at: usize, target: usize) -> Result<(), RomError> {
    let delta = i64::try_from(target).unwrap_or_default() - i64::try_from(at + 4).unwrap_or_default();
    put_long(data, at, u32::try_from(delta & 0xFFFF_FFFF).unwrap_or_default())
}

/// Parses hex text (`"DC282FD3"`) into bytes.
pub(super) fn hex(s: &str) -> Result<Vec<u8>, RomError> {
    let digits: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return Err(malformed(format!("odd hex text {s}")));
    }
    digits
        .chunks(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|p| u8::from_str_radix(p, 16).ok())
                .ok_or_else(|| malformed(format!("bad hex text {s}")))
        })
        .collect()
}

/// Where the hex `needle` is in `data`, when it is there exactly once and not at the very start (as the original's
/// searches, which treat offset 0 as not found).
pub(super) fn find(data: &[u8], needle: &str) -> Result<Option<usize>, RomError> {
    let needle = hex(needle)?;
    if needle.is_empty() || needle.len() > data.len() {
        return Ok(None);
    }
    let mut hits = data
        .windows(needle.len())
        .enumerate()
        .filter(|(_, w)| *w == needle.as_slice());
    let first = hits.next().map(|(i, _)| i);
    Ok(match (first, hits.next()) {
        (Some(at), None) if at > 0 => Some(at),
        _ => None,
    })
}

/// [`find`], pointing past the needle.
pub(super) fn find_after(data: &[u8], needle: &str) -> Result<Option<usize>, RomError> {
    Ok(find(data, needle)?.map(|at| at + needle.len() / 2))
}

/// The ROM's parts as read and changed.
#[derive(Debug)]
pub(super) struct Store {
    rom: NdsRom,
    text_paths: [String; 2],
    narcs: BTreeMap<String, Narc>,
    dirty_narcs: BTreeSet<String>,
    overlays: BTreeMap<usize, Vec<u8>>,
    dirty_overlays: BTreeSet<usize>,
    arm9: Option<Vec<u8>>,
    arm9_dirty: bool,
    files: BTreeMap<String, Vec<u8>>,
    dirty_files: BTreeSet<String>,
    texts: BTreeMap<(Text, usize), Vec<String>>,
    dirty_texts: BTreeSet<(Text, usize)>,
    /// After the ARM9 was extended: where the ITCM copying pointers are, and the space for repointed data.
    extension: Option<Extension>,
    /// The extension to make before the ARM9 first changes: patch and repoint sizes, and the copying code's prefix.
    planned_extension: Option<(usize, usize, String)>,
}

#[derive(Debug)]
struct Extension {
    tcm_pointers: usize,
    free: FreedSpace,
}

impl Store {
    pub(super) fn new(rom: NdsRom, strings: String, story: String) -> Self {
        Self {
            rom,
            text_paths: [strings, story],
            narcs: BTreeMap::new(),
            dirty_narcs: BTreeSet::new(),
            overlays: BTreeMap::new(),
            dirty_overlays: BTreeSet::new(),
            arm9: None,
            arm9_dirty: false,
            files: BTreeMap::new(),
            dirty_files: BTreeSet::new(),
            texts: BTreeMap::new(),
            dirty_texts: BTreeSet::new(),
            extension: None,
            planned_extension: None,
        }
    }

    /// Extends the ARM9 (see [`Store::extend_arm9`]) right before it first changes.
    pub(super) fn plan_arm9_extension(&mut self, patch: usize, repoint: usize, prefix: &str) {
        self.planned_extension = Some((patch, repoint, prefix.to_owned()));
    }

    fn make_planned_extension(&mut self) -> Result<(), RomError> {
        if let Some((patch, repoint, prefix)) = self.planned_extension.take() {
            self.extend_arm9(patch, repoint, &prefix)?;
        }
        Ok(())
    }

    /// Drops what was read and not changed, to free memory.
    pub(super) fn forget_unchanged(&mut self) {
        let dirty = &self.dirty_narcs;
        self.narcs.retain(|k, _| dirty.contains(k));
        let dirty = &self.dirty_overlays;
        self.overlays.retain(|k, _| dirty.contains(k));
        if !self.arm9_dirty {
            self.arm9 = None;
        }
        let dirty = &self.dirty_files;
        self.files.retain(|k, _| dirty.contains(k));
        let dirty = &self.dirty_texts;
        self.texts.retain(|k, _| dirty.contains(k));
    }

    fn load_narc(&mut self, path: &str) -> Result<(), RomError> {
        if !self.narcs.contains_key(path) {
            let narc = self.rom.narc(path)?;
            self.narcs.insert(path.to_owned(), narc);
        }
        Ok(())
    }

    pub(super) fn narc(&mut self, path: &str) -> Result<&Narc, RomError> {
        self.load_narc(path)?;
        Ok(&self.narcs[path])
    }

    pub(super) fn narc_mut(&mut self, path: &str) -> Result<&mut Narc, RomError> {
        self.load_narc(path)?;
        self.dirty_narcs.insert(path.to_owned());
        self.narcs
            .get_mut(path)
            .ok_or_else(|| malformed(format!("no NARC {path}")))
    }

    /// File `index` of the NARC at `path`.
    pub(super) fn narc_file(&mut self, path: &str, index: usize) -> Result<&[u8], RomError> {
        self.narc(path)?
            .files
            .get(index)
            .map(Vec::as_slice)
            .ok_or_else(|| malformed(format!("{path} has no file {index}")))
    }

    pub(super) fn narc_file_mut(&mut self, path: &str, index: usize) -> Result<&mut Vec<u8>, RomError> {
        self.narc_mut(path)?
            .files
            .get_mut(index)
            .ok_or_else(|| malformed(format!("{path} has no file {index}")))
    }

    fn load_overlay(&mut self, number: usize) -> Result<(), RomError> {
        if !self.overlays.contains_key(&number) {
            let data = self.rom.overlay(number)?;
            self.overlays.insert(number, data);
        }
        Ok(())
    }

    pub(super) fn overlay(&mut self, number: usize) -> Result<&[u8], RomError> {
        self.load_overlay(number)?;
        Ok(&self.overlays[&number])
    }

    pub(super) fn overlay_mut(&mut self, number: usize) -> Result<&mut Vec<u8>, RomError> {
        self.load_overlay(number)?;
        self.dirty_overlays.insert(number);
        self.overlays
            .get_mut(&number)
            .ok_or_else(|| malformed(format!("no overlay {number}")))
    }

    /// Where overlay `number` is loaded in RAM.
    pub(super) fn overlay_address(&self, number: usize) -> Result<usize, RomError> {
        self.rom
            .overlays()
            .get(number)
            .map(|o| o.ram_address as usize)
            .ok_or_else(|| malformed(format!("no overlay {number}")))
    }

    fn load_arm9(&mut self) -> Result<(), RomError> {
        if self.arm9.is_none() {
            self.arm9 = Some(self.rom.arm9()?);
        }
        Ok(())
    }

    pub(super) fn arm9(&mut self) -> Result<&[u8], RomError> {
        self.load_arm9()?;
        self.arm9.as_deref().ok_or_else(|| malformed("no ARM9"))
    }

    pub(super) fn arm9_mut(&mut self) -> Result<&mut Vec<u8>, RomError> {
        self.make_planned_extension()?;
        self.raw_arm9_mut()
    }

    fn raw_arm9_mut(&mut self) -> Result<&mut Vec<u8>, RomError> {
        self.load_arm9()?;
        self.arm9_dirty = true;
        self.arm9.as_mut().ok_or_else(|| malformed("no ARM9"))
    }

    fn load_file(&mut self, path: &str) -> Result<(), RomError> {
        if !self.files.contains_key(path) {
            let data = self
                .rom
                .file(path)
                .ok_or_else(|| malformed(format!("no file {path}")))?
                .to_vec();
            self.files.insert(path.to_owned(), data);
        }
        Ok(())
    }

    pub(super) fn file_mut(&mut self, path: &str) -> Result<&mut Vec<u8>, RomError> {
        self.load_file(path)?;
        self.dirty_files.insert(path.to_owned());
        self.files
            .get_mut(path)
            .ok_or_else(|| malformed(format!("no file {path}")))
    }

    fn text_path(&self, kind: Text) -> String {
        self.text_paths[usize::from(kind == Text::Story)].clone()
    }

    fn load_strings(&mut self, kind: Text, index: usize) -> Result<(), RomError> {
        if !self.texts.contains_key(&(kind, index)) {
            let path = self.text_path(kind);
            let strings = text::decode(self.narc_file(&path, index)?)?;
            self.texts.insert((kind, index), strings);
        }
        Ok(())
    }

    /// The strings of text file `index`.
    pub(super) fn strings(&mut self, kind: Text, index: usize) -> Result<&[String], RomError> {
        self.load_strings(kind, index)?;
        Ok(&self.texts[&(kind, index)])
    }

    pub(super) fn strings_mut(&mut self, kind: Text, index: usize) -> Result<&mut Vec<String>, RomError> {
        self.load_strings(kind, index)?;
        self.dirty_texts.insert((kind, index));
        self.texts
            .get_mut(&(kind, index))
            .ok_or_else(|| malformed(format!("no text {index}")))
    }

    /// Writes back everything changed that differs from the ROM, and forgets it.
    pub(super) fn flush(&mut self) -> Result<(), RomError> {
        for key in std::mem::take(&mut self.dirty_texts) {
            let Some(strings) = self.texts.remove(&key) else {
                continue;
            };
            let path = self.text_path(key.0);
            let original = self.narc_file(&path, key.1)?.to_vec();
            if text::decode(&original)? != strings {
                let encoded = text::encode(&original, &strings)?;
                *self.narc_file_mut(&path, key.1)? = encoded;
            }
        }
        for path in std::mem::take(&mut self.dirty_narcs) {
            let Some(narc) = self.narcs.remove(&path) else { continue };
            if self.rom.narc(&path)? != narc {
                self.rom.replace_narc(&path, &narc)?;
            }
        }
        for number in std::mem::take(&mut self.dirty_overlays) {
            let Some(data) = self.overlays.remove(&number) else {
                continue;
            };
            if self.rom.overlay(number)? != data {
                self.rom.replace_overlay(number, data)?;
            }
        }
        if std::mem::take(&mut self.arm9_dirty)
            && let Some(data) = self.arm9.take()
            && self.rom.arm9()? != data
        {
            self.rom.replace_arm9(data)?;
        }
        for path in std::mem::take(&mut self.dirty_files) {
            let Some(data) = self.files.remove(&path) else { continue };
            if self.rom.file(&path) != Some(data.as_slice()) {
                self.rom.replace_file(&path, data)?;
            }
        }
        self.forget_unchanged();
        Ok(())
    }

    /// Grows the part of the ARM9 that is copied to ITCM by `patch + repoint` bytes, moving what follows it, as the
    /// original does on loading; the last `repoint` bytes become free space for repointed data. Only once, and
    /// nothing when both are 0.
    fn extend_arm9(&mut self, patch: usize, repoint: usize, prefix: &str) -> Result<(), RomError> {
        let by = patch + repoint;
        if self.extension.is_some() || by == 0 {
            return Ok(());
        }
        let prefix = hex(prefix)?;
        let arm9 = self.raw_arm9_mut()?;
        let tcm_pointers = arm9
            .windows(prefix.len())
            .position(|w| w == prefix.as_slice())
            .ok_or_else(|| malformed("the ITCM copying code is not in the ARM9"))?
            + prefix.len();
        let address = |v: u32| {
            (v as usize)
                .checked_sub(ARM9_ADDRESS)
                .ok_or_else(|| malformed("ARM9 pointer"))
        };
        let dest_pointers = address(long(arm9, tcm_pointers)?)?;
        let itcm_src = address(long(arm9, tcm_pointers + 8)?)?;
        let itcm_size_at = dest_pointers + 4;
        let itcm_size = long(arm9, itcm_size_at)? as usize;
        if itcm_size + by > ITCM_LENGTH {
            return Err(malformed("the ITCM section can't grow past 32 KiB"));
        }
        let dtcm = itcm_src + itcm_size;
        let old_len = arm9.len();
        if dtcm > old_len {
            return Err(malformed("the ITCM section ends past the ARM9"));
        }
        let to_u32 = |v: usize| u32::try_from(v).map_err(|_| malformed("ARM9 too large"));
        // The pointers and the size are written first, then everything from the end of the ITCM section on moves
        // up (with them, when they lie there); the gap keeps a copy of what was there.
        arm9.resize(old_len + by, 0);
        put_long(arm9, tcm_pointers, to_u32(dest_pointers + by + ARM9_ADDRESS)?)?;
        put_long(arm9, tcm_pointers + 4, to_u32(old_len + by + ARM9_ADDRESS)?)?;
        put_long(arm9, itcm_size_at, to_u32(itcm_size + by)?)?;
        arm9.copy_within(dtcm..old_len, dtcm + by);
        let mut free = FreedSpace::default();
        if repoint > 0 {
            free.free(itcm_src + itcm_size + by - repoint, repoint)?;
        }
        self.extension = Some(Extension { tcm_pointers, free });
        Ok(())
    }

    fn itcm(&mut self) -> Result<(usize, usize), RomError> {
        let tcm_pointers = self
            .extension
            .as_ref()
            .map(|e| e.tcm_pointers)
            .ok_or_else(|| malformed("the ARM9 was not extended"))?;
        let arm9 = self.arm9()?;
        let src = (long(arm9, tcm_pointers + 8)? as usize).saturating_sub(ARM9_ADDRESS);
        let size_at = (long(arm9, tcm_pointers)? as usize).saturating_sub(ARM9_ADDRESS) + 4;
        let size = long(arm9, size_at)? as usize;
        Ok((src, size))
    }

    /// Takes `length` bytes of the free space of the extended ARM9.
    pub(super) fn take_arm9_space(&mut self, length: usize) -> Result<usize, RomError> {
        self.make_planned_extension()?;
        self.extension
            .as_mut()
            .and_then(|e| e.free.find_and_unfree(length))
            .ok_or_else(|| malformed(format!("no room for {length} bytes in the ARM9")))
    }

    /// Gives `length` bytes at `at` of the ARM9 back to its free space.
    pub(super) fn free_arm9_space(&mut self, at: usize, length: usize) -> Result<(), RomError> {
        match self.extension.as_mut() {
            Some(e) => e.free.free(at, length),
            None => Ok(()),
        }
    }

    /// Whether the pointer value points into the ARM9 or ITCM in RAM.
    pub(super) fn is_arm9_pointer(&mut self, pointer: u32) -> Result<bool, RomError> {
        let len = self.arm9()?.len();
        let p = pointer as usize;
        Ok(p <= ITCM_END || (ARM9_ADDRESS..ARM9_ADDRESS + len).contains(&p))
    }

    /// The ARM9 offset a pointer into the ARM9 or ITCM stands for.
    pub(super) fn arm9_offset(&mut self, pointer: u32) -> Result<usize, RomError> {
        let p = pointer as usize;
        if p <= ITCM_END {
            let (src, _) = self.itcm()?;
            return Ok(p % ITCM_LENGTH + src);
        }
        let len = self.arm9()?.len();
        if (ARM9_ADDRESS..ARM9_ADDRESS + len).contains(&p) {
            Ok(p - ARM9_ADDRESS)
        } else {
            Err(malformed(format!("{pointer:#x} points neither into the ARM9 nor ITCM")))
        }
    }

    /// The pointer value standing for ARM9 offset `at`: into ITCM when `at` is in the part copied there.
    pub(super) fn arm9_pointer(&mut self, at: usize) -> Result<u32, RomError> {
        let (src, size) = self.itcm()?;
        let len = self.arm9()?.len();
        let value = if (src..src + size).contains(&at) {
            at - src + ITCM_ADDRESS
        } else if at < len {
            at + ARM9_ADDRESS
        } else {
            return Err(malformed(format!("{at:#x} is not in the ARM9")));
        };
        u32::try_from(value).map_err(|_| malformed("ARM9 pointer"))
    }

    pub(super) fn save(&self) -> Result<std::borrow::Cow<'_, [u8]>, RomError> {
        Ok(self.rom.save()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_unique_needles_only() {
        let data = [0, 1, 2, 3, 1, 2, 9];
        assert_eq!(find(&data, "0203").ok().flatten(), Some(2));
        assert_eq!(find(&data, "0102").ok().flatten(), None, "twice");
        assert_eq!(find(&data, "0001").ok().flatten(), None, "at the start");
        assert_eq!(find_after(&data, "0203").ok().flatten(), Some(4));
    }

    #[test]
    fn relative_pointers_round_trip() {
        let mut data = vec![0; 16];
        put_relative_pointer(&mut data, 4, 2).expect("fits");
        assert_eq!(relative_pointer(&data, 4).ok(), Some(2));
        put_relative_pointer(&mut data, 0, 12).expect("fits");
        assert_eq!(relative_pointer(&data, 0).ok(), Some(12));
    }
}
