//! The moves of the TMs and HMs (a table in the ARM9), with the TM items' descriptions and palettes, and the move
//! tutors of Black 2 and White 2 (an overlay).

use ppr_model::{Move, MoveId, Type};

use super::entry::{Family, need};
use super::store::{Store, Text, find, find_after, put_word, word};
use crate::RomError;
use crate::entries::RomEntry;

const TM_DATA_PREFIX: &str = "87038803";
/// TM01 to TM92, then the six HMs, then TM93 to TM95.
const TM_BLOCK_ONE: usize = 92;
const HM_COUNT: usize = 6;
const TM_COUNT: usize = 95;
const TM01_ITEM: usize = 328;
const TM93_ITEM: usize = 618;

const BW1_ITEM_PALETTES_PREFIX: &str = "E903EA03020003000400050006000700";
const BW2_ITEM_PALETTES_PREFIX: &str = "FD03FE03020003000400050006000700";

const TUTOR_COUNT: usize = 60;
const TUTOR_ENTRY_SIZE: usize = 12;

fn table(store: &mut Store) -> Result<usize, RomError> {
    find_after(store.arm9()?, TM_DATA_PREFIX)?
        .ok_or_else(|| RomError::Malformed("the TM table is not in the ARM9".into()))
}

/// The item number of TM `i` (from 0).
fn tm_item(i: usize) -> usize {
    if i < TM_BLOCK_ONE {
        TM01_ITEM + i
    } else {
        TM93_ITEM + i - TM_BLOCK_ONE
    }
}

/// Where the move of TM `i` (from 0) is.
fn tm_at(table: usize, i: usize) -> usize {
    if i < TM_BLOCK_ONE {
        table + i * 2
    } else {
        table + (i + HM_COUNT) * 2
    }
}

/// The TM moves and the HM moves.
pub(super) fn read(store: &mut Store) -> Result<(Vec<MoveId>, Vec<MoveId>), RomError> {
    let at = table(store)?;
    let arm9 = store.arm9()?;
    let tms = (0..TM_COUNT)
        .map(|i| word(arm9, tm_at(at, i)))
        .collect::<Result<_, _>>()?;
    let hms = (0..HM_COUNT)
        .map(|i| word(arm9, at + (TM_BLOCK_ONE + i) * 2))
        .collect::<Result<_, _>>()?;
    Ok((tms, hms))
}

/// The palette of a TM of a move type.
fn palette(t: Option<Type>) -> u16 {
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
        _ => 411,
    }
}

/// Writes the TM moves, their items' descriptions and palettes.
pub(super) fn write(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    moves: &[Move],
    tms: &[MoveId],
) -> Result<(), RomError> {
    let at = table(store)?;
    let arm9 = store.arm9_mut()?;
    for (i, &m) in tms.iter().take(TM_COUNT).enumerate() {
        put_word(arm9, tm_at(at, i), m)?;
    }
    let move_descriptions = store
        .strings(Text::Strings, need(entry, "MoveDescriptionsTextOffset")?)?
        .to_vec();
    let items = store.strings_mut(Text::Strings, need(entry, "ItemDescriptionsTextOffset")?)?;
    for (i, &m) in tms.iter().take(TM_COUNT).enumerate() {
        if let (Some(line), Some(description)) = (items.get_mut(tm_item(i)), move_descriptions.get(usize::from(m))) {
            line.clone_from(description);
        }
    }
    let prefix = family.pick(BW1_ITEM_PALETTES_PREFIX, BW2_ITEM_PALETTES_PREFIX);
    let arm9 = store.arm9_mut()?;
    if let Some(palettes) = find(arm9, prefix)? {
        for (i, &m) in tms.iter().take(TM_COUNT).enumerate() {
            let move_type = moves.iter().find(|mv| mv.number == m).and_then(|mv| mv.move_type);
            put_word(arm9, palettes + tm_item(i) * 4 + 2, palette(move_type))?;
        }
    }
    Ok(())
}

/// The move tutor moves; none in Black and White.
pub(super) fn read_tutors(store: &mut Store, entry: &RomEntry, family: Family) -> Result<Vec<MoveId>, RomError> {
    if family == Family::Bw1 {
        return Ok(Vec::new());
    }
    let base = need(entry, "MoveTutorDataOffset")?;
    let overlay = store.overlay(need(entry, "MoveTutorOvlNumber")?)?;
    (0..TUTOR_COUNT)
        .map(|i| word(overlay, base + i * TUTOR_ENTRY_SIZE))
        .collect()
}

/// Writes the move tutor moves.
pub(super) fn write_tutors(store: &mut Store, entry: &RomEntry, moves: &[MoveId]) -> Result<(), RomError> {
    if moves.len() != TUTOR_COUNT {
        return Ok(());
    }
    let base = need(entry, "MoveTutorDataOffset")?;
    let overlay = store.overlay_mut(need(entry, "MoveTutorOvlNumber")?)?;
    for (i, &m) in moves.iter().enumerate() {
        put_word(overlay, base + i * TUTOR_ENTRY_SIZE, m)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tms_skip_the_hms_in_the_table() {
        assert_eq!((tm_at(100, 0), tm_at(100, 91), tm_at(100, 92)), (100, 282, 296));
        assert_eq!(
            (tm_item(0), tm_item(91), tm_item(92), tm_item(94)),
            (328, 419, 618, 620)
        );
        assert_eq!(
            (palette(Some(Type::Bug)), palette(Some(Type::Ghost)), palette(None)),
            (610, 411, 411)
        );
    }
}
