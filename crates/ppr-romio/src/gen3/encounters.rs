//! Wild encounters of the Gen 3 games, read and written, with location tags and post-game flags by area.
//!
//! The `WildPokemon` table holds 20-byte headers: map bank (u8), map number (u8), two bytes of padding, then
//! pointers to the walking, surfing, rock smash and fishing info; bank and map `0xFF` end it. An info is the
//! encounter rate (u8, padded to 4 bytes) and a pointer to its slots: minimum level, maximum level and species
//! (u16, internal index) each. An info with rate 0 or slots already read is skipped, and fishing's 10
//! slots (Old, Good and Super Rod) are one area.

use std::collections::{BTreeSet, HashMap, HashSet};

use ppr_model::{DexNumber, EncounterKind, EncounterSet, WildEncounter};

use super::tags;
use super::{Family, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;

const HEADER_SIZE: usize = 20;
const SLOT_SIZE: usize = 4;
const END_OF_TABLE: u8 = 0xFF;
/// More headers than any game has; guards against a wrong table offset.
const MAX_HEADERS: usize = 1024;
const MAP_HEADER_SECTION: usize = 0x14;
const RSE_LABEL_SIZE: usize = 8;
const RSE_LABEL_NAME: usize = 4;
/// The first map section with a name in `FireRed` and `LeafGreen`.
const FRLG_FIRST_SECTION: usize = 0x58;
const NAME_MAX: usize = 32;
/// How the text table decodes the control code that Ruby and Sapphire use as a line break in place names.
const CONTROL: &str = "\\c";

/// Header fields that point to an info: offset, slot count, kind and name suffix.
const INFOS: [(usize, usize, EncounterKind, &str); 4] = [
    (4, 12, EncounterKind::Walking, "Grass/Cave"),
    (8, 5, EncounterKind::Surfing, "Surfing"),
    (12, 5, EncounterKind::Interact, "Rock Smash"),
    (16, 10, EncounterKind::Fishing, "Fishing"),
];

/// The encounter sets and, for each, the ROM offset of every slot.
#[derive(Debug, Default)]
pub(super) struct Encounters {
    pub(super) sets: Vec<EncounterSet>,
    pub(super) slots: Vec<Vec<usize>>,
}

/// Reads the wild encounters. `battle_trappers` are banned from the entry's `BattleTrappersBanned` areas.
pub(super) fn read(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    text: &TextTable,
    internal_to_dex: &[DexNumber],
    battle_trappers: &BTreeSet<DexNumber>,
) -> Result<Encounters, RomError> {
    let table = need(entry, "WildPokemon")?;
    let mut names = Locations::new(rom, entry, family, text);
    let mut seen = HashSet::new();
    let mut out = Encounters::default();
    let mut ended = false;
    for at in (0..MAX_HEADERS).map(|h| table + h * HEADER_SIZE) {
        let b = rom.slice(at, 2)?;
        let (bank, map) = (b[0], b[1]);
        if bank == END_OF_TABLE && map == END_OF_TABLE {
            ended = true;
            break;
        }
        let map_name = names.name(bank, map);
        for (field, count, kind, suffix) in INFOS {
            // An invalid pointer means no info.
            let Ok(info) = rom.pointer(at + field) else {
                continue;
            };
            let rate = rom.slice(info, 1)?[0];
            if rate == 0 {
                continue;
            }
            let slots_at = rom.pointer(info + 4)?;
            if !seen.insert(slots_at) {
                continue;
            }
            let mut set = EncounterSet {
                kind,
                map: Some(u16::from_be_bytes([bank, map])),
                name: format!("{map_name} {suffix}"),
                location: None,
                post_game: false,
                post_game_from: None,
                rate,
                banned: Vec::new(),
                encounters: Vec::new(),
            };
            let mut offsets = Vec::new();
            for slot in (0..count).map(|i| slots_at + i * SLOT_SIZE) {
                let b = rom.slice(slot, SLOT_SIZE)?;
                let species = internal_to_dex
                    .get(usize::from(u16::from_le_bytes([b[2], b[3]])))
                    .copied();
                // Empty or invalid slots are left alone.
                if let Some(species) = species.filter(|&n| n != 0) {
                    set.encounters.push(WildEncounter {
                        forme: 0,
                        species,
                        min_level: b[0],
                        max_level: b[1],
                    });
                    offsets.push(slot);
                }
            }
            out.sets.push(set);
            out.slots.push(offsets);
        }
    }
    if !ended {
        return Err(RomError::Malformed(format!(
            "no end of the wild encounter table within {MAX_HEADERS} entries"
        )));
    }
    // Some areas can't have Pokémon with Arena Trap, Shadow Tag or Magnet Pull.
    if let Some(list) = entry.value("BattleTrappersBanned") {
        for index in list
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .filter_map(|v| v.trim().parse::<usize>().ok())
        {
            if let Some(set) = out.sets.get_mut(index) {
                set.banned.extend(battle_trappers);
            }
        }
    }
    tag(&mut out.sets, family)?;
    Ok(out)
}

/// Location tags and post-game areas by area index.
fn tag(sets: &mut [EncounterSet], family: Family) -> Result<(), RomError> {
    let (locations, post_game): (&[&str], &[usize]) = match family {
        Family::RubySapphire => (&tags::LOCATION_TAGS_RS, &tags::POST_GAME_RS),
        Family::Emerald => (&tags::LOCATION_TAGS_EM, &tags::POST_GAME_EM),
        Family::FireRedLeafGreen => (&tags::LOCATION_TAGS_FRLG, &tags::POST_GAME_FRLG),
    };
    if sets.len() != locations.len() {
        return Err(RomError::Malformed(format!(
            "expected {} wild encounter areas, found {}",
            locations.len(),
            sets.len()
        )));
    }
    for (set, location) in sets.iter_mut().zip(locations) {
        set.location = Some((*location).to_owned());
    }
    for &i in post_game {
        sets[i].post_game = true;
    }
    Ok(())
}

/// Writes the sets that changed. The sets must be the loaded ones, in the same order, with the same slot counts.
pub(super) fn write(
    rom: &mut GbaRom,
    loaded: &Encounters,
    sets: &[EncounterSet],
    internal_index: impl Fn(DexNumber) -> Result<u16, RomError>,
) -> Result<(), RomError> {
    if sets.len() != loaded.sets.len() {
        return Err(RomError::Malformed(format!(
            "expected {} encounter sets, got {}",
            loaded.sets.len(),
            sets.len()
        )));
    }
    for ((new, old), offsets) in sets.iter().zip(&loaded.sets).zip(&loaded.slots) {
        if new == old {
            continue;
        }
        if new.encounters.len() != offsets.len() {
            return Err(RomError::Malformed(format!(
                "an encounter set has {} slots, not {}",
                offsets.len(),
                new.encounters.len()
            )));
        }
        for (e, &at) in new.encounters.iter().zip(offsets) {
            let [lo, hi] = internal_index(e.species)?.to_le_bytes();
            rom.write(at, &[e.min_level, e.max_level, lo, hi])?;
        }
    }
    Ok(())
}

/// Names of the places on the town map, looked up through the map headers. Missing data gives an empty name.
struct Locations<'a> {
    rom: &'a GbaRom,
    family: Family,
    text: &'a TextTable,
    map_groups: Option<usize>,
    labels: Option<usize>,
    cache: HashMap<u8, String>,
}

impl<'a> Locations<'a> {
    fn new(rom: &'a GbaRom, entry: &RomEntry, family: Family, text: &'a TextTable) -> Self {
        let offset = |key| entry.int(key).and_then(|v| usize::try_from(v).ok());
        Self {
            rom,
            family,
            text,
            map_groups: offset("MapHeaders"),
            labels: offset("MapLabels"),
            cache: HashMap::new(),
        }
    }

    fn name(&mut self, group: u8, number: u8) -> String {
        let Some(section) = self.section(group, number) else {
            return String::new();
        };
        if let Some(name) = self.cache.get(&section) {
            return name.clone();
        }
        let name = self.label(section).unwrap_or_default();
        self.cache.insert(section, name.clone());
        name
    }

    fn section(&self, group: u8, number: u8) -> Option<u8> {
        let maps = self.rom.pointer(self.map_groups? + 4 * usize::from(group)).ok()?;
        let header = self.rom.pointer(maps + 4 * usize::from(number)).ok()?;
        self.rom.slice(header + MAP_HEADER_SECTION, 1).ok().map(|b| b[0])
    }

    fn label(&self, section: u8) -> Option<String> {
        let labels = self.labels?;
        let pointer_at = if self.family == Family::FireRedLeafGreen {
            labels + 4 * usize::from(section).checked_sub(FRLG_FIRST_SECTION)?
        } else {
            labels + RSE_LABEL_SIZE * usize::from(section) + RSE_LABEL_NAME
        };
        let at = self.rom.pointer(pointer_at).ok()?;
        let len = NAME_MAX.min(self.rom.bytes().len().checked_sub(at)?);
        let name = self.text.decode(self.rom.slice(at, len).ok()?, len);
        // Ruby and Sapphire break long names over two lines with a control code; a space reads better here.
        let name = name
            .replace(CONTROL, " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        (!name.is_empty()).then_some(name)
    }
}
