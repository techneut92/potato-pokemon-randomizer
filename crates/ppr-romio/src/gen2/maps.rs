//! Maps: their names (the landmark each map belongs to), and the item balls and hidden items on them.

use std::collections::BTreeMap;

use super::constants as c;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::text::GbText;
use crate::gb::{BANK, GbRom, bank_of, need};

/// Map header size.
const MAP_HEADER_SIZE: usize = 9;
/// The signpost type of hidden items.
const HIDDEN_ITEM_SIGNPOST: u8 = 7;

/// The names of the maps and the offsets of the item ids on them.
#[derive(Debug, Clone, Default)]
pub(super) struct Maps {
    /// By (group, number), both from 1.
    pub(super) names: BTreeMap<(u8, u8), String>,
    /// The item ids of the hidden items and item balls, map by map.
    pub(super) item_offsets: Vec<usize>,
}

impl Maps {
    /// The map's name; "null" for a map that has none, as the original prints it.
    pub(super) fn name(&self, group: u8, number: u8) -> String {
        self.names
            .get(&(group, number))
            .cloned()
            .unwrap_or_else(|| "null".to_owned())
    }
}

/// The landmark names, with the soft hyphen as a space.
fn landmark_names(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<String>, RomError> {
    let table = need(entry, "LandmarkTableOffset")?;
    let count = need(entry, "LandmarkCount")?;
    (0..count)
        .map(|i| {
            let at = rom.pointer(table + i * 4 + 2)?;
            let name = text.read_string(rom.bytes().get(at..).unwrap_or_default(), usize::MAX, false);
            Ok(name.replace("\\x1F", " "))
        })
        .collect()
}

/// Every map of every group: its name and items.
pub(super) fn load(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Maps, RomError> {
    let landmarks = landmark_names(rom, entry, text)?;
    let headers = need(entry, "MapHeaders")?;
    let headers_end = (bank_of(headers) + 1) * BANK;
    let groups = (0..c::MAP_GROUP_COUNT)
        .map(|g| rom.pointer(headers + g * 2))
        .collect::<Result<Vec<_>, _>>()?;
    let mut maps = Maps::default();
    for (g, &start) in groups.iter().enumerate() {
        let last = g == c::MAP_GROUP_COUNT - 1;
        let end = if last { headers_end } else { groups[g + 1] };
        let max_maps = if last { c::MAPS_IN_LAST_GROUP } else { usize::MAX };
        let mut at = start;
        let mut number = 0;
        while at < end && number < max_maps {
            let group = u8::try_from(g + 1).unwrap_or(u8::MAX);
            let map = u8::try_from(number + 1).unwrap_or(u8::MAX);
            read_map(rom, at, &landmarks, (group, map), &mut maps)?;
            at += MAP_HEADER_SIZE;
            number += 1;
        }
    }
    Ok(maps)
}

/// A map's name, then its hidden items and item balls.
fn read_map(rom: &GbRom, at: usize, landmarks: &[String], key: (u8, u8), maps: &mut Maps) -> Result<(), RomError> {
    let second_header = rom.pointer_in(at + 3, usize::from(rom.byte(at)?))?;
    let landmark = usize::from(rom.byte(at + 5)?);
    if let Some(name) = landmarks.get(landmark) {
        maps.names.insert(key, name.clone());
    }
    // The event header is in the bank of the script header.
    let events_bank = usize::from(rom.byte(second_header + 6)?);
    // Two bytes of filler first.
    let mut offset = rom.pointer_in(second_header + 9, events_bank)? + 2;
    let warps = usize::from(rom.byte(offset)?);
    offset += 1 + warps * 5;
    let triggers = usize::from(rom.byte(offset)?);
    offset += 1 + triggers * 8;
    let signposts = usize::from(rom.byte(offset)?);
    offset += 1;
    for s in 0..signposts {
        if rom.byte(offset + s * 5 + 2)? == HIDDEN_ITEM_SIGNPOST {
            // The event's first two bytes are its flag.
            maps.item_offsets
                .push(rom.pointer_in(offset + s * 5 + 3, events_bank)? + 2);
        }
    }
    offset += signposts * 5;
    let people = usize::from(rom.byte(offset)?);
    offset += 1;
    for p in 0..people {
        // Item balls have the lowest bit of their color and function byte set.
        if rom.byte(offset + p * 13 + 7)? & 1 == 1 {
            maps.item_offsets
                .push(rom.pointer_in(offset + p * 13 + 9, events_bank)?);
        }
    }
    Ok(())
}
