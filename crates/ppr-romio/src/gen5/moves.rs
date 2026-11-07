//! Moves (`MoveData`): read in full, and their name, type, category, power, accuracy and PP written, with the battle
//! messages that name them.

use ppr_model::{
    CriticalChance, Move, MoveCategory, StatChange, StatChangeMoveType, StatChangeType, StatusMoveType, StatusType,
};

use super::species::{type_byte, type_of};
use super::store::{Store, Text, byte, long, put_byte, word};
use crate::RomError;
use crate::gb::{DOUBLE_HIT, MULTIHIT};

/// The moves of Gen 5.
pub(super) const MOVE_COUNT: u16 = 559;
/// The longest move name the game can show.
pub(super) const MAX_NAME_LENGTH: usize = 15;

const TOXIC: u16 = 92;
const POISON_FANG: u16 = 305;
const CHATTER: u16 = 448;
const TRIPLE_KICK: u16 = 167;
const TRAPPING_EFFECT: u16 = 106;

// Move qualities (byte 1).
const NO_DAMAGE_STATUS: u8 = 1;
const NO_DAMAGE_STAT_CHANGE: u8 = 2;
const DAMAGE_STATUS: u8 = 4;
const NO_DAMAGE_STATUS_AND_STAT_CHANGE: u8 = 5;
const DAMAGE_TARGET_DEBUFF: u8 = 6;
const DAMAGE_USER_BUFF: u8 = 7;
const DAMAGE_ABSORB: u8 = 8;

/// Byte offsets in a move's file.
const TYPE: usize = 0;
const QUALITY: usize = 1;
const CATEGORY: usize = 2;
const POWER: usize = 3;
const ACCURACY: usize = 4;
const PP: usize = 5;

fn category(b: u8) -> MoveCategory {
    match b {
        1 => MoveCategory::Physical,
        2 => MoveCategory::Special,
        _ => MoveCategory::Status,
    }
}

fn category_byte(c: MoveCategory) -> u8 {
    match c {
        MoveCategory::Status => 0,
        MoveCategory::Physical => 1,
        MoveCategory::Special => 2,
    }
}

fn stat_change_type(b: u8) -> Option<StatChangeType> {
    Some(match b {
        1 => StatChangeType::Attack,
        2 => StatChangeType::Defense,
        3 => StatChangeType::SpecialAttack,
        4 => StatChangeType::SpecialDefense,
        5 => StatChangeType::Speed,
        6 => StatChangeType::Accuracy,
        7 => StatChangeType::Evasion,
        8 => StatChangeType::All,
        9 => StatChangeType::Special,
        _ => return None,
    })
}

fn status_type(b: u16) -> Option<StatusType> {
    Some(match b {
        1 => StatusType::Paralyze,
        2 => StatusType::Sleep,
        3 => StatusType::Freeze,
        4 => StatusType::Burn,
        5 => StatusType::Poison,
        6 => StatusType::Confusion,
        _ => return None,
    })
}

fn read_move(number: u16, name: String, data: &[u8]) -> Result<Move, RomError> {
    let effect = word(data, 16)?;
    let target = byte(data, 20)?;
    let quality = byte(data, QUALITY)?;
    let status = word(data, 8)?;
    let flags = long(data, 32)?;
    let crit = byte(data, 14)?;
    let recoil_or_absorb = byte(data, 18)?.cast_signed();
    let mut mv = Move {
        number,
        name,
        effect,
        power: u16::from(byte(data, POWER)?),
        pp: byte(data, PP)?,
        accuracy: byte(data, ACCURACY)?,
        move_type: type_of(byte(data, TYPE)?),
        category: category(byte(data, CATEGORY)?),
        target,
        priority: byte(data, 6)?.cast_signed(),
        hit_count_hundredths: if MULTIHIT.contains(&number) {
            317
        } else if DOUBLE_HIT.contains(&number) {
            200
        } else if number == TRIPLE_KICK {
            // The first hit is assumed to land.
            271
        } else {
            100
        },
        makes_contact: flags & 0x001 != 0,
        stat_change_move_type: match quality {
            // All allies or the user.
            NO_DAMAGE_STAT_CHANGE | NO_DAMAGE_STATUS_AND_STAT_CHANGE if target == 6 || target == 7 => {
                StatChangeMoveType::NoDamageUser
            }
            NO_DAMAGE_STAT_CHANGE | NO_DAMAGE_STATUS_AND_STAT_CHANGE => StatChangeMoveType::NoDamageTarget,
            DAMAGE_TARGET_DEBUFF => StatChangeMoveType::DamageTarget,
            DAMAGE_USER_BUFF => StatChangeMoveType::DamageUser,
            _ => StatChangeMoveType::NoneOrUnknown,
        },
        stat_changes: Vec::new(),
        status_move_type: StatusMoveType::NoneOrUnknown,
        status: None,
        status_percent_chance: 0,
        critical_chance: match crit {
            6 => CriticalChance::Guaranteed,
            0 => CriticalChance::Normal,
            _ => CriticalChance::Increased,
        },
        flinch_percent_chance: byte(data, 15)?,
        recoil_percent: 0,
        absorb_percent: 0,
        is_charge: flags & 0x002 != 0,
        is_recharge: flags & 0x004 != 0,
        is_punch: flags & 0x080 != 0,
        is_sound: flags & 0x100 != 0,
        is_trap: effect == TRAPPING_EFFECT || status == 8,
    };
    if quality == DAMAGE_ABSORB {
        mv.absorb_percent = u8::try_from(recoil_or_absorb).unwrap_or_default();
    } else {
        mv.recoil_percent = u8::try_from(-i16::from(recoil_or_absorb)).unwrap_or_default();
    }
    for i in 0..3 {
        if let Some(stat) = stat_change_type(byte(data, 21 + i)?) {
            mv.stat_changes.push(StatChange {
                stat,
                stages: byte(data, 24 + i)?.cast_signed(),
                percent_chance: byte(data, 27 + i)?,
            });
        }
    }
    // Status types beyond the known ones (as trapping) are left out.
    if status < 7 {
        mv.status = status_type(status).map(|s| {
            if s == StatusType::Poison && (number == TOXIC || number == POISON_FANG) {
                StatusType::ToxicPoison
            } else {
                s
            }
        });
        mv.status_percent_chance = if number == CHATTER { 1 } else { byte(data, 10)? };
        mv.status_move_type = match quality {
            NO_DAMAGE_STATUS | NO_DAMAGE_STATUS_AND_STAT_CHANGE => StatusMoveType::NoDamage,
            DAMAGE_STATUS => StatusMoveType::Damage,
            _ => StatusMoveType::NoneOrUnknown,
        };
    }
    Ok(mv)
}

/// Every move, from 1.
pub(super) fn read(store: &mut Store, path: &str, names_text: usize) -> Result<Vec<Move>, RomError> {
    let names = store.strings(Text::Strings, names_text)?.to_vec();
    (1..=MOVE_COUNT)
        .map(|n| {
            let name = names.get(usize::from(n)).cloned().unwrap_or_default();
            read_move(n, name, store.narc_file(path, usize::from(n))?)
        })
        .collect()
}

/// Writes the moves that changed: their data, their names and the battle messages with their names (three per
/// move).
pub(super) fn write(
    store: &mut Store,
    path: &str,
    names_text: usize,
    messages_text: usize,
    old: &[Move],
    new: &[Move],
) -> Result<(), RomError> {
    let renamed: Vec<(usize, &str, &str)> = new
        .iter()
        .zip(old)
        .filter(|(m, b)| m.name != b.name)
        .map(|(m, b)| (usize::from(m.number), b.name.as_str(), m.name.as_str()))
        .collect();
    for (m, before) in new.iter().zip(old) {
        let same = m.category == before.category
            && m.power == before.power
            && m.move_type == before.move_type
            && m.accuracy == before.accuracy
            && m.pp == before.pp;
        if same {
            continue;
        }
        let data = store.narc_file_mut(path, usize::from(m.number))?;
        put_byte(data, CATEGORY, category_byte(m.category))?;
        put_byte(data, POWER, u8::try_from(m.power).unwrap_or(u8::MAX))?;
        put_byte(data, TYPE, m.move_type.map_or(0, type_byte))?;
        // As the original: above 101 is 100.
        put_byte(data, ACCURACY, if m.accuracy > 101 { 100 } else { m.accuracy })?;
        put_byte(data, PP, m.pp)?;
    }
    if renamed.is_empty() {
        return Ok(());
    }
    let names = store.strings_mut(Text::Strings, names_text)?;
    for &(i, _, name) in &renamed {
        if let Some(s) = names.get_mut(i) {
            name.clone_into(s);
        }
    }
    let messages = store.strings_mut(Text::Strings, messages_text)?;
    for &(i, old_name, name) in &renamed {
        for j in 0..3 {
            if let Some(s) = messages.get_mut(i * 3 + j) {
                *s = s.replace(old_name, name);
            }
        }
    }
    Ok(())
}
