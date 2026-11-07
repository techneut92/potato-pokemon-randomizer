//! Static encounters, read and written: the
//! `StaticPokemon{}` of the ROM entry, the hardcoded first battle of Ruby, Sapphire and Emerald or the Ghost Marowak
//! of `FireRed` and `LeafGreen`, and the roaming legendaries.
//!
//! The hardcoded encounters and the roamers only become editable through patches. They are applied to a copy for
//! reading, and to the ROM itself only when the statics are written, so a ROM
//! whose statics stay the same comes out unpatched. The patches write to fixed offsets in free space, which are taken
//! out of the free space when the ROM is loaded so text never lands there.
//!
//! Deliberate choices:
//! - Emerald's roaming Latios and Latias become whatever replaced them on Southern Island.
//! - A Ghost Marowak replacement that can be female is set female again, not only ever male or genderless, so
//!   re-randomizing a ROM cannot leave a female-only species male.
//! - The static music fix looks for a replaced species only among the music entries the game has, never past the
//!   end of the table (which matters when the ROM was randomized before).

use std::collections::BTreeMap;

use ppr_model::{DexNumber, StaticEncounter};

use super::Family;
use super::constants as c;
use crate::entries::{RomEntry, StaticSpot as Spot};
use crate::freespace::FreedSpace;
use crate::gba::GbaRom;
use crate::{RomError, ips};

/// The encounter a patch makes editable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Hardcoded {
    /// The Zigzagoon (Poochyena in Ruby and Sapphire) that attacks Professor Birch.
    FirstBattle {
        patch: ips::Patch,
        species: usize,
        level: usize,
    },
    /// The Ghost Marowak in Pokémon Tower, made female by the game.
    GhostMarowak {
        patch: ips::Patch,
        species: Vec<usize>,
        levels: Vec<usize>,
        gender: usize,
    },
}

/// How the game's roamers are found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Roaming {
    None,
    /// One roamer, the last static. Ruby computes its species, so a patch makes it a constant first.
    RubySapphire {
        ruby: bool,
    },
    /// Raikou, Entei and Suicune, the last three statics, once a patch made them constants.
    FireRedLeafGreen {
        patch: ips::Patch,
    },
    /// Latios and Latias, the same species as the Southern Island statics at these indices.
    Emerald {
        southern_island: Vec<usize>,
    },
}

/// The static music fix: a patch, and the table of species it plays special music for.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Music {
    patch: ips::Patch,
    pool: usize,
}

/// The game's static encounters and how to read and write them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Statics {
    spots: Vec<Spot>,
    eggs: Vec<usize>,
    hardcoded: Option<Hardcoded>,
    roamers: Vec<Spot>,
    roaming: Roaming,
    music: Option<Music>,
    special_music: Vec<DexNumber>,
    internal_count: usize,
    latios_internal: u16,
    create_roamer: Option<usize>,
    find_maps_with_mon: Option<usize>,
}

/// The byte at `at`.
fn byte(rom: &GbaRom, at: usize) -> Result<u8, RomError> {
    Ok(rom.slice(at, 1)?[0])
}

fn offsets(entry: &RomEntry, key: &str) -> Vec<usize> {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| usize::try_from(v).ok())
        .collect()
}

fn offset(entry: &RomEntry, key: &str) -> Result<usize, RomError> {
    entry
        .int(key)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed(format!("ROM entry has no {key}")))
}

fn patch(entry: &RomEntry, key: &str) -> Result<Option<ips::Patch>, RomError> {
    entry.value(key).map(ips::named).transpose()
}

impl Statics {
    /// What the ROM entry says about the statics; `None` when the game's are not supported.
    pub(super) fn new(
        entry: &RomEntry,
        family: Family,
        internal_count: usize,
        latios_internal: u16,
    ) -> Result<Option<Self>, RomError> {
        if entry.int("StaticPokemonSupport") != Some(1) {
            return Ok(None);
        }
        let hardcoded = if let Some(patch) = patch(entry, "StaticFirstBattleTweak")? {
            Some(Hardcoded::FirstBattle {
                patch,
                species: offset(entry, "StaticFirstBattleSpeciesOffset")?,
                level: offset(entry, "StaticFirstBattleLevelOffset")?,
            })
        } else if let Some(patch) = patch(entry, "GhostMarowakTweak")? {
            Some(Hardcoded::GhostMarowak {
                patch,
                species: offsets(entry, "GhostMarowakSpeciesOffsets"),
                levels: offsets(entry, "GhostMarowakLevelOffsets"),
                gender: offset(entry, "GhostMarowakGenderOffset")?,
            })
        } else {
            None
        };
        let roamers: Vec<Spot> = entry.list("RoamingPokemon{}").iter().map(|s| Spot::parse(s)).collect();
        let roaming = match family {
            _ if roamers.is_empty() => Roaming::None,
            Family::RubySapphire => Roaming::RubySapphire {
                ruby: entry.family.eq_ignore_ascii_case("ruby"),
            },
            Family::FireRedLeafGreen => match patch(entry, "RoamingPokemonTweak")? {
                Some(patch) => Roaming::FireRedLeafGreen { patch },
                None => Roaming::None,
            },
            Family::Emerald => Roaming::Emerald {
                southern_island: offsets(entry, "StaticSouthernIslandOffsets"),
            },
        };
        let music = match patch(entry, "NewIndexToMusicTweak")? {
            Some(patch) => Some(Music {
                patch,
                pool: offset(entry, "NewIndexToMusicPoolOffset")?,
            }),
            None => None,
        };
        let special_music = if music.is_some() {
            offsets(entry, "SpecialMusicStatics")
                .into_iter()
                .filter_map(|n| DexNumber::try_from(n).ok())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Some(Self {
            spots: entry.list("StaticPokemon{}").iter().map(|s| Spot::parse(s)).collect(),
            eggs: offsets(entry, "StaticEggPokemonOffsets"),
            hardcoded,
            roamers,
            roaming,
            music,
            special_music,
            internal_count,
            latios_internal,
            create_roamer: entry
                .int("CreateInitialRoamerMonFunctionStartOffset")
                .and_then(|v| usize::try_from(v).ok()),
            find_maps_with_mon: entry
                .int("FindMapsWithMonFunctionStartOffset")
                .and_then(|v| usize::try_from(v).ok()),
        }))
    }

    /// Species that play special music when met as statics, if the game's music can follow their replacements.
    pub(super) fn special_music(&self) -> &[DexNumber] {
        &self.special_music
    }

    /// Takes the space the patches write to out of `free`.
    pub(super) fn reserve(&self, free: &mut FreedSpace) {
        let patches = [
            match &self.hardcoded {
                Some(Hardcoded::FirstBattle { patch, .. } | Hardcoded::GhostMarowak { patch, .. }) => Some(patch),
                None => None,
            },
            match &self.roaming {
                Roaming::FireRedLeafGreen { patch } => Some(patch),
                _ => None,
            },
            self.music.as_ref().map(|m| &m.patch),
        ];
        for record in patches.into_iter().flatten().flat_map(ips::Patch::records) {
            free.reserve(record.offset, record.data.len());
        }
    }

    /// Applies the patches the hardcoded encounter and the roamers need, unless the ROM already has them.
    fn prepare(&self, rom: &mut GbaRom) -> Result<(), RomError> {
        match &self.hardcoded {
            Some(Hardcoded::FirstBattle { patch, species, .. }) if rom.u16(*species)? == 0xFFFF => {
                patch.apply(rom.bytes_mut())?;
            }
            Some(Hardcoded::GhostMarowak { patch, species, .. })
                if species
                    .first()
                    .is_some_and(|&at| rom.u16(at).is_ok_and(|w| w == 0xFFFF)) =>
            {
                patch.apply(rom.bytes_mut())?;
            }
            _ => {}
        }
        let first = self.roamers.first().and_then(|r| r.species.first().copied());
        match (&self.roaming, first) {
            // Before the patch, Ruby's first offset points into the 0x2000000 constant, which reads as 0.
            (Roaming::RubySapphire { ruby: true }, Some(at)) if rom.u16(at)? == 0 => self.patch_ruby_roamer(rom)?,
            (Roaming::FireRedLeafGreen { patch }, Some(at)) if rom.u16(at)? == 0xFFFF => {
                patch.apply(rom.bytes_mut())?;
            }
            // Before the patch, Emerald's first offset holds a pointer, far beyond the internal indices.
            (Roaming::Emerald { .. }, Some(at)) if usize::from(rom.u16(at)?) > self.internal_count => {
                self.patch_emerald_roamer(rom)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Ruby computes Latios's species as `0xCC << 1`; this loads it from a constant
    /// instead, which the roamer's species offsets then point at.
    fn patch_ruby_roamer(&self, rom: &mut GbaRom) -> Result<(), RomError> {
        let (Some(find), Some(create)) = (self.find_maps_with_mon, self.create_roamer) else {
            return Err(RomError::Malformed("ROM entry has no roamer function offsets".into()));
        };
        let latios = u32::from(self.latios_internal).to_le_bytes();
        // ldr r2, [pc, #0x15C]: the second copy of 0x2000000, so the first can hold Latios.
        rom.write(find + 22, &[0x57])?;
        rom.write(find + 128, &latios)?;
        // nop; ldr r0, [pc, #0x70]
        rom.write(find + 12, &[0x00, 0x00, 0x1C, 0x48])?;
        // pop {pc} instead of pop {r0}; bx r0, which frees a word for Latios.
        rom.write(create + 182, &[0x00, 0xBD])?;
        rom.write(create + 184, &latios)?;
        // nop; ldr r6, [pc, #0xA8]
        rom.write(create + 10, &[0x00, 0x00, 0x2A, 0x4E])
    }

    /// Makes Latios's species a constant like Latias's already is.
    fn patch_emerald_roamer(&self, rom: &mut GbaRom) -> Result<(), RomError> {
        let Some(create) = self.create_roamer else {
            return Err(RomError::Malformed("ROM entry has no roamer function offset".into()));
        };
        // ldr r0, [pc, #0x104]: the second copy of 0x03005D8C, so the first can hold Latios.
        rom.write(create + 14, &[0x41])?;
        rom.write(create + 28, &u32::from(self.latios_internal).to_le_bytes())?;
        // cmp r0, #0; ldr r3, [pc, #0x10] in place of a shift that changes nothing.
        rom.write(create + 8, &[0x00, 0x28, 0x04, 0x4B])?;
        // mov r2, r3 where Latios was computed as 0xCC << 1.
        rom.write(create + 48, &[0x1A, 0x46, 0x00, 0x00])
    }

    /// Sets the level of static encounter `index` (for balanced static levels): the level as a 16-bit word at its first
    /// level offset.
    pub(super) fn write_level(&self, rom: &mut GbaRom, index: usize, level: u8) -> Result<(), RomError> {
        if let Some(&at) = self.spots.get(index).and_then(|s| s.levels.first()) {
            rom.write_u16(at, u16::from(level))?;
        }
        Ok(())
    }

    /// The static encounters whose first level is at one of `offsets`.
    pub(super) fn at_levels(&self, offsets: &[i64]) -> Vec<usize> {
        self.spots
            .iter()
            .enumerate()
            .filter(|(_, spot)| {
                spot.levels
                    .first()
                    .is_some_and(|&l| offsets.iter().any(|&o| usize::try_from(o).ok() == Some(l)))
            })
            .map(|(i, _)| i)
            .collect()
    }

    /// The static encounters, read from a patched copy of `rom`.
    pub(super) fn read(&self, rom: &GbaRom, internal_to_dex: &[DexNumber]) -> Result<Vec<StaticEncounter>, RomError> {
        let mut probe = rom.clone();
        self.prepare(&mut probe)?;
        let rom = &probe;
        let species_at = |at: usize| -> Result<DexNumber, RomError> {
            let internal = rom.u16(at)?;
            internal_to_dex
                .get(usize::from(internal))
                .copied()
                .filter(|&n| n != 0)
                .ok_or_else(|| RomError::Malformed(format!("no species {internal} at {at:#x}")))
        };
        let read_spot = |spot: &Spot| -> Result<StaticEncounter, RomError> {
            let at = *spot
                .species
                .first()
                .ok_or_else(|| RomError::Malformed("static encounter without a species offset".into()))?;
            let level = match spot.levels.first() {
                Some(&l) => byte(rom, l)?,
                None => 1,
            };
            Ok(StaticEncounter::new(species_at(at)?, level))
        };

        let mut statics = Vec::new();
        for (i, spot) in self.spots.iter().enumerate() {
            let mut encounter = read_spot(spot)?;
            encounter.egg = self.eggs.contains(&i);
            statics.push(encounter);
        }
        match &self.hardcoded {
            Some(Hardcoded::FirstBattle { species, level, .. }) => {
                statics.push(StaticEncounter::new(species_at(*species)?, byte(rom, *level)?));
            }
            Some(Hardcoded::GhostMarowak { species, levels, .. }) => {
                let (Some(&s), Some(&l)) = (species.first(), levels.first()) else {
                    return Err(RomError::Malformed("Ghost Marowak without offsets".into()));
                };
                statics.push(StaticEncounter::new(species_at(s)?, byte(rom, l)?));
            }
            None => {}
        }
        match &self.roaming {
            Roaming::None => {}
            Roaming::RubySapphire { .. } => statics.push(read_spot(&self.roamers[0])?),
            Roaming::FireRedLeafGreen { .. } => {
                for roamer in &self.roamers {
                    statics.push(read_spot(roamer)?);
                }
            }
            Roaming::Emerald { southern_island } => {
                for (roamer, &index) in self.roamers.iter().zip(southern_island) {
                    let level = read_spot(roamer)?.level;
                    statics
                        .get_mut(index)
                        .ok_or_else(|| RomError::Malformed(format!("no Southern Island static {index}")))?
                        .linked_levels
                        .push(level);
                }
            }
        }
        Ok(statics)
    }

    /// How many statics [`Statics::read`] gives.
    fn count(&self) -> usize {
        let roamers = match self.roaming {
            Roaming::None | Roaming::Emerald { .. } => 0,
            Roaming::RubySapphire { .. } => 1,
            Roaming::FireRedLeafGreen { .. } => self.roamers.len(),
        };
        self.spots.len() + usize::from(self.hardcoded.is_some()) + roamers
    }

    /// Writes the static encounters, after patching what needs it. `gender_ratio` gives a species' gender ratio.
    pub(super) fn write(
        &self,
        rom: &mut GbaRom,
        statics: &[StaticEncounter],
        internal: impl Fn(DexNumber) -> Result<u16, RomError>,
        gender_ratio: impl Fn(&GbaRom, DexNumber) -> Result<u8, RomError>,
    ) -> Result<(), RomError> {
        if statics.len() != self.count() {
            return Err(RomError::Malformed(format!(
                "the game has {} static encounters, got {}",
                self.count(),
                statics.len()
            )));
        }
        self.prepare(rom)?;
        let write_spot = |rom: &mut GbaRom, spot: &Spot, species: DexNumber, level: u8, all_levels: bool| {
            let value = internal(species)?;
            for &at in &spot.species {
                rom.write_u16(at, value)?;
            }
            let levels = if all_levels {
                &spot.levels[..]
            } else {
                &spot.levels[..spot.levels.len().min(1)]
            };
            for &at in levels {
                rom.write(at, &[level])?;
            }
            Ok::<_, RomError>(())
        };

        for (spot, s) in self.spots.iter().zip(statics) {
            write_spot(rom, spot, s.species, s.level, false)?;
        }
        let hardcoded = &statics[self.spots.len()..];
        match (&self.hardcoded, hardcoded.first()) {
            (Some(Hardcoded::FirstBattle { species, level, .. }), Some(s)) => {
                rom.write_u16(*species, internal(s.species)?)?;
                rom.write(*level, &[s.level])?;
            }
            (
                Some(Hardcoded::GhostMarowak {
                    species,
                    levels,
                    gender,
                    ..
                }),
                Some(s),
            ) => {
                write_spot(
                    rom,
                    &Spot {
                        species: species.clone(),
                        levels: levels.clone(),
                    },
                    s.species,
                    s.level,
                    true,
                )?;
                // The game makes this Pokémon female, and loops forever for a species that can't be.
                let ratio = gender_ratio(rom, s.species)?;
                let value = if matches!(ratio, c::GENDER_MALE | c::GENDER_UNKNOWN) {
                    ratio
                } else {
                    c::GENDER_FEMALE
                };
                rom.write(*gender, &[value])?;
            }
            _ => {}
        }
        match &self.roaming {
            Roaming::None => {}
            Roaming::RubySapphire { .. } | Roaming::FireRedLeafGreen { .. } => {
                let first = statics.len() - self.roamers.len();
                for (roamer, s) in self.roamers.iter().zip(&statics[first..]) {
                    write_spot(rom, roamer, s.species, s.level, true)?;
                }
            }
            Roaming::Emerald { southern_island } => {
                for (roamer, &index) in self.roamers.iter().zip(southern_island) {
                    let s = &statics[index];
                    let level = s.linked_levels.first().copied().unwrap_or(s.level);
                    write_spot(rom, roamer, s.species, level, true)?;
                }
            }
        }
        Ok(())
    }

    /// The special music of each replaced species in [`Statics::special_music`]
    /// now plays for its replacement.
    pub(super) fn correct_music(
        &self,
        rom: &mut GbaRom,
        old: &[StaticEncounter],
        new: &[StaticEncounter],
        internal_to_dex: &[DexNumber],
        internal: impl Fn(DexNumber) -> Result<u16, RomError>,
    ) -> Result<(), RomError> {
        let Some(music) = &self.music else { return Ok(()) };
        // One replacement per species, the last one wins.
        let changes: BTreeMap<DexNumber, DexNumber> = old
            .iter()
            .zip(new)
            .filter(|(o, _)| self.special_music.contains(&o.species))
            .map(|(o, n)| (o.species, n.species))
            .collect();
        if changes.is_empty() {
            return Ok(());
        }
        music.patch.apply(rom.bytes_mut())?;
        let entries: Vec<usize> = (0..self.special_music.len())
            .map(|i| music.pool + i * c::MUSIC_POOL_ENTRY_SIZE)
            .collect();
        let mut replaced = Vec::new();
        for (old, new) in changes {
            let found = entries.iter().copied().find(|&at| {
                !replaced.contains(&at)
                    && rom
                        .u16(at)
                        .is_ok_and(|i| internal_to_dex.get(usize::from(i)) == Some(&old))
            });
            if let Some(at) = found {
                rom.write_u16(at, internal(new)?)?;
                replaced.push(at);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spots_with_species_and_level_offsets() {
        let spot = Spot::parse("{Species=[0x1A04A3, 0x15DE26, 0x15DE2D], Level=[0x15DE28]} // Groudon");
        assert_eq!(spot.species, [0x1A_04A3, 0x15_DE26, 0x15_DE2D]);
        assert_eq!(spot.levels, [0x15_DE28]);
        let egg = Spot::parse("{Species=[0x14E79A]} // Wynaut Egg");
        assert_eq!(egg.species, [0x14_E79A]);
        assert_eq!(egg.levels, Vec::<usize>::new());
    }
}
