//! TMs, HMs and move tutors: the moves (tables in the executable), which species learn them (bit fields in the
//! personal data) and what the TM items say and look like.
//!
//! The TM table has TM01 to TM92, the HMs, then TM93 to TM100 a little further on. Omega Ruby and Alpha Sapphire
//! keep Rock Smash and Dive as HMs 6 and 7 at their own places. The move tutors of Omega Ruby and Alpha Sapphire are
//! a list ended by 0xFFFF in which 0x26E marks the end of each tutor, and a table of the tutor shops.
//!
//! Writing a TM also gives its item the move's description and the item palette of the move's type.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, Move, MoveId};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom, find, find_after};
use crate::RomError;

/// Ends a tutor in the tutor list.
const TUTOR_SEPARATOR: u16 = 0x26E;
const TUTOR_LIST_END: u16 = 0xFFFF;
/// The distance between two moves of the tutor shop table.
const TUTOR_SHOP_STRIDE: usize = 8;
/// The bytes of TM and HM flags.
const TM_HM_BYTES: usize = 14;

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

fn tm_table(code: &[u8]) -> Result<usize, RomError> {
    find_after(code, c::TM_DATA_PREFIX).ok_or_else(|| malformed("no TM table"))
}

/// Where TM `i` (from 0) is.
fn tm_offset(table: usize, family: Family, i: usize) -> usize {
    if i < c::TM_BLOCK_ONE {
        table + i * 2
    } else {
        table + family.tm_block_two_start() * 2 + (i - c::TM_BLOCK_ONE) * 2
    }
}

/// TM01 to TM100.
pub(crate) fn read_tms(code: &[u8], family: Family) -> Result<Vec<MoveId>, RomError> {
    let table = tm_table(code)?;
    (0..c::TM_COUNT)
        .map(|i| code.u16_at(tm_offset(table, family, i)))
        .collect()
}

/// The HMs: five, and Rock Smash and Dive in Omega Ruby and Alpha Sapphire.
pub(crate) fn read_hms(code: &[u8], family: Family) -> Result<Vec<MoveId>, RomError> {
    let at = tm_table(code)? + c::TM_BLOCK_ONE * 2;
    let mut hms = (0..c::HM_BLOCK_ONE)
        .map(|i| code.u16_at(at + i * 2))
        .collect::<Result<Vec<_>, _>>()?;
    if family == Family::Oras {
        hms.push(code.u16_at(at + c::ROCK_SMASH_OFFSET_ORAS)?);
        hms.push(code.u16_at(at + c::DIVE_OFFSET_ORAS)?);
    }
    Ok(hms)
}

/// The item of TM `i` (from 0).
fn tm_item(i: usize) -> usize {
    let first = if i < c::TM_BLOCK_ONE {
        usize::from(c::TM01)
    } else if i < c::TM_BLOCK_ONE + c::TM_BLOCK_TWO {
        usize::from(c::TM93) - c::TM_BLOCK_ONE
    } else {
        usize::from(c::TM96) - c::TM_BLOCK_ONE - c::TM_BLOCK_TWO
    };
    first + i
}

/// The text files the TM items use.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TmTexts<'a> {
    pub(crate) path: &'a str,
    pub(crate) item_descriptions: usize,
    pub(crate) move_descriptions: usize,
}

/// Writes the TMs, with their item descriptions and palettes.
pub(crate) fn write_tms(
    rom: &mut Rom,
    family: Family,
    tms: &[MoveId],
    moves: &[Move],
    texts: TmTexts<'_>,
) -> Result<(), RomError> {
    if tms.len() != c::TM_COUNT {
        return Err(malformed(format!("{} TMs, not {}", tms.len(), c::TM_COUNT)));
    }
    let table = tm_table(rom.code())?;
    for (i, &mv) in tms.iter().enumerate() {
        let at = tm_offset(table, family, i);
        if rom.code().u16_at(at)? != mv {
            rom.code_mut().set_u16(at, mv)?;
        }
    }

    let mut items = rom.texts(texts.path, texts.item_descriptions)?;
    let descriptions = rom.texts(texts.path, texts.move_descriptions)?;
    for (i, &mv) in tms.iter().enumerate() {
        let text = descriptions
            .get(usize::from(mv))
            .ok_or_else(|| malformed(format!("no description of move {mv}")))?;
        let item = items
            .get_mut(tm_item(i))
            .ok_or_else(|| malformed(format!("no description of item {}", tm_item(i))))?;
        text.clone_into(item);
    }
    rom.set_texts(texts.path, texts.item_descriptions, &items)?;

    if let Some(palettes) = find_after(rom.code(), c::ITEM_PALETTES_PREFIX) {
        for (i, &mv) in tms.iter().enumerate() {
            let move_type = moves.iter().find(|m| m.number == mv).and_then(|m| m.move_type);
            let at = palettes + tm_item(i) * 4;
            let palette = c::tm_palette(move_type);
            if rom.code().u16_at(at)? != palette {
                rom.code_mut().set_u16(at, palette)?;
            }
        }
    }
    Ok(())
}

/// The move tutor moves of Omega Ruby and Alpha Sapphire, every tutor's in turn; none in X and Y.
pub(crate) fn read_tutors(code: &[u8], family: Family) -> Result<Vec<MoveId>, RomError> {
    if family != Family::Oras {
        return Ok(Vec::new());
    }
    let Some(mut at) = find(code, c::TUTORS_LOCATOR) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    loop {
        let value = code.u16_at(at)?;
        at += 2;
        match value {
            TUTOR_LIST_END => return Ok(out),
            TUTOR_SEPARATOR => {}
            mv => out.push(mv),
        }
    }
}

/// Writes the move tutor moves: the first ones into the tutor shops, all of them into the list.
pub(crate) fn write_tutors(rom: &mut Rom, moves: &[MoveId]) -> Result<(), RomError> {
    if let Some(at) = find_after(rom.code(), c::TUTORS_SHOP_PREFIX) {
        for (i, &mv) in moves.iter().take(c::TUTOR_MOVE_COUNT).enumerate() {
            if rom.code().u16_at(at + i * TUTOR_SHOP_STRIDE)? != mv {
                rom.code_mut().set_u16(at + i * TUTOR_SHOP_STRIDE, mv)?;
            }
        }
    }
    if let Some(mut at) = find(rom.code(), c::TUTORS_LOCATOR) {
        for &mv in moves {
            if rom.code().u16_at(at)? == TUTOR_SEPARATOR {
                at += 2;
            }
            if rom.code().u16_at(at)? != mv {
                rom.code_mut().set_u16(at, mv)?;
            }
            at += 2;
        }
    }
    Ok(())
}

fn flag(data: &[u8], at: usize, bit: usize) -> Result<bool, RomError> {
    Ok(data.u8_at(at + bit / 8)? >> (bit % 8) & 1 == 1)
}

fn set_flag(data: &mut [u8], at: usize, bit: usize, on: bool) -> Result<(), RomError> {
    let byte = data.u8_at(at + bit / 8)?;
    let mask = 1 << (bit % 8);
    data.set_u8(at + bit / 8, if on { byte | mask } else { byte & !mask })
}

/// TM01 to TM100, then the HMs.
pub(crate) fn read_tm_compat(data: &[u8], family: Family) -> Result<Vec<bool>, RomError> {
    let count = c::TM_COUNT + family.hm_count();
    debug_assert!(count <= TM_HM_BYTES * 8);
    (0..count).map(|bit| flag(data, c::BS_TM_COMPAT, bit)).collect()
}

/// Writes the TM and HM flags the game data holds; other bits stay.
pub(crate) fn write_tm_compat(data: &mut [u8], flags: &[bool]) -> Result<(), RomError> {
    for (bit, &on) in flags.iter().enumerate().take(TM_HM_BYTES * 8) {
        set_flag(data, c::BS_TM_COMPAT, bit, on)?;
    }
    Ok(())
}

/// The bit of each tutor move: each tutor has a 32-bit field of its own.
fn tutor_bits() -> impl Iterator<Item = (usize, usize)> {
    c::TUTOR_SIZES
        .iter()
        .enumerate()
        .flat_map(|(tutor, &size)| (0..size).map(move |bit| (c::BS_TUTOR_COMPAT + tutor * 4, bit)))
}

/// The move tutor flags of Omega Ruby and Alpha Sapphire, every tutor's in turn.
pub(crate) fn read_tutor_compat(data: &[u8]) -> Result<Vec<bool>, RomError> {
    tutor_bits().map(|(at, bit)| flag(data, at, bit)).collect()
}

/// Writes the move tutor flags the game data holds; other bits stay.
pub(crate) fn write_tutor_compat(data: &mut [u8], flags: &[bool]) -> Result<(), RomError> {
    for ((at, bit), &on) in tutor_bits().zip(flags) {
        set_flag(data, at, bit, on)?;
    }
    Ok(())
}

/// Flags by species.
pub(crate) type Compat = BTreeMap<DexNumber, Vec<bool>>;

/// The TM/HM and tutor compatibility of every species, read from its personal data.
pub(crate) fn read_all_compat(rom: &mut Rom, path: &str, family: Family) -> Result<(Compat, Compat), RomError> {
    let mut tms = BTreeMap::new();
    let mut tutors = BTreeMap::new();
    for n in 1..=super::species::entry_count(family) {
        let data = rom.garc_file(path, false, n)?;
        let number = DexNumber::try_from(n).map_err(|_| malformed("species number"))?;
        tms.insert(number, read_tm_compat(&data, family)?);
        if family == Family::Oras {
            tutors.insert(number, read_tutor_compat(&data)?);
        }
    }
    Ok((tms, tutors))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tm_items_follow_the_three_blocks() {
        assert_eq!(tm_item(0), 328);
        assert_eq!(tm_item(91), 419);
        assert_eq!(tm_item(92), 618);
        assert_eq!(tm_item(94), 620);
        assert_eq!(tm_item(95), 690);
        assert_eq!(tm_item(99), 694);
    }

    #[test]
    fn tm_offsets_skip_the_hms() {
        assert_eq!(tm_offset(100, Family::Xy, 91), 100 + 182);
        assert_eq!(tm_offset(100, Family::Xy, 92), 100 + 194);
        assert_eq!(tm_offset(100, Family::Oras, 92), 100 + 196);
    }

    #[test]
    fn tutor_flags_are_per_tutor_fields() {
        let mut data = vec![0u8; 0x50];
        let mut flags = vec![false; 63];
        flags[0] = true;
        flags[15] = true;
        flags[62] = true;
        write_tutor_compat(&mut data, &flags).expect("fits");
        assert_eq!(data[64], 1);
        assert_eq!(data[68], 1);
        assert_eq!(data[76 + 1], 0x40);
        assert_eq!(read_tutor_compat(&data).expect("fits"), flags);
    }

    #[test]
    fn tutor_list_skips_separators() {
        let mut code = vec![0u8; 4];
        code.extend(crate::gba::hex(c::TUTORS_LOCATOR).expect("hex"));
        code.extend([0x6E, 0x02, 0x05, 0x00, 0xFF, 0xFF]);
        let moves = read_tutors(&code, Family::Oras).expect("reads");
        assert_eq!(moves, [0x1C2, 0x157, 0xA2, 0x212, 0x144, 0x1BA, 5]);
    }
}
