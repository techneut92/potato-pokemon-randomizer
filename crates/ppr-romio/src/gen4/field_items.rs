//! Items on the ground and hidden items, and the items Pickup finds; read and written.
//!
//! Item balls are scripts in one script file: a list of relative pointers to scripts, ending in `FD13`, each
//! script setting variable `0x8008` to its item first. Some entries of the list are no item balls and are skipped
//! (`ItemBallsSkip`). Hidden items are a table in the ARM9. Only items that randomizers may place are field items;
//! the others stay where they are.
//!
//! The Pickup tables are found by their first items, so the tables of a ROM whose Pickup items changed may not be
//! found again.

use ppr_model::{Item, ItemId};

use super::Rom;
use super::constants::Family;
use super::files::{find, long, put_word, word};
use crate::RomError;

const SCRIPT_LIST_END: u16 = 0xFD13;
/// The "set variable" script command.
const DPPT_SET_VAR: u16 = 0x28;
const HGSS_SET_VAR: u16 = 0x29;
/// The variable an item ball's script sets to its item.
const ITEM_VARIABLE: u16 = 0x8008;
/// Hidden item table entries are this long; the item comes first.
const HIDDEN_ITEM_SIZE: usize = 8;

/// Where Pickup's tables are found in the battle overlay, and how many items they have.
const PICKUP_TABLE_LOCATOR: &str = "110012001A000300";
const RARE_PICKUP_TABLE_LOCATOR: &str = "19005C00DD00";
const COMMON_PICKUP_ITEMS: usize = 18;
const RARE_PICKUP_ITEMS: usize = 11;

/// Where a field item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Place {
    /// In the item ball script file.
    Script(usize),
    /// In the ARM9's hidden item table.
    Arm9(usize),
}

/// Every field item and where it is, in game order: item balls, then hidden items.
fn all(rom: &mut Rom) -> Result<Vec<(Place, ItemId)>, RomError> {
    let set_var = if rom.family() == Family::HeartGoldSoulSilver {
        HGSS_SET_VAR
    } else {
        DPPT_SET_VAR
    };
    let skip = rom.entry.array("ItemBallsSkip");
    let file = rom.entry.need("ItemBallsScriptOffset")?;
    let path = rom.entry.file("Scripts")?;
    let scripts = rom.files.narc_file(&path, file)?;
    let mut found = Vec::new();
    let mut skipped = 0;
    let mut offset = 0;
    while word(scripts, offset)? != SCRIPT_LIST_END {
        let target = relative_pointer(scripts, offset)?;
        offset += 4;
        if skip.get(skipped) == Some(&(offset / 4 - 1)) {
            skipped += 1;
            continue;
        }
        if word(scripts, target)? == set_var && word(scripts, target + 2)? == ITEM_VARIABLE {
            found.push((Place::Script(target + 4), word(scripts, target + 4)?));
        }
    }
    let table = rom.entry.int("HiddenItemTableOffset");
    let arm9 = rom.files.arm9_ref();
    for i in 0..rom.entry.int("HiddenItemCount") {
        let at = table + i * HIDDEN_ITEM_SIZE;
        found.push((Place::Arm9(at), word(arm9, at)?));
    }
    Ok(found)
}

fn relative_pointer(data: &[u8], at: usize) -> Result<usize, RomError> {
    let relative = i64::from(long(data, at)?.cast_signed());
    usize::try_from(i64::try_from(at + 4).unwrap_or_default() + relative)
        .map_err(|_| RomError::Malformed("script pointer before the file".into()))
}

/// The field items randomizers may place, and where each is.
pub(super) fn read(rom: &mut Rom, items: &[Item]) -> Result<(Vec<Place>, Vec<ItemId>), RomError> {
    let allowed = |id: ItemId| items.iter().any(|i| i.id == id && i.allowed);
    Ok(all(rom)?.into_iter().filter(|&(_, id)| allowed(id)).unzip())
}

/// Writes the field items that changed.
pub(super) fn write(rom: &mut Rom, places: &[Place], loaded: &[ItemId], items: &[ItemId]) -> Result<(), RomError> {
    if items.len() != places.len() {
        return Err(RomError::Malformed(format!(
            "the game has {} field items, got {}",
            places.len(),
            items.len()
        )));
    }
    let path = rom.entry.file("Scripts")?;
    let file = rom.entry.need("ItemBallsScriptOffset")?;
    for ((&place, &id), &old) in places.iter().zip(items).zip(loaded) {
        if id == old {
            continue;
        }
        match place {
            Place::Script(at) => put_word(rom.files.narc_file(&path, file)?, at, id)?,
            Place::Arm9(at) => put_word(rom.files.arm9(), at, id)?,
        }
    }
    Ok(())
}

/// Where the common and the rare Pickup tables are in the battle overlay, found by their first items.
pub(super) type PickupTables = Option<(usize, usize)>;

/// The items Pickup finds, the common table then the rare one, and where the tables are. The chances are fixed by
/// the table position.
pub(super) fn read_pickup(rom: &mut Rom) -> Result<(PickupTables, Vec<ItemId>), RomError> {
    let overlay = rom.files.overlay(rom.entry.need("BattleOvlNumber")?)?;
    let Some((common, rare)) = find(overlay, PICKUP_TABLE_LOCATOR).zip(find(overlay, RARE_PICKUP_TABLE_LOCATOR)) else {
        return Ok((None, Vec::new()));
    };
    let mut items = Vec::with_capacity(COMMON_PICKUP_ITEMS + RARE_PICKUP_ITEMS);
    for i in 0..COMMON_PICKUP_ITEMS {
        items.push(word(overlay, common + i * 2)?);
    }
    for i in 0..RARE_PICKUP_ITEMS {
        items.push(word(overlay, rare + i * 2)?);
    }
    Ok((Some((common, rare)), items))
}

/// Writes the items Pickup finds.
pub(super) fn write_pickup(rom: &mut Rom, tables: PickupTables, items: &[ItemId]) -> Result<(), RomError> {
    let Some((common, rare)) = tables else {
        return Ok(());
    };
    if items.len() != COMMON_PICKUP_ITEMS + RARE_PICKUP_ITEMS {
        return Err(RomError::Malformed(format!(
            "Pickup has {} items, got {}",
            COMMON_PICKUP_ITEMS + RARE_PICKUP_ITEMS,
            items.len()
        )));
    }
    let overlay = rom.files.overlay(rom.entry.need("BattleOvlNumber")?)?;
    for (i, &id) in items.iter().enumerate() {
        let at = if i < COMMON_PICKUP_ITEMS {
            common + i * 2
        } else {
            rare + (i - COMMON_PICKUP_ITEMS) * 2
        };
        put_word(overlay, at, id)?;
    }
    Ok(())
}
