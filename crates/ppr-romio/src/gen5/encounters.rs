//! Wild encounters (`WildPokemon`): one file per encounter table, with four seasons when its size says so. The game
//! data sees spring; writing copies it to the other seasons, as the original does, and rebuilds the Pokédex area
//! data (`PokedexAreaData`). Areas are named after their map (`MapTableFile`) and tagged with their location and
//! whether they are post-game.
//!
//! Deliberate choices:
//! - A slot's species is its alternate forme's own number (from 650) for a forme with data of its own; the formes
//!   the tables use for "any" (30 and 31), and Black 2's Corphish with a forme it lacks, read as forme 0 but are
//!   written back as they were while the slot keeps its species and forme.

use std::collections::BTreeMap;

use ppr_model::{EncounterKind, EncounterSet, WildEncounter};

use super::constants as c;
use super::entry::{Family, need};
use super::species::SPECIES_COUNT;
use super::store::{Store, Text, byte, put_byte, put_word, word};
use crate::RomError;
use crate::entries::RomEntry;

/// The bytes of one season of an encounter file.
const SEASON_LENGTH: usize = 232;
const SEASONS: usize = 4;
/// Where the slots start, after the seven rates.
const SLOTS: usize = 8;
const SLOT_SIZE: usize = 4;
/// Slots, names and kinds of the seven encounter types.
const TYPES: [(usize, &str, EncounterKind); 7] = [
    (12, "Grass/Cave", EncounterKind::Walking),
    (12, "Doubles Grass", EncounterKind::Walking),
    (12, "Shaking Spots", EncounterKind::Ambush),
    (5, "Surfing", EncounterKind::Surfing),
    (5, "Surfing Spots", EncounterKind::Ambush),
    (5, "Fishing", EncounterKind::Fishing),
    (5, "Fishing Spots", EncounterKind::Ambush),
];
/// Bytes of a map header, and where its name and wild encounter file are.
const MAP_HEADER: usize = 48;
const MAP_NAME: usize = 26;
const MAP_WILD: usize = 20;
const CORPHISH: u16 = 341;
/// Pokédex area data: per species, a byte for "the same in every season", then per season an "unobtainable" byte
/// and a byte per area.
const BW1_AREA_DATA_LENGTH: usize = 249;
const BW2_AREA_DATA_LENGTH: usize = 345;
const BW1_AREA_COUNT: usize = 61;
const BW2_AREA_COUNT: usize = 85;
// Areas of Black 2 and White 2 that only one of them has, by encounter file.
const BW2_ROUTE_4_AREA: i16 = 40;
const BW2_VICTORY_ROAD_AREA: i16 = 76;
const BW2_REVERSAL_MOUNTAIN_AREA: i16 = 73;
const B2_ROUTE_4_FILE: usize = 104;
const W2_ROUTE_4_FILE: usize = 105;
const B2_VICTORY_ROAD_ROOMS: [usize; 2] = [71, 73];
const W2_VICTORY_ROAD_ROOMS: [usize; 2] = [78, 79];
const B2_REVERSAL_MOUNTAIN: std::ops::RangeInclusive<usize> = 49..=54;
const W2_REVERSAL_MOUNTAIN: std::ops::RangeInclusive<usize> = 55..=60;

/// The post-game areas of Black and White, by area.
const BW1_POST_GAME: [usize; 136] = [
    233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 273, 274, 275, 276, 277, 278, 279, 127, 128, 129, 130, 131, 132,
    133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 249, 250, 251, 252, 253, 254, 255, 256, 257, 258, 259,
    260, 261, 262, 263, 264, 265, 266, 267, 268, 269, 270, 271, 272, 280, 281, 282, 300, 301, 302, 303, 156, 157, 158,
    159, 160, 161, 162, 151, 152, 153, 154, 163, 2, 3, 6, 7, 11, 12, 25, 26, 84, 85, 93, 94, 107, 108, 149, 150, 169,
    170, 178, 179, 184, 185, 190, 191, 195, 196, 205, 206, 223, 224, 228, 229, 291, 292, 298, 299, 306, 307, 14, 16,
    35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52,
];

/// The post-game areas of Black 2 and White 2, by area.
const BW2_POST_GAME: [usize; 206] = [
    355, 356, 357, 358, 359, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255, 256,
    257, 258, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 121, 122, 123, 124, 125, 126, 305, 306, 307, 308, 309, 310, 311,
    312, 313, 314, 315, 316, 289, 290, 291, 292, 293, 294, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 350, 351,
    352, 353, 354, 394, 395, 396, 445, 446, 447, 448, 407, 408, 409, 410, 411, 412, 413, 7, 8, 9, 10, 11, 0, 1, 2, 3,
    288, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 54, 55, 56, 57, 58, 59, 60, 61, 24, 25, 26, 27, 273, 274, 275,
    276, 277, 278, 279, 259, 260, 261, 262, 263, 264, 14, 15, 18, 19, 22, 23, 82, 83, 94, 95, 99, 100, 104, 105, 109,
    110, 114, 115, 119, 120, 129, 130, 133, 134, 139, 140, 145, 146, 190, 191, 200, 201, 208, 209, 218, 219, 225, 226,
    235, 236, 269, 270, 286, 287, 320, 321, 325, 326, 335, 336, 368, 369, 378, 379, 385, 386, 392, 393, 405, 406, 418,
    419, 425, 426, 432, 433, 439, 440, 443, 444, 451, 452,
];

/// The locations of the areas of Black and White, in order: how many areas, and where.
const BW1_LOCATIONS: [(usize, &str); 44] = [
    (4, "STRIATON CITY"),
    (4, "DRIFTVEIL CITY"),
    (5, "ICIRRUS CITY"),
    (4, "DREAMYARD"),
    (10, "PINWHEEL FOREST"),
    (2, "DESERT RESORT"),
    (31, "RELIC CASTLE"),
    (3, "COLD STORAGE"),
    (6, "CHARGESTONE CAVE"),
    (8, "TWIST MOUNTAIN"),
    (11, "DRAGONSPIRAL TOWER"),
    (39, "VICTORY ROAD"),
    (18, "GIANT CHASM"),
    (6, "P2 LABORATORY"),
    (4, "UNDELLA BAY"),
    (1, "DRIFTVEIL DRAWBRIDGE"),
    (7, "VILLAGE BRIDGE"),
    (1, "MARVELOUS BRIDGE"),
    (7, "ROUTE 1"),
    (2, "ROUTE 2"),
    (7, "ROUTE 3"),
    (12, "WELLSPRING CAVE"),
    (5, "ROUTE 4"),
    (3, "ROUTE 5"),
    (7, "ROUTE 6"),
    (6, "MISTRALTON CAVE"),
    (3, "ROUTE 7"),
    (4, "CELESTIAL TOWER"),
    (5, "ROUTE 8"),
    (5, "MOOR OF ICIRRUS"),
    (3, "ROUTE 9"),
    (10, "CHALLENGER'S CAVE"),
    (6, "ROUTE 10"),
    (7, "ROUTE 11"),
    (3, "ROUTE 12"),
    (7, "ROUTE 13"),
    (7, "ROUTE 14"),
    (7, "ABUNDANT SHRINE"),
    (3, "ROUTE 15"),
    (3, "ROUTE 16"),
    (7, "LOSTLORN FOREST"),
    (7, "ROUTE 18"),
    (4, "UNDELLA TOWN"),
    (4, "ROUTE 17"),
];

/// The locations of the areas of Black 2 and White 2.
const BW2_LOCATIONS: [(usize, &str); 59] = [
    (4, "STRIATON CITY"),
    (3, "CASTELIA CITY"),
    (5, "ICIRRUS CITY"),
    (4, "ASPERTIA CITY"),
    (4, "VIRBANK CITY"),
    (4, "HUMILAU CITY"),
    (4, "DREAMYARD"),
    (10, "PINWHEEL FOREST"),
    (2, "DESERT RESORT"),
    (8, "RELIC CASTLE"),
    (6, "CHARGESTONE CAVE"),
    (8, "TWIST MOUNTAIN"),
    (11, "DRAGONSPIRAL TOWER"),
    (2, "VICTORY ROAD"),
    (21, "GIANT CHASM"),
    (25, "CASTELIA SEWERS"),
    (6, "P2 LABORATORY"),
    (4, "UNDELLA BAY"),
    (10, "FLOCCESY RANCH"),
    (9, "VIRBANK COMPLEX"),
    (27, "REVERSAL MOUNTAIN"),
    (10, "STRANGE HOUSE"),
    (42, "VICTORY ROAD"),
    (10, "RELIC PASSAGE"),
    (26, "CLAY TUNNEL"),
    (8, "SEASIDE CAVE"),
    (7, "NATURE PRESERVE"),
    (1, "DRIFTVEIL DRAWBRIDGE"),
    (7, "VILLAGE BRIDGE"),
    (1, "MARVELOUS BRIDGE"),
    (7, "ROUTE 1"),
    (2, "ROUTE 2"),
    (7, "ROUTE 3"),
    (12, "WELLSPRING CAVE"),
    (10, "ROUTE 4"),
    (3, "ROUTE 5"),
    (7, "ROUTE 6"),
    (6, "MISTRALTON CAVE"),
    (3, "ROUTE 7"),
    (4, "CELESTIAL TOWER"),
    (5, "ROUTE 8"),
    (5, "MOOR OF ICIRRUS"),
    (3, "ROUTE 9"),
    (7, "ROUTE 11"),
    (3, "ROUTE 12"),
    (7, "ROUTE 13"),
    (7, "ROUTE 14"),
    (7, "ABUNDANT SHRINE"),
    (3, "ROUTE 15"),
    (3, "ROUTE 16"),
    (7, "LOSTLORN FOREST"),
    (7, "ROUTE 18"),
    (6, "ROUTE 19"),
    (7, "ROUTE 20"),
    (7, "ROUTE 22"),
    (7, "ROUTE 23"),
    (4, "UNDELLA TOWN"),
    (4, "ROUTE 17"),
    (4, "ROUTE 21"),
];

/// What the encounter sets were read with: the forme every slot stores.
#[derive(Debug, Clone, Default)]
pub(super) struct Formes(Vec<Vec<u8>>);

/// The name of each map's wild encounter file.
fn map_names(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    path: &str,
) -> Result<BTreeMap<usize, String>, RomError> {
    let names = store
        .strings(Text::Strings, need(entry, "MapNamesTextOffset")?)?
        .to_vec();
    let headers = store.narc_file(path, 0)?;
    let mut out = BTreeMap::new();
    for map in 0..headers.len() / MAP_HEADER {
        let at = map * MAP_HEADER;
        let name = names
            .get(usize::from(byte(headers, at + MAP_NAME)?))
            .cloned()
            .unwrap_or_default();
        let wild = match family {
            Family::Bw2 => Some(byte(headers, at + MAP_WILD)?)
                .filter(|&w| w != 0xFF)
                .map(usize::from),
            Family::Bw1 => Some(word(headers, at + MAP_WILD)?)
                .filter(|&w| w != 0xFFFF)
                .map(usize::from),
        };
        if let Some(w) = wild {
            out.insert(w, name);
        }
    }
    Ok(out)
}

/// The encounter sets of spring, tagged.
pub(super) fn read(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    paths: &super::Paths,
) -> Result<(Vec<EncounterSet>, Formes), RomError> {
    let names = map_names(store, entry, family, &paths.map_table)?;
    let files = store.narc(&paths.wild)?.files.clone();
    let mut sets = Vec::new();
    let mut formes = Vec::new();
    for (index, data) in files.iter().enumerate() {
        let name = names.get(&index).map_or("? Unknown ?", String::as_str);
        let mut at = SLOTS;
        for (i, &(count, type_name, kind)) in TYPES.iter().enumerate() {
            let rate = byte(data, i)?;
            if rate != 0 {
                let mut encounters = Vec::new();
                let mut own = Vec::new();
                for s in 0..count {
                    let slot = at + s * SLOT_SIZE;
                    let value = word(data, slot)?;
                    let base = value & 0x7FF;
                    // The tables have formes 30 and 31 for "any", and Black 2 a Corphish with a forme it lacks.
                    let stored = u8::try_from(value >> 11).unwrap_or_default();
                    let forme = if stored >= 30 || base == CORPHISH { 0 } else { stored };
                    encounters.push(WildEncounter {
                        forme,
                        species: super::forme_species(family, base, forme),
                        min_level: byte(data, slot + 2)?,
                        max_level: byte(data, slot + 3)?,
                    });
                    own.push(stored);
                }
                sets.push(EncounterSet {
                    kind,
                    map: u16::try_from(index).ok(),
                    name: format!("{name} {type_name}"),
                    location: None,
                    post_game: false,
                    post_game_from: None,
                    rate,
                    banned: Vec::new(),
                    encounters,
                });
                formes.push(own);
            }
            at += count * SLOT_SIZE;
        }
    }
    tag(&mut sets, family)?;
    Ok((sets, Formes(formes)))
}

/// Gives each set its location and marks the post-game ones.
fn tag(sets: &mut [EncounterSet], family: Family) -> Result<(), RomError> {
    let (locations, post_game): (&[(usize, &str)], &[usize]) = match family {
        Family::Bw1 => (&BW1_LOCATIONS, &BW1_POST_GAME),
        Family::Bw2 => (&BW2_LOCATIONS, &BW2_POST_GAME),
    };
    let tags: Vec<&str> = locations
        .iter()
        .flat_map(|&(count, tag)| std::iter::repeat_n(tag, count))
        .collect();
    if tags.len() != sets.len() {
        return Err(RomError::Malformed(format!(
            "{} encounter sets where {} were expected",
            sets.len(),
            tags.len()
        )));
    }
    for (set, tag) in sets.iter_mut().zip(tags) {
        set.location = Some(tag.to_owned());
    }
    for &i in post_game {
        if let Some(set) = sets.get_mut(i) {
            set.post_game = true;
        }
    }
    Ok(())
}

/// Writes the sets into spring and copies spring to the other seasons, then rebuilds the Pokédex area data.
pub(super) fn write(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    paths: &super::Paths,
    formes: &Formes,
    old: &[EncounterSet],
    new: &[EncounterSet],
) -> Result<(), RomError> {
    if new.len() != old.len() || new.len() != formes.0.len() {
        return Err(RomError::Malformed("the encounter sets must keep their number".into()));
    }
    let narc = store.narc_mut(&paths.wild)?;
    let mut set = 0;
    for data in &mut narc.files {
        let mut at = SLOTS;
        for (i, &(count, _, _)) in TYPES.iter().enumerate() {
            if byte(data, i)? != 0 {
                let (encounters, before, own) = (&new[set].encounters, &old[set].encounters, &formes.0[set]);
                for (s, &e) in encounters.iter().enumerate().take(count) {
                    let keep = before
                        .get(s)
                        .is_some_and(|b| (b.species, b.forme) == (e.species, e.forme));
                    let forme = if keep {
                        own.get(s).copied().unwrap_or_default()
                    } else {
                        e.forme
                    };
                    if forme >= 32 {
                        return Err(RomError::Malformed(format!("wild forme {forme}")));
                    }
                    let slot = at + s * SLOT_SIZE;
                    put_word(data, slot, (u16::from(forme) << 11) + super::base_species(e.species))?;
                    put_byte(data, slot + 2, e.min_level)?;
                    put_byte(data, slot + 3, e.max_level)?;
                }
                set += 1;
            }
            at += count * SLOT_SIZE;
        }
        if data.len() > SEASON_LENGTH {
            for season in 1..SEASONS {
                data.copy_within(0..SEASON_LENGTH, season * SEASON_LENGTH);
            }
        }
    }
    let files = narc.files.clone();
    update_area_data(store, entry, family, paths, &files)
}

/// The Pokédex area of encounter file `file`, `None` for none or for an area of the other version.
fn area_of(entry: &RomEntry, family: Family, file: usize) -> Option<usize> {
    let map: &[i16] = family.pick(&c::BW1_WILD_FILE_TO_AREA, &c::BW2_WILD_FILE_TO_AREA);
    let area = *map.get(file)?;
    if family == Family::Bw2 {
        // The third letter of the game code is D for White 2, E for Black 2.
        let white = entry.code.as_bytes().get(2) == Some(&b'D');
        let other_version = match area {
            BW2_ROUTE_4_AREA => {
                (file == B2_ROUTE_4_FILE && white)
                    || (file == W2_ROUTE_4_FILE && entry.code.as_bytes().get(2) == Some(&b'E'))
            }
            BW2_VICTORY_ROAD_AREA if white => B2_VICTORY_ROAD_ROOMS.contains(&file),
            BW2_VICTORY_ROAD_AREA => W2_VICTORY_ROAD_ROOMS.contains(&file),
            BW2_REVERSAL_MOUNTAIN_AREA if white => B2_REVERSAL_MOUNTAIN.contains(&file),
            BW2_REVERSAL_MOUNTAIN_AREA => W2_REVERSAL_MOUNTAIN.contains(&file),
            _ => false,
        };
        if other_version {
            return None;
        }
    }
    usize::try_from(area).ok()
}

/// Rebuilds where the Pokédex says each species lives, by season, from the encounter files.
fn update_area_data(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    paths: &super::Paths,
    encounters: &[Vec<u8>],
) -> Result<(), RomError> {
    let length = family.pick(BW1_AREA_DATA_LENGTH, BW2_AREA_DATA_LENGTH);
    let areas = family.pick(BW1_AREA_COUNT, BW2_AREA_COUNT);
    let species = usize::from(SPECIES_COUNT);
    let mut files = vec![vec![0u8; length]; species];
    for (file, data) in encounters.iter().enumerate() {
        let area = area_of(entry, family, file);
        for season in 0..SEASONS {
            let start = if data.len() > SEASON_LENGTH {
                season * SEASON_LENGTH
            } else {
                0
            };
            let mut at = SLOTS;
            for (i, &(count, _, _)) in TYPES.iter().enumerate() {
                if byte(data, start + i)? != 0 {
                    for s in 0..count {
                        let n = usize::from(word(data, start + at + s * SLOT_SIZE)? & 0x3FF);
                        if let (Some(area), Some(out)) = (area, n.checked_sub(1).and_then(|n| files.get_mut(n))) {
                            out[season * (areas + 1) + 2 + area] |= 1 << i;
                        }
                    }
                }
                at += count * SLOT_SIZE;
            }
        }
    }
    let narc = store.narc_mut(&paths.area_data)?;
    for (i, file) in files.into_iter().enumerate() {
        let mut file = file;
        for season in 0..SEASONS {
            let row = season * (areas + 1);
            if file[row + 2..row + 2 + areas].iter().all(|&b| b == 0) {
                file[row + 1] = 1;
            }
        }
        let same_every_season = (0..areas).all(|a| {
            let spring = file[a + 2];
            (1..SEASONS).all(|s| file[s * (areas + 1) + a + 2] == spring)
        });
        if same_every_season {
            file[0] = 1;
        }
        if let Some(slot) = narc.files.get_mut(i) {
            *slot = file;
        }
    }
    Ok(())
}
