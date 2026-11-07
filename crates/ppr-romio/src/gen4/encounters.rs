//! Wild encounters, read and written, with their map names, location tags and post-game flags, and the
//! Pokédex's area data, which follows them.
//!
//! Diamond, Pearl and Platinum have a file per map of grass (12 slots), the slots that swarms, the Poké Radar,
//! the time of day and the Game Boy Advance slot replace, and the surfing and fishing slots; outside it are the
//! Mt. Coronet Feebas tiles, the honey trees, and the rotating Pokémon of the Trophy Garden and the Great Marsh.
//! `HeartGold` and `SoulSilver` have a file per map of grass (levels shared by morning, day and night), radio,
//! surfing, Rock Smash, fishing and swarm slots; outside it are the headbutt trees and the Bug Catching Contest.
//! Encounters by time of day are read as the day's.
//!
//! Deliberate choices:
//! - Only the areas that changed are written, so an unchanged game saves byte for byte; the time of day slots of
//!   a Diamond, Pearl or Platinum grass area get the species of the slots they replace when that area changed.
//! - Areas whose slots may be empty (radio, swarms, headbutt trees, the Bug Catching Contest) are written back to
//!   the slots they were read from, also when an empty slot sits between full ones.
//! - A night fishing replacement of species 0 leaves the fishing slot it would replace as it is in the Pokédex.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, EncounterKind, EncounterSet, WildEncounter, WildLevels};

use super::Rom;
use super::constants::{self as c, Family};
use super::encounter_tags as tags;
use super::files::{byte, find_after, long, put, put_long, put_word, word};
use super::pokedex_maps as dex;
use crate::RomError;

/// Unown, whose letters can't all be met outside the Ruins.
pub(super) const WILD_BANNED: [DexNumber; 1] = [201];

/// The slot each of the 20 replacing slots of a grass area replaces: swarm ×2, day ×2, night ×2, Poké Radar ×4,
/// Game Boy Advance ×10.
const DPPT_ALTERNATE_SLOTS: [usize; 20] = [0, 1, 2, 3, 2, 3, 4, 5, 10, 11, 8, 9, 8, 9, 8, 9, 8, 9, 8, 9];
const DPPT_WATER_AREA_SIZE: usize = 44;
const DPPT_WATER_START: usize = 204;
const DPPT_WATER_NAMES: [&str; 5] = ["Surfing", "Filler", "Old Rod", "Good Rod", "Super Rod"];
const DPPT_TIME_OF_DAY: usize = 108;
const FEEBAS_LEVEL_PREFIX: &str = "019813B0F0BD";
const HONEY_TREE_LEVEL_PREFIX: &str = "F0BDF0B589B0051C0C1C";
const MT_CORONET_MAP: u16 = 10;
const TROPHY_GARDEN_MAP: u16 = 117;
/// The Trophy Garden and Great Marsh grass areas, whose slots 6 and 7 give the rotating Pokémon their levels.
const TROPHY_GARDEN_GRASS_DP: usize = 304;
const TROPHY_GARDEN_GRASS_PT: usize = 308;
const MARSH_GRASS_DP: [usize; 6] = [76, 82, 88, 94, 100, 102];
const MARSH_GRASS_PT: [usize; 6] = [76, 82, 88, 94, 100, 106];

const HGSS_AMOUNTS: [usize; 6] = [0, 5, 2, 5, 5, 5];
const HGSS_NON_WALKING: [(&str, EncounterKind); 6] = [
    ("", EncounterKind::Unused),
    ("Surfing", EncounterKind::Surfing),
    ("Rock Smash", EncounterKind::Interact),
    ("Old Rod", EncounterKind::Fishing),
    ("Good Rod", EncounterKind::Fishing),
    ("Super Rod", EncounterKind::Fishing),
];
const NATIONAL_PARK_MAP: u16 = 23;
/// The second half of the National Park, merged into the first.
const NATIONAL_PARK_SECOND_MAP: u16 = 24;
/// A headbutt file of an unused copy of Route 16.
const UNUSED_HEADBUTT_FILE: usize = 24;
const HEADBUTT_SLOTS: usize = 18;
const BCC_AREAS: [(usize, &str); 4] = [
    (0, "Bug Catching Contest (Pre-National Dex)"),
    (80, "Bug Catching Contest (Post-National Dex, Tuesdays)"),
    (160, "Bug Catching Contest (Post-National Dex, Thursdays)"),
    (240, "Bug Catching Contest (Post-National Dex, Saturdays)"),
];
const HGSS_GOOD_ROD_REPLACED: usize = 3;
const HGSS_SUPER_ROD_REPLACED: usize = 1;
const HGSS_NIGHT_FISHING: usize = 192;

/// The Pokédex area data: files per species and time of day, this far apart.
const POKEDEX_AREA_DATA_SIZE: usize = 495;
const DEX_MT_CORONET: u32 = 3;
const DEX_GREAT_MARSH: u32 = 4;
const DEX_TROPHY_GARDEN: u32 = 14;
const DEX_FLOAROMA_MEADOW: u32 = 21;
const DEX_HONEY_TREES: [u32; 20] = [
    6, 7, 17, 18, 19, 20, 21, 22, 23, 24, 26, 27, 28, 29, 30, 31, 34, 36, 37, 50,
];

/// Where an encounter area is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Place {
    /// Grass of a Diamond, Pearl or Platinum map file: 12 slots of level and species.
    DpptGrass(usize),
    /// The replacing slots of a map file that are used, at these offsets.
    DpptReplacing(usize, Vec<usize>),
    /// Surfing or fishing slots of a map file: 5 slots of levels and species.
    DpptWater(usize, usize),
    Feebas,
    /// A honey tree group: 6 species in a file of the extra encounters.
    HoneyTree(usize),
    /// Rotating species in a file of the extra encounters, at the levels of grass areas.
    Rotating(usize),
    /// Grass of a `HeartGold` or `SoulSilver` map file: 12 levels, and 12 species for each time of day.
    HgssGrass(usize),
    /// Species words of a map file, at these offsets.
    HgssSpecies(usize, Vec<usize>),
    /// Surfing, Rock Smash or fishing slots of a map file: levels and species.
    HgssWater(usize, usize, usize),
    /// Headbutt slots of a headbutt file, at these offsets.
    Headbutt(usize, Vec<usize>),
    /// Bug Catching Contest slots, at these offsets.
    Contest(Vec<usize>),
}

/// Where each encounter area of the game data is.
#[derive(Debug, Clone, Default)]
pub(super) struct Encounters {
    places: Vec<Place>,
}

impl Encounters {
    /// How each set stores its levels, in game-data order. Grass keeps one level per slot; slots that only store a
    /// species take their levels from grass slots; all honey trees share one range, so their levels stay as they
    /// are rather than an edit of one tree changing every other.
    pub(super) fn levels(&self) -> Vec<WildLevels> {
        self.places
            .iter()
            .map(|place| match place {
                Place::DpptGrass(_) | Place::HgssGrass(_) => WildLevels::Single,
                Place::DpptReplacing(..) | Place::HoneyTree(_) | Place::Rotating(_) | Place::HgssSpecies(..) => {
                    WildLevels::Fixed
                }
                Place::DpptWater(..)
                | Place::Feebas
                | Place::HgssWater(..)
                | Place::Headbutt(..)
                | Place::Contest(_) => WildLevels::Range,
            })
            .collect()
    }
}

fn kind_and_name(map_name: &str, what: &str) -> String {
    format!("{map_name} {what}")
}

fn set(name: String, map: Option<u16>, kind: EncounterKind, rate: u8, encounters: Vec<WildEncounter>) -> EncounterSet {
    EncounterSet {
        kind,
        map,
        name,
        location: None,
        post_game: false,
        post_game_from: None,
        rate,
        banned: Vec::new(),
        encounters,
    }
}

fn level(v: u32) -> u8 {
    u8::try_from(v).unwrap_or(u8::MAX)
}

fn species(v: u32) -> Result<DexNumber, RomError> {
    DexNumber::try_from(v).map_err(|_| RomError::Malformed(format!("species {v:#x}")))
}

fn at_level(species: DexNumber, level: u8) -> WildEncounter {
    WildEncounter {
        forme: 0,
        species,
        min_level: level,
        max_level: level,
    }
}

/// Map names by wild encounter map.
type MapNames = BTreeMap<u16, String>;

/// The name of each map of wild encounters (by wild encounter file) and, in `HeartGold` and `SoulSilver`, of each
/// map header (for the headbutt files).
fn map_names(rom: &mut Rom) -> Result<(MapNames, BTreeMap<usize, String>), RomError> {
    let path = rom.entry.file("MapTableFile")?;
    let headers = rom.files.file(&path)?.len() / 16;
    let base = rom.entry.need("MapTableARM9Offset")?;
    let names = rom.strings_of("MapNamesTextOffset")?;
    let wide = rom.entry.int("MapTableNameIndexSize") == 2;
    let hgss = rom.family() == Family::HeartGoldSoulSilver;
    let arm9 = rom.files.arm9_ref();
    let (mut wild, mut headbutt) = (BTreeMap::new(), BTreeMap::new());
    for map in 0..headers {
        let at = base + map * 24;
        let index = if wide {
            usize::from(word(arm9, at + 18)?)
        } else {
            usize::from(byte(arm9, at + 18)?)
        };
        let name = names.get(index).cloned().unwrap_or_default();
        if hgss {
            let wild_set = byte(arm9, at)?;
            if wild_set != 0xFF {
                wild.insert(u16::from(wild_set), name.clone());
            }
            headbutt.insert(map, name);
        } else {
            let wild_set = word(arm9, at + 14)?;
            if wild_set != 0xFFFF {
                wild.insert(wild_set, name);
            }
        }
    }
    Ok((wild, headbutt))
}

/// Reads the encounter areas, tagged.
pub(super) fn read(rom: &mut Rom) -> Result<(Encounters, Vec<EncounterSet>), RomError> {
    let (mut wild_names, headbutt_names) = map_names(rom)?;
    let mut sets = Vec::new();
    let mut places = Vec::new();
    if rom.family() == Family::HeartGoldSoulSilver {
        read_main_hgss(rom, &mut wild_names, &mut sets, &mut places)?;
        read_headbutt(rom, &wild_names, &headbutt_names, &mut sets, &mut places)?;
        read_contest(rom, &mut sets, &mut places)?;
    } else {
        read_main_dppt(rom, &mut wild_names, &mut sets, &mut places)?;
        read_extra_dppt(rom, &mut sets, &mut places)?;
    }
    tag(rom.family(), &mut sets)?;
    Ok((Encounters { places }, sets))
}

fn read_main_dppt(
    rom: &mut Rom,
    names: &mut BTreeMap<u16, String>,
    sets: &mut Vec<EncounterSet>,
    places: &mut Vec<Place>,
) -> Result<(), RomError> {
    let path = rom.entry.file("WildPokemon")?;
    let files = rom.files.narc(&path)?.files.clone();
    for (file, b) in files.iter().enumerate() {
        let map = u16::try_from(file).map_err(|_| RomError::Malformed("too many maps".into()))?;
        let name = names.entry(map).or_insert_with(|| "? Unknown ?".to_owned()).clone();
        let rate = long(b, 0)?;
        if rate != 0 {
            let grass = (0..12)
                .map(|i| Ok(at_level(species(long(b, 8 + i * 8)?)?, level(long(b, 4 + i * 8)?))))
                .collect::<Result<Vec<_>, RomError>>()?;
            let mut replacing = Vec::new();
            let mut offsets = Vec::new();
            for (i, &slot) in DPPT_ALTERNATE_SLOTS.iter().enumerate() {
                if (2..=5).contains(&i) {
                    // Time of day slots, read as the day's.
                    continue;
                }
                let at = 100 + i * 4 + if i >= 10 { 24 } else { 0 };
                let n = long(b, at)?;
                if (1..=c::SPECIES_COUNT).contains(&(n as usize)) {
                    replacing.push(at_level(species(n)?, grass[slot].min_level));
                    offsets.push(at);
                }
            }
            let rate = level(rate);
            sets.push(set(
                kind_and_name(&name, "Grass/Cave"),
                Some(map),
                EncounterKind::Walking,
                rate,
                grass,
            ));
            places.push(Place::DpptGrass(file));
            if !replacing.is_empty() {
                sets.push(set(
                    kind_and_name(&name, "Swarm/Radar/GBA"),
                    Some(map),
                    EncounterKind::Special,
                    rate,
                    replacing,
                ));
                places.push(Place::DpptReplacing(file, offsets));
            }
        }
        for (i, what) in DPPT_WATER_NAMES.iter().enumerate() {
            if i == 1 {
                continue;
            }
            let at = DPPT_WATER_START + i * DPPT_WATER_AREA_SIZE;
            let rate = long(b, at)?;
            if rate == 0 {
                continue;
            }
            let encounters = (0..5)
                .map(|j| {
                    let levels = long(b, at + 4 + j * 8)?;
                    Ok(WildEncounter {
                        forme: 0,
                        species: species(long(b, at + 8 + j * 8)?)?,
                        min_level: level(levels >> 8),
                        max_level: level(levels & 0xFF),
                    })
                })
                .collect::<Result<Vec<_>, RomError>>()?;
            let kind = if i == 0 {
                EncounterKind::Surfing
            } else {
                EncounterKind::Fishing
            };
            sets.push(set(
                kind_and_name(&name, what),
                Some(map),
                kind,
                level(rate),
                encounters,
            ));
            places.push(Place::DpptWater(file, at + 4));
        }
    }
    Ok(())
}

/// `count` species of an extra encounter file, at level 1 until their levels are known.
fn extra_species(data: &[u8], count: usize) -> Result<Vec<WildEncounter>, RomError> {
    (0..count)
        .map(|i| Ok(at_level(species(long(data, i * 4)?)?, 1)))
        .collect()
}

fn with_levels(encounters: &mut [WildEncounter], min: u8, max: u8) {
    for e in encounters {
        e.min_level = min;
        e.max_level = max;
    }
}

fn read_extra_dppt(rom: &mut Rom, sets: &mut Vec<EncounterSet>, places: &mut Vec<Place>) -> Result<(), RomError> {
    let path = rom.entry.file("ExtraEncounters")?;
    let extra = rom.files.narc(&path)?.files.clone();
    let overlay = rom.files.overlay(rom.entry.need("EncounterOvlNumber")?)?.clone();
    let file = |i: usize| {
        extra
            .get(i)
            .ok_or_else(|| RomError::Malformed(format!("no extra encounter file {i}")))
    };

    let mut feebas = extra_species(file(0)?, 1)?;
    if let Some(at) = find_after(&overlay, FEEBAS_LEVEL_PREFIX) {
        with_levels(&mut feebas, byte(&overlay, at + 4)?, byte(&overlay, at)?);
    }
    sets.push(set(
        "Mt. Coronet Feebas Tiles".into(),
        Some(MT_CORONET_MAP),
        EncounterKind::Special,
        1,
        feebas,
    ));
    places.push(Place::Feebas);

    for (i, &tree) in rom.entry.array("HoneyTreeOffsets").iter().enumerate() {
        let mut encounters = extra_species(file(tree)?, 6)?;
        if let Some(at) = find_after(&overlay, HONEY_TREE_LEVEL_PREFIX) {
            // The original code can't be read for its lowest level, which is 5; once written, it holds it.
            let min = if byte(&overlay, at + 46)? == 0x0B && byte(&overlay, at + 47)? == 0x2E {
                5
            } else {
                byte(&overlay, at + 46)?
            };
            with_levels(&mut encounters, min, byte(&overlay, at + 102)?);
        }
        sets.push(set(
            format!("Honey Tree Group {}", i + 1),
            None,
            EncounterKind::Interact,
            1,
            encounters,
        ));
        places.push(Place::HoneyTree(tree));
    }

    let dp = rom.family() == Family::DiamondPearl;
    let grass_level = |sets: &[EncounterSet], area: usize, slot: usize| {
        sets.get(area)
            .and_then(|s| s.encounters.get(slot))
            .map(|e| e.min_level)
            .ok_or_else(|| RomError::Malformed(format!("no grass area {area}")))
    };
    let garden = if dp {
        TROPHY_GARDEN_GRASS_DP
    } else {
        TROPHY_GARDEN_GRASS_PT
    };
    let (a, b) = (grass_level(sets, garden, 6)?, grass_level(sets, garden, 7)?);
    let mut encounters = extra_species(file(8)?, 16)?;
    with_levels(&mut encounters, a.min(b), a.max(b));
    sets.push(set(
        "Trophy Garden Rotating Pokemon (via Mr. Backlot)".into(),
        Some(TROPHY_GARDEN_MAP),
        EncounterKind::Special,
        1,
        encounters,
    ));
    places.push(Place::Rotating(8));

    let marsh: &[usize] = if dp { &MARSH_GRASS_DP } else { &MARSH_GRASS_PT };
    let mut levels = Vec::new();
    for &area in marsh {
        levels.push(grass_level(sets, area, 6)?);
        levels.push(grass_level(sets, area, 7)?);
    }
    let (min, max) = (
        levels.iter().copied().min().unwrap_or(100).min(100),
        levels.iter().copied().max().unwrap_or(0),
    );
    for (file_index, status) in [(9, "(Post-National Dex)"), (10, "(Pre-National Dex)")] {
        let mut encounters = extra_species(file(file_index)?, 32)?;
        with_levels(&mut encounters, min, max);
        sets.push(set(
            format!("Great Marsh Rotating Pokemon {status}"),
            None,
            EncounterKind::Special,
            1,
            encounters,
        ));
        places.push(Place::Rotating(file_index));
    }
    Ok(())
}

/// The species words at `offsets` that are not 0, at level 1, and where they are.
fn optional_species(
    b: &[u8],
    offsets: impl IntoIterator<Item = usize>,
) -> Result<(Vec<WildEncounter>, Vec<usize>), RomError> {
    let (mut encounters, mut at) = (Vec::new(), Vec::new());
    for o in offsets {
        let n = word(b, o)?;
        if n != 0 {
            encounters.push(at_level(n, 1));
            at.push(o);
        }
    }
    Ok((encounters, at))
}

fn read_main_hgss(
    rom: &mut Rom,
    names: &mut BTreeMap<u16, String>,
    sets: &mut Vec<EncounterSet>,
    places: &mut Vec<Place>,
) -> Result<(), RomError> {
    let path = rom.entry.file("WildPokemon")?;
    let files = rom.files.narc(&path)?.files.clone();
    for (file, b) in files.iter().enumerate() {
        let mut map = u16::try_from(file).map_err(|_| RomError::Malformed("too many maps".into()))?;
        if map == NATIONAL_PARK_SECOND_MAP {
            map = NATIONAL_PARK_MAP;
        }
        let name = names.entry(map).or_insert_with(|| "? Unknown ?".to_owned()).clone();
        let rates: Vec<u8> = (0..6).map(|i| byte(b, i)).collect::<Result<_, _>>()?;
        if rates[0] != 0 {
            let grass = (0..12)
                .map(|i| Ok(at_level(word(b, 44 + i * 2)?, byte(b, 8 + i)?)))
                .collect::<Result<Vec<_>, RomError>>()?;
            sets.push(set(
                kind_and_name(&name, "Grass/Cave"),
                Some(map),
                EncounterKind::Walking,
                rates[0],
                grass,
            ));
            places.push(Place::HgssGrass(file));
        }
        let mut optional = |what: &str, offsets: std::ops::Range<usize>, kind, sets: &mut Vec<EncounterSet>| {
            let (encounters, at) = optional_species(b, offsets.step_by(2))?;
            if !encounters.is_empty() {
                sets.push(set(kind_and_name(&name, what), Some(map), kind, 1, encounters));
                places.push(Place::HgssSpecies(file, at));
            }
            Ok::<_, RomError>(())
        };
        optional("Hoenn/Sinnoh Radio", 92..100, EncounterKind::Special, sets)?;
        let mut at = 100;
        let mut water = Vec::new();
        for i in 1..6 {
            if rates[i] != 0 {
                let encounters = (0..HGSS_AMOUNTS[i])
                    .map(|j| {
                        Ok(WildEncounter {
                            forme: 0,
                            species: word(b, at + 2 + j * 4)?,
                            min_level: byte(b, at + j * 4)?,
                            max_level: byte(b, at + 1 + j * 4)?,
                        })
                    })
                    .collect::<Result<Vec<_>, RomError>>()?;
                let (what, kind) = HGSS_NON_WALKING[i];
                water.push((
                    set(kind_and_name(&name, what), Some(map), kind, rates[i], encounters),
                    Place::HgssWater(file, at, HGSS_AMOUNTS[i]),
                ));
            }
            at += 4 * HGSS_AMOUNTS[i];
        }
        for (s, p) in water {
            sets.push(s);
            places.push(p);
        }
        let mut optional = |what: &str, offsets: std::ops::Range<usize>, kind, sets: &mut Vec<EncounterSet>| {
            let (encounters, at) = optional_species(b, offsets.step_by(2))?;
            if !encounters.is_empty() {
                sets.push(set(kind_and_name(&name, what), Some(map), kind, 1, encounters));
                places.push(Place::HgssSpecies(file, at));
            }
            Ok::<_, RomError>(())
        };
        optional("Swarms", at..at + 4, EncounterKind::Special, sets)?;
        optional(
            "Night Fishing Replacement",
            at + 4..at + 6,
            EncounterKind::Fishing,
            sets,
        )?;
        optional("Fishing Swarm", at + 6..at + 8, EncounterKind::Special, sets)?;
    }
    Ok(())
}

fn read_headbutt(
    rom: &mut Rom,
    wild_names: &BTreeMap<u16, String>,
    headbutt_names: &BTreeMap<usize, String>,
    sets: &mut Vec<EncounterSet>,
    places: &mut Vec<Place>,
) -> Result<(), RomError> {
    let path = rom.entry.file("HeadbuttPokemon")?;
    let files = rom.files.narc(&path)?.files.clone();
    let mut last_created = u16::try_from(wild_names.len()).unwrap_or(u16::MAX).wrapping_sub(1);
    let mut created = BTreeSet::new();
    for (file, b) in files.iter().enumerate() {
        // A file of four bytes counts the trees of a map without any.
        if b.len() == 4 || file == UNUSED_HEADBUTT_FILE {
            continue;
        }
        let (mut encounters, mut at) = (Vec::new(), Vec::new());
        for i in 0..HEADBUTT_SLOTS {
            let o = 4 + i * 4;
            let n = word(b, o)?;
            if n != 0 {
                encounters.push(WildEncounter {
                    forme: 0,
                    species: n,
                    min_level: byte(b, o + 2)?,
                    max_level: byte(b, o + 3)?,
                });
                at.push(o);
            }
        }
        if encounters.is_empty() {
            continue;
        }
        let name = headbutt_names.get(&file).cloned().unwrap_or_default();
        // Headbutt files count maps differently; the map is the wild encounter map of the same name, or a new
        // number of its own.
        let map = if let Some((&m, _)) = wild_names.iter().find(|(_, n)| **n == name) {
            m
        } else {
            loop {
                last_created = last_created.wrapping_add(1);
                if !wild_names.contains_key(&last_created) && !created.contains(&last_created) {
                    break;
                }
            }
            created.insert(last_created);
            last_created
        };
        sets.push(set(
            kind_and_name(&name, "Headbutt"),
            Some(map),
            EncounterKind::Interact,
            1,
            encounters,
        ));
        places.push(Place::Headbutt(file, at));
    }
    Ok(())
}

fn read_contest(rom: &mut Rom, sets: &mut Vec<EncounterSet>, places: &mut Vec<Place>) -> Result<(), RomError> {
    let path = rom.entry.file("BCCWilds")?;
    let b = rom.files.file(&path)?.clone();
    for (start, name) in BCC_AREAS {
        let (mut encounters, mut at) = (Vec::new(), Vec::new());
        for i in 0..10 {
            let o = start + i * 8;
            let n = word(&b, o)?;
            if n != 0 {
                encounters.push(WildEncounter {
                    forme: 0,
                    species: n,
                    min_level: byte(&b, o + 2)?,
                    max_level: byte(&b, o + 3)?,
                });
                at.push(o);
            }
        }
        if !encounters.is_empty() {
            sets.push(set(
                name.to_owned(),
                Some(NATIONAL_PARK_MAP),
                EncounterKind::Special,
                1,
                encounters,
            ));
            places.push(Place::Contest(at));
        }
    }
    Ok(())
}

/// The location tags of the areas in order, and the post-game areas, whole and from an encounter on.
fn tag(family: Family, sets: &mut [EncounterSet]) -> Result<(), RomError> {
    let expand = |runs: &[(usize, &'static str)]| -> Vec<&'static str> {
        runs.iter().flat_map(|&(n, t)| std::iter::repeat_n(t, n)).collect()
    };
    let (locations, post_game, partly): (Vec<&str>, &[usize], &[usize]) = match family {
        Family::DiamondPearl => (expand(tags::TAGS_DP), &tags::POST_GAME_DP, &[]),
        Family::Platinum => {
            let mut dp = expand(tags::TAGS_DP);
            let rest = dp.split_off(tags::PT_EXTRA_GREAT_MARSH_AT);
            dp.extend(["GREAT MARSH"; 4]);
            dp.extend(rest);
            (dp, &tags::POST_GAME_PT, &[])
        }
        Family::HeartGoldSoulSilver => {
            let mut all = expand(tags::TAGS_HGSS);
            all.extend(tags::TAGS_HGSS_HEADBUTT);
            all.extend(expand(tags::TAGS_HGSS_AFTER_HEADBUTT));
            (all, &tags::POST_GAME_HGSS, &tags::PARTLY_POST_GAME_HGSS)
        }
    };
    if sets.len() != locations.len() {
        return Err(RomError::Malformed(format!(
            "{} encounter areas, where {} were expected",
            sets.len(),
            locations.len()
        )));
    }
    for (s, l) in sets.iter_mut().zip(locations) {
        s.location = Some(l.to_owned());
    }
    for &i in post_game {
        sets[i].post_game = true;
    }
    for &i in partly {
        sets[i].post_game_from = Some(tags::PARTLY_POST_GAME_CUTOFF);
    }
    Ok(())
}

/// Writes the areas that changed, and the Pokédex area data when any did.
pub(super) fn write(
    rom: &mut Rom,
    state: &Encounters,
    loaded: &[EncounterSet],
    sets: &[EncounterSet],
) -> Result<(), RomError> {
    if sets.len() != state.places.len() {
        return Err(RomError::Malformed(format!(
            "{} encounter areas, where the game has {}",
            sets.len(),
            state.places.len()
        )));
    }
    let wild = rom.entry.file("WildPokemon")?;
    let mut honey_tree_levels = None;
    for ((place, new), old) in state.places.iter().zip(sets).zip(loaded) {
        if new.encounters == old.encounters {
            continue;
        }
        write_area(rom, &wild, place, new)?;
        if let (Place::HoneyTree(_), Some(e)) = (place, new.encounters.first()) {
            honey_tree_levels = Some((e.min_level, e.max_level));
        }
    }
    if let Some((min, max)) = honey_tree_levels {
        write_honey_tree_levels(rom, min, max)?;
    }
    if sets
        .iter()
        .zip(loaded)
        .any(|(new, old)| new.encounters != old.encounters)
    {
        if rom.family() == Family::HeartGoldSoulSilver {
            update_pokedex_hgss(rom, sets)?;
        } else {
            update_pokedex_dppt(rom, sets)?;
        }
    }
    Ok(())
}

/// Writes an area where it is stored.
#[expect(clippy::too_many_lines, reason = "one block per way an area is stored")]
fn write_area(rom: &mut Rom, wild: &str, place: &Place, new: &EncounterSet) -> Result<(), RomError> {
    let e = &new.encounters;
    let need = |n: usize| {
        if e.len() == n {
            Ok(())
        } else {
            Err(RomError::Malformed(format!(
                "{} has {n} encounters, not {}",
                new.name,
                e.len()
            )))
        }
    };
    match place {
        Place::DpptGrass(file) => {
            need(12)?;
            one_level(new)?;
            let b = rom.files.narc_file(wild, *file)?;
            for (i, enc) in e.iter().enumerate() {
                put_long(b, 4 + i * 8, u32::from(enc.min_level))?;
                put_long(b, 8 + i * 8, u32::from(c::base_species(enc.species)))?;
            }
            // The time of day slots get the species of the slots they replace.
            for i in 0..4 {
                let at = DPPT_TIME_OF_DAY + 4 * i;
                let n = long(b, at)? as usize;
                if (1..=c::SPECIES_COUNT).contains(&n) {
                    put_long(
                        b,
                        at,
                        u32::from(c::base_species(e[DPPT_ALTERNATE_SLOTS[i + 2]].species)),
                    )?;
                }
            }
        }
        Place::DpptReplacing(file, offsets) => {
            need(offsets.len())?;
            let b = rom.files.narc_file(wild, *file)?;
            for (enc, &at) in e.iter().zip(offsets) {
                put_long(b, at, u32::from(c::base_species(enc.species)))?;
            }
        }
        Place::DpptWater(file, at) => {
            need(5)?;
            let b = rom.files.narc_file(wild, *file)?;
            for (i, enc) in e.iter().enumerate() {
                put_long(
                    b,
                    at + i * 8,
                    (u32::from(enc.min_level) << 8) + u32::from(enc.max_level),
                )?;
                put_long(b, at + 4 + i * 8, u32::from(c::base_species(enc.species)))?;
            }
        }
        Place::Feebas => {
            need(1)?;
            let overlay = rom.files.overlay(rom.entry.need("EncounterOvlNumber")?)?;
            if let Some(at) = find_after(overlay, FEEBAS_LEVEL_PREFIX) {
                put(overlay, at, &[e[0].max_level])?;
                put(overlay, at + 4, &[e[0].min_level])?;
            }
            write_extra(rom, 0, e)?;
        }
        Place::HoneyTree(file) => {
            need(6)?;
            write_extra(rom, *file, e)?;
        }
        Place::Rotating(file) => write_extra(rom, *file, e)?,
        Place::HgssGrass(file) => {
            need(12)?;
            one_level(new)?;
            let b = rom.files.narc_file(wild, *file)?;
            for (i, enc) in e.iter().enumerate() {
                put(b, 8 + i, &[enc.min_level])?;
                // Morning, day and night alike.
                for start in [20, 44, 68] {
                    put_word(b, start + i * 2, c::base_species(enc.species))?;
                }
            }
        }
        Place::HgssSpecies(file, offsets) => {
            need(offsets.len())?;
            let b = rom.files.narc_file(wild, *file)?;
            for (enc, &at) in e.iter().zip(offsets) {
                put_word(b, at, c::base_species(enc.species))?;
            }
        }
        Place::HgssWater(file, at, count) => {
            need(*count)?;
            let b = rom.files.narc_file(wild, *file)?;
            for (i, enc) in e.iter().enumerate() {
                put(b, at + i * 4, &[enc.min_level, enc.max_level])?;
                put_word(b, at + 2 + i * 4, c::base_species(enc.species))?;
            }
        }
        Place::Headbutt(file, offsets) => {
            need(offsets.len())?;
            let path = rom.entry.file("HeadbuttPokemon")?;
            let b = rom.files.narc_file(&path, *file)?;
            write_slots(b, offsets, e)?;
        }
        Place::Contest(offsets) => {
            need(offsets.len())?;
            let path = rom.entry.file("BCCWilds")?;
            let b = rom.files.file(&path)?;
            write_slots(b, offsets, e)?;
        }
    }
    Ok(())
}

/// Fails on a level range in a grass area, whose slots store one level each.
fn one_level(set: &EncounterSet) -> Result<(), RomError> {
    match set.encounters.iter().find(|e| e.max_level != e.min_level) {
        Some(e) => Err(RomError::Malformed(format!(
            "{} has a level range {}-{}, but grass slots store one level",
            set.name, e.min_level, e.max_level
        ))),
        None => Ok(()),
    }
}

/// Species words and levels at `offsets`.
fn write_slots(b: &mut [u8], offsets: &[usize], encounters: &[WildEncounter]) -> Result<(), RomError> {
    for (enc, &at) in encounters.iter().zip(offsets) {
        put_word(b, at, c::base_species(enc.species))?;
        put(b, at + 2, &[enc.min_level, enc.max_level])?;
    }
    Ok(())
}

fn write_extra(rom: &mut Rom, file: usize, encounters: &[WildEncounter]) -> Result<(), RomError> {
    let path = rom.entry.file("ExtraEncounters")?;
    let b = rom.files.narc_file(&path, file)?;
    for (i, enc) in encounters.iter().enumerate() {
        put_long(b, i * 4, u32::from(c::base_species(enc.species)))?;
    }
    Ok(())
}

/// The honey tree levels, which the encounter code holds: the original code can't hold every lowest level and
/// checks for the original range, so it becomes `mov r0, #min`, nothing in place of the check, and `add r0, r0,
/// r6`, r6 being a random number from 0 to the range, which the code gets by dividing a random word by a divisor.
fn write_honey_tree_levels(rom: &mut Rom, min: u8, max: u8) -> Result<(), RomError> {
    let overlay = rom.files.overlay(rom.entry.need("EncounterOvlNumber")?)?;
    let Some(at) = find_after(overlay, HONEY_TREE_LEVEL_PREFIX) else {
        return Ok(());
    };
    put(overlay, at + 46, &[min, 0x20, 0, 0, 0, 0, 0, 0, 0x80, 0x19])?;
    put(overlay, at + 102, &[max])?;
    let range = u32::from(max.saturating_sub(min));
    put_long(overlay, at + 148, 0xFFFF / (range + 1) + 1)
}

/// The Pokédex's areas: for each species, the maps where it is met by time of day, and the maps of special
/// encounters.
#[derive(Debug)]
struct DexAreas {
    by_time: Vec<[BTreeSet<u32>; 3]>,
}

impl DexAreas {
    fn new() -> Self {
        Self {
            by_time: vec![Default::default(); c::SPECIES_COUNT],
        }
    }

    fn add(&mut self, species: DexNumber, time: usize, map: u32) {
        if let Some(s) = usize::from(species)
            .checked_sub(1)
            .and_then(|i| self.by_time.get_mut(i))
        {
            s[time].insert(map);
        }
    }
}

#[derive(Debug)]
struct DexSpecial {
    maps: Vec<BTreeSet<u32>>,
}

impl DexSpecial {
    fn new() -> Self {
        Self {
            maps: vec![BTreeSet::new(); c::SPECIES_COUNT],
        }
    }

    fn add(&mut self, species: DexNumber, map: u32) {
        if let Some(s) = usize::from(species).checked_sub(1).and_then(|i| self.maps.get_mut(i)) {
            s.insert(map);
        }
    }
}

/// A Pokédex area file: the maps as 32-bit words, then a 0 word.
fn area_file(maps: &BTreeSet<u32>) -> Vec<u8> {
    let mut out: Vec<u8> = maps.iter().flat_map(|m| m.to_le_bytes()).collect();
    out.extend([0; 4]);
    out
}

fn dex_map(overworld: &[i16], dungeon: &[i16], i: usize) -> Option<(bool, u32)> {
    let ow = overworld.get(i).copied().unwrap_or(-1);
    let dun = dungeon.get(i).copied().unwrap_or(-1);
    if ow != -1 {
        u32::try_from(ow).ok().map(|m| (true, m))
    } else if dun != -1 {
        u32::try_from(dun).ok().map(|m| (false, m))
    } else {
        None
    }
}

fn set_area_file(rom: &mut Rom, path: &str, index: usize, maps: &BTreeSet<u32>) -> Result<(), RomError> {
    *rom.files.narc_file(path, index)? = area_file(maps);
    Ok(())
}

/// The special areas outside the map files: the Feebas tiles, the honey trees and the rotating Pokémon.
fn add_extra_dppt(
    sets: &[EncounterSet],
    dun_pre: &mut DexSpecial,
    dun_post: &mut DexSpecial,
    ow_pre: &mut DexSpecial,
    ow_post: &mut DexSpecial,
) {
    for s in sets {
        let species = s.encounters.iter().map(|e| e.species);
        if s.name.contains("Mt. Coronet Feebas Tiles") {
            for n in species {
                dun_pre.add(n, DEX_MT_CORONET);
                dun_post.add(n, DEX_MT_CORONET);
            }
        } else if s.name.contains("Honey Tree Group 1") || s.name.contains("Honey Tree Group 2") {
            for n in species {
                dun_pre.add(n, DEX_FLOAROMA_MEADOW);
                dun_post.add(n, DEX_FLOAROMA_MEADOW);
                for m in DEX_HONEY_TREES {
                    ow_pre.add(n, m);
                    ow_post.add(n, m);
                }
            }
        } else if s.name.contains("Trophy Garden Rotating Pokemon") {
            for n in species {
                dun_post.add(n, DEX_TROPHY_GARDEN);
            }
        } else if s.name.contains("Great Marsh Rotating Pokemon (Post-National Dex)") {
            for n in species {
                dun_post.add(n, DEX_GREAT_MARSH);
            }
        } else if s.name.contains("Great Marsh Rotating Pokemon (Pre-National Dex)") {
            for n in species {
                dun_pre.add(n, DEX_GREAT_MARSH);
            }
        }
    }
}

fn update_pokedex_dppt(rom: &mut Rom, sets: &[EncounterSet]) -> Result<(), RomError> {
    let wild = rom.entry.file("WildPokemon")?;
    let files = rom.files.narc(&wild)?.files.clone();
    let (mut dun, mut ow) = (DexAreas::new(), DexAreas::new());
    let (mut dun_pre, mut dun_post, mut ow_pre, mut ow_post) = (
        DexSpecial::new(),
        DexSpecial::new(),
        DexSpecial::new(),
        DexSpecial::new(),
    );
    for (i, b) in files.iter().enumerate() {
        let Some((overworld, map)) = dex_map(&dex::DPPT_OVERWORLD, &dex::DPPT_DUNGEON, i) else {
            continue;
        };
        let (target, special) = if overworld {
            (&mut ow, &mut ow_post)
        } else {
            (&mut dun, &mut dun_post)
        };
        if long(b, 0)? != 0 {
            for slot in 0..12 {
                let n = species(long(b, 8 + slot * 8)?)?;
                target.add(n, 0, map);
                // Slots 2 and 3 are the morning's; day and night have their own.
                if slot != 2 && slot != 3 {
                    target.add(n, 1, map);
                    target.add(n, 2, map);
                }
            }
            for slot in 0..4 {
                let n = long(b, DPPT_TIME_OF_DAY + 4 * slot)?;
                if (1..=c::SPECIES_COUNT).contains(&(n as usize)) {
                    target.add(species(n)?, if slot > 1 { 2 } else { 1 }, map);
                }
            }
            // Of the replacing slots only the Poké Radar's show.
            for slot in 6..10 {
                let n = long(b, 100 + slot * 4)?;
                if (1..=c::SPECIES_COUNT).contains(&(n as usize)) {
                    special.add(species(n)?, map);
                }
            }
        }
        for water in [0, 2, 3, 4] {
            let at = DPPT_WATER_START + water * DPPT_WATER_AREA_SIZE;
            if long(b, at)? == 0 {
                continue;
            }
            for slot in 0..5 {
                let n = species(long(b, at + 8 + slot * 8)?)?;
                for time in 0..3 {
                    target.add(n, time, map);
                }
            }
        }
    }
    add_extra_dppt(sets, &mut dun_pre, &mut dun_post, &mut ow_pre, &mut ow_post);
    let path = rom.entry.file("PokedexAreaData")?;
    let dun_index = rom.entry.need("PokedexAreaDataDungeonIndex")?;
    let ow_index = rom.entry.need("PokedexAreaDataOverworldIndex")?;
    let specials = [
        (
            rom.entry.need("PokedexAreaDataDungeonSpecialPreNationalIndex")?,
            &dun_pre,
        ),
        (
            rom.entry.need("PokedexAreaDataDungeonSpecialPostNationalIndex")?,
            &dun_post,
        ),
        (
            rom.entry.need("PokedexAreaDataOverworldSpecialPreNationalIndex")?,
            &ow_pre,
        ),
        (
            rom.entry.need("PokedexAreaDataOverworldSpecialPostNationalIndex")?,
            &ow_post,
        ),
    ];
    write_dex(rom, &path, (dun_index, &dun), (ow_index, &ow), &specials)
}

fn write_dex(
    rom: &mut Rom,
    path: &str,
    dungeon: (usize, &DexAreas),
    overworld: (usize, &DexAreas),
    specials: &[(usize, &DexSpecial)],
) -> Result<(), RomError> {
    for pk in 1..=c::SPECIES_COUNT {
        for time in 0..3 {
            for (base, areas) in [dungeon, overworld] {
                set_area_file(
                    rom,
                    path,
                    base + pk + time * POKEDEX_AREA_DATA_SIZE,
                    &areas.by_time[pk - 1][time],
                )?;
            }
        }
        for &(base, special) in specials {
            set_area_file(rom, path, base + pk, &special.maps[pk - 1])?;
        }
    }
    Ok(())
}

fn update_pokedex_hgss(rom: &mut Rom, sets: &[EncounterSet]) -> Result<(), RomError> {
    let wild = rom.entry.file("WildPokemon")?;
    let files = rom.files.narc(&wild)?.files.clone();
    let (mut dun, mut ow) = (DexAreas::new(), DexAreas::new());
    let (mut dun_special, mut ow_special) = (DexSpecial::new(), DexSpecial::new());
    for (i, b) in files.iter().enumerate() {
        let Some((overworld, map)) = dex_map(&dex::HGSS_OVERWORLD, &dex::HGSS_DUNGEON, i) else {
            continue;
        };
        let (target, special) = if overworld {
            (&mut ow, &mut ow_special)
        } else {
            (&mut dun, &mut dun_special)
        };
        let rates: Vec<u8> = (0..6).map(|r| byte(b, r)).collect::<Result<_, _>>()?;
        if rates[0] != 0 {
            for time in 0..3 {
                for slot in 0..12 {
                    target.add(word(b, 20 + time * 24 + slot * 2)?, time, map);
                }
            }
        }
        for o in (92..100).step_by(2) {
            let n = word(b, o)?;
            if n != 0 {
                special.add(n, map);
            }
        }
        let mut at = 100;
        for r in 1..4 {
            if rates[r] != 0 {
                for slot in 0..HGSS_AMOUNTS[r] {
                    let n = word(b, at + 2 + slot * 4)?;
                    for time in 0..3 {
                        target.add(n, time, map);
                    }
                }
            }
            at += 4 * HGSS_AMOUNTS[r];
        }
        // The good and super rods: at night, the night fishing replacement takes a slot.
        let night = word(b, HGSS_NIGHT_FISHING)?;
        for (r, start, replaced) in [(4, at, HGSS_GOOD_ROD_REPLACED), (5, at + 20, HGSS_SUPER_ROD_REPLACED)] {
            if rates[r] == 0 {
                continue;
            }
            for slot in 0..5 {
                let n = word(b, start + 2 + slot * 4)?;
                target.add(n, 0, map);
                target.add(n, 1, map);
                let at_night = if slot == replaced && night != 0 { night } else { n };
                target.add(at_night, 2, map);
            }
        }
    }
    if let Some(first) = sets.iter().position(|s| s.name.contains("Route 1 Headbutt")) {
        for i in 0..dex::HGSS_HEADBUTT_OVERWORLD.len() {
            let Some(area) = sets.get(first + i) else {
                break;
            };
            let Some((overworld, map)) = dex_map(&dex::HGSS_HEADBUTT_OVERWORLD, &dex::HGSS_HEADBUTT_DUNGEON, i) else {
                continue;
            };
            for e in &area.encounters {
                if overworld {
                    ow_special.add(e.species, map);
                } else {
                    dun_special.add(e.species, map);
                }
            }
        }
    }
    let path = rom.entry.file("PokedexAreaData")?;
    let dun_index = rom.entry.need("PokedexAreaDataDungeonIndex")?;
    let ow_index = rom.entry.need("PokedexAreaDataOverworldIndex")?;
    let specials = [
        (rom.entry.need("PokedexAreaDataDungeonSpecialIndex")?, &dun_special),
        (rom.entry.need("PokedexAreaDataOverworldSpecialIndex")?, &ow_special),
    ];
    write_dex(rom, &path, (dun_index, &dun), (ow_index, &ow), &specials)
}

#[cfg(test)]
mod tests {
    use super::super::tests::{load, real_rom, save};
    use super::*;
    use crate::GameHandler;

    #[test]
    fn grass_slots_store_one_level() {
        let mut grass = set(
            "Route 201 Grass/Cave".into(),
            Some(1),
            EncounterKind::Walking,
            20,
            vec![at_level(1, 5); 12],
        );
        assert!(one_level(&grass).is_ok());
        grass.encounters[4].max_level = 7;
        let err = one_level(&grass).expect_err("a range doesn't fit");
        assert!(err.to_string().contains("level range 5-7"), "{err}");
    }

    /// With `PPR_TEST_ROMS`: a level range in a Platinum grass area fails instead of losing its highest level.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_grass_level_range_fails() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes);
        let mut data = h.game_data().clone();
        let grass = data
            .encounters
            .iter()
            .position(|s| s.kind == EncounterKind::Walking)
            .expect("grass");
        let e = &mut data.encounters[grass].encounters[0];
        e.max_level = e.min_level + 1;
        let err = h.apply(&data).expect_err("grass stores one level");
        assert!(err.to_string().contains("grass slots store one level"), "{err}");
    }

    #[test]
    fn tag_lists_have_an_area_each() {
        let count = |runs: &[(usize, &str)]| runs.iter().map(|r| r.0).sum::<usize>();
        assert_eq!(count(tags::TAGS_DP), 561);
        let hgss = count(tags::TAGS_HGSS) + tags::TAGS_HGSS_HEADBUTT.len() + count(tags::TAGS_HGSS_AFTER_HEADBUTT);
        assert_eq!(hgss, 857);
        assert!(tags::POST_GAME_PT.iter().all(|&i| i < 565));
        assert!(tags::POST_GAME_HGSS.iter().all(|&i| i < hgss));
    }

    #[test]
    fn pokedex_area_files_end_with_zero() {
        assert_eq!(area_file(&BTreeSet::from([3, 1])), [1, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0]);
    }

    /// With `PPR_TEST_ROMS`: the Pokédex area data built from the unchanged encounters, against the game's own.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_pokedex_areas_match_the_game() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes);
        let sets = h.game_data().encounters.clone();
        let path = h.rom.entry.file("PokedexAreaData").expect("entry");
        let before = h.rom.files.narc(&path).expect("narc").files.clone();
        update_pokedex_dppt(&mut h.rom, &sets).expect("updates");
        let after = &h.rom.files.narc(&path).expect("narc").files;
        let differing: Vec<usize> = (0..before.len()).filter(|&i| before[i] != after[i]).collect();
        assert!(
            differing.is_empty(),
            "{} files differ, first {:?}",
            differing.len(),
            &differing[..differing.len().min(10)]
        );
    }

    /// With `PPR_TEST_ROMS`: changed species and levels of every kind of area read back, and an unchanged
    /// write saves byte for byte.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_encounters_read_back() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes.clone());
        let mut data = h.game_data().clone();
        assert_eq!(data.encounters.len(), 565);
        assert_eq!(data.wild_banned, [201]);
        for (i, s) in data.encounters.iter_mut().enumerate() {
            for (j, e) in s.encounters.iter_mut().enumerate() {
                e.species = DexNumber::try_from(1 + (i * 7 + j) % 493).expect("small");
                if e.min_level > 2 {
                    if e.max_level == e.min_level {
                        e.max_level -= 1;
                    }
                    e.min_level -= 1;
                }
            }
        }
        h.apply(&data).expect("applies");
        let reloaded = load(save(&h));
        let back = &reloaded.game_data().encounters;
        for (a, b) in back.iter().zip(&data.encounters) {
            let species = |s: &EncounterSet| s.encounters.iter().map(|e| e.species).collect::<Vec<_>>();
            assert_eq!(species(a), species(b), "{}", a.name);
        }
        // Levels that are stored per area (not taken from other areas) read back too.
        let route = back.iter().position(|s| s.name.ends_with("Surfing")).expect("surfing");
        assert_eq!(back[route].encounters, data.encounters[route].encounters);
        let grass = back
            .iter()
            .position(|s| s.kind == EncounterKind::Walking)
            .expect("grass");
        assert_eq!(back[grass].encounters, data.encounters[grass].encounters);
        let honey = back.iter().position(|s| s.name == "Honey Tree Group 1").expect("honey");
        assert_eq!(back[honey].encounters, data.encounters[honey].encounters);
        assert_eq!(reloaded.game_data().encounters.len(), 565);

        let mut unchanged = load(bytes.clone());
        let same = unchanged.game_data().clone();
        unchanged.apply(&same).expect("applies");
        assert!(save(&unchanged) == bytes, "an unchanged game saves byte for byte");
    }
}
