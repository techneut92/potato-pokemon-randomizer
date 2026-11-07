//! Game Boy and Game Boy Color ROMs (Gen 1 and 2): banked two-byte pointers, the free space at the end of banks, and data that moves into
//! that space when it grows.
//!
//! The chunks a ROM entry marks as unused (`UnusedChunk<…>`) are not zeroed while loading: the free space list
//! alone says what may be written, and bytes are only zeroed when data actually moves out of them, so an unchanged
//! game saves byte for byte.
//!
//! Data that is rewritten frees its old bytes first and then takes the first free chunk that fits, in address order:
//! in the pointer's bank, then in bank 0 ([`GbRom::rewrite_same_bank`]), or in any unreserved bank and its own bank
//! last ([`GbRom::rewrite_indirect_bank`]). So data that grows can reuse the space of data rewritten before it, as
//! freed neighbours merge; a list rewritten in order, like the shops, moves into the space of the ones before it.
//! Free space is looked at bank by bank, so a chunk that runs over a bank border counts in both banks. Placement
//! matches the original byte for byte; the Gen 1 and 2 shop tests check it against the reference.

pub(crate) mod pic;
pub(crate) mod text;

use crate::RomError;
use ppr_model::MoveId;

use crate::entries::RomEntry;
use crate::freespace::FreedSpace;

/// Bytes per ROM bank.
pub(crate) const BANK: usize = 0x4000;
/// The byte of unused space.
const FREE_BYTE: u8 = 0;

/// A GB ROM in memory, with the space the randomizer may write to.
#[derive(Debug, Clone)]
pub(crate) struct GbRom {
    bytes: Vec<u8>,
    free: FreedSpace,
    /// Banks free space is not taken from unless asked for by number (`ReservedBanks`).
    reserved: Vec<bool>,
}

fn out_of_range(offset: usize) -> RomError {
    RomError::Malformed(format!("offset {offset:#x} is outside the ROM"))
}

impl GbRom {
    /// The ROM with the free space its entry describes: the end of some banks, unused
    /// banks and unused chunks.
    pub(crate) fn new(bytes: Vec<u8>, entry: &RomEntry) -> Result<Self, RomError> {
        let banks = bytes.len() / BANK;
        let mut reserved = vec![false; banks];
        for b in entry.array("ReservedBanks").unwrap_or_default() {
            if let Some(r) = usize::try_from(b).ok().and_then(|b| reserved.get_mut(b)) {
                *r = true;
            }
        }
        let mut rom = Self {
            bytes,
            free: FreedSpace::default(),
            reserved,
        };
        for (bank, margin) in bank_end_margins(entry) {
            rom.free_unused_space_at_end_of_bank(bank, margin)?;
        }
        for bank in entry.array("UnusedBanks").unwrap_or_default() {
            let bank = usize::try_from(bank).map_err(|_| RomError::Malformed("negative bank".into()))?;
            let range = rom.slice(bank * BANK, BANK)?;
            if range.iter().all(|&b| b == FREE_BYTE) {
                rom.free.free(bank * BANK, BANK)?;
            } else {
                // The ROM was modified: only what looks unused at the end is freed.
                tracing::warn!("bank {bank:#x} was expected to be empty; the ROM may be modified");
                rom.free_unused_space_at_end_of_bank(bank, 0x100)?;
            }
        }
        for (offset, length) in unused_chunks(entry) {
            rom.free.free(offset, length)?;
        }
        Ok(rom)
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The bytes, for patches that know where they go.
    pub(crate) fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub(crate) fn len(&self) -> usize {
        self.bytes.len()
    }

    pub(crate) fn slice(&self, offset: usize, len: usize) -> Result<&[u8], RomError> {
        self.bytes
            .get(offset..offset.checked_add(len).ok_or_else(|| out_of_range(offset))?)
            .ok_or_else(|| out_of_range(offset))
    }

    pub(crate) fn byte(&self, offset: usize) -> Result<u8, RomError> {
        self.bytes.get(offset).copied().ok_or_else(|| out_of_range(offset))
    }

    /// A little-endian word.
    pub(crate) fn word(&self, offset: usize) -> Result<u16, RomError> {
        let b = self.slice(offset, 2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    pub(crate) fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), RomError> {
        // A write at offset 0 is always a bug.
        if offset == 0 && !data.is_empty() {
            return Err(RomError::Malformed("write at offset 0".into()));
        }
        self.bytes
            .get_mut(offset..offset + data.len())
            .ok_or_else(|| out_of_range(offset))?
            .copy_from_slice(data);
        Ok(())
    }

    pub(crate) fn write_byte(&mut self, offset: usize, value: u8) -> Result<(), RomError> {
        *self.bytes.get_mut(offset).ok_or_else(|| out_of_range(offset))? = value;
        Ok(())
    }

    /// The high or low four bits.
    pub(crate) fn nybble(&self, offset: usize, high: bool) -> Result<u8, RomError> {
        let b = self.byte(offset)?;
        Ok(if high { b >> 4 } else { b & 0xF })
    }

    /// Writes the high or low four bits.
    pub(crate) fn write_nybble(&mut self, offset: usize, high: bool, value: u8) -> Result<(), RomError> {
        let b = self.byte(offset)?;
        let v = value & 0xF;
        let new = if high { (b & 0x0F) | (v << 4) } else { (b & 0xF0) | v };
        self.write_byte(offset, new)
    }

    /// The two-byte pointer at `offset`, into the bank `offset` is in.
    pub(crate) fn pointer(&self, offset: usize) -> Result<usize, RomError> {
        self.pointer_in(offset, bank_of(offset))
    }

    /// The two-byte pointer at `offset`, into `bank` (bank 0 when it points below 0x4000).
    pub(crate) fn pointer_in(&self, offset: usize, bank: usize) -> Result<usize, RomError> {
        Ok(calculate_offset(self.word(offset)?, bank))
    }

    /// Writes a two-byte pointer to `target` at `offset`.
    pub(crate) fn write_pointer(&mut self, offset: usize, target: usize) -> Result<(), RomError> {
        self.write(offset, &gb_pointer(target).to_le_bytes())
    }

    fn free_unused_space_at_end_of_bank(&mut self, bank: usize, margin: usize) -> Result<(), RomError> {
        let end = (bank + 1) * BANK - 1;
        if end >= self.bytes.len() {
            return Err(out_of_range(end));
        }
        let min_start = bank * BANK;
        // The loop stops on the last used byte (or just before the bank), then steps forward and adds the margin.
        let mut start = end;
        while start >= min_start && self.bytes[start] == FREE_BYTE {
            if start == 0 {
                break;
            }
            start -= 1;
        }
        let start = start + 1 + margin;
        if start <= end {
            self.free.free(start, end - start + 1)?;
        }
        Ok(())
    }

    /// Zeroes data and makes its space available.
    pub(crate) fn free_space(&mut self, offset: usize, length: usize) -> Result<(), RomError> {
        self.bytes
            .get_mut(offset..offset + length)
            .ok_or_else(|| out_of_range(offset))?
            .fill(FREE_BYTE);
        self.free.free(offset, length)
    }

    fn take_in_bank(&mut self, length: usize, bank: usize) -> Option<usize> {
        self.free
            .find_and_unfree_within(bank * BANK, (bank + 1) * BANK - 1, length)
    }

    /// Replaces the `old_length` bytes the pointer at `pointer_offset` points to with
    /// `data`, in the pointer's bank or bank 0. Returns where the data went.
    pub(crate) fn rewrite_same_bank(
        &mut self,
        pointer_offset: usize,
        data: &[u8],
        old_length: usize,
    ) -> Result<usize, RomError> {
        let old = self.pointer(pointer_offset)?;
        self.free_space(old, old_length)?;
        self.repoint_same_bank(pointer_offset, data)
    }

    /// Writes `data` to free space in the bank of the pointer at `pointer_offset`, or bank 0, and points there; the
    /// old data stays, for data other pointers share. Returns where the data went.
    pub(crate) fn repoint_same_bank(&mut self, pointer_offset: usize, data: &[u8]) -> Result<usize, RomError> {
        let bank = bank_of(pointer_offset);
        let at = self
            .take_in_bank(data.len(), bank)
            .or_else(|| self.take_in_bank(data.len(), 0))
            .ok_or_else(|| {
                RomError::Malformed(format!(
                    "both bank {bank:#x} and bank 0 are full; can't find {} free bytes",
                    data.len()
                ))
            })?;
        self.write_pointer(pointer_offset, at)?;
        self.write(at, data)?;
        Ok(at)
    }

    /// Like [`rewrite_same_bank`](Self::rewrite_same_bank) for data whose bank
    /// is set by the bytes at `bank_offsets`; the data may move to any unreserved bank, its own last.
    pub(crate) fn rewrite_indirect_bank(
        &mut self,
        pointer_offset: usize,
        bank_offsets: &[usize],
        data: &[u8],
        old_length: usize,
    ) -> Result<usize, RomError> {
        let first = *bank_offsets
            .first()
            .ok_or_else(|| RomError::Malformed("no bank offsets".into()))?;
        let bank = usize::from(self.byte(first)?);
        for &b in bank_offsets {
            if usize::from(self.byte(b)?) != bank {
                return Err(RomError::Malformed(format!(
                    "bank byte at {b:#x} differs from {first:#x}"
                )));
            }
        }
        let old = self.pointer_in(pointer_offset, bank)?;
        self.free_space(old, old_length)?;
        let unreserved: Vec<usize> = (0..self.reserved.len()).filter(|&b| !self.reserved[b]).collect();
        let at = unreserved
            .into_iter()
            .find_map(|b| self.take_in_bank(data.len(), b))
            .or_else(|| self.take_in_bank(data.len(), bank))
            .ok_or_else(|| RomError::Malformed(format!("ROM full; can't find {} free bytes", data.len())))?;
        self.write_pointer(pointer_offset, at)?;
        self.write(at, data)?;
        let new_bank = u8::try_from(bank_of(at)).map_err(|_| out_of_range(at))?;
        for &b in bank_offsets {
            self.write_byte(b, new_bank)?;
        }
        Ok(at)
    }

    /// Free space of `length` bytes in the first unreserved bank that has it, taken.
    pub(crate) fn take_unreserved(&mut self, length: usize) -> Option<usize> {
        let free: Vec<usize> = (0..self.reserved.len()).filter(|&b| !self.reserved[b]).collect();
        free.into_iter().find_map(|b| self.take_in_bank(length, b))
    }

    /// Checks that the pointers at `secondary` (into `bank`) point to `old`, the data the pointer at `primary`
    /// pointed to, and points them to `new`.
    fn rewrite_secondary_pointers(
        &mut self,
        primary: usize,
        secondary: &[usize],
        bank: usize,
        old: usize,
        new: usize,
    ) -> Result<(), RomError> {
        for &p in secondary {
            if p != primary && self.pointer_in(p, bank)? != old {
                return Err(RomError::Malformed(format!(
                    "the pointer at {p:#x} doesn't point to {old:#x} like the one at {primary:#x}"
                )));
            }
        }
        for &p in secondary {
            self.write_pointer(p, new)?;
        }
        Ok(())
    }

    /// [`rewrite_indirect_bank`](Self::rewrite_indirect_bank) for data more pointers share: those at `secondary`
    /// follow the one at `pointer_offset`.
    pub(crate) fn rewrite_indirect_bank_shared(
        &mut self,
        pointer_offset: usize,
        secondary: &[usize],
        bank_offsets: &[usize],
        data: &[u8],
        old_length: usize,
    ) -> Result<usize, RomError> {
        let first = *bank_offsets
            .first()
            .ok_or_else(|| RomError::Malformed("no bank offsets".into()))?;
        let bank = usize::from(self.byte(first)?);
        let old = self.pointer_in(pointer_offset, bank)?;
        let at = self.rewrite_indirect_bank(pointer_offset, bank_offsets, data, old_length)?;
        self.rewrite_secondary_pointers(pointer_offset, secondary, bank, old, at)?;
        Ok(at)
    }

    /// Replaces the `old_length` bytes the pointer at `pointer_offset` points to in `bank` with `data`, in the same
    /// bank; the pointers at `secondary` follow. Returns where the data went.
    pub(crate) fn rewrite_specific_bank(
        &mut self,
        pointer_offset: usize,
        secondary: &[usize],
        bank: usize,
        data: &[u8],
        old_length: usize,
    ) -> Result<usize, RomError> {
        let old = self.pointer_in(pointer_offset, bank)?;
        self.free_space(old, old_length)?;
        let at = self.take_in_bank(data.len(), bank).ok_or_else(|| {
            RomError::Malformed(format!("bank {bank:#x} is full; can't find {} free bytes", data.len()))
        })?;
        self.write_pointer(pointer_offset, at)?;
        self.write(at, data)?;
        self.rewrite_secondary_pointers(pointer_offset, secondary, bank, old, at)?;
        Ok(at)
    }

    /// Bytes up to and including the `count`-th `terminator`.
    pub(crate) fn length_with_terminators(
        &self,
        offset: usize,
        terminator: u8,
        count: usize,
    ) -> Result<usize, RomError> {
        let mut seen = 0;
        let mut len = 0;
        while seen < count {
            if self.byte(offset + len)? == terminator {
                seen += 1;
            }
            len += 1;
        }
        Ok(len)
    }

    /// Where `needle` occurs, if exactly once.
    pub(crate) fn find_unique(&self, needle: &[u8]) -> Option<usize> {
        let mut found = self
            .bytes
            .windows(needle.len())
            .enumerate()
            .filter(|(_, w)| *w == needle);
        let first = found.next()?.0;
        found.next().is_none().then_some(first)
    }
}

/// An entry value the handler cannot do without.
pub(crate) fn need(entry: &RomEntry, key: &str) -> Result<usize, RomError> {
    entry
        .int(key)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no valid {key}", entry.name)))
}

/// An entry value that is 0 when absent.
pub(crate) fn optional(entry: &RomEntry, key: &str) -> usize {
    entry.int(key).and_then(|v| usize::try_from(v).ok()).unwrap_or_default()
}

/// Sets the editor's limits for in-game trade texts from the ROM entry. A text field of the trade table holds the
/// text and its terminator, and the randomizer's own limits (10 and 7 characters, as in the original) stay the most;
/// where the entry keeps the game's texts, the handler doesn't write them, so they can't change.
pub(crate) fn trade_text_rules(trades: &mut ppr_model::TradeRules, entry: &RomEntry) {
    trades.texts = optional(entry, "CanChangeTrainerText") > 0;
    let room = |key: &str, max: usize| optional(entry, key).saturating_sub(1).min(max);
    trades.max_nickname_length = room("TradeNameLength", trades.max_nickname_length);
    trades.max_ot_name_length = room("TradeOTLength", trades.max_ot_name_length);
}

/// The offsets of an entry array; empty when it has none.
pub(crate) fn offsets(entry: &RomEntry, key: &str) -> Vec<usize> {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|o| usize::try_from(o).ok())
        .collect()
}

/// A `TMText[]` value, `[number,offset,template]`, with text the ROM entries store encoded twice repaired.
pub(crate) fn tm_text(s: &str) -> Option<(usize, usize, String)> {
    let inner = s.trim().strip_prefix('[')?.strip_suffix(']')?;
    let mut parts = inner.splitn(3, ',');
    let number = usize::try_from(crate::entries::ini::parse_int(parts.next()?.trim())?).ok()?;
    let offset = usize::try_from(crate::entries::ini::parse_int(parts.next()?.trim())?).ok()?;
    Some((number, offset, text::repair_double_encoding(parts.next()?)))
}

/// Moves without power that aren't status moves.
pub(crate) const NO_POWER_NON_STATUS: [MoveId; 22] = [
    12, 32, 49, 67, 68, 69, 82, 90, 101, 117, 149, 162, 175, 216, 217, 218, 222, 243, 251, 255, 279, 329,
];
/// Moves that hit two to five times.
pub(crate) const MULTIHIT: [MoveId; 14] = [3, 4, 31, 42, 131, 140, 154, 198, 292, 331, 333, 350, 541, 594];
/// Moves that hit twice.
pub(crate) const DOUBLE_HIT: [MoveId; 8] = [24, 41, 155, 458, 530, 544, 742, 751];

/// The bank `offset` is in.
pub(crate) fn bank_of(offset: usize) -> usize {
    offset / BANK
}

/// The ROM offset of `pointer` in `bank`.
pub(crate) fn calculate_offset(pointer: u16, bank: usize) -> usize {
    let p = usize::from(pointer);
    if p < BANK { p } else { p % BANK + bank * BANK }
}

/// The two-byte pointer to `offset`.
pub(crate) fn gb_pointer(offset: usize) -> u16 {
    let p = if offset < BANK { offset } else { offset % BANK + BANK };
    // Below 0x8000 by construction.
    u16::try_from(p).unwrap_or_default()
}

/// `BankEndFreeSpaceMargin<bank>=margin` entries.
fn bank_end_margins(entry: &RomEntry) -> Vec<(usize, usize)> {
    entry_keys(entry, "BankEndFreeSpaceMargin<")
        .filter_map(|(key, value)| {
            let bank = crate::entries::ini::parse_int(key)?;
            let margin = crate::entries::ini::parse_int(value)?;
            Some((usize::try_from(bank).ok()?, usize::try_from(margin).ok()?))
        })
        .collect()
}

/// `UnusedChunk<name>=[offset, length]` entries; `null` removes an inherited one.
fn unused_chunks(entry: &RomEntry) -> Vec<(usize, usize)> {
    entry_keys(entry, "UnusedChunk<")
        .filter_map(|(_, value)| {
            let v = crate::entries::ini::parse_array(value)?;
            match v.as_slice() {
                [o, l] => Some((usize::try_from(*o).ok()?, usize::try_from(*l).ok()?)),
                _ => None,
            }
        })
        .collect()
}

/// The `prefix…>` keys of an entry, as (what is between the brackets, value).
fn entry_keys<'a>(entry: &'a RomEntry, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
    entry.keys_with_prefix(prefix).filter_map(move |(key, value)| {
        let inner = key.strip_prefix(prefix)?.strip_suffix('>')?;
        (value != "null").then_some((inner, value))
    })
}

/// What the real-ROM tests of Gen 1 and Gen 2 compare against the reference.
#[cfg(test)]
pub(crate) mod testing {
    use std::fmt::Write as _;

    use ppr_model::PlayerCharacter;

    /// The bytes that changed, a line per run: its offset and the new bytes, in hex.
    pub(crate) fn diff(before: &[u8], after: &[u8]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < before.len() {
            if before[i] == after[i] {
                i += 1;
                continue;
            }
            let start = i;
            while i < before.len() && before[i] != after[i] {
                i += 1;
            }
            let _ = write!(out, "{start:x} ");
            for b in &after[start..i] {
                let _ = write!(out, "{b:02x}");
            }
            out.push('\n');
        }
        if out.is_empty() {
            out.push('\n');
        }
        out
    }

    pub(crate) fn fnv1a(data: &str) -> u64 {
        data.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        })
    }

    /// The reference's diffs of player graphics packs in `PPR_REFERENCE_DUMPS/cpg`, named
    /// `<prefix>-<folder>-<entry>-<character>.diff` (the folder's entries for the game counted from 0): (pack id,
    /// character, diff).
    pub(crate) fn reference_player_graphics(prefix: &str) -> Vec<(String, PlayerCharacter, String)> {
        let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") else {
            return Vec::new();
        };
        let Ok(files) = std::fs::read_dir(std::path::PathBuf::from(dir).join("cpg")) else {
            return Vec::new();
        };
        let mut diffs: Vec<_> = files
            .filter_map(Result::ok)
            .filter_map(|f| {
                let name = f.file_name().to_string_lossy().into_owned();
                let rest = name.strip_prefix(prefix)?.strip_prefix('-')?.strip_suffix(".diff")?;
                let mut parts = rest.rsplitn(3, '-');
                let pc = match parts.next()? {
                    "1" => PlayerCharacter::Pc1,
                    _ => PlayerCharacter::Pc2,
                };
                let n: usize = parts.next()?.parse().ok()?;
                let folder = parts.next()?;
                let id = if n == 0 {
                    folder.to_owned()
                } else {
                    format!("{folder}/{}", n + 1)
                };
                Some((id, pc, std::fs::read_to_string(f.path()).ok()?))
            })
            .collect();
        diffs.sort_by(|a, b| (&a.0, a.1 == PlayerCharacter::Pc2).cmp(&(&b.0, b.1 == PlayerCharacter::Pc2)));
        diffs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointers_reach_into_their_bank_or_bank_zero() {
        assert_eq!(calculate_offset(0x1234, 5), 0x1234);
        assert_eq!(calculate_offset(0x4567, 5), 5 * BANK + 0x567);
        assert_eq!(gb_pointer(0x1234), 0x1234);
        assert_eq!(gb_pointer(5 * BANK + 0x567), 0x4567);
        assert_eq!(bank_of(5 * BANK + 0x567), 5);
    }

    /// The trade text limits of a ROM entry: (texts can change, nickname, original trainer name).
    fn trade_texts(generation: u8, name: &str) -> (bool, usize, usize) {
        let g = ppr_model::Generation::new(generation).expect("1..=7");
        let entry = crate::entries::catalog(g)
            .entries
            .iter()
            .find(|e| e.name == name)
            .expect(name);
        let mut t = ppr_model::TradeRules {
            texts: true,
            max_nickname_length: 10,
            max_ot_name_length: if generation == 1 { 0 } else { 7 },
            original_trainer: generation == 2,
            max_ot_id: if generation == 1 { 0 } else { 65_535 },
            held_item: generation == 2,
            ivs: 0,
            max_iv: 15,
            random_iv: None,
        };
        trade_text_rules(&mut t, entry);
        (t.texts, t.max_nickname_length, t.max_ot_name_length)
    }

    #[test]
    fn trade_text_limits_come_from_the_rom_entry() {
        // Fields of 11 characters: the randomizer's limits of 10 and 7.
        assert_eq!(trade_texts(1, "Yellow (U)"), (true, 10, 0));
        assert_eq!(trade_texts(2, "Crystal (U)"), (true, 10, 7));
        // The translated Green has the Japanese table, with room for 4 characters and the terminator.
        assert_eq!(trade_texts(1, "Green (J)(T-Eng)"), (true, 4, 0));
        // The Japanese games keep their texts.
        assert_eq!(trade_texts(1, "Red (J)"), (false, 4, 0));
        assert_eq!(trade_texts(1, "Blue (J)"), (false, 3, 0));
        assert_eq!(trade_texts(2, "Crystal (J)"), (false, 3, 2));
    }
}
