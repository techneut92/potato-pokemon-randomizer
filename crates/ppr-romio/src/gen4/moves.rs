//! Moves, read (with the stat changes, status and other details their effects imply) and written, with the
//! battle messages that name them.
//!
//! Deliberate choices:
//! - Future Sight and Doom Desire don't flinch; their case does not fall through into the flinching moves'.

use ppr_model::{
    CriticalChance as Crit, Move, MoveId, StatChange, StatChangeMoveType as Scm, StatChangeType as St,
    StatusMoveType as Smt, StatusType as Status, Type,
};

use super::Rom;
use super::constants as c;
use super::files::{byte, put, put_word, word};
use crate::RomError;

/// The longest move name the game can show.
pub(super) const MAX_NAME_LENGTH: usize = 15;

/// The target number of moves that change their user's stats.
const TARGET_USER: u16 = 16;

/// Gen 4's move effect numbers.
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
    pub(super) const STOMP: u16 = 150;
    pub(super) const SOLARBEAM: u16 = 151;
    pub(super) const THUNDER: u16 = 152;
    pub(super) const FLY: u16 = 155;
    pub(super) const DEFENSE_CURL: u16 = 156;
    pub(super) const FAKE_OUT: u16 = 158;
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
    pub(super) const DAMAGE_POISON_WITH_INCREASED_CRIT: u16 = 209;
    pub(super) const NO_DAMAGE_SP_ATK_AND_SP_DEF_PLUS_ONE: u16 = 211;
    pub(super) const NO_DAMAGE_ATK_AND_SPE_PLUS_ONE: u16 = 212;
    pub(super) const DAMAGE_USER_SPE_MINUS_ONE: u16 = 218;
    pub(super) const DAMAGE_USER_DEF_AND_SP_DEF_MINUS_ONE: u16 = 229;
    pub(super) const FLARE_BLITZ: u16 = 253;
    pub(super) const DIVE: u16 = 255;
    pub(super) const DIG: u16 = 256;
    pub(super) const BLIZZARD: u16 = 260;
    pub(super) const VOLT_TACKLE: u16 = 262;
    pub(super) const BOUNCE: u16 = 263;
    pub(super) const NO_DAMAGE_SP_ATK_MINUS_TWO: u16 = 265;
    pub(super) const CHATTER: u16 = 267;
    pub(super) const DAMAGE_RECOIL_50_PERCENT: u16 = 269;
    pub(super) const DAMAGE_SP_DEF_MINUS_TWO: u16 = 271;
    pub(super) const SHADOW_FORCE: u16 = 272;
    pub(super) const FIRE_FANG: u16 = 273;
    pub(super) const ICE_FANG: u16 = 274;
    pub(super) const THUNDER_FANG: u16 = 275;
    pub(super) const DAMAGE_USER_SP_ATK_PLUS_ONE: u16 = 276;
}

use effect as e;

/// The moves, 1 to 467, from the move data NARC: effect, category, power, type, accuracy, PP, effect chance,
/// target, priority and flags (bit 0: contact).
pub(super) fn read(rom: &mut Rom) -> Result<Vec<Move>, RomError> {
    let names = rom.strings_of("MoveNamesTextOffset")?;
    let path = rom.entry.file("MoveData")?;
    let narc = rom.files.narc(&path)?;
    (1..=c::MOVE_COUNT)
        .map(|i| {
            let b = narc
                .files
                .get(i)
                .ok_or_else(|| RomError::Malformed(format!("no data for move {i}")))?;
            let number = MoveId::try_from(i).map_err(|_| RomError::Malformed("too many moves".into()))?;
            let mut mv = Move {
                effect: word(b, 0)?,
                accuracy: byte(b, 5)?,
                pp: byte(b, 6)?,
                // The "???" type, Curse's, reads as none.
                move_type: c::type_from_byte(byte(b, 4)?),
                target: u8::try_from(word(b, 8)?).unwrap_or(u8::MAX),
                priority: i8::from_le_bytes([byte(b, 10)?]),
                hit_count_hundredths: hit_count(number),
                makes_contact: byte(b, 11)? & 1 != 0,
                is_punch: c::PUNCH_MOVES.contains(&number),
                is_sound: c::SOUND_MOVES.contains(&number),
                name: names.get(i).cloned().unwrap_or_default(),
                ..Move::basic(
                    number,
                    "",
                    u16::from(byte(b, 3)?),
                    Type::Normal,
                    c::category_from_byte(byte(b, 2)?),
                )
            };
            let target = word(b, 8)?;
            let chance = byte(b, 7)?;
            stat_changes(&mut mv, target, chance);
            status(&mut mv, chance);
            misc(&mut mv, chance);
            Ok(mv)
        })
        .collect()
}

fn hit_count(number: MoveId) -> u16 {
    if c::MULTIHIT_MOVES.contains(&number) {
        300
    } else if c::DOUBLE_HIT_MOVES.contains(&number) {
        200
    } else if number == c::TRIPLE_KICK {
        // This assumes the first hit lands.
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
fn stat_changes(mv: &mut Move, target: u16, chance: u8) {
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
        | e::NO_DAMAGE_ATK_AND_SPE_PLUS_ONE
        | e::NO_DAMAGE_SP_ATK_MINUS_TWO => {
            if target == TARGET_USER {
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
        | e::DAMAGE_ACCURACY_MINUS_ONE
        | e::DAMAGE_SP_DEF_MINUS_TWO => Scm::DamageTarget,
        e::DAMAGE_USER_DEF_PLUS_ONE
        | e::DAMAGE_USER_ATK_PLUS_ONE
        | e::DAMAGE_USER_ALL_PLUS_ONE
        | e::DAMAGE_USER_ATK_AND_DEF_MINUS_ONE
        | e::DAMAGE_USER_SP_ATK_MINUS_TWO
        | e::DAMAGE_USER_SPE_MINUS_ONE
        | e::DAMAGE_USER_DEF_AND_SP_DEF_MINUS_ONE
        | e::DAMAGE_USER_SP_ATK_PLUS_ONE => Scm::DamageUser,
        _ => return,
    };
    let changes: &[(St, i8)] = match mv.effect {
        e::NO_DAMAGE_ATK_PLUS_ONE | e::DAMAGE_USER_ATK_PLUS_ONE => &[(St::Attack, 1)],
        e::NO_DAMAGE_DEF_PLUS_ONE | e::DAMAGE_USER_DEF_PLUS_ONE | e::DEFENSE_CURL => &[(St::Defense, 1)],
        e::NO_DAMAGE_SP_ATK_PLUS_ONE | e::FLATTER | e::DAMAGE_USER_SP_ATK_PLUS_ONE => &[(St::SpecialAttack, 1)],
        e::NO_DAMAGE_EVASION_PLUS_ONE | e::MINIMIZE => &[(St::Evasion, 1)],
        e::NO_DAMAGE_ATK_MINUS_ONE | e::DAMAGE_ATK_MINUS_ONE => &[(St::Attack, -1)],
        e::NO_DAMAGE_DEF_MINUS_ONE | e::DAMAGE_DEF_MINUS_ONE => &[(St::Defense, -1)],
        e::NO_DAMAGE_SPE_MINUS_ONE | e::DAMAGE_SPE_MINUS_ONE | e::DAMAGE_USER_SPE_MINUS_ONE => &[(St::Speed, -1)],
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
        e::NO_DAMAGE_SP_DEF_MINUS_TWO | e::DAMAGE_SP_DEF_MINUS_TWO => &[(St::SpecialDefense, -2)],
        e::DAMAGE_SP_ATK_MINUS_ONE => &[(St::SpecialAttack, -1)],
        e::DAMAGE_SP_DEF_MINUS_ONE => &[(St::SpecialDefense, -1)],
        e::DAMAGE_USER_ALL_PLUS_ONE => &[(St::All, 1)],
        e::CHARGE => &[(St::SpecialDefense, 1)],
        e::DAMAGE_USER_ATK_AND_DEF_MINUS_ONE | e::NO_DAMAGE_ATK_AND_DEF_MINUS_ONE => {
            &[(St::Attack, -1), (St::Defense, -1)]
        }
        e::DAMAGE_USER_SP_ATK_MINUS_TWO | e::NO_DAMAGE_SP_ATK_MINUS_TWO => &[(St::SpecialAttack, -2)],
        e::NO_DAMAGE_DEF_AND_SP_DEF_PLUS_ONE => &[(St::Defense, 1), (St::SpecialDefense, 1)],
        e::NO_DAMAGE_ATK_AND_DEF_PLUS_ONE => &[(St::Attack, 1), (St::Defense, 1)],
        e::NO_DAMAGE_SP_ATK_AND_SP_DEF_PLUS_ONE => &[(St::SpecialAttack, 1), (St::SpecialDefense, 1)],
        e::NO_DAMAGE_ATK_AND_SPE_PLUS_ONE => &[(St::Attack, 1), (St::Speed, 1)],
        e::DAMAGE_USER_DEF_AND_SP_DEF_MINUS_ONE => &[(St::Defense, -1), (St::SpecialDefense, -1)],
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
        | e::DAMAGE_POISON_WITH_INCREASED_CRIT
        | e::FLARE_BLITZ
        | e::BLIZZARD
        | e::VOLT_TACKLE
        | e::BOUNCE
        | e::CHATTER
        | e::FIRE_FANG
        | e::ICE_FANG
        | e::THUNDER_FANG => Smt::Damage,
        _ => return,
    };
    mv.status = match mv.effect {
        e::NO_DAMAGE_SLEEP => Some(Status::Sleep),
        e::DAMAGE_POISON | e::NO_DAMAGE_POISON | e::TWINEEDLE | e::DAMAGE_POISON_WITH_INCREASED_CRIT => {
            Some(Status::Poison)
        }
        e::DAMAGE_BURN
        | e::DAMAGE_BURN_AND_THAW_USER
        | e::NO_DAMAGE_BURN
        | e::BLAZE_KICK
        | e::FLARE_BLITZ
        | e::FIRE_FANG => Some(Status::Burn),
        e::DAMAGE_FREEZE | e::BLIZZARD | e::ICE_FANG => Some(Status::Freeze),
        e::DAMAGE_PARALYZE | e::NO_DAMAGE_PARALYZE | e::THUNDER | e::VOLT_TACKLE | e::BOUNCE | e::THUNDER_FANG => {
            Some(Status::Paralyze)
        }
        e::TOXIC | e::POISON_FANG => Some(Status::ToxicPoison),
        e::NO_DAMAGE_CONFUSION | e::DAMAGE_CONFUSION | e::SWAGGER | e::FLATTER | e::TEETER_DANCE | e::CHATTER => {
            Some(Status::Confusion)
        }
        _ => None,
    };
    if mv.status_move_type == Smt::Damage {
        // Chatter's chance to confuse depends on the Chatot's recording; it counts as 1%.
        mv.status_percent_chance = match chance {
            0 if mv.number == c::CHATTER => 1,
            other => always_if_zero(other),
        };
    }
}

/// Other details the move's effect implies: critical hit chance, flinching, recoil, draining, charging,
/// recharging and trapping.
fn misc(mv: &mut Move, chance: u8) {
    match mv.effect {
        e::INCREASED_CRIT | e::BLAZE_KICK | e::DAMAGE_POISON_WITH_INCREASED_CRIT => {
            mv.critical_chance = Crit::Increased;
        }
        e::FUTURE_SIGHT_AND_DOOM_DESIRE => mv.critical_chance = Crit::None,
        e::FLINCH | e::SNORE | e::TWISTER | e::STOMP | e::FAKE_OUT | e::FIRE_FANG | e::ICE_FANG | e::THUNDER_FANG => {
            mv.flinch_percent_chance = chance;
        }
        e::DAMAGE_ABSORB | e::DREAM_EATER => mv.absorb_percent = 50,
        e::DAMAGE_RECOIL_25_PERCENT => mv.recoil_percent = 25,
        e::DAMAGE_RECOIL_33_PERCENT | e::FLARE_BLITZ | e::VOLT_TACKLE => mv.recoil_percent = 33,
        e::DAMAGE_RECOIL_50_PERCENT => mv.recoil_percent = 50,
        e::BINDING | e::TRAPPING => mv.is_trap = true,
        e::SKULL_BASH | e::SOLARBEAM | e::FLY | e::DIVE | e::DIG | e::BOUNCE | e::SHADOW_FORCE => mv.is_charge = true,
        e::RECHARGE => mv.is_recharge = true,
        e::RAZOR_WIND => {
            mv.critical_chance = Crit::Increased;
            mv.is_charge = true;
        }
        e::SKY_ATTACK => {
            mv.critical_chance = Crit::Increased;
            mv.flinch_percent_chance = chance;
            mv.is_charge = true;
        }
        _ => {}
    }
}

/// Writes the moves that changed: effect, category, power, type, accuracy (at most 100) and PP, and the names,
/// in the move names and in the three battle messages of each move ("X used Y!").
pub(super) fn write(rom: &mut Rom, loaded: &[Move], moves: &[Move]) -> Result<(), RomError> {
    let path = rom.entry.file("MoveData")?;
    for (_, mv) in loaded.iter().zip(moves).filter(|(old, mv)| old != mv) {
        let b = rom.files.narc_file(&path, usize::from(mv.number))?;
        put_word(b, 0, mv.effect)?;
        let power = u8::try_from(mv.power).map_err(|_| RomError::Malformed(format!("power {}", mv.power)))?;
        let type_byte = mv.move_type.map_or(c::UNKNOWN_TYPE, c::type_to_byte);
        put(
            b,
            2,
            &[
                c::category_to_byte(mv.category),
                power,
                type_byte,
                mv.accuracy.min(100),
                mv.pp,
            ],
        )?;
    }
    let renamed: Vec<(usize, &str, &str)> = loaded
        .iter()
        .zip(moves)
        .filter(|(old, mv)| old.name != mv.name)
        .map(|(old, mv)| (usize::from(mv.number), old.name.as_str(), mv.name.as_str()))
        .collect();
    if renamed.is_empty() {
        return Ok(());
    }
    let names_index = rom.entry.need("MoveNamesTextOffset")?;
    let mut names = rom.files.strings(names_index)?;
    let messages_index = rom.entry.need("BattleMoveNamesTextOffset")?;
    let mut messages = rom.files.strings(messages_index)?;
    for &(i, old, new) in &renamed {
        if let Some(name) = names.get_mut(i) {
            new.clone_into(name);
        }
        for message in messages.iter_mut().skip(i * 3).take(3) {
            *message = message.replace(old, new);
        }
    }
    rom.files.set_strings(names_index, names)?;
    rom.files.set_strings(messages_index, messages)
}
