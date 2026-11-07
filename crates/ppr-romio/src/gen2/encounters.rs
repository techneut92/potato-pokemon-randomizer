//! Wild Pokémon: grass and caves, surfing, fishing, headbutting trees, smashing rocks and the Bug-Catching
//! Contest, and their location tags.
//!
//! The game data holds the view without time of day: a grass or cave set is the daytime one, and is written to
//! morning, day and night alike; a fishing slot whose Pokémon depends on the time of day reads the daytime one, and
//! is written over the marker that points to the time-specific table, so it no longer depends on the time. The
//! view by time of day (three grass and cave sets per map, the time-specific fishing sets apart) is here too, for
//! when a setting asks for it.
//!
//! - The fishing sets' Super Rod slots are post-game, and only those
//!   ([`ppr_model::EncounterSet::post_game_from`]).

use ppr_model::{DexNumber, EncounterKind, EncounterSet, WildEncounter};

use super::constants::{self as c, Family};
use super::maps::Maps;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::{GbRom, need};

const TABLE_END: u8 = 0xFF;
const TIMES_OF_DAY: [&str; 3] = ["Morning", "Day", "Night"];

/// Where one slot is read from and written to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Slot {
    /// The species and level bytes read.
    read: (usize, usize),
    /// The species and level bytes written (all times of day of a grass set, or the slot of the main fishing
    /// table that pointed to the time-specific one).
    write: Vec<(usize, usize)>,
    /// The highest level's byte, when the slot has one.
    max_level: Option<usize>,
}

impl Slot {
    fn at(species: usize, level: usize) -> Self {
        Self {
            read: (species, level),
            write: vec![(species, level)],
            max_level: None,
        }
    }
}

/// The encounter sets and where each slot is.
#[derive(Debug, Clone, Default)]
pub(super) struct Encounters {
    pub(super) sets: Vec<EncounterSet>,
    pub(super) slots: Vec<Vec<Slot>>,
}

impl Encounters {
    fn push(
        &mut self,
        rom: &GbRom,
        kind: EncounterKind,
        name: String,
        rate: u8,
        slots: Vec<Slot>,
    ) -> Result<(), RomError> {
        let encounters = slots
            .iter()
            .map(|s| {
                let level = rom.byte(s.read.1)?;
                Ok(WildEncounter {
                    forme: 0,
                    species: DexNumber::from(rom.byte(s.read.0)?),
                    min_level: level,
                    max_level: match s.max_level {
                        Some(at) => rom.byte(at)?,
                        None => level,
                    },
                })
            })
            .collect::<Result<Vec<_>, RomError>>()?;
        self.sets.push(EncounterSet {
            kind,
            map: None,
            name,
            location: None,
            post_game: false,
            post_game_from: None,
            rate,
            banned: Vec::new(),
            encounters,
        });
        self.slots.push(slots);
        Ok(())
    }
}

/// The encounter sets, tagged; by time of day when `timed`.
pub(super) fn read(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    maps: &Maps,
    timed: bool,
) -> Result<Encounters, RomError> {
    let mut out = Encounters::default();
    let mut at = need(entry, "WildPokemonOffset")?;
    // Johto, Kanto, then the swarms: grass and caves, then surfing.
    for _ in 0..3 {
        at = read_land(rom, maps, at, timed, &mut out)?;
        at = read_sea(rom, maps, at, &mut out)?;
    }
    read_fishing(rom, entry, family, timed, &mut out)?;
    let mut at = need(entry, "HeadbuttWildsOffset")?;
    let names: &[&str] = match family {
        Family::GoldSilver => &c::HEADBUTT_AREA_NAMES_GS,
        Family::Crystal => &c::HEADBUTT_AREA_NAMES_CRYSTAL,
    };
    for i in 0..need(entry, "HeadbuttTableSize")? {
        let mut slots = Vec::new();
        // Each slot is a chance, the species and the level.
        while rom.byte(at)? != TABLE_END {
            slots.push(Slot::at(at + 1, at + 2));
            at += 3;
        }
        at += 1;
        let name = names.get(i).copied().unwrap_or_default().to_owned();
        out.push(rom, EncounterKind::Interact, name, 0, slots)?;
    }
    let mut at = need(entry, "BCCWildsOffset")?;
    let mut slots = Vec::new();
    // A chance, the species, and the lowest and highest levels.
    while rom.byte(at)? != TABLE_END {
        slots.push(Slot {
            max_level: Some(at + 3),
            ..Slot::at(at + 1, at + 2)
        });
        at += 4;
    }
    out.push(rom, EncounterKind::Walking, "Bug Catching Contest".into(), 0, slots)?;
    tag(&mut out.sets, family, timed)?;
    Ok(out)
}

/// The grass and cave sets of one table, until its end; returns where the next table starts. Each map has a rate
/// and seven slots (level, species) for each time of day.
fn read_land(rom: &GbRom, maps: &Maps, mut at: usize, timed: bool, out: &mut Encounters) -> Result<usize, RomError> {
    let size = c::LAND_SLOTS * 2;
    while rom.byte(at)? != TABLE_END {
        let map = maps.name(rom.byte(at)?, rom.byte(at + 1)?);
        let slot = |time: usize, j: usize| {
            let level = at + 5 + time * size + j * 2;
            (level + 1, level)
        };
        if timed {
            for (time, name) in TIMES_OF_DAY.iter().enumerate() {
                let slots = (0..c::LAND_SLOTS)
                    .map(|j| {
                        let (species, level) = slot(time, j);
                        Slot::at(species, level)
                    })
                    .collect();
                let name = format!("{map} Grass/Cave ({name})");
                out.push(rom, EncounterKind::Walking, name, rom.byte(at + 2 + time)?, slots)?;
            }
        } else {
            let slots = (0..c::LAND_SLOTS)
                .map(|j| Slot {
                    read: slot(1, j),
                    write: (0..3).map(|time| slot(time, j)).collect(),
                    max_level: None,
                })
                .collect();
            out.push(
                rom,
                EncounterKind::Walking,
                format!("{map} Grass/Cave"),
                rom.byte(at + 3)?,
                slots,
            )?;
        }
        at += 5 + 3 * size;
    }
    Ok(at + 1)
}

/// The surfing sets of one table, until its end; returns where the next table starts. Each map has a rate and three
/// slots (level, species).
fn read_sea(rom: &GbRom, maps: &Maps, mut at: usize, out: &mut Encounters) -> Result<usize, RomError> {
    while rom.byte(at)? != TABLE_END {
        let map = maps.name(rom.byte(at)?, rom.byte(at + 1)?);
        let slots = (0..c::SEA_SLOTS)
            .map(|j| Slot::at(at + 4 + j * 2, at + 3 + j * 2))
            .collect();
        out.push(
            rom,
            EncounterKind::Surfing,
            format!("{map} Surfing"),
            rom.byte(at + 2)?,
            slots,
        )?;
        at += 3 + c::SEA_SLOTS * 2;
    }
    Ok(at + 1)
}

/// The fishing areas: eleven slots each of a chance, the species and the level; species 0 points to the
/// time-specific table, with the level as the index there. That table follows, four slots (species, level) per
/// area: morning and day, then night, twice.
fn read_fishing(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    timed: bool,
    out: &mut Encounters,
) -> Result<(), RomError> {
    let root = need(entry, "FishingWildsOffset")?;
    let time_specific = root + c::FISHING_SLOT_SIZE * c::FISHING_SLOTS * c::FISHING_AREA_COUNT;
    let mut at = root;
    for k in 0..c::FISHING_AREA_COUNT {
        let mut slots = Vec::new();
        for _ in 0..c::FISHING_SLOTS {
            let (species, level) = (at + 1, at + 2);
            at += c::FISHING_SLOT_SIZE;
            if rom.byte(species)? != 0 {
                slots.push(Slot::at(species, level));
            } else if !timed {
                // The daytime Pokémon of the time-specific slot.
                let day = time_specific + usize::from(rom.byte(level)?) * 4 + 2;
                slots.push(Slot {
                    read: (day, day + 1),
                    write: vec![(species, level)],
                    max_level: None,
                });
            }
        }
        let kind = if family == Family::Crystal && c::CRYSTAL_UNUSED_FISHING_AREAS.contains(&k) {
            EncounterKind::Unused
        } else {
            EncounterKind::Fishing
        };
        out.push(rom, kind, format!("Fishing {}", c::FISHING_AREA_NAMES[k]), 0, slots)?;
    }
    if timed {
        for k in 0..c::TIME_SPECIFIC_FISHING_AREA_COUNT {
            let slots = (0..c::TIME_SPECIFIC_FISHING_SLOTS)
                .map(|i| {
                    let species = at + i * 2;
                    Slot::at(species, species + 1)
                })
                .collect();
            at += c::TIME_SPECIFIC_FISHING_SLOTS * 2;
            let name = format!("Time-Specific Fishing {}", k + 1);
            out.push(rom, EncounterKind::Fishing, name, 0, slots)?;
        }
    }
    Ok(())
}

/// The location tags of the view by time of day, from those of the view without: grass and cave sets three times,
/// and the time-specific fishing sets after the others.
fn timed_tags(family: Family) -> Vec<&'static str> {
    let (tags, swarms, time_specific): (&[&str], usize, usize) = match family {
        Family::GoldSilver => (&c::TAGS_GS, 157, 170),
        Family::Crystal => (&c::TAGS_CRYSTAL, 155, 167),
    };
    let mut out = Vec::new();
    for (i, &tag) in tags.iter().enumerate() {
        if i == time_specific {
            out.extend(c::TIME_SPECIFIC_FISHING_TAGS);
        }
        let land = i < 61 || (99..129).contains(&i) || (153..swarms).contains(&i);
        let copies = if land { 3 } else { 1 };
        out.extend(std::iter::repeat_n(tag, copies));
    }
    out
}

/// The location tags and post-game flags of the sets.
fn tag(sets: &mut [EncounterSet], family: Family, timed: bool) -> Result<(), RomError> {
    let tags: Vec<&str> = if timed {
        timed_tags(family)
    } else {
        match family {
            Family::GoldSilver => c::TAGS_GS.to_vec(),
            Family::Crystal => c::TAGS_CRYSTAL.to_vec(),
        }
    };
    if sets.len() != tags.len() {
        return Err(RomError::Malformed(format!(
            "expected {} wild encounter areas, found {}",
            tags.len(),
            sets.len()
        )));
    }
    for (set, tag) in sets.iter_mut().zip(tags) {
        set.location = Some(tag.to_owned());
    }
    let (post_game, partly): (&[usize], &[usize]) = match (family, timed) {
        (Family::GoldSilver, false) => (&c::POST_GAME_GS, &c::PARTLY_POST_GAME_GS),
        (Family::GoldSilver, true) => (&c::POST_GAME_GS_TIMED, &c::PARTLY_POST_GAME_GS_TIMED),
        (Family::Crystal, false) => (&c::POST_GAME_CRYSTAL, &c::PARTLY_POST_GAME_CRYSTAL),
        (Family::Crystal, true) => (&c::POST_GAME_CRYSTAL_TIMED, &c::PARTLY_POST_GAME_CRYSTAL_TIMED),
    };
    for &i in post_game {
        sets[i].post_game = true;
    }
    for (n, &i) in partly.iter().enumerate() {
        sets[i].post_game_from = Some(if !timed {
            c::SUPER_ROD_FROM
        } else if n < c::FISHING_AREA_COUNT {
            c::SUPER_ROD_FROM_TIMED[0]
        } else {
            c::SUPER_ROD_FROM_TIMED[1]
        });
    }
    Ok(())
}

/// Writes the sets that changed.
pub(super) fn write(
    rom: &mut GbRom,
    loaded: &Encounters,
    sets: &[EncounterSet],
    species_byte: &dyn Fn(DexNumber) -> Result<u8, RomError>,
) -> Result<(), RomError> {
    if sets.len() != loaded.sets.len() {
        return Err(RomError::Malformed(format!(
            "expected {} encounter sets, got {}",
            loaded.sets.len(),
            sets.len()
        )));
    }
    for ((new, old), slots) in sets.iter().zip(&loaded.sets).zip(&loaded.slots) {
        if new == old {
            continue;
        }
        if new.encounters.len() != slots.len() {
            return Err(RomError::Malformed(format!(
                "{} must keep {} slots",
                new.name,
                slots.len()
            )));
        }
        for (e, s) in new.encounters.iter().zip(slots) {
            let species = species_byte(e.species)?;
            for &(species_at, level_at) in &s.write {
                rom.write_byte(species_at, species)?;
                rom.write_byte(level_at, e.min_level)?;
            }
            if let Some(at) = s.max_level {
                rom.write_byte(at, e.max_level)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_tags_triple_the_grass_and_add_the_time_specific_fishing() {
        let gs = timed_tags(Family::GoldSilver);
        let crystal = timed_tags(Family::Crystal);
        // The post-game lists by time of day reach the last sets.
        assert!(c::POST_GAME_GS_TIMED.iter().all(|&i| i < gs.len()));
        assert!(c::PARTLY_POST_GAME_CRYSTAL_TIMED.iter().all(|&i| i < crystal.len()));
        assert_eq!(gs[0..3], ["SPROUT TOWER"; 3]);
        assert_eq!(gs.last(), Some(&"BUG CATCHING CONTEST"));
        assert_eq!(crystal.last(), Some(&"BUG CATCHING CONTEST"));
        let first_ts = |tags: &[&str]| tags.iter().filter(|&&t| t == "FISHING SHORE").count();
        assert_eq!((first_ts(&gs), first_ts(&crystal)), (2, 2));
        assert_eq!(gs[c::PARTLY_POST_GAME_GS_TIMED[12]], "FISHING SHORE");
        assert_eq!(crystal[c::PARTLY_POST_GAME_CRYSTAL_TIMED[12]], "FISHING SHORE");
    }
}
