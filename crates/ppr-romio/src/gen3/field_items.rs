//! Items on the ground and hidden items: where they are on the maps, read and written.

use ppr_model::{Item, ItemId};

use super::constants as c;
use super::{Family, items, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;

const PERSON_SIZE: usize = 24;
const PERSON_SPRITE: usize = 1;
const PERSON_SCRIPT: usize = 16;
const SIGNPOST_SIZE: usize = 12;
const SIGNPOST_TYPE: usize = 5;
const SIGNPOST_ITEM: usize = 8;
/// Signpost types 5 to 7 are hidden items.
const HIDDEN_ITEM_TYPES: std::ops::RangeInclusive<u8> = 5..=7;

/// The pointer at `offset`; `None` when it does not point into the ROM.
fn pointer(rom: &GbaRom, offset: usize) -> Option<usize> {
    rom.pointer(offset).ok()
}

/// The number of maps in each bank. A list of pointers ends at the first invalid
/// one, or where another list starts.
pub(super) fn bank_sizes(rom: &GbaRom, map_headers: usize) -> Vec<usize> {
    let mut banks: Vec<usize> = Vec::new();
    let mut offset = map_headers;
    loop {
        if banks.iter().any(|&b| map_headers < b && offset >= b) {
            break;
        }
        let Some(bank) = pointer(rom, offset) else { break };
        banks.push(bank);
        offset += 4;
    }
    banks
        .iter()
        .map(|&base| {
            let mut count = 0;
            let mut offset = base;
            loop {
                if banks.iter().any(|&b| base < b && offset >= b) || (base < map_headers && offset >= map_headers) {
                    break;
                }
                if pointer(rom, offset).is_none() {
                    break;
                }
                count += 1;
                offset += 4;
            }
            count
        })
        .collect()
}

/// Where the item of each item ball script and hidden item is, in map order.
pub(super) fn offsets(rom: &GbaRom, map_headers: usize, item_ball_sprite: u8) -> Result<Vec<usize>, RomError> {
    let mut found = Vec::new();
    for (bank, &maps) in bank_sizes(rom, map_headers).iter().enumerate() {
        let bank_at = rom.pointer(map_headers + bank * 4)?;
        for map in 0..maps {
            let header = rom.pointer(bank_at + map * 4)?;
            let Some(events) = pointer(rom, header + 4) else {
                continue;
            };
            let counts = rom.slice(events, 4)?;
            let (people, signposts) = (usize::from(counts[0]), usize::from(counts[3]));
            if people > 0 {
                let people_at = rom.pointer(events + 4)?;
                for p in 0..people {
                    let person = people_at + p * PERSON_SIZE;
                    if rom.slice(person + PERSON_SPRITE, 1)?[0] != item_ball_sprite {
                        continue;
                    }
                    let Some(script) = pointer(rom, person + PERSON_SCRIPT) else {
                        continue;
                    };
                    if is_item_ball_script(rom, script)? {
                        found.push(script + 3);
                    }
                }
            }
            if signposts > 0 {
                let signposts_at = rom.pointer(events + 16)?;
                for s in 0..signposts {
                    let signpost = signposts_at + s * SIGNPOST_SIZE;
                    let kind = rom.slice(signpost + SIGNPOST_TYPE, 1)?[0];
                    // Item 0 is coins.
                    if HIDDEN_ITEM_TYPES.contains(&kind) && rom.u16(signpost + SIGNPOST_ITEM)? != 0 {
                        found.push(signpost + SIGNPOST_ITEM);
                    }
                }
            }
        }
    }
    Ok(found)
}

/// `setorcopyvar 0x8000, item; setorcopyvar 0x8001, count; callstd 0 or 1`.
fn is_item_ball_script(rom: &GbaRom, script: usize) -> Result<bool, RomError> {
    let b = rom.slice(script, 12)?;
    Ok(b[0] == 0x1A
        && b[1] == 0x00
        && b[2] == 0x80
        && b[5] == 0x1A
        && b[6] == 0x01
        && b[7] == 0x80
        && b[10] == 0x09
        && (b[11] == 0x00 || b[11] == 0x01))
}

/// The field items and where they are.
pub(super) fn load(rom: &GbaRom, entry: &RomEntry, all: &[Item]) -> Result<(Vec<usize>, Vec<ItemId>), RomError> {
    let sprite = u8::try_from(need(entry, "ItemBallPic")?).unwrap_or(u8::MAX);
    read(rom, &offsets(rom, need(entry, "MapHeaders")?, sprite)?, all)
}

/// TMs that random field items keep, as no other place gives them.
pub(super) fn required_tms(family: Family) -> Vec<ItemId> {
    match family {
        Family::RubySapphire => c::RS_REQUIRED_FIELD_TMS.to_vec(),
        Family::Emerald => c::E_REQUIRED_FIELD_TMS.to_vec(),
        Family::FireRedLeafGreen => c::FRLG_REQUIRED_FIELD_TMS.to_vec(),
    }
}

fn allowed(all: &[Item], id: ItemId) -> bool {
    all.iter().any(|i| i.id == id && i.allowed)
}

/// The allowed items at `offsets`, with the offsets they are at.
pub(super) fn read(rom: &GbaRom, offsets: &[usize], all: &[Item]) -> Result<(Vec<usize>, Vec<ItemId>), RomError> {
    let mut kept = Vec::new();
    let mut ids = Vec::new();
    for &at in offsets {
        let id = items::standard(rom.u16(at)?);
        if allowed(all, id) {
            kept.push(at);
            ids.push(id);
        }
    }
    Ok((kept, ids))
}

/// Writes the field items, after checking that TMs replace TMs.
pub(super) fn write(
    rom: &mut GbaRom,
    offsets: &[usize],
    old: &[ItemId],
    new: &[ItemId],
    all: &[Item],
) -> Result<(), RomError> {
    let is_tm = |id: ItemId| all.iter().any(|i| i.id == id && i.tm);
    if new.len() != old.len() || old.iter().zip(new).any(|(&o, &n)| is_tm(o) != is_tm(n)) {
        return Err(RomError::Malformed(
            "field items must keep their number, TMs replacing TMs".into(),
        ));
    }
    for (&at, &id) in offsets.iter().zip(new) {
        let internal = items::internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {id}")))?;
        rom.write_u16(at, internal)?;
    }
    Ok(())
}
