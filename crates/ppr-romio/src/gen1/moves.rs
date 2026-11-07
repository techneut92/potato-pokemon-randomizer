//! Moves: their names and data, with the stat changes, statuses and other effects read from the effect byte.
//!
//! - The chance of a damaging move's stat drop is 33%, the game's 85/256.
//! - Accuracy is kept as a whole percentage, rounded down. A move whose accuracy did not
//!   change keeps its original byte when the move is written, so other changes don't shift it by rounding.

use ppr_model::{
    CriticalChance, Move, MoveCategory, MoveId, PHYSICAL_TYPES, StatChange, StatChangeMoveType as Scm,
    StatChangeType as St, StatusMoveType as Smt, StatusType as Status, Type,
};

use super::constants::{self as c, effect as e};
use super::need;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::GbRom;
use crate::gb::text::{GbText, length_of_string_at};

/// The longest move name that fits.
pub(super) const MAX_NAME_LENGTH: usize = 12;
const ENTRY_SIZE: usize = 6;
/// The game's chance of a damaging move's stat drop, 85 in 256, in percent.
const STAT_DROP_CHANCE: u8 = 33;

/// Where the move names are: the pointer and the bytes that hold its bank.
fn name_pointer(rom: &GbRom, entry: &RomEntry) -> Result<(usize, Vec<usize>, usize), RomError> {
    let pointer = need(entry, "NamesTableOffset")? + 2;
    let banks: Vec<usize> = entry
        .array("MoveNamesBankOffsets")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|b| usize::try_from(b).ok())
        .collect();
    let first = *banks
        .first()
        .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no MoveNamesBankOffsets", entry.name)))?;
    let names = rom.pointer_in(pointer, usize::from(rom.byte(first)?))?;
    Ok((pointer, banks, names))
}

/// The names of moves 1 to `MoveCount`, by internal index (index 0 is empty).
pub(super) fn read_names(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<String>, RomError> {
    let (_, _, mut offset) = name_pointer(rom, entry)?;
    let count = need(entry, "MoveCount")?;
    let mut names = vec![String::new()];
    for _ in 1..=count {
        names.push(text.read_string(rom.bytes().get(offset..).unwrap_or_default(), usize::MAX, false));
        offset += length_of_string_at(rom.bytes(), offset, false);
    }
    Ok(names)
}

/// The moves, and the internal index of each move number (index 0 unused). Slots without an animation
/// or named "Nothing" (Pokémon Brown) are skipped, and the moves numbered in order.
pub(super) fn read(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<(Vec<Move>, Vec<u8>), RomError> {
    let names = read_names(rom, entry, text)?;
    let data = need(entry, "MoveDataOffset")?;
    let mut moves = Vec::new();
    let mut internal_of = vec![0u8];
    for (i, name) in names.iter().enumerate().skip(1) {
        let b = rom.slice(data + (i - 1) * ENTRY_SIZE, ENTRY_SIZE)?;
        if b[0] == 0 || name == "Nothing" {
            continue;
        }
        let number = MoveId::try_from(moves.len() + 1).map_err(|_| RomError::Malformed("too many moves".into()))?;
        internal_of.push(u8::try_from(i).map_err(|_| RomError::Malformed("move index past 255".into()))?);
        let move_type = c::type_from_byte(b[3]);
        let power = u16::from(b[2]);
        let category = if power == 0 && !c::NO_POWER_NON_STATUS.contains(&number) {
            MoveCategory::Status
        } else if move_type.is_some_and(|t| PHYSICAL_TYPES.contains(&t)) {
            MoveCategory::Physical
        } else {
            MoveCategory::Special
        };
        let internal = MoveId::try_from(i).unwrap_or_default();
        let mut mv = Move {
            effect: u16::from(b[1]),
            pp: b[5],
            accuracy: accuracy_percent(b[4]),
            move_type,
            // The internal index, not the number.
            hit_count_hundredths: if c::MULTIHIT.contains(&internal) {
                300
            } else if c::DOUBLE_HIT.contains(&internal) {
                200
            } else {
                100
            },
            ..Move::basic(number, name, power, Type::Normal, category)
        };
        stat_changes(&mut mv);
        status(&mut mv);
        misc(&mut mv);
        moves.push(mv);
    }
    Ok((moves, internal_of))
}

/// The accuracy byte (of 255) as a whole percentage, rounded down.
fn accuracy_percent(b: u8) -> u8 {
    u8::try_from(u16::from(b) * 100 / 255).unwrap_or(100)
}

/// The accuracy percentage back to a byte.
fn accuracy_byte(percent: u8) -> u8 {
    u8::try_from((u32::from(percent) * 255 + 50) / 100).unwrap_or(u8::MAX)
}

/// The stat changes of a move's effect.
fn stat_changes(mv: &mut Move) {
    let change: (St, i8) = match mv.effect {
        e::NO_DAMAGE_ATK_PLUS_ONE => (St::Attack, 1),
        e::NO_DAMAGE_DEF_PLUS_ONE => (St::Defense, 1),
        e::NO_DAMAGE_SPECIAL_PLUS_ONE => (St::Special, 1),
        e::NO_DAMAGE_EVASION_PLUS_ONE => (St::Evasion, 1),
        e::NO_DAMAGE_ATK_MINUS_ONE | e::DAMAGE_ATK_MINUS_ONE => (St::Attack, -1),
        e::NO_DAMAGE_DEF_MINUS_ONE | e::DAMAGE_DEF_MINUS_ONE => (St::Defense, -1),
        e::NO_DAMAGE_SPE_MINUS_ONE | e::DAMAGE_SPE_MINUS_ONE => (St::Speed, -1),
        e::NO_DAMAGE_ACCURACY_MINUS_ONE => (St::Accuracy, -1),
        e::NO_DAMAGE_ATK_PLUS_TWO => (St::Attack, 2),
        e::NO_DAMAGE_DEF_PLUS_TWO => (St::Defense, 2),
        e::NO_DAMAGE_SPE_PLUS_TWO => (St::Speed, 2),
        e::NO_DAMAGE_SPECIAL_PLUS_TWO => (St::Special, 2),
        e::NO_DAMAGE_DEF_MINUS_TWO => (St::Defense, -2),
        e::DAMAGE_SPECIAL_MINUS_ONE => (St::Special, -1),
        _ => return,
    };
    let damaging = matches!(
        mv.effect,
        e::DAMAGE_ATK_MINUS_ONE | e::DAMAGE_DEF_MINUS_ONE | e::DAMAGE_SPE_MINUS_ONE | e::DAMAGE_SPECIAL_MINUS_ONE
    );
    mv.stat_change_move_type = if damaging {
        Scm::DamageTarget
    } else if change.1 < 0 {
        Scm::NoDamageTarget
    } else {
        Scm::NoDamageUser
    };
    mv.stat_changes = vec![StatChange {
        stat: change.0,
        stages: change.1,
        percent_chance: if damaging { STAT_DROP_CHANCE } else { 0 },
    }];
}

/// The status a move's effect inflicts.
fn status(mv: &mut Move) {
    let (kind, status, chance) = match mv.effect {
        e::NO_DAMAGE_SLEEP => (Smt::NoDamage, Status::Sleep, 0),
        e::NO_DAMAGE_CONFUSION => (Smt::NoDamage, Status::Confusion, 0),
        e::NO_DAMAGE_POISON => (Smt::NoDamage, Status::Poison, 0),
        e::NO_DAMAGE_PARALYZE => (Smt::NoDamage, Status::Paralyze, 0),
        e::DAMAGE_POISON_20 | e::TWINEEDLE => (Smt::Damage, Status::Poison, 20),
        e::DAMAGE_POISON_40 => (Smt::Damage, Status::Poison, 40),
        e::DAMAGE_BURN_10 => (Smt::Damage, Status::Burn, 10),
        e::DAMAGE_BURN_30 => (Smt::Damage, Status::Burn, 30),
        e::DAMAGE_FREEZE_10 => (Smt::Damage, Status::Freeze, 10),
        e::DAMAGE_FREEZE_30 => (Smt::Damage, Status::Freeze, 30),
        e::DAMAGE_PARALYZE_10 => (Smt::Damage, Status::Paralyze, 10),
        e::DAMAGE_PARALYZE_30 => (Smt::Damage, Status::Paralyze, 30),
        e::DAMAGE_CONFUSION => (Smt::Damage, Status::Confusion, 10),
        _ => return,
    };
    mv.status_move_type = kind;
    mv.status = Some(if status == Status::Poison && mv.number == c::TOXIC {
        Status::ToxicPoison
    } else {
        status
    });
    mv.status_percent_chance = chance;
}

/// Crits, flinching, recoil, draining, charging and recharging, from a move's effect.
fn misc(mv: &mut Move) {
    match mv.effect {
        e::FLINCH_10 => mv.flinch_percent_chance = 10,
        e::FLINCH_30 => mv.flinch_percent_chance = 30,
        e::DAMAGE_ABSORB | e::DREAM_EATER => mv.absorb_percent = 50,
        e::DAMAGE_RECOIL => mv.recoil_percent = 25,
        e::CHARGE | e::FLY => mv.is_charge = true,
        e::HYPER_BEAM => mv.is_recharge = true,
        _ => {}
    }
    if c::INCREASED_CRIT_MOVES.contains(&mv.number) {
        mv.critical_chance = CriticalChance::Increased;
    }
}

/// Writes what changed: effect, power, type, accuracy and PP, and the names
/// (which move to free space when they grow).
pub(super) fn write(
    rom: &mut GbRom,
    entry: &RomEntry,
    text: &GbText,
    internal_of: &[u8],
    old: &[Move],
    new: &[Move],
) -> Result<(), RomError> {
    let data = need(entry, "MoveDataOffset")?;
    for (o, m) in old.iter().zip(new) {
        if o == m {
            continue;
        }
        let internal = usize::from(internal_of[usize::from(m.number)]);
        let at = data + (internal - 1) * ENTRY_SIZE + 1;
        let accuracy = if o.accuracy == m.accuracy {
            rom.byte(at + 3)?
        } else {
            accuracy_byte(m.accuracy)
        };
        let move_type = m
            .move_type
            .map(|t| c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 1 has no type {t:?}"))))
            .transpose()?
            .unwrap_or(0);
        let effect = u8::try_from(m.effect).map_err(|_| RomError::Malformed("move effect past 255".into()))?;
        let power = u8::try_from(m.power).unwrap_or(u8::MAX);
        rom.write(at, &[effect, power, move_type, accuracy, m.pp])?;
    }
    if old.iter().zip(new).any(|(o, m)| o.name != m.name) {
        write_names(rom, entry, text, internal_of, new)?;
    }
    Ok(())
}

/// Every name, in internal order, terminated, where the old ones were or in free space.
fn write_names(
    rom: &mut GbRom,
    entry: &RomEntry,
    text: &GbText,
    internal_of: &[u8],
    moves: &[Move],
) -> Result<(), RomError> {
    let (pointer, banks, names_at) = name_pointer(rom, entry)?;
    let count = need(entry, "MoveCount")?;
    let mut by_internal = read_names(rom, entry, text)?;
    for m in moves {
        by_internal[usize::from(internal_of[usize::from(m.number)])].clone_from(&m.name);
    }
    let bytes: Vec<u8> = by_internal.iter().skip(1).flat_map(|n| text.terminated(n)).collect();
    let mut old_length = 0;
    for _ in 0..count {
        old_length += length_of_string_at(rom.bytes(), names_at + old_length, false);
    }
    rom.rewrite_indirect_bank(pointer, &banks, &bytes, old_length)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accuracy_round_trips_through_the_byte() {
        assert_eq!(accuracy_percent(255), 100);
        assert_eq!(accuracy_byte(100), 255);
        assert_eq!(accuracy_percent(242), 94);
        assert_eq!(accuracy_percent(accuracy_byte(85)), 85);
    }
}
