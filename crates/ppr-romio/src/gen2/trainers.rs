//! Trainers: their classes, names, teams with held items and moves, and tags.
//!
//! - Only the trainer classes whose trainers changed are written, so unchanged data stays where it is; a class
//!   that grows moves to free space in its bank.
//! - A class name is only written when it takes as many bytes as the old one, as the game finds a name by counting
//!   the names before it.

use ppr_model::{DexNumber, GameData, Trainer, TrainerPokemon};

use super::constants as c;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::text::{GbText, TERMINATOR, length_of_string_at};
use crate::gb::{GbRom, need, offsets};

/// The team has held items.
const HAS_ITEMS: u8 = 2;
/// The team has moves of its own.
const HAS_MOVES: u8 = 1;

/// The names of the trainer classes.
pub(super) fn class_names(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<String>, RomError> {
    let mut offset = need(entry, "TrainerClassNamesOffset")?;
    let count = need(entry, "TrainerClassAmount")?;
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        names.push(text.read_string(rom.bytes().get(offset..).unwrap_or_default(), usize::MAX, false));
        offset += length_of_string_at(rom.bytes(), offset, false);
    }
    Ok(names)
}

/// The trainer counts of the classes.
fn class_counts(entry: &RomEntry) -> Result<Vec<usize>, RomError> {
    let counts = offsets(entry, "TrainerDataClassCounts");
    if counts.len() == need(entry, "TrainerClassAmount")? {
        Ok(counts)
    } else {
        Err(RomError::Malformed("conflicting count of trainer classes".into()))
    }
}

/// The name and team of the trainer at `offset`, and the trainer's length in bytes: the name, a byte saying
/// whether the team has items and moves, then each Pokémon's level, species, item and four moves, until 0xFF.
fn read_trainer(
    rom: &GbRom,
    text: &GbText,
    mut offset: usize,
) -> Result<(String, Vec<TrainerPokemon>, usize), RomError> {
    let start = offset;
    let name = text.read_string(rom.bytes().get(offset..).unwrap_or_default(), usize::MAX, false);
    offset += length_of_string_at(rom.bytes(), offset, false);
    let kind = rom.byte(offset)?;
    offset += 1;
    let mut team = Vec::new();
    while rom.byte(offset)? != c::TRAINER_DATA_TERMINATOR {
        let mut tp = TrainerPokemon::new(DexNumber::from(rom.byte(offset + 1)?), rom.byte(offset)?);
        offset += 2;
        if kind & HAS_ITEMS != 0 {
            let item = c::item_standard(rom.byte(offset)?);
            tp.held_item = (item != 0).then_some(item);
            offset += 1;
        }
        if kind & HAS_MOVES != 0 {
            for (i, m) in tp.moves.iter_mut().enumerate() {
                *m = u16::from(rom.byte(offset + i)?);
            }
            offset += 4;
        }
        team.push(tp);
    }
    Ok((name, team, offset + 1 - start))
}

/// The trainers, tagged.
pub(super) fn read(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<Trainer>, RomError> {
    let table = need(entry, "TrainerDataTableOffset")?;
    let mut trainers = Vec::new();
    for (class, &count) in class_counts(entry)?.iter().enumerate() {
        let mut offset = rom.pointer(table + class * 2)?;
        for _ in 0..count {
            let (name, team, length) = read_trainer(rom, text, offset)?;
            let index =
                u16::try_from(trainers.len() + 1).map_err(|_| RomError::Malformed("too many trainers".into()))?;
            let class = u8::try_from(class).unwrap_or(u8::MAX);
            trainers.push(Trainer::new(index, name, class, team));
            offset += length;
        }
    }
    for &(class, number, tag) in c::TRAINER_TAGS.iter().chain(&c::GYM_TRAINER_TAGS) {
        // A tag for a trainer the game doesn't have is skipped.
        if let Some(t) = trainers
            .iter_mut()
            .filter(|t| t.class + 1 == u16::from(class))
            .nth(number)
        {
            t.tag = Some(tag.to_owned());
        }
    }
    Ok(trainers)
}

/// The Elite Four and the champion, by their place in the list, from 1.
pub(super) fn elite_four(trainers: &[Trainer]) -> Vec<u16> {
    trainers
        .iter()
        .enumerate()
        .filter(|(_, t)| {
            t.tag
                .as_deref()
                .is_some_and(|tag| tag.contains("ELITE") || tag.contains("CHAMPION"))
        })
        .filter_map(|(i, _)| u16::try_from(i + 1).ok())
        .collect()
}

/// A trainer's bytes: items when any Pokémon holds one, moves when any Pokémon has its own; a Pokémon to know its
/// level-up moves gets them.
fn trainer_bytes(
    t: &Trainer,
    data: &GameData,
    text: &GbText,
    species_byte: &dyn Fn(DexNumber) -> Result<u8, RomError>,
) -> Result<Vec<u8>, RomError> {
    let mut bytes = text.encode(&t.name);
    bytes.push(TERMINATOR);
    let kind = t.poke_type();
    bytes.push(kind);
    for p in &t.pokemon {
        bytes.extend([p.level, species_byte(p.species)?]);
        if kind & HAS_ITEMS != 0 {
            let item = match p.held_item {
                Some(id) => {
                    c::item_internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 2 has no item {id}")))?
                }
                None => 0,
            };
            bytes.push(item);
        }
        if kind & HAS_MOVES != 0 {
            let moves = if p.reset_moves {
                data.moves_at_level(p.species, p.level)
            } else {
                p.moves
            };
            for m in moves {
                bytes.push(u8::try_from(m).map_err(|_| RomError::Malformed(format!("Gen 2 has no move {m}")))?);
            }
        }
    }
    bytes.push(c::TRAINER_DATA_TERMINATOR);
    Ok(bytes)
}

/// Writes the classes whose trainers changed, moving a class to free space in its bank when it grows.
pub(super) fn write(
    rom: &mut GbRom,
    entry: &RomEntry,
    text: &GbText,
    old: &[Trainer],
    data: &GameData,
    species_byte: &dyn Fn(DexNumber) -> Result<u8, RomError>,
) -> Result<(), RomError> {
    let new = &data.trainers;
    if old.len() != new.len() {
        return Err(RomError::Malformed(format!(
            "expected {} trainers, got {}",
            old.len(),
            new.len()
        )));
    }
    let table = need(entry, "TrainerDataTableOffset")?;
    let mut first = 0;
    for (class, &count) in class_counts(entry)?.iter().enumerate() {
        let range = first..first + count;
        first += count;
        if count == 0 || old[range.clone()] == new[range.clone()] {
            continue;
        }
        let mut bytes = Vec::new();
        for t in &new[range.clone()] {
            bytes.extend(trainer_bytes(t, data, text, species_byte)?);
        }
        let pointer = table + class * 2;
        let mut old_length = 0;
        let mut offset = rom.pointer(pointer)?;
        for _ in range {
            let (_, _, length) = read_trainer(rom, text, offset)?;
            old_length += length;
            offset += length;
        }
        rom.rewrite_same_bank(pointer, &bytes, old_length)?;
    }
    Ok(())
}

/// Writes the class names, when the ROM entry allows it, each only if it takes as many bytes as the old one.
pub(super) fn write_class_names(
    rom: &mut GbRom,
    entry: &RomEntry,
    text: &GbText,
    old: &[String],
    names: &[String],
) -> Result<(), RomError> {
    if entry.int("CanChangeTrainerText").unwrap_or_default() == 0 {
        return Ok(());
    }
    let mut offset = need(entry, "TrainerClassNamesOffset")?;
    for (i, name) in names.iter().enumerate() {
        let old_length = length_of_string_at(rom.bytes(), offset, false);
        if old.get(i) != Some(name) {
            let bytes = text.terminated(name);
            if bytes.len() == old_length {
                rom.write(offset, &bytes)?;
            } else {
                tracing::warn!(
                    "trainer class name {name:?} does not take {} bytes; kept the old one",
                    old_length - 1
                );
            }
        }
        offset += old_length;
    }
    Ok(())
}
