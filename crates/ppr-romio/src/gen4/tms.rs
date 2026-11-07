//! TMs and HMs, move tutors, and who can learn them; with what the game says about the TMs' and tutors' moves.
//!
//! The TM and HM moves are words in the ARM9 binary. Writing the TMs also rewrites their item descriptions (the
//! move's description, reflowed), their item colours (by the move's type), the texts that name their moves, the
//! Game Corner and Battle Frontier prize lists and the Battle Frontier tutor scripts. Tutor moves are in the field
//! overlay; Diamond and Pearl have none. In `HeartGold` and `SoulSilver` the last tutor is the Headbutt tutor of
//! Ilex Forest, whose script names the move itself.
//!
//! Deliberate choices:
//! - Only the TMs that changed get their texts rewritten, so the others keep their line breaks; a text's old move
//!   name is the name as loaded, which is the one the text holds.
//! - When the tutor compatibility's last byte is only partly used, its unused bits keep their value, and only
//!   those; all the tutors' bits are written.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, GameData, Move, MoveId, Type};

use super::Rom;
use super::constants::{self as c, Family};
use super::files::{find, find_after, put_word, word};
use crate::RomError;
use crate::textfmt::reflow;

/// The item palette of each type's TMs; Curse's "???" type has the Ghost one.
fn tm_palette(t: Option<Type>) -> u16 {
    match t {
        Some(Type::Fighting) => 398,
        Some(Type::Dragon) => 399,
        Some(Type::Water) => 400,
        Some(Type::Psychic) => 401,
        Some(Type::Normal) => 402,
        Some(Type::Poison) => 403,
        Some(Type::Ice) => 404,
        Some(Type::Grass) => 405,
        Some(Type::Fire) => 406,
        Some(Type::Dark) => 407,
        Some(Type::Steel) => 408,
        Some(Type::Electric) => 409,
        Some(Type::Ground) => 410,
        Some(Type::Rock) => 412,
        Some(Type::Flying) => 413,
        Some(Type::Bug) => 610,
        Some(Type::Ghost | Type::Fairy) | None => 411,
    }
}

/// The script file and offsets of the Ilex Forest Headbutt tutor, and the text file of Ilex Forest.
const ILEX_FOREST_SCRIPT_FILE: usize = 92;
const ILEX_FOREST_STRINGS_FILE: usize = 115;
const HEADBUTT_TUTOR_SCRIPT_OFFSETS: [usize; 4] = [0xF55, 0xFC5, 0x100A, 0x104C];

/// Where the TM moves start in the ARM9, the HM moves following.
fn tm_data(rom: &mut Rom) -> Option<usize> {
    let prefix = if rom.family().is_dppt() {
        c::DPPT_TM_DATA_PREFIX
    } else {
        c::HGSS_TM_DATA_PREFIX
    };
    find_after(rom.files.arm9(), prefix)
}

/// The TM moves and the HM moves.
pub(super) fn read(rom: &mut Rom) -> Result<(Vec<MoveId>, Vec<MoveId>), RomError> {
    let at = tm_data(rom).ok_or_else(|| RomError::Malformed("no TM data in the ARM9".into()))?;
    let arm9 = rom.files.arm9_ref();
    let moves = (0..c::TM_COUNT + c::HM_COUNT)
        .map(|i| word(arm9, at + i * 2))
        .collect::<Result<Vec<_>, _>>()?;
    let (tms, hms) = moves.split_at(c::TM_COUNT);
    Ok((tms.to_vec(), hms.to_vec()))
}

/// Which TMs and HMs each species and alternate forme can learn: 13 bytes of flags in its stats file.
pub(super) fn read_compat(rom: &mut Rom) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    rom.species_numbers()
        .map(|number| {
            let i = usize::from(number);
            let stats = rom.stats(number)?;
            let bytes = stats
                .get(c::BS_TM_HM_COMPAT..c::BS_TM_HM_COMPAT + c::TM_HM_COMPAT_BYTES)
                .ok_or_else(|| RomError::Malformed(format!("species {i} has no TM compatibility")))?;
            Ok((number, flags(bytes, c::TM_COUNT + c::HM_COUNT)))
        })
        .collect()
}

/// The first `count` bits of `bytes`, lowest bit first.
fn flags(bytes: &[u8], count: usize) -> Vec<bool> {
    (0..count).map(|i| bytes[i / 8] & (1 << (i % 8)) != 0).collect()
}

/// `flags` as bytes, lowest bit first.
fn flag_bytes(flags: &[bool], len: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; len];
    for (i, _) in flags.iter().enumerate().filter(|&(_, &f)| f).take(len * 8) {
        bytes[i / 8] |= 1 << (i % 8);
    }
    bytes
}

/// The move tutor moves; none in Diamond and Pearl.
pub(super) fn read_tutors(rom: &mut Rom) -> Result<Vec<MoveId>, RomError> {
    if rom.family() == Family::DiamondPearl {
        return Ok(Vec::new());
    }
    let at = rom.entry.need("MoveTutorMovesOffset")?;
    let count = rom.entry.need("MoveTutorCount")?;
    let size = rom.entry.need("MoveTutorBytesCount")?;
    let overlay = rom.files.overlay(rom.entry.need("FieldOvlNumber")?)?;
    (0..count).map(|i| word(overlay, at + i * size)).collect()
}

/// The data that holds the tutor compatibility: a file in `HeartGold` and `SoulSilver`, an overlay otherwise.
fn tutor_compat_data(rom: &mut Rom) -> Result<&mut Vec<u8>, RomError> {
    if rom.family().is_dppt() {
        let number = rom.entry.need("MoveTutorCompatOvlNumber")?;
        rom.files.overlay(number)
    } else {
        let path = rom.entry.file("MoveTutorCompat")?;
        rom.files.file(&path)
    }
}

/// Which tutor moves each species and alternate forme can learn; the formes' flags follow the species'.
pub(super) fn read_tutor_compat(rom: &mut Rom, count: usize) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    if rom.family() == Family::DiamondPearl {
        return Ok(BTreeMap::new());
    }
    let base = rom.entry.need("MoveTutorCompatOffset")?;
    let size = rom.entry.need("MoveTutorCompatBytesCount")?;
    let numbers = rom.species_numbers();
    let data = tutor_compat_data(rom)?;
    numbers
        .map(|number| {
            let i = usize::from(number);
            let at = base + (i - 1) * size;
            let bytes = data
                .get(at..at + size)
                .ok_or_else(|| RomError::Malformed(format!("species {i} has no tutor compatibility")))?;
            Ok((number, flags(bytes, count.min(size * 8))))
        })
        .collect()
}

/// Writes what changed of the TMs, tutors and compatibility.
pub(super) fn write(rom: &mut Rom, loaded: &GameData, data: &GameData) -> Result<(), RomError> {
    if data.tm_moves != loaded.tm_moves {
        write_tms(rom, &loaded.tm_moves, &data.tm_moves, &loaded.moves, &data.moves)?;
    }
    if data.tutor_moves != loaded.tutor_moves {
        write_tutors(rom, &data.tutor_moves, &loaded.moves, &data.moves)?;
    }
    if data.tm_hm_compat != loaded.tm_hm_compat {
        for (&number, f) in &data.tm_hm_compat {
            if loaded.tm_hm_compat.get(&number) == Some(f) {
                continue;
            }
            let bytes = flag_bytes(f, c::TM_HM_COMPAT_BYTES);
            let stats = rom.stats(number)?;
            super::files::put(stats, c::BS_TM_HM_COMPAT, &bytes)?;
        }
    }
    if data.tutor_compat != loaded.tutor_compat {
        write_tutor_compat(rom, &loaded.tutor_compat, &data.tutor_compat, data.tutor_moves.len())?;
    }
    Ok(())
}

fn move_named(moves: &[Move], number: MoveId) -> Result<&Move, RomError> {
    moves
        .iter()
        .find(|m| m.number == number)
        .ok_or_else(|| RomError::Malformed(format!("no move {number}")))
}

/// The TM moves, with their item descriptions and colours, the texts naming them and the prize lists.
fn write_tms(rom: &mut Rom, old: &[MoveId], tms: &[MoveId], loaded: &[Move], moves: &[Move]) -> Result<(), RomError> {
    if tms.len() != c::TM_COUNT {
        return Err(RomError::Malformed(format!("Gen 4 has {} TMs", c::TM_COUNT)));
    }
    let Some(at) = tm_data(rom) else {
        return Ok(());
    };
    for (i, &m) in tms.iter().enumerate() {
        put_word(rom.files.arm9(), at + i * 2, m)?;
    }

    // The item descriptions, from the move descriptions, which have five lines.
    let descriptions_index = rom.entry.need("ItemDescriptionsTextOffset")?;
    let mut descriptions = rom.files.strings(descriptions_index)?;
    let move_descriptions = rom.strings_of("MoveDescriptionsTextOffset")?;
    let line = rom.chars_per_line();
    for (i, &m) in tms.iter().enumerate() {
        let index = usize::from(c::TM01) + i;
        if let (Some(d), Some(source)) = (descriptions.get_mut(index), move_descriptions.get(usize::from(m))) {
            *d = reflow(source, line, |w| w.chars().count());
        }
    }
    rom.files.set_strings(descriptions_index, descriptions)?;

    // The item colours; not finding them is no great loss.
    let palettes = if rom.family() == Family::DiamondPearl {
        c::DP_ITEM_PALETTES_PREFIX
    } else {
        c::PTHGSS_ITEM_PALETTES_PREFIX
    };
    if let Some(p) = find(rom.files.arm9_ref(), palettes) {
        for (i, &m) in tms.iter().enumerate() {
            let palette = tm_palette(move_named(moves, m)?.move_type);
            put_word(rom.files.arm9(), p + i * 8 + 2, palette)?;
        }
    }

    let line_length = rom.chars_per_line();
    for (i, (&before, &after)) in old.iter().zip(tms).enumerate().filter(|(_, (b, a))| b != a) {
        let tm = i + 1;
        if let Some(texts) = rom.entry.tm_texts.get(&tm).cloned() {
            let mut from = move_named(loaded, before)?.name.clone();
            let mut to = move_named(moves, after)?.name.clone();
            if rom.family() == Family::HeartGoldSoulSilver && before == c::ROAR {
                // A text shouts Roar's name.
                from = from.to_uppercase();
                to = to.to_uppercase();
            }
            let replacements = BTreeMap::from([(from, to)]);
            let text_files: BTreeSet<usize> = texts.iter().map(|t| t.file).collect();
            for file in text_files {
                rom.files.replace_names(file, &replacements, line_length)?;
            }
        }
        let name = &move_named(moves, after)?.name;
        if let Some(text) = rom.entry.tm_texts_game_corner.get(&tm).copied() {
            set_prize_name(rom, text.file, text.string, name)?;
        }
        if let Some(&offset) = rom.entry.frontier_tm_scripts.get(&tm) {
            let path = rom.entry.file("Scripts")?;
            let file = rom.entry.need("FrontierScriptNumber")?;
            put_word(rom.files.narc_file(&path, file)?, offset, after)?;
        }
        if let Some(&string) = rom.entry.frontier_tm_texts.get(&tm) {
            let file = rom.entry.need("MiscUITextOffset")?;
            set_prize_name(rom, file, string, name)?;
        }
    }
    Ok(())
}

/// A prize list entry: the move's name up to the first control code, in capitals when the old one was.
fn set_prize_name(rom: &mut Rom, file: usize, string: usize, name: &str) -> Result<(), RomError> {
    let mut strings = rom.files.strings(file)?;
    let Some(s) = strings.get_mut(string) else {
        return Ok(());
    };
    let Some(end) = s.find('\\') else {
        return Ok(());
    };
    let old = &s[..end];
    let name = if old == old.to_uppercase() {
        name.to_uppercase()
    } else {
        name.to_owned()
    };
    *s = format!("{name}{}", &s[end..]);
    rom.files.set_strings(file, strings)
}

/// The tutor moves; in `HeartGold` and `SoulSilver` the Ilex Forest script and texts follow the last one.
fn write_tutors(rom: &mut Rom, tutors: &[MoveId], loaded: &[Move], moves: &[Move]) -> Result<(), RomError> {
    if rom.family() == Family::DiamondPearl {
        return Ok(());
    }
    let at = rom.entry.need("MoveTutorMovesOffset")?;
    let count = rom.entry.need("MoveTutorCount")?;
    let size = rom.entry.need("MoveTutorBytesCount")?;
    if tutors.len() != count {
        return Err(RomError::Malformed(format!("this game has {count} tutor moves")));
    }
    let overlay = rom.files.overlay(rom.entry.need("FieldOvlNumber")?)?;
    for (i, &m) in tutors.iter().enumerate() {
        put_word(overlay, at + i * size, m)?;
    }
    if rom.family() == Family::HeartGoldSoulSilver
        && let Some(&last) = tutors.last()
    {
        let path = rom.entry.file("Scripts")?;
        let script = rom.files.narc_file(&path, ILEX_FOREST_SCRIPT_FILE)?;
        for offset in HEADBUTT_TUTOR_SCRIPT_OFFSETS {
            put_word(script, offset, last)?;
        }
        let replacements = BTreeMap::from([(
            move_named(loaded, c::HEADBUTT)?.name.clone(),
            move_named(moves, last)?.name.clone(),
        )]);
        let line_length = rom.chars_per_line();
        rom.files
            .replace_names(ILEX_FOREST_STRINGS_FILE, &replacements, line_length)?;
    }
    Ok(())
}

/// The tutor compatibility of the species whose compatibility changed.
fn write_tutor_compat(
    rom: &mut Rom,
    loaded: &BTreeMap<DexNumber, Vec<bool>>,
    compat: &BTreeMap<DexNumber, Vec<bool>>,
    count: usize,
) -> Result<(), RomError> {
    if rom.family() == Family::DiamondPearl {
        return Ok(());
    }
    let base = rom.entry.need("MoveTutorCompatOffset")?;
    let size = rom.entry.need("MoveTutorCompatBytesCount")?;
    let numbers = rom.species_numbers();
    let data = tutor_compat_data(rom)?;
    for (&number, f) in compat {
        if loaded.get(&number) == Some(f) || !numbers.contains(&number) {
            continue;
        }
        let new = flag_bytes(&f[..f.len().min(count)], size);
        let at = base + (usize::from(number) - 1) * size;
        for (j, &b) in new.iter().enumerate() {
            let used = count.saturating_sub(j * 8).min(8);
            if used == 0 {
                break;
            }
            let keep = !(u8::MAX >> (8 - used));
            let old = data
                .get_mut(at + j)
                .ok_or_else(|| RomError::Malformed("tutor compatibility out of range".into()))?;
            *old = (*old & keep) | b;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_round_trip() {
        let f = flags(&[0b0000_0101, 0b1000_0000], 16);
        assert!(f[0] && !f[1] && f[2] && f[15]);
        assert_eq!(flag_bytes(&f, 2), [0b0000_0101, 0b1000_0000]);
    }

    #[test]
    fn palettes_by_type() {
        assert_eq!(tm_palette(None), 411);
        assert_eq!(tm_palette(Some(Type::Bug)), 610);
    }
}
