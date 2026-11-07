//! Wild encounters: the areas of each map in the wild encounter GARC, and in X and Y the falling and rustling bush
//! encounters of the field module. Writing them also rebuilds the Pokédex area data.
//!
//! A map's file has its encounter data at the word at 0x10 plus 0x10 (X and Y) or 0xE (Omega Ruby and Alpha
//! Sapphire). Each slot is a species with the forme in its top 5 bits, a level and a maximum level. Areas without
//! any species are not read. Omega Ruby and Alpha Sapphire keep a copy of every map's data in the last file.
//!
//! Deliberate choices:
//! - A slot's species is its alternate forme's own number (from 722) for a forme with data of its own. The
//!   "regional forme" and "any forme" markers (30 and 31), and formes the species doesn't have, are read as forme 0;
//!   a slot keeps its forme as stored while its species and forme stay.
//! - The falling and rustling bush encounters store no forme, so their sets ban every alternate forme.
//! - A map file too short for its header counts as a map without encounters.
//! - A falling or rustling bush encounter whose map cannot be told (no map or several with its name) has no map
//!   index.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, EncounterKind, EncounterSet, WildEncounter, WildLevels};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom};
use super::species::FormeTable;
use crate::RomError;

const XY_DATA_LENGTH: usize = 0x178;
const ORAS_DATA_LENGTH: usize = 0xF6;
/// The bytes of a map's data copied to the last file in Omega Ruby and Alpha Sapphire.
const ORAS_COPY_LENGTH: usize = 0xF4;

/// One area of a map: offset in the map's data, slots, name suffix, kind and Pokédex area flag.
struct AreaKind {
    offset: usize,
    slots: usize,
    suffix: &'static str,
    kind: EncounterKind,
    dex_flag: u32,
}

const fn area(offset: usize, slots: usize, suffix: &'static str, kind: EncounterKind, dex_flag: u32) -> AreaKind {
    AreaKind {
        offset,
        slots,
        suffix,
        kind,
        dex_flag,
    }
}

const XY_AREAS: [AreaKind; 13] = [
    area(0, 12, "Grass/Cave", EncounterKind::Walking, 0x1),
    area(48, 12, "Yellow Flowers", EncounterKind::Walking, 0x2),
    area(96, 12, "Purple Flowers", EncounterKind::Walking, 0x4),
    area(144, 12, "Red Flowers", EncounterKind::Walking, 0x8),
    area(192, 12, "Rough Terrain/Tall Grass", EncounterKind::Walking, 0x10),
    area(240, 5, "Surf", EncounterKind::Surfing, 0x20),
    area(260, 5, "Rock Smash", EncounterKind::Interact, 0x40),
    area(280, 3, "Old Rod", EncounterKind::Fishing, 0x80),
    area(292, 3, "Good Rod", EncounterKind::Fishing, 0x100),
    area(304, 3, "Super Rod", EncounterKind::Fishing, 0x200),
    area(316, 5, "Common Horde", EncounterKind::Walking, 0x400),
    area(336, 5, "Uncommon Horde", EncounterKind::Walking, 0x400),
    area(356, 5, "Rare Horde", EncounterKind::Walking, 0x400),
];

/// The `DexNav` area's Pokédex flag depends on whether the map has grass (0x04) or not (0x08); 0 stands for that.
const ORAS_AREAS: [AreaKind; 11] = [
    area(0, 12, "Grass/Cave", EncounterKind::Walking, 0x1),
    area(48, 12, "Long Grass", EncounterKind::Walking, 0x2),
    area(96, 3, "DexNav Foreign Encounter", EncounterKind::Special, 0),
    area(108, 5, "Surf", EncounterKind::Surfing, 0x10),
    area(128, 5, "Rock Smash", EncounterKind::Interact, 0x20),
    area(148, 3, "Old Rod", EncounterKind::Fishing, 0x40),
    area(160, 3, "Good Rod", EncounterKind::Fishing, 0x80),
    area(172, 3, "Super Rod", EncounterKind::Fishing, 0x100),
    area(184, 5, "Common Horde", EncounterKind::Walking, 0x200),
    area(204, 5, "Uncommon Horde", EncounterKind::Walking, 0x200),
    area(224, 5, "Rare Horde", EncounterKind::Walking, 0x200),
];

/// Where an encounter set is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Place {
    /// In map file `file`, at `offset` of the file; the formes as stored, and each slot's species and forme as
    /// read.
    Map {
        file: usize,
        offset: usize,
        formes: Vec<u16>,
        loaded: Vec<(DexNumber, u8)>,
    },
    /// In the field module at `offset`.
    Field { offset: usize },
}

/// The places of the encounter sets, in game-data order.
pub(crate) type Places = Vec<Place>;

/// How each set stores its levels, in game-data order: the falling and rustling spots of X and Y one level per slot,
/// every other set a range per slot.
pub(crate) fn levels(places: &Places) -> Vec<WildLevels> {
    places
        .iter()
        .map(|p| match p {
            Place::Map { .. } => WildLevels::Range,
            Place::Field { .. } => WildLevels::Single,
        })
        .collect()
}

/// The files the encounters are in.
pub(crate) struct Files<'a> {
    pub(crate) wild: &'a str,
    pub(crate) field: Option<&'a str>,
    pub(crate) dex_area: &'a str,
    pub(crate) family: Family,
    pub(crate) formes: &'a FormeTable,
}

/// The offset of a map's encounter data in its file, if it has any.
fn data_offset(file: &[u8], family: Family) -> Option<usize> {
    let start = i64::from(i32::from_le_bytes(file.get(0x10..0x14)?.try_into().ok()?));
    match family {
        Family::Xy => {
            let at = usize::try_from(start + 0x10).ok()?;
            (file.len().checked_sub(at)? >= XY_DATA_LENGTH).then_some(at)
        }
        Family::Oras => {
            let at = start + 0xE;
            let end = i64::from(i32::from_le_bytes(file.get(0x14..0x18)?.try_into().ok()?));
            let at_u = usize::try_from(at).ok()?;
            (end - at >= i64::try_from(ORAS_DATA_LENGTH).ok()? && at_u + ORAS_DATA_LENGTH <= file.len()).then_some(at_u)
        }
    }
}

fn areas(family: Family) -> &'static [AreaKind] {
    match family {
        Family::Xy => &XY_AREAS,
        Family::Oras => &ORAS_AREAS,
    }
}

/// The map files: all but the last in X and Y, all but the last two in Omega Ruby and Alpha Sapphire.
fn map_count(len: usize, family: Family) -> usize {
    match family {
        Family::Xy => len.saturating_sub(1),
        Family::Oras => len.saturating_sub(2),
    }
}

/// The slots of an area: (species, forme, level, maximum level), empty slots left out.
fn read_slots(data: &[u8], at: usize, slots: usize) -> Result<Vec<(DexNumber, u16, u8, u8)>, RomError> {
    let mut out = Vec::new();
    for i in 0..slots {
        let word = data.u16_at(at + i * 4)?;
        let species = word & 0x7FF;
        if species != 0 {
            out.push((
                species,
                word >> 11,
                data.u8_at(at + i * 4 + 2)?,
                data.u8_at(at + i * 4 + 3)?,
            ));
        }
    }
    Ok(out)
}

/// The map names, by map file: from the zone data.
fn map_names(
    rom: &mut Rom,
    wild: &str,
    zone_file: usize,
    names: &[String],
) -> Result<BTreeMap<usize, String>, RomError> {
    let zone = rom.garc_file(wild, true, zone_file)?;
    let mut out = BTreeMap::new();
    for map in 0..zone_file {
        let at = map * 56 + 0x1C;
        let index = usize::from(zone.u8_at(at)?) + 0x100 * usize::from(zone.u8_at(at + 1)? & 1);
        out.insert(map, names.get(index).cloned().unwrap_or_default());
    }
    Ok(out)
}

/// The map index of a field encounter, from the first two words of its name: the only map whose name starts with
/// them, 0 for none and `None` for several.
fn field_map(names: &BTreeMap<usize, String>, name: &str) -> Option<u16> {
    let words: Vec<&str> = name.splitn(3, ' ').take(2).collect();
    let prefix = words.join(" ");
    let mut found = None;
    for (&map, n) in names {
        if n.starts_with(&prefix) {
            if found.is_some() {
                return None;
            }
            found = Some(map);
        }
    }
    Some(found.map_or(0, |m| u16::try_from(m).unwrap_or(0)))
}

fn field_set(
    name: String,
    map: Option<u16>,
    field: &[u8],
    offset: usize,
    alt_formes: &[DexNumber],
) -> Result<EncounterSet, RomError> {
    let mut encounters = Vec::new();
    for i in 0..7 {
        let species = field.u16_at(offset + 4 + i * 8)?;
        let level = field.u8_at(offset + 8 + i * 8)?;
        if species != 0 {
            encounters.push(WildEncounter {
                forme: 0,
                species,
                min_level: level,
                max_level: level,
            });
        }
    }
    Ok(EncounterSet {
        kind: EncounterKind::Ambush,
        map,
        name,
        location: None,
        post_game: false,
        post_game_from: None,
        rate: 0,
        banned: alt_formes.to_vec(),
        encounters,
    })
}

/// Reads the encounter sets and where they are.
pub(crate) fn read(
    rom: &mut Rom,
    files: &Files<'_>,
    zone_file: usize,
    map_name_texts: &[String],
) -> Result<(Vec<EncounterSet>, Places), RomError> {
    let names = map_names(rom, files.wild, zone_file, map_name_texts)?;
    let mut sets = Vec::new();
    let mut places = Vec::new();
    let len = rom.garc(files.wild, true)?.len();
    for i in 0..map_count(len, files.family) {
        let file = rom.garc_file(files.wild, true, i)?;
        let Some(at) = data_offset(&file, files.family) else {
            continue;
        };
        let map_name = names.get(&i).map_or("? Unknown ?", String::as_str);
        for a in areas(files.family) {
            let slots = read_slots(&file, at + a.offset, a.slots)?;
            if slots.is_empty() {
                continue;
            }
            let loaded: Vec<(DexNumber, u8)> = slots
                .iter()
                .map(|&(base, stored, _, _)| {
                    let forme = files.formes.holder_forme(base, u32::from(stored));
                    (files.formes.species(base, forme), forme)
                })
                .collect();
            sets.push(EncounterSet {
                kind: a.kind,
                map: u16::try_from(i).ok(),
                name: format!("{map_name} {}", a.suffix),
                location: None,
                post_game: false,
                post_game_from: None,
                rate: 1,
                banned: Vec::new(),
                encounters: slots
                    .iter()
                    .zip(&loaded)
                    .map(|(&(_, _, min, max), &(species, forme))| WildEncounter {
                        forme,
                        species,
                        min_level: min,
                        max_level: max,
                    })
                    .collect(),
            });
            places.push(Place::Map {
                file: i,
                offset: at + a.offset,
                formes: slots.iter().map(|s| s.1).collect(),
                loaded,
            });
        }
    }
    if let (Family::Xy, Some(field_path)) = (files.family, files.field) {
        let field = rom.file(field_path)?.to_vec();
        let alt_formes: Vec<DexNumber> = files
            .formes
            .formes
            .keys()
            .filter_map(|&n| DexNumber::try_from(n).ok())
            .collect();
        for (offset, count, table) in [
            (
                c::FALLING_ENCOUNTER_OFFSET,
                c::FALLING_ENCOUNTER_COUNT,
                &c::FALLING_ENCOUNTER_NAMES[..],
            ),
            (
                c::RUSTLING_BUSH_ENCOUNTER_OFFSET,
                c::RUSTLING_BUSH_ENCOUNTER_COUNT,
                &c::RUSTLING_BUSH_ENCOUNTER_NAMES[..],
            ),
        ] {
            let (mut name, mut start, mut map) = (table[0].1, 0, Some(0));
            for i in 0..count {
                if let Some(&(_, n)) = table.iter().find(|(first, _)| *first == i) {
                    name = n;
                    start = i;
                    map = field_map(&names, n);
                }
                let at = offset + i * c::FIELD_ENCOUNTER_SIZE;
                sets.push(field_set(
                    format!("{name} #{}", i - start + 1),
                    map,
                    &field,
                    at,
                    &alt_formes,
                )?);
                places.push(Place::Field { offset: at });
            }
        }
    }
    tag(&mut sets, files.family)?;
    Ok((sets, places))
}

/// Sets each set's location, and the post-game sets of Omega Ruby and Alpha Sapphire.
fn tag(sets: &mut [EncounterSet], family: Family) -> Result<(), RomError> {
    let tags: &[(usize, &str)] = match family {
        Family::Xy => &c::XY_LOCATION_TAGS,
        Family::Oras => &c::ORAS_LOCATION_TAGS,
    };
    let locations: Vec<&str> = tags.iter().flat_map(|&(n, t)| std::iter::repeat_n(t, n)).collect();
    if locations.len() != sets.len() {
        return Err(RomError::Malformed(format!(
            "{} wild areas, expected {}",
            sets.len(),
            locations.len()
        )));
    }
    for (set, location) in sets.iter_mut().zip(locations) {
        set.location = Some(location.to_owned());
    }
    if family == Family::Oras {
        for &i in &c::ORAS_POST_GAME_AREAS {
            if let Some(set) = sets.get_mut(i) {
                set.post_game = true;
            }
        }
    }
    Ok(())
}

/// Writes the encounter sets that changed and rebuilds the Pokédex area data.
pub(crate) fn write(
    rom: &mut Rom,
    files: &Files<'_>,
    places: &Places,
    old: &[EncounterSet],
    new: &[EncounterSet],
) -> Result<(), RomError> {
    if new.len() != places.len() {
        return Err(RomError::Malformed(format!(
            "this game has {} wild areas, not {}",
            places.len(),
            new.len()
        )));
    }
    let mut maps: BTreeMap<usize, Vec<u8>> = BTreeMap::new();
    for ((place, set), old_set) in places.iter().zip(new).zip(old) {
        if set == old_set {
            continue;
        }
        match place {
            Place::Map {
                file,
                offset,
                formes,
                loaded,
            } => {
                if !maps.contains_key(file) {
                    maps.insert(*file, rom.garc_file(files.wild, true, *file)?);
                }
                let data = maps
                    .get_mut(file)
                    .ok_or_else(|| RomError::Malformed("map file".into()))?;
                for (i, e) in set.encounters.iter().enumerate().take(formes.len()) {
                    let forme = if loaded[i] == (e.species, e.forme) {
                        formes[i]
                    } else {
                        u16::from(e.forme)
                    };
                    let base = files.formes.base(e.species);
                    data.set_u16(offset + i * 4, (forme << 11) | (base & 0x7FF))?;
                    data.set_u8(offset + i * 4 + 2, e.min_level)?;
                    data.set_u8(offset + i * 4 + 3, e.max_level)?;
                }
            }
            Place::Field { offset } => {
                let field_path = files
                    .field
                    .ok_or_else(|| RomError::Malformed("no field module".into()))?;
                let field = rom.file_mut(field_path)?;
                for (i, e) in set.encounters.iter().enumerate().take(7) {
                    field.set_u16(offset + 4 + i * 8, files.formes.base(e.species))?;
                    field.set_u8(offset + 8 + i * 8, e.min_level)?;
                }
            }
        }
    }
    if files.family == Family::Oras && !maps.is_empty() {
        let last = rom.garc(files.wild, true)?.len() - 1;
        let mut copy = rom.garc_file(files.wild, true, last)?;
        for (&file, data) in &maps {
            let Some(at) = data_offset(data, files.family) else {
                continue;
            };
            let copy_at = usize::try_from(copy.u32_at((file + 1) * 4)?)
                .unwrap_or(usize::MAX)
                .saturating_add(0xE);
            let target = copy
                .get_mut(copy_at..copy_at + ORAS_COPY_LENGTH)
                .ok_or_else(|| RomError::Malformed("the copy of the wild data is too short".into()))?;
            target.copy_from_slice(&data[at..at + ORAS_COPY_LENGTH]);
        }
        rom.set_garc_file(files.wild, true, last, copy)?;
    }
    for (file, data) in maps {
        rom.set_garc_file(files.wild, true, file, data)?;
    }
    write_dex_areas(rom, files)
}

/// Rebuilds the Pokédex area data from the encounters: per species and area, a word of flags for how it is found.
fn write_dex_areas(rom: &mut Rom, files: &Files<'_>) -> Result<(), RomError> {
    let per_species = files.family.area_data_length();
    let mut dex = vec![0u8; (c::SPECIES_COUNT + 1) * per_species];
    if files.family == Family::Xy {
        // This byte is 0x10 for every species in the game's data.
        for at in (0..dex.len()).step_by(per_species) {
            dex[at + 133] = 0x10;
        }
    }
    let table: &[i16] = match files.family {
        Family::Xy => &c::XY_MAP_TO_DEX_AREA,
        Family::Oras => &c::ORAS_MAP_TO_DEX_AREA,
    };
    let mark = |dex: &mut [u8], species: DexNumber, area: i16, flag: u32| -> Result<(), RomError> {
        let Ok(area) = usize::try_from(area) else { return Ok(()) };
        let at = usize::from(species) * per_species + area * 4;
        let value = dex.u32_at(at)? | flag;
        dex.set_u32(at, value)
    };
    let mut map = 0;
    let len = rom.garc(files.wild, true)?.len();
    for i in 0..map_count(len, files.family) {
        let file = rom.garc_file(files.wild, true, i)?;
        let Some(at) = data_offset(&file, files.family) else {
            continue;
        };
        let area = table.get(map).copied().unwrap_or(-1);
        map += 1;
        if area < 0 {
            continue;
        }
        let has_grass = !read_slots(&file, at, 12)?.is_empty();
        for a in areas(files.family) {
            let flag = match a.dex_flag {
                0 if has_grass => 0x04,
                0 => 0x08,
                f => f,
            };
            for (species, _, _, _) in read_slots(&file, at + a.offset, a.slots)? {
                if usize::from(species) <= c::SPECIES_COUNT {
                    mark(&mut dex, species, area, flag)?;
                }
            }
        }
    }
    if let (Family::Xy, Some(field_path)) = (files.family, files.field) {
        let field = rom.file(field_path)?.to_vec();
        let offsets = (0..c::FALLING_ENCOUNTER_COUNT)
            .map(|i| c::FALLING_ENCOUNTER_OFFSET + i * c::FIELD_ENCOUNTER_SIZE)
            .chain(
                (0..c::RUSTLING_BUSH_ENCOUNTER_COUNT)
                    .map(|i| c::RUSTLING_BUSH_ENCOUNTER_OFFSET + i * c::FIELD_ENCOUNTER_SIZE),
            );
        for offset in offsets {
            let area = table.get(map).copied().unwrap_or(-1);
            map += 1;
            for i in 0..7 {
                let species = field.u16_at(offset + 4 + i * 8)?;
                if species != 0 && usize::from(species) <= c::SPECIES_COUNT {
                    mark(&mut dex, species, area, 0x800)?;
                }
            }
        }
    }
    rom.set_garc_file(files.dex_area, false, 0, dex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_maps_need_a_single_match() {
        let names: BTreeMap<usize, String> = [
            (3, "Route 6".to_owned()),
            (7, "Victory Road".to_owned()),
            (8, "Victory Road".to_owned()),
        ]
        .into_iter()
        .collect();
        assert_eq!(field_map(&names, "Route 6 Rustling Bush Encounter"), Some(3));
        assert_eq!(field_map(&names, "Victory Road Outside 2 Sky Encounter"), None);
        assert_eq!(field_map(&names, "Terminus Cave Ceiling Encounter"), Some(0));
    }

    #[test]
    fn data_offsets_need_room() {
        let mut file = vec![0u8; 0x20 + XY_DATA_LENGTH];
        file[0x10] = 0x10;
        assert_eq!(data_offset(&file, Family::Xy), Some(0x20));
        file.pop();
        assert_eq!(data_offset(&file, Family::Xy), None);
        assert_eq!(data_offset(&[0; 4], Family::Xy), None);
    }
}
