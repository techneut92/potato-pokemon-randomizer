//! Trainers: their classes and names, teams, tags and the rival's starter positions.
//!
//! - Only the trainer classes whose teams changed are written, so unchanged data stays where it is.
//! - The abbreviated in-battle class names are written over the abbreviations.
//! - A class name is only written when it takes as many bytes as the old one, as the game finds a name by
//!   counting the names before it.

use ppr_model::{DexNumber, Trainer, TrainerPokemon};

use super::constants::{self as c, Family};
use super::need;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::GbRom;
use crate::gb::text::{GbText, length_of_string_at};

const SPECIAL_TRAINER: u8 = 0xFF;
const TERMINATOR: u8 = 0;

/// The trainer counts of the classes.
fn class_counts(entry: &RomEntry) -> Result<Vec<usize>, RomError> {
    let counts: Vec<usize> = entry
        .array("TrainerDataClassCounts")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|n| usize::try_from(n).ok())
        .collect();
    if counts.len() == c::TRAINER_CLASS_COUNT {
        Ok(counts)
    } else {
        Err(RomError::Malformed("conflicting count of trainer classes".into()))
    }
}

/// Where the full class names start.
fn class_names_offset(entry: &RomEntry) -> Result<(usize, Option<usize>), RomError> {
    let offsets: Vec<usize> = entry
        .array("TrainerClassNamesOffsets")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|o| usize::try_from(o).ok())
        .collect();
    let full = *offsets
        .last()
        .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no TrainerClassNamesOffsets", entry.name)))?;
    let abbreviated = (offsets.len() == 2).then(|| offsets[0]);
    Ok((full, abbreviated))
}

/// The names of the trainer classes.
pub(super) fn class_names(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<String>, RomError> {
    let (mut offset, _) = class_names_offset(entry)?;
    let mut names = Vec::with_capacity(c::TRAINER_CLASS_COUNT);
    for _ in 0..c::TRAINER_CLASS_COUNT {
        names.push(text.read_string(rom.bytes().get(offset..).unwrap_or_default(), usize::MAX, false));
        offset += length_of_string_at(rom.bytes(), offset, false);
    }
    Ok(names)
}

/// The team, and the trainer's length in bytes.
fn read_trainer(
    rom: &GbRom,
    mut offset: usize,
    dex_of: &dyn Fn(u8) -> DexNumber,
) -> Result<(Vec<TrainerPokemon>, usize), RomError> {
    let start = offset;
    let kind = rom.byte(offset)?;
    offset += 1;
    let mut team = Vec::new();
    while rom.byte(offset)? != TERMINATOR {
        if kind == SPECIAL_TRAINER {
            team.push(TrainerPokemon::new(dex_of(rom.byte(offset + 1)?), rom.byte(offset)?));
            offset += 2;
        } else {
            team.push(TrainerPokemon::new(dex_of(rom.byte(offset)?), kind));
            offset += 1;
        }
    }
    Ok((team, offset + 1 - start))
}

/// The trainers, tagged.
pub(super) fn read(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    dex_of: &dyn Fn(u8) -> DexNumber,
) -> Result<Vec<Trainer>, RomError> {
    let table = need(entry, "TrainerDataTableOffset")?;
    let mut trainers = Vec::new();
    for (class, &count) in class_counts(entry)?.iter().enumerate() {
        let mut offset = rom.pointer(table + class * 2)?;
        for _ in 0..count {
            let (team, length) = read_trainer(rom, offset, dex_of)?;
            let index =
                u16::try_from(trainers.len() + 1).map_err(|_| RomError::Malformed("too many trainers".into()))?;
            let class = u8::try_from(class).unwrap_or(u8::MAX);
            trainers.push(Trainer::new(index, "", class, team));
            offset += length;
        }
    }
    for (class, number, tag) in c::trainer_tags(family) {
        trainer_mut(&mut trainers, class, number)?.tag = Some(tag.to_owned());
    }
    for &(class, number, position) in c::forced_starter_positions(family) {
        trainer_mut(&mut trainers, class, number)?.force_starter_position = Some(position);
    }
    Ok(trainers)
}

/// Where the `number`-th trainer of class `class` (from 1) is.
pub(super) fn index_of(trainers: &[Trainer], class: u8, number: usize) -> Option<usize> {
    trainers
        .iter()
        .enumerate()
        .filter(|(_, t)| t.class + 1 == u16::from(class))
        .nth(number)
        .map(|(i, _)| i)
}

/// The `number`-th trainer of class `class` (from 1).
fn trainer_mut(trainers: &mut [Trainer], class: u8, number: usize) -> Result<&mut Trainer, RomError> {
    trainers
        .iter_mut()
        .filter(|t| t.class + 1 == u16::from(class))
        .nth(number)
        .ok_or_else(|| RomError::Malformed(format!("no trainer number {number} of class {class}")))
}

/// The Elite Four and the champion.
pub(super) fn elite_four(trainers: &[Trainer]) -> Vec<u16> {
    trainers
        .iter()
        .filter(|t| {
            t.tag
                .as_deref()
                .is_some_and(|tag| tag.contains("ELITE") || tag.contains("RIVAL8"))
        })
        .map(|t| t.index)
        .collect()
}

/// One level for the whole team when it can, else a level for each Pokémon.
fn trainer_bytes(t: &Trainer, internal_of: &dyn Fn(DexNumber) -> Result<u8, RomError>) -> Result<Vec<u8>, RomError> {
    let first = t
        .pokemon
        .first()
        .ok_or_else(|| RomError::Malformed(format!("trainer {} has no Pokémon", t.index)))?;
    let mut bytes = Vec::new();
    if t.pokemon.iter().all(|p| p.level == first.level) {
        bytes.push(first.level);
        for p in &t.pokemon {
            bytes.push(internal_of(p.species)?);
        }
    } else {
        bytes.push(SPECIAL_TRAINER);
        for p in &t.pokemon {
            bytes.extend([p.level, internal_of(p.species)?]);
        }
    }
    bytes.push(TERMINATOR);
    Ok(bytes)
}

/// Writes the classes whose trainers changed: the class's teams move to free space in their
/// bank when they grow. Then the custom moves of the gym leaders are switched off, and in Red and Blue the
/// champion's.
pub(super) fn write(
    rom: &mut GbRom,
    entry: &RomEntry,
    family: Family,
    old: &[Trainer],
    new: &[Trainer],
    internal_of: &dyn Fn(DexNumber) -> Result<u8, RomError>,
) -> Result<(), RomError> {
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
            bytes.extend(trainer_bytes(t, internal_of)?);
        }
        let pointer = table + class * 2;
        let mut old_length = 0;
        let mut offset = rom.pointer(pointer)?;
        for _ in range {
            let (_, length) = read_trainer(rom, offset, &|_| 0)?;
            old_length += length;
            offset += length;
        }
        rom.rewrite_same_bank(pointer, &bytes, old_length)?;
    }
    // The table of extra trainer moves, emptied.
    rom.write_byte(need(entry, "ExtraTrainerMovesTableOffset")?, 0xFF)?;
    if family == Family::RedBlue {
        // The jump to the champion's custom moves, a fixed distance before the gym leaders' table.
        let jump = need(entry, "GymLeaderMovesTableOffset")?
            .checked_sub(c::CHAMP_RIVAL_OFFSET_FROM_GYM_LEADER_MOVES)
            .ok_or_else(|| RomError::Malformed("GymLeaderMovesTableOffset too small".into()))?;
        rom.write(jump, &[super::Z80_NOP, super::Z80_NOP])?;
    }
    Ok(())
}

/// Writes the class names, when the ROM entry allows it: the full names, and the abbreviated in-battle ones.
pub(super) fn write_class_names(
    rom: &mut GbRom,
    entry: &RomEntry,
    text: &GbText,
    names: &[String],
) -> Result<(), RomError> {
    if entry.int("CanChangeTrainerText").unwrap_or_default() <= 0 {
        return Ok(());
    }
    let (mut offset, abbreviated) = class_names_offset(entry)?;
    for name in names.iter().take(c::TRAINER_CLASS_COUNT) {
        let old_length = length_of_string_at(rom.bytes(), offset, false);
        write_same_length(rom, text, offset, old_length, name)?;
        offset += old_length;
    }
    if let Some(mut offset) = abbreviated {
        for &class in &c::ABBREVIATED_TRAINER_CLASSES {
            let old_length = length_of_string_at(rom.bytes(), offset, false);
            if let Some(name) = names.get(class - 1) {
                write_same_length(rom, text, offset, old_length, name)?;
            }
            offset += old_length;
        }
    }
    Ok(())
}

/// A name in the `old_length` bytes (terminator included) at `offset`, if it takes exactly those bytes.
fn write_same_length(
    rom: &mut GbRom,
    text: &GbText,
    offset: usize,
    old_length: usize,
    name: &str,
) -> Result<(), RomError> {
    let bytes = text.terminated(name);
    if bytes.len() == old_length {
        rom.write(offset, &bytes)
    } else {
        tracing::warn!(
            "trainer class name {name:?} does not take {} bytes; kept the old one",
            old_length - 1
        );
        Ok(())
    }
}
