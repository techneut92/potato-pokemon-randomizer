//! The items Pickup finds, read and written. Emerald's table holds two bytes an
//! item, the others four (the item and its chance). The chances do not change, so they are not read.
//!
//! The table is found by its first items, so the table of a ROM whose Pickup items changed is not found
//! again.

use ppr_model::ItemId;

use super::{Family, found_at, items};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::{GbaRom, hex};

/// The table, found by its first items, and its entry size; `None` when it is not found.
pub(super) fn table(rom: &GbaRom, entry: &RomEntry, family: Family) -> Result<Option<(usize, usize)>, RomError> {
    let Some(locator) = entry.value("PickupTableStartLocator") else {
        return Ok(None);
    };
    let size = if family == Family::Emerald { 2 } else { 4 };
    Ok(found_at(rom.find(&hex(locator)?)).map(|at| (at, size)))
}

fn count(entry: &RomEntry) -> usize {
    entry
        .int("PickupItemCount")
        .and_then(|n| usize::try_from(n).ok())
        .unwrap_or_default()
}

/// The Pickup items, without the chances.
pub(super) fn read(rom: &GbaRom, entry: &RomEntry, table: Option<(usize, usize)>) -> Result<Vec<ItemId>, RomError> {
    let Some((at, size)) = table else {
        return Ok(Vec::new());
    };
    (0..count(entry))
        .map(|i| Ok(items::standard(rom.u16(at + size * i)?)))
        .collect()
}

/// Writes the Pickup items.
pub(super) fn write(rom: &mut GbaRom, table: Option<(usize, usize)>, pickup: &[ItemId]) -> Result<(), RomError> {
    let Some((at, size)) = table else {
        return Ok(());
    };
    for (i, &id) in pickup.iter().enumerate() {
        let internal = items::internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {id}")))?;
        rom.write_u16(at + size * i, internal)?;
    }
    Ok(())
}
