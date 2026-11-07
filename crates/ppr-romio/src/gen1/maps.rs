//! Maps: which maps exist, their names and item balls, and the hidden items.

use super::constants::{self as c, Family};
use super::need;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::text::GbText;
use crate::gb::{GbRom, bank_of};

/// Unused map slots.
const SKIPPED_MAPS: [usize; 2] = [0xED, 0xFF];
/// Maps up to here have a name of their own in the map name table; later ones share names by ranges.
const EXTERNAL_MAP_COUNT: usize = 0x25;

/// The maps reachable from Pallet Town, with the offsets of the item ids of their item balls.
#[derive(Debug, Clone, Default)]
pub(super) struct Maps {
    /// By map number; `None` for maps that aren't reached.
    pub(super) items: Vec<Option<Vec<usize>>>,
    /// By map number; empty for maps without a name.
    pub(super) names: Vec<String>,
}

/// The maps reachable from map 0, and the names of the maps.
pub(super) fn load(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Maps, RomError> {
    let banks = need(entry, "MapBanks")?;
    let addresses = need(entry, "MapAddresses")?;
    let mut items: Vec<Option<Vec<usize>>> = vec![None; 256];
    let mut todo = vec![0usize];
    while let Some(id) = todo.pop() {
        if items[id].is_some() || SKIPPED_MAPS.contains(&id) {
            continue;
        }
        let (offsets, next) = preload(rom, banks, addresses, id)?;
        items[id] = Some(offsets);
        todo.extend(next.into_iter().rev());
    }
    let names = names(rom, entry, text, &items)?;
    Ok(Maps { items, names })
}

/// Its item balls, and the maps its connections and warps lead to.
fn preload(rom: &GbRom, banks: usize, addresses: usize, id: usize) -> Result<(Vec<usize>, Vec<usize>), RomError> {
    let addr = rom.pointer_in(addresses + id * 2, usize::from(rom.byte(banks + id)?))?;
    let bank = bank_of(addr);
    let connect = rom.byte(addr + 9)?;
    let connections = usize::try_from((connect & 0xF).count_ones()).unwrap_or_default();
    let mut next = Vec::new();
    let cons = addr + 10;
    for i in 0..connections {
        next.push(usize::from(rom.byte(cons + i * 11)?));
    }
    let objects = rom.pointer_in(cons + connections * 11, bank)?;
    // +0 is the border tile, +1 the warp count.
    let warps = usize::from(rom.byte(objects + 1)?);
    let mut offs = objects + 2;
    for _ in 0..warps {
        next.push(usize::from(rom.byte(offs + 3)?));
        offs += 4;
    }
    let signs = usize::from(rom.byte(offs)?);
    offs += 1 + signs * 3;
    let entities = usize::from(rom.byte(offs)?);
    offs += 1;
    let mut items = Vec::new();
    for _ in 0..entities {
        let text_id = rom.byte(offs + 5)?;
        if text_id & (1 << 6) != 0 {
            // A trainer.
            offs += 8;
        } else if text_id & (1 << 7) != 0 && rom.byte(offs + 6)? != 0 {
            items.push(offs + 6);
            offs += 7;
        } else {
            offs += 6;
        }
    }
    Ok((items, next))
}

/// The first maps by the name table; later ones by name ranges, numbered, or as a
/// building when they share an outside map's name.
fn names(rom: &GbRom, entry: &RomEntry, text: &GbText, maps: &[Option<Vec<usize>>]) -> Result<Vec<String>, RomError> {
    let read = |offset: usize| text.read_string(rom.bytes().get(offset..).unwrap_or_default(), usize::MAX, false);
    let mut names = vec![String::new(); 256];
    let mut table = need(entry, "MapNameTableOffset")?;
    let mut external = Vec::new();
    for name in names.iter_mut().take(EXTERNAL_MAP_COUNT) {
        let at = rom.pointer(table + 1)?;
        external.push(at);
        *name = read(at);
        table += 3;
    }
    let mut last_max = EXTERNAL_MAP_COUNT;
    let mut counts: Vec<(usize, usize)> = Vec::new();
    while rom.byte(table)? != 0xFF {
        let max = usize::from(rom.byte(table)?);
        let at = rom.pointer(table + 2)?;
        let name = read(at).trim().to_owned();
        let reached = (last_max..max).filter(|&i| maps.get(i).is_some_and(Option::is_some));
        if external.contains(&at) {
            for i in reached {
                names[i] = format!("{name} (Building)");
            }
        } else {
            let mut count = counts.iter().find(|(o, _)| *o == at).map_or(0, |&(_, n)| n);
            for i in reached {
                count += 1;
                names[i] = format!("{name} ({count})");
            }
            match counts.iter_mut().find(|(o, _)| *o == at) {
                Some(c) => c.1 = count,
                None => counts.push((at, count)),
            }
        }
        last_max = max;
        table += 4;
    }
    Ok(names)
}

/// The item ids of the item balls, map by map, then of the hidden items.
pub(super) fn field_item_offsets(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    maps: &Maps,
) -> Result<Vec<usize>, RomError> {
    let mut offsets: Vec<usize> = maps.items.iter().flatten().flatten().copied().collect();
    let mut table = need(entry, "HiddenObjectMapPointerTable")?;
    let routine = need(entry, "HiddenItemRoutine")?;
    match family {
        Family::Yellow => {
            while rom.byte(table)? != c::HIDDEN_OBJECT_MAPS_TERMINATOR {
                offsets.extend(hidden_items(rom, rom.pointer(table + 1)?, routine)?);
                table += 3;
            }
        }
        Family::RedBlue => {
            // The pointer table has no terminator, but the map list of the same length before it does.
            let mut list = need(entry, "HiddenObjectMapList")?;
            while rom.byte(list)? != c::HIDDEN_OBJECT_MAPS_TERMINATOR {
                offsets.extend(hidden_items(rom, rom.pointer(table)?, routine)?);
                list += 1;
                table += 2;
            }
        }
    }
    Ok(offsets)
}

/// The item ids of the hidden objects that run the hidden item routine.
fn hidden_items(rom: &GbRom, mut offset: usize, routine: usize) -> Result<Vec<usize>, RomError> {
    let mut items = Vec::new();
    while rom.byte(offset)? != c::HIDDEN_OBJECTS_TERMINATOR {
        if rom.pointer_in(offset + 4, usize::from(rom.byte(offset + 3)?))? == routine {
            items.push(offset + 2);
        }
        offset += 6;
    }
    Ok(items)
}
