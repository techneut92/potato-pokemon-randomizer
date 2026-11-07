//! Trainers and trainer classes, read (with Emerald's Mossdeep Steven and the trainer tags) and written.
//!
//! Deliberate choices:
//! - Only trainers that changed are written, so unchanged teams stay where they are and keep an IV byte of 0.
//! - Mossdeep Steven's Pokémon whose moves are to be reset get the moves of their new species, so a new species
//!   never knows its predecessor's moves.

use std::collections::BTreeMap;

use ppr_model::{BattleStyle, DexNumber, GameData, ItemId, MultiBattleStatus, Trainer, TrainerPokemon, Type};

use super::space::Space;
use super::{Family, items, need, trades, trainer_tags as tags};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;

/// Emerald's Meteor Falls Steven: the trainer Mossdeep Steven takes his name and class from.
const METEOR_FALLS_STEVEN: usize = 804;
/// Mossdeep Steven's team: three Pokémon of 20 bytes.
pub(super) const STEVEN_TEAM: usize = 3;
const STEVEN_ENTRY: usize = 20;
const STEVEN_MOVES: usize = 12;
/// The highest IV byte, which means 31 IVs.
const MAX_IV_BYTE: u16 = 255;
const MAX_IVS: u16 = 31;

/// The table's layout, from the ROM entry.
#[derive(Debug, Clone, Copy)]
pub(super) struct Table {
    at: usize,
    count: usize,
    entry_size: usize,
    name_length: usize,
}

impl Table {
    pub(super) fn of(entry: &RomEntry) -> Result<Self, RomError> {
        Ok(Self {
            at: need(entry, "TrainerData")?,
            count: need(entry, "TrainerCount")?,
            entry_size: need(entry, "TrainerEntrySize")?,
            name_length: need(entry, "TrainerNameLength")?,
        })
    }

    fn entry(self, index: usize) -> usize {
        self.at + index * self.entry_size
    }

    fn battle_style(self, at: usize) -> usize {
        at + self.entry_size - 16
    }

    fn pokemon_count(self, at: usize) -> usize {
        at + self.entry_size - 8
    }

    fn pokemon_pointer(self, at: usize) -> usize {
        at + self.entry_size - 4
    }
}

/// The trainer class names.
pub(super) fn class_names(rom: &GbaRom, entry: &RomEntry, text: &TextTable) -> Result<Vec<String>, RomError> {
    let at = need(entry, "TrainerClassNames")?;
    let length = need(entry, "TrainerClassNameLength")?;
    (0..need(entry, "TrainerClassCount")?)
        .map(|i| Ok(text.decode(rom.slice(at + i * length, length)?, usize::MAX)))
        .collect()
}

/// Writes the trainer class names.
pub(super) fn write_class_names(
    rom: &mut GbaRom,
    entry: &RomEntry,
    text: &TextTable,
    names: &[String],
) -> Result<(), RomError> {
    let at = need(entry, "TrainerClassNames")?;
    let length = need(entry, "TrainerClassNameLength")?;
    for (i, name) in names.iter().enumerate().take(need(entry, "TrainerClassCount")?) {
        trades::write_name(rom, at + i * length, name, length, text)?;
    }
    Ok(())
}

/// The species at an internal index.
fn species(internal_to_dex: &[DexNumber], internal: u16) -> Result<DexNumber, RomError> {
    internal_to_dex
        .get(usize::from(internal))
        .copied()
        .filter(|&n| n != 0)
        .ok_or_else(|| RomError::Malformed(format!("a trainer has internal species {internal}, which isn't one")))
}

fn level(word: u16) -> Result<u8, RomError> {
    u8::try_from(word).map_err(|_| RomError::Malformed(format!("a trainer Pokémon has level {word}")))
}

fn held_item(internal: u16) -> Option<ItemId> {
    Some(items::standard(internal)).filter(|&i| i != 0)
}

/// The trainers from index 1, Emerald's Mossdeep Steven after them, then the tags.
pub(super) fn read(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    text: &TextTable,
    internal_to_dex: &[DexNumber],
) -> Result<Vec<Trainer>, RomError> {
    let table = Table::of(entry)?;
    let mut trainers = Vec::new();
    for i in 1..table.count {
        let at = table.entry(i);
        let poke_type = rom.slice(at, 1)?[0];
        let class = rom.slice(at + 1, 1)?[0];
        let name = text.decode(&rom.bytes()[at + 4..], usize::MAX);
        let count = usize::from(rom.slice(table.pokemon_count(at), 1)?[0]);
        let team = rom.pointer(table.pokemon_pointer(at))?;
        let size = if poke_type & 1 == 1 { 16 } else { 8 };
        let mut pokemon = Vec::with_capacity(count);
        for p in 0..count {
            let base = team + p * size;
            let mut tp = TrainerPokemon::new(
                species(internal_to_dex, rom.u16(base + 4)?)?,
                level(rom.u16(base + 2)?)?,
            );
            tp.ivs = u8::try_from((rom.u16(base)? & 0xFF) * MAX_IVS / MAX_IV_BYTE).unwrap_or_default();
            let mut moves_at = base + 6;
            if poke_type & 2 == 2 {
                tp.held_item = held_item(rom.u16(base + 6)?);
                moves_at += 2;
            }
            if poke_type & 1 == 1 {
                for (m, slot) in tp.moves.iter_mut().enumerate() {
                    *slot = rom.u16(moves_at + m * 2)?;
                }
            }
            pokemon.push(tp);
        }
        let mut trainer = Trainer::new(u16::try_from(i).unwrap_or_default(), name, class, pokemon);
        if rom.slice(table.battle_style(at), 1)?[0] == 1 {
            trainer.battle_style = BattleStyle::Double;
        }
        trainers.push(trainer);
    }
    if family == Family::Emerald {
        let steven = read_mossdeep_steven(rom, entry, &trainers, internal_to_dex)?;
        trainers.push(steven);
    }
    tag(&mut trainers, entry, family);
    Ok(trainers)
}

/// Steven's team in the Mossdeep tag battle isn't a trainer to the game, just
/// Pokémon data; he takes Meteor Falls Steven's name and class.
fn read_mossdeep_steven(
    rom: &GbaRom,
    entry: &RomEntry,
    trainers: &[Trainer],
    internal_to_dex: &[DexNumber],
) -> Result<Trainer, RomError> {
    let at = need(entry, "MossdeepStevenTeamOffset")?;
    let meteor_falls = trainers
        .get(METEOR_FALLS_STEVEN - 1)
        .ok_or_else(|| RomError::Malformed("Emerald has no Meteor Falls Steven".into()))?;
    let mut pokemon = Vec::with_capacity(STEVEN_TEAM);
    for i in 0..STEVEN_TEAM {
        let base = at + i * STEVEN_ENTRY;
        let bytes = rom.slice(base, 4)?;
        let mut tp = TrainerPokemon::new(species(internal_to_dex, rom.u16(base)?)?, bytes[3]);
        tp.ivs = bytes[2];
        for (m, slot) in tp.moves.iter_mut().enumerate() {
            *slot = rom.u16(base + STEVEN_MOVES + m * 2)?;
        }
        pokemon.push(tp);
    }
    let index = u16::try_from(trainers.len() + 1).unwrap_or_default();
    let mut steven = Trainer::new(index, meteor_falls.name.clone(), 0, pokemon);
    steven.class = meteor_falls.class;
    Ok(steven)
}

/// Tags the trainers of the game family, and in Emerald sets which ones fight multi battles.
fn tag(trainers: &mut [Trainer], entry: &RomEntry, family: Family) {
    let lists: [&[(&str, &[u16])]; 2] = match family {
        Family::RubySapphire if entry.family.eq_ignore_ascii_case("ruby") => [tags::RS, tags::RUBY],
        Family::RubySapphire => [tags::RS, tags::SAPPHIRE],
        Family::Emerald => [tags::EMERALD, &[]],
        Family::FireRedLeafGreen => [tags::FRLG, &[]],
    };
    for &(tag, indices) in lists.into_iter().flatten() {
        for &i in indices {
            if let Some(t) = trainers.get_mut(usize::from(i) - 1) {
                t.tag = Some(tag.to_owned());
            }
        }
    }
    if family == Family::Emerald {
        for (status, indices) in [
            (MultiBattleStatus::Always, &tags::EMERALD_ALWAYS_MULTI[..]),
            (MultiBattleStatus::Potential, &tags::EMERALD_POTENTIAL_MULTI[..]),
        ] {
            for &i in indices {
                if let Some(t) = trainers.get_mut(usize::from(i) - 1) {
                    t.multi_battle = status;
                }
            }
        }
    }
}

/// The type theme of each gym leader and Elite Four member, by tag.
pub(super) fn gym_and_elite_themes(family: Family) -> BTreeMap<String, Type> {
    use Type::{
        Dark, Dragon, Electric, Fighting, Fire, Flying, Ghost, Grass, Ground, Ice, Normal, Poison, Psychic, Rock,
        Steel, Water,
    };
    let themes: &[(&str, Type)] = match family {
        Family::FireRedLeafGreen => &[
            ("ELITE1", Ice),
            ("ELITE2", Fighting),
            ("ELITE3", Ghost),
            ("ELITE4", Dragon),
            ("GYM1", Rock),
            ("GYM2", Water),
            ("GYM3", Electric),
            ("GYM4", Grass),
            ("GYM5", Poison),
            ("GYM6", Psychic),
            ("GYM7", Fire),
            ("GYM8", Ground),
        ],
        Family::RubySapphire | Family::Emerald => &[
            ("CHAMPION", if family == Family::Emerald { Water } else { Steel }),
            ("ELITE1", Dark),
            ("ELITE2", Ghost),
            ("ELITE3", Ice),
            ("ELITE4", Dragon),
            ("GYM1", Rock),
            ("GYM2", Fighting),
            ("GYM3", Electric),
            ("GYM4", Fire),
            ("GYM5", Normal),
            ("GYM6", Flying),
            ("GYM7", Psychic),
            ("GYM8", Water),
        ],
    };
    themes.iter().map(|&(g, t)| (g.to_owned(), t)).collect()
}

/// Writes the trainers that changed.
pub(super) fn write(
    space: &mut Space<'_>,
    entry: &RomEntry,
    family: Family,
    text: &TextTable,
    old: &[Trainer],
    data: &GameData,
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    let table = Table::of(entry)?;
    let regular = table.count.saturating_sub(1);
    if data.trainers.len() != old.len() || data.trainers.len() < regular {
        return Err(RomError::Malformed("the trainers must keep their number".into()));
    }
    for (i, (trainer, before)) in data.trainers.iter().zip(old).take(regular).enumerate() {
        if trainer == before {
            continue;
        }
        let at = table.entry(i + 1);
        let bytes = pokemon_bytes(trainer, data, &internal_of)?;
        let old_type = space.rom.slice(at, 1)?[0];
        let old_count = usize::from(space.rom.slice(table.pokemon_count(at), 1)?[0]);
        let old_length = old_count * if old_type & 1 == 1 { 16 } else { 8 };
        space.rewrite(table.pokemon_pointer(at), old_length, &bytes, &[])?;
        space.rom.write(at, &[trainer.poke_type()])?;
        trades::write_name(space.rom, at + 4, &trainer.name, table.name_length, text)?;
        let count = u8::try_from(trainer.pokemon.len())
            .map_err(|_| RomError::Malformed(format!("trainer {} has too many Pokémon", trainer.index)))?;
        space.rom.write(table.pokemon_count(at), &[count])?;
        if trainer.forced_battle_style {
            let double = u8::from(trainer.battle_style == BattleStyle::Double);
            space.rom.write(table.battle_style(at), &[double])?;
        }
    }
    if family == Family::Emerald
        && let (Some(steven), Some(before)) = (data.trainers.last(), old.last())
        && steven != before
    {
        write_mossdeep_steven(space.rom, entry, steven, data, &internal_of)?;
    }
    Ok(())
}

fn internal(internal_of: &impl Fn(DexNumber) -> Option<u16>, species: DexNumber) -> Result<u16, RomError> {
    internal_of(species).ok_or_else(|| RomError::Malformed(format!("no species {species}")))
}

/// A trainer's team as bytes: 16 bytes a Pokémon with custom moves, 8 otherwise.
fn pokemon_bytes(
    trainer: &Trainer,
    data: &GameData,
    internal_of: &impl Fn(DexNumber) -> Option<u16>,
) -> Result<Vec<u8>, RomError> {
    let custom_moves = trainer.pokemon_have_custom_moves();
    let have_items = trainer.pokemon_have_items();
    let size = if custom_moves { 16 } else { 8 };
    let mut bytes = vec![0; trainer.pokemon.len() * size];
    for (tp, chunk) in trainer.pokemon.iter().zip(bytes.chunks_mut(size)) {
        // 1 is added against the truncation of reading.
        let iv_byte = (1 + u16::from(tp.ivs) * MAX_IV_BYTE / MAX_IVS).min(MAX_IV_BYTE);
        let item = if have_items {
            tp.held_item.and_then(items::internal).unwrap_or_default()
        } else {
            0
        };
        let mut words = vec![iv_byte, u16::from(tp.level), internal(internal_of, tp.species)?];
        if have_items || !custom_moves {
            words.push(item);
        }
        if custom_moves {
            let moves = if tp.reset_moves {
                data.moves_at_level(tp.species, tp.level)
            } else {
                tp.moves
            };
            words.extend(moves);
        }
        for (w, slot) in words.iter().zip(chunk.chunks_mut(2)) {
            slot.copy_from_slice(&w.to_le_bytes());
        }
    }
    Ok(bytes)
}

/// Writes Mossdeep Steven's team: his three Pokémon in place; anything beyond them is not his.
fn write_mossdeep_steven(
    rom: &mut GbaRom,
    entry: &RomEntry,
    steven: &Trainer,
    data: &GameData,
    internal_of: &impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    let at = need(entry, "MossdeepStevenTeamOffset")?;
    if steven.pokemon.len() < STEVEN_TEAM {
        return Err(RomError::Malformed("Mossdeep Steven needs three Pokémon".into()));
    }
    for (i, tp) in steven.pokemon.iter().take(STEVEN_TEAM).enumerate() {
        let base = at + i * STEVEN_ENTRY;
        rom.write_u16(base, internal(internal_of, tp.species)?)?;
        rom.write(base + 2, &[tp.ivs, tp.level])?;
        let moves = if tp.reset_moves {
            data.moves_at_level(tp.species, tp.level)
        } else {
            tp.moves
        };
        for (m, &mv) in moves.iter().enumerate() {
            rom.write_u16(base + STEVEN_MOVES + m * 2, mv)?;
        }
    }
    Ok(())
}
