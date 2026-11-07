//! Moves, read (with the stat changes, status and other details their effects imply) and written.
//!
//! Deliberate choices:
//! - Future Sight, Doom Desire and Spit Up don't flinch; their case does not fall through into the flinching
//!   moves'.

use ppr_model::{
    CriticalChance as Crit, Move, MoveCategory, MoveId, PHYSICAL_TYPES, StatChange, StatChangeMoveType as Scm,
    StatChangeType as St, StatusMoveType as Smt, StatusType as Status, Type,
};

use super::constants as c;
use super::trades::write_name;
use super::{entry_offset, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;

/// The longest move name the game can hold.
pub(super) const MAX_NAME_LENGTH: usize = 12;

/// Size of a move's entry in the move data.
const ENTRY_SIZE: usize = 0xC;
/// The target number of moves that change their user's stats.
const TARGET_USER: u8 = 16;

/// Damaging moves without a base power, like Seismic Toss.
const NO_POWER_NON_STATUS: [MoveId; 22] = [
    12, 32, 49, 67, 68, 69, 82, 90, 101, 117, 149, 162, 175, 216, 217, 218, 222, 243, 251, 255, 279, 329,
];
/// Moves that hit two to five times (three on average).
const MULTIHIT: [MoveId; 14] = [3, 4, 31, 42, 131, 140, 154, 198, 292, 331, 333, 350, 541, 594];
/// Moves that hit twice.
const DOUBLE_HIT: [MoveId; 8] = [24, 41, 155, 458, 530, 544, 742, 751];
/// Sound-based moves.
const SOUND: [MoveId; 12] = [45, 46, 47, 48, 103, 173, 195, 215, 253, 304, 319, 320];
const TRIPLE_KICK: MoveId = 167;
const BOUNCE: MoveId = 340;

/// Gen 3's move effect numbers.
mod effect {
    pub(super) const NO_DAMAGE_SLEEP: u16 = 1;
    pub(super) const DAMAGE_POISON: u16 = 2;
    pub(super) const DAMAGE_ABSORB: u16 = 3;
    pub(super) const DAMAGE_BURN: u16 = 4;
    pub(super) const DAMAGE_FREEZE: u16 = 5;
    pub(super) const DAMAGE_PARALYZE: u16 = 6;
    pub(super) const DREAM_EATER: u16 = 8;
    pub(super) const NO_DAMAGE_ATK_PLUS_ONE: u16 = 10;
    pub(super) const NO_DAMAGE_DEF_PLUS_ONE: u16 = 11;
    pub(super) const NO_DAMAGE_SP_ATK_PLUS_ONE: u16 = 13;
    pub(super) const NO_DAMAGE_EVASION_PLUS_ONE: u16 = 16;
    pub(super) const NO_DAMAGE_ATK_MINUS_ONE: u16 = 18;
    pub(super) const NO_DAMAGE_DEF_MINUS_ONE: u16 = 19;
    pub(super) const NO_DAMAGE_SPE_MINUS_ONE: u16 = 20;
    pub(super) const NO_DAMAGE_ACCURACY_MINUS_ONE: u16 = 23;
    pub(super) const NO_DAMAGE_EVASION_MINUS_ONE: u16 = 24;
    pub(super) const FLINCH: u16 = 31;
    pub(super) const TOXIC: u16 = 33;
    pub(super) const RAZOR_WIND: u16 = 39;
    pub(super) const BINDING: u16 = 42;
    pub(super) const INCREASED_CRIT: u16 = 43;
    pub(super) const DAMAGE_RECOIL_25_PERCENT: u16 = 48;
    pub(super) const NO_DAMAGE_CONFUSION: u16 = 49;
    pub(super) const NO_DAMAGE_ATK_PLUS_TWO: u16 = 50;
    pub(super) const NO_DAMAGE_DEF_PLUS_TWO: u16 = 51;
    pub(super) const NO_DAMAGE_SPE_PLUS_TWO: u16 = 52;
    pub(super) const NO_DAMAGE_SP_ATK_PLUS_TWO: u16 = 53;
    pub(super) const NO_DAMAGE_SP_DEF_PLUS_TWO: u16 = 54;
    pub(super) const NO_DAMAGE_ATK_MINUS_TWO: u16 = 58;
    pub(super) const NO_DAMAGE_DEF_MINUS_TWO: u16 = 59;
    pub(super) const NO_DAMAGE_SPE_MINUS_TWO: u16 = 60;
    pub(super) const NO_DAMAGE_SP_DEF_MINUS_TWO: u16 = 62;
    pub(super) const NO_DAMAGE_POISON: u16 = 66;
    pub(super) const NO_DAMAGE_PARALYZE: u16 = 67;
    pub(super) const DAMAGE_ATK_MINUS_ONE: u16 = 68;
    pub(super) const DAMAGE_DEF_MINUS_ONE: u16 = 69;
    pub(super) const DAMAGE_SPE_MINUS_ONE: u16 = 70;
    pub(super) const DAMAGE_SP_ATK_MINUS_ONE: u16 = 71;
    pub(super) const DAMAGE_SP_DEF_MINUS_ONE: u16 = 72;
    pub(super) const DAMAGE_ACCURACY_MINUS_ONE: u16 = 73;
    pub(super) const SKY_ATTACK: u16 = 75;
    pub(super) const DAMAGE_CONFUSION: u16 = 76;
    pub(super) const TWINEEDLE: u16 = 77;
    pub(super) const RECHARGE: u16 = 80;
    pub(super) const SNORE: u16 = 92;
    pub(super) const TRAPPING: u16 = 106;
    pub(super) const MINIMIZE: u16 = 108;
    pub(super) const SWAGGER: u16 = 118;
    pub(super) const DAMAGE_BURN_AND_THAW_USER: u16 = 125;
    pub(super) const DAMAGE_USER_DEF_PLUS_ONE: u16 = 138;
    pub(super) const DAMAGE_USER_ATK_PLUS_ONE: u16 = 139;
    pub(super) const DAMAGE_USER_ALL_PLUS_ONE: u16 = 140;
    pub(super) const SKULL_BASH: u16 = 145;
    pub(super) const TWISTER: u16 = 146;
    pub(super) const FUTURE_SIGHT_AND_DOOM_DESIRE: u16 = 148;
    pub(super) const FLINCH_WITH_MINIMIZE_BONUS: u16 = 150;
    pub(super) const SOLARBEAM: u16 = 151;
    pub(super) const THUNDER: u16 = 152;
    pub(super) const SEMI_INVULNERABLE: u16 = 155;
    pub(super) const DEFENSE_CURL: u16 = 156;
    pub(super) const FAKE_OUT: u16 = 158;
    pub(super) const SPIT_UP: u16 = 161;
    pub(super) const FLATTER: u16 = 166;
    pub(super) const NO_DAMAGE_BURN: u16 = 167;
    pub(super) const CHARGE: u16 = 174;
    pub(super) const DAMAGE_USER_ATK_AND_DEF_MINUS_ONE: u16 = 182;
    pub(super) const DAMAGE_RECOIL_33_PERCENT: u16 = 198;
    pub(super) const TEETER_DANCE: u16 = 199;
    pub(super) const BLAZE_KICK: u16 = 200;
    pub(super) const POISON_FANG: u16 = 202;
    pub(super) const DAMAGE_USER_SP_ATK_MINUS_TWO: u16 = 204;
    pub(super) const NO_DAMAGE_ATK_AND_DEF_MINUS_ONE: u16 = 205;
    pub(super) const NO_DAMAGE_DEF_AND_SP_DEF_PLUS_ONE: u16 = 206;
    pub(super) const NO_DAMAGE_ATK_AND_DEF_PLUS_ONE: u16 = 208;
    pub(super) const POISON_TAIL: u16 = 209;
    pub(super) const NO_DAMAGE_SP_ATK_AND_SP_DEF_PLUS_ONE: u16 = 211;
    pub(super) const NO_DAMAGE_ATK_AND_SPE_PLUS_ONE: u16 = 212;
}

use effect as e;

/// Where the moves are: data, names and name length.
struct Layout {
    count: usize,
    data: usize,
    names: usize,
    name_len: usize,
}

fn layout(rom: &GbaRom, entry: &RomEntry) -> Result<Layout, RomError> {
    Ok(Layout {
        count: need(entry, "MoveCount")?,
        data: entry_offset(rom, entry, "MoveData")?,
        names: entry_offset(rom, entry, "MoveNames")?,
        name_len: need(entry, "MoveNameLength")?,
    })
}

/// The moves, 1 to `MoveCount`.
pub(super) fn read(rom: &GbaRom, entry: &RomEntry, text: &TextTable) -> Result<Vec<Move>, RomError> {
    let l = layout(rom, entry)?;
    (1..=l.count)
        .map(|i| {
            let b = rom.slice(l.data + i * ENTRY_SIZE, ENTRY_SIZE)?;
            let number = MoveId::try_from(i).map_err(|_| RomError::Malformed("too many moves".into()))?;
            // The unused "???" type, Curse's, reads as none.
            let move_type = c::type_from_byte(b[2]);
            let power = u16::from(b[1]);
            let category = if power == 0 && !NO_POWER_NON_STATUS.contains(&number) {
                MoveCategory::Status
            } else if move_type.is_some_and(|t| PHYSICAL_TYPES.contains(&t)) {
                MoveCategory::Physical
            } else {
                MoveCategory::Special
            };
            let mut mv = Move {
                effect: u16::from(b[0]),
                pp: b[4],
                accuracy: b[3],
                target: b[6],
                priority: i8::from_le_bytes([b[7]]),
                hit_count_hundredths: hit_count(number),
                makes_contact: b[8] & 1 != 0,
                is_sound: SOUND.contains(&number),
                move_type,
                ..Move::basic(number, "", power, Type::Normal, category)
            };
            mv.name = text.decode(rom.slice(l.names + i * l.name_len, l.name_len)?, l.name_len);
            let chance = b[5];
            stat_changes(&mut mv, chance);
            status(&mut mv, chance);
            misc(&mut mv, chance);
            Ok(mv)
        })
        .collect()
}

fn hit_count(number: MoveId) -> u16 {
    if MULTIHIT.contains(&number) {
        300
    } else if DOUBLE_HIT.contains(&number) {
        200
    } else if number == TRIPLE_KICK {
        // "this assumes the first hit lands"
        271
    } else {
        100
    }
}

/// A secondary effect's chance; 0 in the data means always.
fn always_if_zero(chance: u8) -> u8 {
    if chance == 0 { 100 } else { chance }
}

/// The stat changes the move's effect implies.
fn stat_changes(mv: &mut Move, chance: u8) {
    mv.stat_change_move_type = match mv.effect {
        e::NO_DAMAGE_ATK_PLUS_ONE
        | e::NO_DAMAGE_DEF_PLUS_ONE
        | e::NO_DAMAGE_SP_ATK_PLUS_ONE
        | e::NO_DAMAGE_EVASION_PLUS_ONE
        | e::NO_DAMAGE_ATK_MINUS_ONE
        | e::NO_DAMAGE_DEF_MINUS_ONE
        | e::NO_DAMAGE_SPE_MINUS_ONE
        | e::NO_DAMAGE_ACCURACY_MINUS_ONE
        | e::NO_DAMAGE_EVASION_MINUS_ONE
        | e::NO_DAMAGE_ATK_PLUS_TWO
        | e::NO_DAMAGE_DEF_PLUS_TWO
        | e::NO_DAMAGE_SPE_PLUS_TWO
        | e::NO_DAMAGE_SP_ATK_PLUS_TWO
        | e::NO_DAMAGE_SP_DEF_PLUS_TWO
        | e::NO_DAMAGE_ATK_MINUS_TWO
        | e::NO_DAMAGE_DEF_MINUS_TWO
        | e::NO_DAMAGE_SPE_MINUS_TWO
        | e::NO_DAMAGE_SP_DEF_MINUS_TWO
        | e::MINIMIZE
        | e::SWAGGER
        | e::DEFENSE_CURL
        | e::FLATTER
        | e::CHARGE
        | e::NO_DAMAGE_ATK_AND_DEF_MINUS_ONE
        | e::NO_DAMAGE_DEF_AND_SP_DEF_PLUS_ONE
        | e::NO_DAMAGE_ATK_AND_DEF_PLUS_ONE
        | e::NO_DAMAGE_SP_ATK_AND_SP_DEF_PLUS_ONE
        | e::NO_DAMAGE_ATK_AND_SPE_PLUS_ONE => {
            if mv.target == TARGET_USER {
                Scm::NoDamageUser
            } else {
                Scm::NoDamageTarget
            }
        }
        e::DAMAGE_ATK_MINUS_ONE
        | e::DAMAGE_DEF_MINUS_ONE
        | e::DAMAGE_SPE_MINUS_ONE
        | e::DAMAGE_SP_ATK_MINUS_ONE
        | e::DAMAGE_SP_DEF_MINUS_ONE
        | e::DAMAGE_ACCURACY_MINUS_ONE => Scm::DamageTarget,
        e::DAMAGE_USER_DEF_PLUS_ONE
        | e::DAMAGE_USER_ATK_PLUS_ONE
        | e::DAMAGE_USER_ALL_PLUS_ONE
        | e::DAMAGE_USER_ATK_AND_DEF_MINUS_ONE
        | e::DAMAGE_USER_SP_ATK_MINUS_TWO => Scm::DamageUser,
        _ => return,
    };
    let changes: &[(St, i8)] = match mv.effect {
        e::NO_DAMAGE_ATK_PLUS_ONE | e::DAMAGE_USER_ATK_PLUS_ONE => &[(St::Attack, 1)],
        e::NO_DAMAGE_DEF_PLUS_ONE | e::DAMAGE_USER_DEF_PLUS_ONE | e::DEFENSE_CURL => &[(St::Defense, 1)],
        e::NO_DAMAGE_SP_ATK_PLUS_ONE | e::FLATTER => &[(St::SpecialAttack, 1)],
        e::NO_DAMAGE_EVASION_PLUS_ONE | e::MINIMIZE => &[(St::Evasion, 1)],
        e::NO_DAMAGE_ATK_MINUS_ONE | e::DAMAGE_ATK_MINUS_ONE => &[(St::Attack, -1)],
        e::NO_DAMAGE_DEF_MINUS_ONE | e::DAMAGE_DEF_MINUS_ONE => &[(St::Defense, -1)],
        e::NO_DAMAGE_SPE_MINUS_ONE | e::DAMAGE_SPE_MINUS_ONE => &[(St::Speed, -1)],
        e::NO_DAMAGE_ACCURACY_MINUS_ONE | e::DAMAGE_ACCURACY_MINUS_ONE => &[(St::Accuracy, -1)],
        e::NO_DAMAGE_EVASION_MINUS_ONE => &[(St::Evasion, -1)],
        e::NO_DAMAGE_ATK_PLUS_TWO | e::SWAGGER => &[(St::Attack, 2)],
        e::NO_DAMAGE_DEF_PLUS_TWO => &[(St::Defense, 2)],
        e::NO_DAMAGE_SPE_PLUS_TWO => &[(St::Speed, 2)],
        e::NO_DAMAGE_SP_ATK_PLUS_TWO => &[(St::SpecialAttack, 2)],
        e::NO_DAMAGE_SP_DEF_PLUS_TWO => &[(St::SpecialDefense, 2)],
        e::NO_DAMAGE_ATK_MINUS_TWO => &[(St::Attack, -2)],
        e::NO_DAMAGE_DEF_MINUS_TWO => &[(St::Defense, -2)],
        e::NO_DAMAGE_SPE_MINUS_TWO => &[(St::Speed, -2)],
        e::NO_DAMAGE_SP_DEF_MINUS_TWO => &[(St::SpecialDefense, -2)],
        e::DAMAGE_SP_ATK_MINUS_ONE => &[(St::SpecialAttack, -1)],
        e::DAMAGE_SP_DEF_MINUS_ONE => &[(St::SpecialDefense, -1)],
        e::DAMAGE_USER_ALL_PLUS_ONE => &[(St::All, 1)],
        e::CHARGE => &[(St::SpecialDefense, 1)],
        e::DAMAGE_USER_ATK_AND_DEF_MINUS_ONE | e::NO_DAMAGE_ATK_AND_DEF_MINUS_ONE => {
            &[(St::Attack, -1), (St::Defense, -1)]
        }
        e::DAMAGE_USER_SP_ATK_MINUS_TWO => &[(St::SpecialAttack, -2)],
        e::NO_DAMAGE_DEF_AND_SP_DEF_PLUS_ONE => &[(St::Defense, 1), (St::SpecialDefense, 1)],
        e::NO_DAMAGE_ATK_AND_DEF_PLUS_ONE => &[(St::Attack, 1), (St::Defense, 1)],
        e::NO_DAMAGE_SP_ATK_AND_SP_DEF_PLUS_ONE => &[(St::SpecialAttack, 1), (St::SpecialDefense, 1)],
        e::NO_DAMAGE_ATK_AND_SPE_PLUS_ONE => &[(St::Attack, 1), (St::Speed, 1)],
        _ => &[],
    };
    let damaging = matches!(mv.stat_change_move_type, Scm::DamageTarget | Scm::DamageUser);
    let percent_chance = if damaging { always_if_zero(chance) } else { 0 };
    mv.stat_changes = changes
        .iter()
        .map(|&(stat, stages)| StatChange {
            stat,
            stages,
            percent_chance,
        })
        .collect();
}

/// The status condition the move's effect implies.
fn status(mv: &mut Move, chance: u8) {
    if mv.number == BOUNCE {
        // "GF hardcoded this, so we have to as well"
        mv.status_move_type = Smt::Damage;
        mv.status = Some(Status::Paralyze);
        mv.status_percent_chance = chance;
        return;
    }
    mv.status_move_type = match mv.effect {
        e::NO_DAMAGE_SLEEP
        | e::TOXIC
        | e::NO_DAMAGE_CONFUSION
        | e::NO_DAMAGE_POISON
        | e::NO_DAMAGE_PARALYZE
        | e::NO_DAMAGE_BURN
        | e::SWAGGER
        | e::FLATTER
        | e::TEETER_DANCE => Smt::NoDamage,
        e::DAMAGE_POISON
        | e::DAMAGE_BURN
        | e::DAMAGE_FREEZE
        | e::DAMAGE_PARALYZE
        | e::DAMAGE_CONFUSION
        | e::TWINEEDLE
        | e::DAMAGE_BURN_AND_THAW_USER
        | e::THUNDER
        | e::BLAZE_KICK
        | e::POISON_FANG
        | e::POISON_TAIL => Smt::Damage,
        _ => return,
    };
    mv.status = match mv.effect {
        e::NO_DAMAGE_SLEEP => Some(Status::Sleep),
        e::DAMAGE_POISON | e::NO_DAMAGE_POISON | e::TWINEEDLE | e::POISON_TAIL => Some(Status::Poison),
        e::DAMAGE_BURN | e::DAMAGE_BURN_AND_THAW_USER | e::NO_DAMAGE_BURN | e::BLAZE_KICK => Some(Status::Burn),
        e::DAMAGE_FREEZE => Some(Status::Freeze),
        e::DAMAGE_PARALYZE | e::NO_DAMAGE_PARALYZE | e::THUNDER => Some(Status::Paralyze),
        e::TOXIC | e::POISON_FANG => Some(Status::ToxicPoison),
        e::NO_DAMAGE_CONFUSION | e::DAMAGE_CONFUSION | e::SWAGGER | e::FLATTER | e::TEETER_DANCE => {
            Some(Status::Confusion)
        }
        _ => None,
    };
    if mv.status_move_type == Smt::Damage {
        mv.status_percent_chance = always_if_zero(chance);
    }
}

/// Other details the move's effect implies: critical hit chance, flinching, recoil, draining, charging,
/// recharging and trapping.
fn misc(mv: &mut Move, chance: u8) {
    match mv.effect {
        e::INCREASED_CRIT | e::BLAZE_KICK | e::POISON_TAIL => mv.critical_chance = Crit::Increased,
        e::FUTURE_SIGHT_AND_DOOM_DESIRE | e::SPIT_UP => mv.critical_chance = Crit::None,
        e::FLINCH | e::SNORE | e::TWISTER | e::FLINCH_WITH_MINIMIZE_BONUS | e::FAKE_OUT => {
            mv.flinch_percent_chance = chance;
        }
        e::DAMAGE_ABSORB | e::DREAM_EATER => mv.absorb_percent = 50,
        e::DAMAGE_RECOIL_25_PERCENT => mv.recoil_percent = 25,
        e::DAMAGE_RECOIL_33_PERCENT => mv.recoil_percent = 33,
        e::BINDING | e::TRAPPING => mv.is_trap = true,
        e::RAZOR_WIND | e::SKULL_BASH | e::SOLARBEAM | e::SEMI_INVULNERABLE => mv.is_charge = true,
        e::RECHARGE => mv.is_recharge = true,
        e::SKY_ATTACK => {
            mv.critical_chance = Crit::Increased;
            mv.flinch_percent_chance = chance;
            mv.is_charge = true;
        }
        _ => {}
    }
}

/// Writes the moves that changed: name, effect, power, type, accuracy (at most 100) and PP.
pub(super) fn write(
    rom: &mut GbaRom,
    entry: &RomEntry,
    text: &TextTable,
    loaded: &[Move],
    moves: &[Move],
) -> Result<(), RomError> {
    let l = layout(rom, entry)?;
    for (old, mv) in loaded.iter().zip(moves).filter(|(old, mv)| old != mv) {
        let i = usize::from(mv.number);
        if old.name != mv.name {
            write_name(rom, l.names + i * l.name_len, &mv.name, l.name_len, text)?;
        }
        let type_byte = match mv.move_type {
            Some(t) => c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no type {t:?}")))?,
            None => c::UNKNOWN_TYPE,
        };
        let effect = u8::try_from(mv.effect).map_err(|_| RomError::Malformed(format!("effect {}", mv.effect)))?;
        let power = u8::try_from(mv.power).map_err(|_| RomError::Malformed(format!("power {}", mv.power)))?;
        rom.write(
            l.data + i * ENTRY_SIZE,
            &[effect, power, type_byte, mv.accuracy.min(100), mv.pp],
        )?;
    }
    Ok(())
}
