//! Moves: their names and data, with the stat changes, statuses and other effects read from the effect byte and
//! the chance of the secondary effect.
//!
//! - Priority is stored with 0 as normal, as later games count it: the game's 1 is 0, Quick Attack +1, Protect
//!   and Endure +2, Roar, Whirlwind, Counter and Mirror Coat −1.
//! - Accuracy and secondary chances are whole percentages, rounded down. A move whose accuracy did not change keeps
//!   its original byte when the move is written, so other changes don't shift it by rounding; a changed accuracy is
//!   written as the smallest byte that reads as the same percentage, where rounding `percent × 2.55` reads 90% back
//!   as 89%.

use ppr_model::{
    CriticalChance, Move, MoveCategory, MoveId, PHYSICAL_TYPES, StatChange, StatChangeMoveType as Scm,
    StatChangeType as St, StatusMoveType as Smt, StatusType as Status, Type,
};

use super::constants::{self as c, effect as e};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::text::{GbText, length_of_string_at};
use crate::gb::{DOUBLE_HIT, GbRom, MULTIHIT, NO_POWER_NON_STATUS, need};

/// The longest move name that fits.
pub(super) const MAX_NAME_LENGTH: usize = 12;
const ENTRY_SIZE: usize = 7;

/// Where the move names are: the pointer, the byte that holds its bank, and the names.
fn name_pointer(rom: &GbRom, entry: &RomEntry) -> Result<(usize, usize, usize), RomError> {
    let table = need(entry, "NamesTableOffset")?;
    let (bank, pointer) = (table + 3, table + 4);
    let names = rom.pointer_in(pointer, usize::from(rom.byte(bank)?))?;
    Ok((pointer, bank, names))
}

/// The names of the moves, by number (index 0 is empty).
fn read_names(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<String>, RomError> {
    let (_, _, mut offset) = name_pointer(rom, entry)?;
    let mut names = vec![String::new()];
    for _ in 1..=c::MOVE_COUNT {
        names.push(text.read_string(rom.bytes().get(offset..).unwrap_or_default(), usize::MAX, false));
        offset += length_of_string_at(rom.bytes(), offset, false);
    }
    Ok(names)
}

/// A byte of 255 as a whole percentage, rounded down as the double arithmetic of the original does it.
fn percent(b: u8) -> u8 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "between 0 and 100 by construction"
    )]
    let p = (f64::from(b) / 255.0 * 100.0) as u8;
    p
}

/// A percentage back to a byte of 255: the smallest byte that reads as that percentage again.
fn percent_byte(percent: u8) -> u8 {
    u8::try_from((u32::from(percent.min(100)) * 255).div_ceil(100)).unwrap_or(u8::MAX)
}

/// Priority from the effect, 0 for normal.
fn priority(effect: u16) -> i8 {
    match effect {
        e::PRIORITY_HIT => 1,
        e::PROTECT | e::ENDURE => 2,
        e::FORCE_SWITCH | e::COUNTER | e::MIRROR_COAT => -1,
        _ => 0,
    }
}

/// The 251 moves.
pub(super) fn read(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<Move>, RomError> {
    let names = read_names(rom, entry, text)?;
    let data = need(entry, "MoveDataOffset")?;
    let mut moves = Vec::with_capacity(c::MOVE_COUNT);
    for (i, name) in names.iter().enumerate().skip(1) {
        let b = rom.slice(data + (i - 1) * ENTRY_SIZE, ENTRY_SIZE)?;
        let number = MoveId::try_from(i).map_err(|_| RomError::Malformed("too many moves".into()))?;
        let move_type = c::type_from_byte(b[3]);
        let power = u16::from(b[2]);
        let category = if power == 0 && !NO_POWER_NON_STATUS.contains(&number) {
            MoveCategory::Status
        } else if move_type.is_some_and(|t| PHYSICAL_TYPES.contains(&t)) {
            MoveCategory::Physical
        } else {
            MoveCategory::Special
        };
        let effect = u16::from(b[1]);
        let mut mv = Move {
            effect,
            pp: b[5],
            accuracy: percent(b[4]),
            move_type,
            priority: priority(effect),
            hit_count_hundredths: if MULTIHIT.contains(&number) {
                300
            } else if DOUBLE_HIT.contains(&number) {
                200
            } else if number == c::TRIPLE_KICK {
                // When the first hit lands.
                271
            } else {
                100
            },
            ..Move::basic(number, name, power, Type::Normal, category)
        };
        let chance = percent(b[6]);
        stat_changes(&mut mv, chance);
        status(&mut mv, chance);
        misc(&mut mv, chance);
        moves.push(mv);
    }
    Ok(moves)
}

/// The stat changes of a move's effect.
fn stat_changes(mv: &mut Move, chance: u8) {
    let (stat, stages): (St, i8) = match mv.effect {
        e::NO_DAMAGE_ATK_PLUS_ONE | e::DAMAGE_USER_ATK_PLUS_ONE => (St::Attack, 1),
        e::NO_DAMAGE_DEF_PLUS_ONE | e::DAMAGE_USER_DEF_PLUS_ONE | e::DEFENSE_CURL => (St::Defense, 1),
        e::NO_DAMAGE_SP_ATK_PLUS_ONE => (St::SpecialAttack, 1),
        e::NO_DAMAGE_EVASION_PLUS_ONE => (St::Evasion, 1),
        e::NO_DAMAGE_ATK_MINUS_ONE | e::DAMAGE_ATK_MINUS_ONE => (St::Attack, -1),
        e::NO_DAMAGE_DEF_MINUS_ONE | e::DAMAGE_DEF_MINUS_ONE => (St::Defense, -1),
        e::NO_DAMAGE_SPE_MINUS_ONE | e::DAMAGE_SPE_MINUS_ONE => (St::Speed, -1),
        e::NO_DAMAGE_ACCURACY_MINUS_ONE | e::DAMAGE_ACCURACY_MINUS_ONE => (St::Accuracy, -1),
        e::NO_DAMAGE_EVASION_MINUS_ONE => (St::Evasion, -1),
        e::NO_DAMAGE_ATK_PLUS_TWO | e::SWAGGER => (St::Attack, 2),
        e::NO_DAMAGE_DEF_PLUS_TWO => (St::Defense, 2),
        e::NO_DAMAGE_SPE_PLUS_TWO => (St::Speed, 2),
        e::NO_DAMAGE_SP_DEF_PLUS_TWO => (St::SpecialDefense, 2),
        e::NO_DAMAGE_ATK_MINUS_TWO => (St::Attack, -2),
        e::NO_DAMAGE_DEF_MINUS_TWO => (St::Defense, -2),
        e::NO_DAMAGE_SPE_MINUS_TWO => (St::Speed, -2),
        e::NO_DAMAGE_SP_DEF_MINUS_TWO => (St::SpecialDefense, -2),
        e::DAMAGE_SP_DEF_MINUS_ONE => (St::SpecialDefense, -1),
        e::DAMAGE_USER_ALL_PLUS_ONE => (St::All, 1),
        _ => return,
    };
    mv.stat_change_move_type = match mv.effect {
        e::DAMAGE_ATK_MINUS_ONE
        | e::DAMAGE_DEF_MINUS_ONE
        | e::DAMAGE_SPE_MINUS_ONE
        | e::DAMAGE_SP_DEF_MINUS_ONE
        | e::DAMAGE_ACCURACY_MINUS_ONE => Scm::DamageTarget,
        e::DAMAGE_USER_DEF_PLUS_ONE | e::DAMAGE_USER_ATK_PLUS_ONE | e::DAMAGE_USER_ALL_PLUS_ONE => Scm::DamageUser,
        e::SWAGGER => Scm::NoDamageTarget,
        _ if stages < 0 => Scm::NoDamageTarget,
        _ => Scm::NoDamageUser,
    };
    let damaging = matches!(mv.stat_change_move_type, Scm::DamageTarget | Scm::DamageUser);
    mv.stat_changes = vec![StatChange {
        stat,
        stages,
        percent_chance: match (damaging, chance) {
            (false, _) => 0,
            (true, 0) => 100,
            (true, c) => c,
        },
    }];
}

/// The status a move's effect inflicts.
fn status(mv: &mut Move, chance: u8) {
    let (kind, status) = match mv.effect {
        e::NO_DAMAGE_SLEEP => (Smt::NoDamage, Status::Sleep),
        e::TOXIC => (Smt::NoDamage, Status::ToxicPoison),
        e::NO_DAMAGE_CONFUSION | e::SWAGGER => (Smt::NoDamage, Status::Confusion),
        e::NO_DAMAGE_POISON => (Smt::NoDamage, Status::Poison),
        e::NO_DAMAGE_PARALYZE => (Smt::NoDamage, Status::Paralyze),
        e::DAMAGE_POISON | e::TWINEEDLE => (Smt::Damage, Status::Poison),
        e::DAMAGE_BURN | e::DAMAGE_BURN_AND_THAW_USER => (Smt::Damage, Status::Burn),
        e::DAMAGE_FREEZE => (Smt::Damage, Status::Freeze),
        e::DAMAGE_PARALYZE | e::THUNDER => (Smt::Damage, Status::Paralyze),
        e::DAMAGE_CONFUSION => (Smt::Damage, Status::Confusion),
        _ => return,
    };
    mv.status_move_type = kind;
    mv.status = Some(status);
    if kind == Smt::Damage {
        mv.status_percent_chance = if chance == 0 { 100 } else { chance };
    }
}

/// Crits, flinching, recoil, draining, trapping, charging and recharging, from a move's effect.
fn misc(mv: &mut Move, chance: u8) {
    match mv.effect {
        e::FLINCH | e::SNORE | e::TWISTER | e::STOMP => mv.flinch_percent_chance = chance,
        e::DAMAGE_ABSORB | e::DREAM_EATER => mv.absorb_percent = 50,
        e::DAMAGE_RECOIL => mv.recoil_percent = 25,
        e::FLAIL_AND_REVERSAL | e::FUTURE_SIGHT => mv.critical_chance = CriticalChance::None,
        e::BINDING | e::TRAPPING => mv.is_trap = true,
        e::RAZOR_WIND | e::SKY_ATTACK | e::SKULL_BASH | e::SOLARBEAM | e::SEMI_INVULNERABLE => mv.is_charge = true,
        e::HYPER_BEAM => mv.is_recharge = true,
        _ => {}
    }
    if c::INCREASED_CRIT_MOVES.contains(&mv.number) {
        mv.critical_chance = CriticalChance::Increased;
    }
}

/// Writes what changed: effect, power, type, accuracy and PP, and the names (which move to free space in any bank
/// when they grow).
pub(super) fn write(
    rom: &mut GbRom,
    entry: &RomEntry,
    text: &GbText,
    old: &[Move],
    new: &[Move],
) -> Result<(), RomError> {
    let data = need(entry, "MoveDataOffset")?;
    for (o, m) in old.iter().zip(new) {
        if o == m {
            continue;
        }
        let at = data + (usize::from(m.number) - 1) * ENTRY_SIZE + 1;
        let accuracy = if o.accuracy == m.accuracy {
            rom.byte(at + 3)?
        } else {
            percent_byte(m.accuracy)
        };
        let move_type = match m.move_type {
            Some(t) => c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 2 has no type {t:?}")))?,
            None => c::UNKNOWN_TYPE,
        };
        let effect = u8::try_from(m.effect).map_err(|_| RomError::Malformed("move effect past 255".into()))?;
        let power = u8::try_from(m.power).unwrap_or(u8::MAX);
        rom.write(at, &[effect, power, move_type, accuracy, m.pp])?;
    }
    if old.iter().zip(new).any(|(o, m)| o.name != m.name) {
        let (pointer, bank, names_at) = name_pointer(rom, entry)?;
        let mut by_number = read_names(rom, entry, text)?;
        for m in new {
            if let Some(slot) = by_number.get_mut(usize::from(m.number)) {
                slot.clone_from(&m.name);
            }
        }
        let bytes: Vec<u8> = by_number.iter().skip(1).flat_map(|n| text.terminated(n)).collect();
        let mut old_length = 0;
        for _ in 0..c::MOVE_COUNT {
            old_length += length_of_string_at(rom.bytes(), names_at + old_length, false);
        }
        rom.rewrite_indirect_bank(pointer, &[bank], &bytes, old_length)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentages_round_down_and_back() {
        assert_eq!(percent(255), 100);
        assert_eq!(percent(76), 29);
        assert_eq!(percent(51), 20);
        assert_eq!(percent_byte(100), 255);
        for p in 0..=100 {
            assert_eq!(percent(percent_byte(p)), p);
        }
    }

    #[test]
    fn priority_counts_from_normal() {
        assert_eq!(priority(e::PRIORITY_HIT), 1);
        assert_eq!(priority(e::PROTECT), 2);
        assert_eq!(priority(e::COUNTER), -1);
        assert_eq!(priority(0), 0);
    }
}
