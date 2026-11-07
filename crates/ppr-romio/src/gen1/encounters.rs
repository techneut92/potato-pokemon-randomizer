//! Wild Pokémon: grass, cave and water areas, the fishing rods, and their location tags.
//!
//! An area that several maps share is read once, named after all of them, and has no map.

use ppr_model::{DexNumber, EncounterKind, EncounterSet, WildEncounter};

use super::constants::{self as c, Family};
use super::maps::Maps;
use super::need;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::GbRom;

/// Where one slot's species and level bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Slot {
    species: usize,
    level: usize,
}

impl Slot {
    /// A slot stored as level, then species.
    fn level_first(at: usize) -> Self {
        Self {
            species: at + 1,
            level: at,
        }
    }

    /// A slot stored as species, then level.
    fn species_first(at: usize) -> Self {
        Self {
            species: at,
            level: at + 1,
        }
    }
}

/// The encounter sets and where each slot is.
#[derive(Debug, Clone, Default)]
pub(super) struct Encounters {
    pub(super) sets: Vec<EncounterSet>,
    pub(super) slots: Vec<Vec<Slot>>,
}

struct Reader<'a> {
    rom: &'a GbRom,
    dex_of: &'a dyn Fn(u8) -> DexNumber,
    ghost_marowak: DexNumber,
    out: Encounters,
}

impl Reader<'_> {
    fn area(
        &mut self,
        kind: EncounterKind,
        name: String,
        map: Option<u16>,
        rate: u8,
        slots: Vec<Slot>,
    ) -> Result<(), RomError> {
        let encounters = slots
            .iter()
            .map(|s| {
                let level = self.rom.byte(s.level)?;
                Ok(WildEncounter {
                    forme: 0,
                    species: (self.dex_of)(self.rom.byte(s.species)?),
                    min_level: level,
                    max_level: level,
                })
            })
            .collect::<Result<Vec<_>, RomError>>()?;
        self.out.sets.push(EncounterSet {
            kind,
            map,
            name,
            location: None,
            post_game: false,
            post_game_from: None,
            rate,
            banned: Vec::new(),
            encounters,
        });
        self.out.slots.push(slots);
        Ok(())
    }

    fn ban_ghost_marowak(&mut self) {
        if let Some(set) = self.out.sets.last_mut() {
            set.banned = vec![self.ghost_marowak];
        }
    }
}

fn map_number(map: usize) -> Option<u16> {
    u16::try_from(map).ok()
}

/// The encounter areas, tagged.
#[expect(clippy::too_many_lines, reason = "one pass over the grass, water and special areas")]
pub(super) fn read(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    maps: &Maps,
    dex_of: &dyn Fn(u8) -> DexNumber,
    ghost_marowak: DexNumber,
) -> Result<Encounters, RomError> {
    let mut r = Reader {
        rom,
        dex_of,
        ghost_marowak,
        out: Encounters::default(),
    };
    let name = |map: usize| maps.names.get(map).cloned().unwrap_or_default();

    // Grass or cave, then water, for every map; an area shared by maps is read the first time.
    let mut table = need(entry, "WildPokemonTableOffset")?;
    let mut map = 0usize;
    let mut seen: Vec<(usize, Vec<usize>)> = Vec::new();
    while rom.word(table)? != c::ENCOUNTER_TABLE_END {
        let at = rom.pointer(table)?;
        if let Some((_, sets)) = seen.iter().find(|(o, _)| *o == at) {
            for &i in sets {
                let set = &mut r.out.sets[i];
                set.name = format!("{}, {}", set.name, name(map));
                // Shared by several maps; map 0 keeps its index.
                if set.map.is_some_and(|m| m > 0) {
                    set.map = None;
                }
            }
        } else {
            let mut sets = Vec::new();
            let mut offset = at;
            for surfing in [false, true] {
                let rate = rom.byte(offset)?;
                offset += 1;
                if rate == 0 {
                    continue;
                }
                let (kind, what) = if surfing {
                    (EncounterKind::Surfing, "Surfing")
                } else {
                    (EncounterKind::Walking, "Grass/Cave")
                };
                let slots = (0..c::ENCOUNTER_TABLE_SIZE)
                    .map(|s| Slot::level_first(offset + s * 2))
                    .collect();
                offset += c::ENCOUNTER_TABLE_SIZE * 2;
                r.area(kind, format!("{what} on {}", name(map)), map_number(map), rate, slots)?;
                if c::TOWER_MAPS.contains(&map) {
                    r.ban_ghost_marowak();
                }
                sets.push(r.out.sets.len() - 1);
            }
            seen.push((at, sets));
        }
        map += 1;
        table += 2;
    }

    let old_rod = need(entry, "OldRodOffset")?;
    let slot = Slot {
        species: old_rod + 1,
        level: old_rod + 2,
    };
    r.area(EncounterKind::Fishing, "Old Rod Fishing".into(), None, 0, vec![slot])?;
    r.ban_ghost_marowak();
    let good_rod = need(entry, "GoodRodOffset")?;
    let slots = (0..2).map(|s| Slot::level_first(good_rod + s * 2)).collect();
    r.area(EncounterKind::Fishing, "Good Rod Fishing".into(), None, 0, slots)?;
    r.ban_ghost_marowak();

    let mut table = need(entry, "SuperRodTableOffset")?;
    match family {
        Family::Yellow => {
            while rom.byte(table)? != 0xFF {
                let map = usize::from(rom.byte(table)?);
                table += 1;
                let slots = (0..c::YELLOW_SUPER_ROD_TABLE_SIZE)
                    .map(|s| Slot::species_first(table + s * 2))
                    .collect();
                table += c::YELLOW_SUPER_ROD_TABLE_SIZE * 2;
                let what = format!("Super Rod Fishing on {}", name(map));
                r.area(EncounterKind::Fishing, what, map_number(map), 0, slots)?;
                r.ban_ghost_marowak();
            }
        }
        Family::RedBlue => {
            let mut seen: Vec<(usize, usize)> = Vec::new();
            while rom.byte(table)? != 0xFF {
                let map = usize::from(rom.byte(table)?);
                let at = rom.pointer(table + 1)?;
                table += 3;
                if let Some(&(_, i)) = seen.iter().find(|(o, _)| *o == at) {
                    let set = &mut r.out.sets[i];
                    set.name = format!("{}, {}", set.name, name(map));
                    if set.map.is_some_and(|m| m > 0) {
                        set.map = None;
                    }
                    continue;
                }
                let count = usize::from(rom.byte(at)?);
                let slots = (0..count).map(|s| Slot::level_first(at + 1 + s * 2)).collect();
                let what = format!("Super Rod Fishing on {}", name(map));
                r.area(EncounterKind::Fishing, what, map_number(map), 0, slots)?;
                r.ban_ghost_marowak();
                seen.push((at, r.out.sets.len() - 1));
            }
        }
    }

    let mut out = r.out;
    tag(&mut out.sets, family, entry.name == "Blue (J)")?;
    Ok(out)
}

/// The location tags and post-game flags of the areas.
fn tag(sets: &mut [EncounterSet], family: Family, japanese_blue: bool) -> Result<(), RomError> {
    let locations = c::location_tags(family, japanese_blue);
    let post_game: &[usize] = match (family, japanese_blue) {
        (Family::Yellow, _) => &c::POST_GAME_YELLOW,
        (Family::RedBlue, true) => &c::POST_GAME_JAPANESE_BLUE,
        (Family::RedBlue, false) => &c::POST_GAME_RBG,
    };
    if sets.len() != locations.len() {
        return Err(RomError::Malformed(format!(
            "expected {} wild encounter areas, found {}",
            locations.len(),
            sets.len()
        )));
    }
    for (set, location) in sets.iter_mut().zip(locations) {
        set.location = Some(location.to_owned());
    }
    for &i in post_game {
        sets[i].post_game = true;
    }
    Ok(())
}

/// Writes the sets that changed.
pub(super) fn write(
    rom: &mut GbRom,
    loaded: &Encounters,
    sets: &[EncounterSet],
    internal_of: &dyn Fn(DexNumber) -> Result<u8, RomError>,
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
            rom.write_byte(s.species, internal_of(e.species)?)?;
            rom.write_byte(s.level, e.min_level)?;
        }
    }
    Ok(())
}
