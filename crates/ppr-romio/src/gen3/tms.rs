//! TM and move tutor moves, read and written. Writing TMs also gives the TM items their move's type colour and
//! description. Both rewrite what people say about the moves.

use ppr_model::{Move, MoveId};

use super::constants as c;
use super::space::Space;
use super::{Family, items, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;
use crate::textfmt::reflow;

/// The number of TMs.
pub(super) const TM_COUNT: usize = 50;
/// The standard item number of TM01.
const TM01: u16 = 328;
/// Where the pointer to an item's description is in its `ItemData` entry.
const ITEM_DESCRIPTION: usize = 0x14;
/// Characters per line of an item description in Ruby, Sapphire and Emerald.
const RSE_ITEM_DESCRIPTION_LINE: usize = 18;

/// The TM moves.
pub(super) fn read(rom: &GbaRom, entry: &RomEntry) -> Result<Vec<MoveId>, RomError> {
    let at = need(entry, "TmMoves")?;
    (0..TM_COUNT).map(|i| rom.u16(at + i * 2)).collect()
}

/// Writes the TM moves: the table, and Emerald's and `FireRed`'s copy of it.
pub(super) fn write_moves(rom: &mut GbaRom, entry: &RomEntry, tms: &[MoveId]) -> Result<(), RomError> {
    let at = need(entry, "TmMoves")?;
    for (i, &m) in tms.iter().enumerate() {
        rom.write_u16(at + i * 2, m)?;
    }
    if let Some(copy) = entry
        .int("TmMovesDuplicate")
        .and_then(|o| usize::try_from(o).ok())
        .filter(|&o| o > 0)
    {
        let table = rom.slice(at, TM_COUNT * 2)?.to_vec();
        rom.write(copy, &table)?;
    }
    Ok(())
}

/// The internal item number of TM `i` (from 0).
fn item(i: usize) -> Result<usize, RomError> {
    let standard = TM01 + u16::try_from(i).unwrap_or_default();
    items::internal(standard)
        .map(usize::from)
        .ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {standard}")))
}

fn move_of(moves: &[Move], number: MoveId) -> Result<&Move, RomError> {
    moves
        .iter()
        .find(|m| m.number == number)
        .ok_or_else(|| RomError::Malformed(format!("no move {number}")))
}

/// Each TM item gets the palette of its move's type.
pub(super) fn write_palettes(
    rom: &mut GbaRom,
    entry: &RomEntry,
    tms: &[MoveId],
    moves: &[Move],
) -> Result<(), RomError> {
    let Some(images) = entry
        .int("ItemImages")
        .and_then(|o| usize::try_from(o).ok())
        .filter(|&o| o > 0)
    else {
        return Ok(());
    };
    let palettes = entry.array("TmPals").unwrap_or_default();
    for (i, &m) in tms.iter().enumerate() {
        let type_byte = move_of(moves, m)?
            .move_type
            .and_then(c::type_to_byte)
            .unwrap_or(c::UNKNOWN_TYPE);
        let palette = palettes
            .get(usize::from(type_byte))
            .and_then(|&p| usize::try_from(p).ok())
            .ok_or_else(|| RomError::Malformed(format!("no TM palette for type byte {type_byte}")))?;
        rom.write_pointer(images + item(i)? * 8 + 4, palette)?;
    }
    Ok(())
}

/// Each TM item's description becomes its move's. `FireRed` and `LeafGreen` share the
/// move's text; Ruby, Sapphire and Emerald get a copy with item-description lines.
pub(super) fn write_item_text(
    space: &mut Space<'_>,
    entry: &RomEntry,
    family: Family,
    text: &TextTable,
    tms: &[MoveId],
) -> Result<(), RomError> {
    let Some(descriptions) = entry
        .int("MoveDescriptions")
        .and_then(|o| usize::try_from(o).ok())
        .filter(|&o| o > 0)
    else {
        return Ok(());
    };
    let items = super::entry_offset(space.rom, entry, "ItemData")?;
    let entry_size = need(entry, "ItemEntrySize")?;
    for (i, &m) in tms.iter().enumerate() {
        let item_pointer = items + item(i)? * entry_size + ITEM_DESCRIPTION;
        let move_pointer = descriptions + (usize::from(m) - 1) * 4;
        let description = space.rom.pointer(move_pointer)?;
        if family == Family::FireRedLeafGreen {
            space.rom.write_pointer(item_pointer, description)?;
        } else {
            let old = text.decode(&space.rom.bytes()[description..], usize::MAX);
            let reflowed = reflow(&old, RSE_ITEM_DESCRIPTION_LINE, |w| text.encode(w).len());
            let mut bytes = text.encode(&reflowed);
            bytes.push(super::space::FREE_SPACE_BYTE);
            space
                .rewrite_string(item_pointer, &bytes, &[])
                .map_err(|e| RomError::Malformed(format!("couldn't insert new item description: {e}")))?;
        }
    }
    Ok(())
}

/// The move tutor moves: Emerald's and `FireRed`'s tutors; Ruby and Sapphire have none.
pub(super) fn read_tutors(rom: &GbaRom, entry: &RomEntry, family: Family) -> Result<Vec<MoveId>, RomError> {
    if family == Family::RubySapphire {
        return Ok(Vec::new());
    }
    let at = need(entry, "MoveTutorData")?;
    (0..need(entry, "MoveTutorMoves")?)
        .map(|i| rom.u16(at + i * 2))
        .collect()
}

/// Writes the move tutor moves.
pub(super) fn write_tutors(rom: &mut GbaRom, entry: &RomEntry, tutors: &[MoveId]) -> Result<(), RomError> {
    if tutors.len() != need(entry, "MoveTutorMoves")? {
        return Err(RomError::Malformed("wrong number of move tutor moves".into()));
    }
    let at = need(entry, "MoveTutorData")?;
    for (i, &m) in tutors.iter().enumerate() {
        rom.write_u16(at + i * 2, m)?;
    }
    Ok(())
}
