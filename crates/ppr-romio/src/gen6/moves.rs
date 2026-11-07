//! Moves, level-up moves and egg moves.
//!
//! X and Y keep each move in a file of its own; Omega Ruby and Alpha Sapphire pack them all into the first file
//! ("WD" archive). Only the type, category, power, accuracy and PP of a move are written, with the names.
//!
//! Deliberate choices:
//! - Move names are written to both name files only when a name changed, so changing a move's power leaves the
//!   text alone.
//! - Egg moves are written over the old ones, as many as the species had; the count is never changed.

use std::collections::BTreeMap;

use ppr_formats::ctr::mini;
use ppr_model::{
    CriticalChance, DexNumber, LevelUpMove, Move, MoveCategory, MoveId, StatChange, StatChangeMoveType, StatChangeType,
    StatusMoveType, StatusType,
};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom};
use crate::RomError;
use crate::gb::{DOUBLE_HIT, MULTIHIT};

/// The longest move name the games show.
pub(crate) const MAX_NAME_LENGTH: usize = 24;
const PACK_ID: [u8; 2] = *b"WD";

const STAT_CHANGE_TYPES: [StatChangeType; 9] = [
    StatChangeType::Attack,
    StatChangeType::Defense,
    StatChangeType::SpecialAttack,
    StatChangeType::SpecialDefense,
    StatChangeType::Speed,
    StatChangeType::Accuracy,
    StatChangeType::Evasion,
    StatChangeType::All,
    StatChangeType::Special,
];
const STATUS_TYPES: [StatusType; 6] = [
    StatusType::Paralyze,
    StatusType::Sleep,
    StatusType::Freeze,
    StatusType::Burn,
    StatusType::Poison,
    StatusType::Confusion,
];

/// The data of every move, 1 to the move count.
fn move_files(rom: &mut Rom, path: &str, family: Family) -> Result<Vec<Vec<u8>>, RomError> {
    let count = family.move_count();
    match family {
        Family::Xy => (0..=count).map(|i| rom.garc_file(path, false, i)).collect(),
        Family::Oras => {
            let packed = rom.garc_file(path, false, 0)?;
            let files = mini::unpack(&packed, PACK_ID)?
                .ok_or_else(|| RomError::Malformed("the move data is not a WD archive".into()))?;
            if files.len() <= count {
                return Err(RomError::Malformed("too few moves".into()));
            }
            Ok(files)
        }
    }
}

fn signed(b: u8) -> i8 {
    i8::from_le_bytes([b])
}

/// One move from its data.
fn read_move(number: MoveId, name: &str, d: &[u8]) -> Result<Move, RomError> {
    let effect = d.u16_at(16)?;
    let target = d.u8_at(20)?;
    let quality = signed(d.u8_at(1)?);
    let status_byte = d.u16_at(8)?;
    let flags = d.u32_at(32)?;
    let recoil_or_absorb = i16::from(signed(d.u8_at(18)?));
    let crit = d.u8_at(14)?;
    let mut mv = Move {
        number,
        name: name.to_owned(),
        effect,
        power: u16::from(d.u8_at(3)?),
        pp: d.u8_at(5)?,
        accuracy: d.u8_at(4)?,
        move_type: c::type_of(d.u8_at(0)?),
        category: match d.u8_at(2)? {
            1 => MoveCategory::Physical,
            2 => MoveCategory::Special,
            _ => MoveCategory::Status,
        },
        target,
        priority: signed(d.u8_at(6)?),
        hit_count_hundredths: if MULTIHIT.contains(&number) {
            317
        } else if DOUBLE_HIT.contains(&number) {
            200
        } else if number == c::TRIPLE_KICK {
            // Assuming the first hit lands.
            271
        } else {
            100
        },
        makes_contact: flags & 0x001 != 0,
        stat_change_move_type: match quality {
            c::Q_NO_DAMAGE_STAT_CHANGE | c::Q_NO_DAMAGE_STATUS_AND_STAT_CHANGE => match target {
                6 | 7 => StatChangeMoveType::NoDamageUser,
                2 => StatChangeMoveType::NoDamageAlly,
                8 => StatChangeMoveType::NoDamageAll,
                _ => StatChangeMoveType::NoDamageTarget,
            },
            c::Q_DAMAGE_TARGET_DEBUFF => StatChangeMoveType::DamageTarget,
            c::Q_DAMAGE_USER_BUFF => StatChangeMoveType::DamageUser,
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
        flinch_percent_chance: d.u8_at(15)?,
        recoil_percent: 0,
        absorb_percent: 0,
        is_charge: flags & 0x002 != 0,
        is_recharge: flags & 0x004 != 0,
        is_punch: flags & 0x080 != 0,
        is_sound: flags & 0x100 != 0,
        is_trap: status_byte == 8 || c::TRAPPING_EFFECTS.contains(&effect),
    };
    if quality == c::Q_DAMAGE_ABSORB {
        mv.absorb_percent = u8::try_from(recoil_or_absorb).unwrap_or(0);
    } else {
        mv.recoil_percent = u8::try_from(-recoil_or_absorb).unwrap_or(0);
    }
    for i in 0..3 {
        let kind = usize::from(d.u8_at(21 + i)?);
        if let Some(&stat) = kind.checked_sub(1).and_then(|k| STAT_CHANGE_TYPES.get(k)) {
            mv.stat_changes.push(StatChange {
                stat,
                stages: signed(d.u8_at(24 + i)?),
                percent_chance: d.u8_at(27 + i)?,
            });
        }
    }
    // Status numbers past Confusion (such as trapping) have no status of their own.
    if status_byte < 7 {
        mv.status = usize::from(status_byte)
            .checked_sub(1)
            .and_then(|s| STATUS_TYPES.get(s).copied())
            .map(|s| {
                if s == StatusType::Poison && (number == c::TOXIC || number == c::POISON_FANG) {
                    StatusType::ToxicPoison
                } else {
                    s
                }
            });
        mv.status_percent_chance = d.u8_at(10)?;
        mv.status_move_type = match quality {
            c::Q_NO_DAMAGE_STATUS | c::Q_NO_DAMAGE_STATUS_AND_STAT_CHANGE => StatusMoveType::NoDamage,
            c::Q_DAMAGE_STATUS => StatusMoveType::Damage,
            _ => StatusMoveType::NoneOrUnknown,
        };
    }
    Ok(mv)
}

/// Reads every move.
pub(crate) fn read(rom: &mut Rom, path: &str, family: Family, names: &[String]) -> Result<Vec<Move>, RomError> {
    let files = move_files(rom, path, family)?;
    (1..=family.move_count())
        .map(|i| {
            let name = names.get(i).map_or("", String::as_str);
            read_move(MoveId::try_from(i).unwrap_or_default(), name, &files[i])
        })
        .collect()
}

fn category_byte(c: MoveCategory) -> u8 {
    match c {
        MoveCategory::Status => 0,
        MoveCategory::Physical => 1,
        MoveCategory::Special => 2,
    }
}

/// Writes the type, category, power, accuracy and PP of every move that changed; `names_file` gets the names when
/// one changed.
pub(crate) fn write(
    rom: &mut Rom,
    path: &str,
    text: &str,
    names_file: usize,
    family: Family,
    old: &[Move],
    new: &[Move],
) -> Result<(), RomError> {
    let mut files = move_files(rom, path, family)?;
    let mut changed = Vec::new();
    for (o, m) in old.iter().zip(new) {
        let Some(d) = files.get_mut(usize::from(m.number)) else {
            continue;
        };
        if (o.category, o.power, o.move_type, o.accuracy, o.pp) == (m.category, m.power, m.move_type, m.accuracy, m.pp)
        {
            continue;
        }
        d.set_u8(2, category_byte(m.category))?;
        d.set_u8(3, m.power.to_le_bytes()[0])?;
        d.set_u8(0, m.move_type.map_or(0, c::type_byte))?;
        d.set_u8(4, if m.accuracy > 101 { 100 } else { m.accuracy })?;
        d.set_u8(5, m.pp)?;
        changed.push(usize::from(m.number));
    }
    match family {
        Family::Xy => {
            for i in changed {
                rom.set_garc_file(path, false, i, files[i].clone())?;
            }
        }
        Family::Oras if !changed.is_empty() => {
            rom.set_garc_file(path, false, 0, mini::pack(&files, PACK_ID)?)?;
        }
        Family::Oras => {}
    }
    if old.iter().zip(new).any(|(o, m)| o.name != m.name) {
        let mut names = rom.texts(text, names_file)?;
        for m in new {
            if let Some(slot) = names.get_mut(usize::from(m.number)) {
                slot.clone_from(&m.name);
            }
        }
        rom.set_texts(text, names_file, &names)?;
        rom.set_texts(text, names_file + 1, &names)?;
    }
    Ok(())
}

/// The level-up moves of the 721 species: (move, level) pairs up to a pair of 0xFFFF.
pub(crate) fn read_learnsets(
    rom: &mut Rom,
    path: &str,
    family: Family,
) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let mut out = BTreeMap::new();
    for i in 1..=super::species::entry_count(family) {
        let d = rom.garc_file(path, false, i)?;
        let mut moves = Vec::new();
        let mut at = 0;
        while d.u16_at(at)? != 0xFFFF || d.u16_at(at + 2)? != 0xFFFF {
            moves.push(LevelUpMove {
                move_id: d.u16_at(at)?,
                level: u8::try_from(d.u16_at(at + 2)?).unwrap_or(u8::MAX),
            });
            at += 4;
        }
        out.insert(DexNumber::try_from(i).unwrap_or_default(), moves);
    }
    Ok(out)
}

/// Writes the level-up moves of the species whose moves changed.
pub(crate) fn write_learnsets(
    rom: &mut Rom,
    path: &str,
    family: Family,
    old: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
    new: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
) -> Result<(), RomError> {
    for (&n, moves) in new {
        if usize::from(n) > super::species::entry_count(family) || old.get(&n) == Some(moves) {
            continue;
        }
        let mut d = Vec::with_capacity(moves.len() * 4 + 4);
        for m in moves {
            d.extend_from_slice(&m.move_id.to_le_bytes());
            d.extend_from_slice(&u16::from(m.level).to_le_bytes());
        }
        d.extend_from_slice(&[0xFF; 4]);
        rom.set_garc_file(path, false, usize::from(n), d)?;
    }
    Ok(())
}

/// The egg moves of the 721 species: a count, then the moves.
pub(crate) fn read_egg_moves(rom: &mut Rom, path: &str) -> Result<BTreeMap<DexNumber, Vec<MoveId>>, RomError> {
    let mut out = BTreeMap::new();
    for i in 1..=c::SPECIES_COUNT {
        let d = rom.garc_file(path, false, i)?;
        let count = usize::from(d.u16_at(0)?);
        let moves = (0..count).map(|j| d.u16_at(2 + j * 2)).collect::<Result<Vec<_>, _>>()?;
        out.insert(DexNumber::try_from(i).unwrap_or_default(), moves);
    }
    Ok(out)
}

/// Writes the egg moves over the old ones.
pub(crate) fn write_egg_moves(
    rom: &mut Rom,
    path: &str,
    old: &BTreeMap<DexNumber, Vec<MoveId>>,
    new: &BTreeMap<DexNumber, Vec<MoveId>>,
) -> Result<(), RomError> {
    for (&n, moves) in new {
        if usize::from(n) > c::SPECIES_COUNT || old.get(&n) == Some(moves) {
            continue;
        }
        let mut d = rom.garc_file(path, false, usize::from(n))?;
        let capacity = usize::from(d.u16_at(0)?);
        for (j, &m) in moves.iter().take(capacity).enumerate() {
            d.set_u16(2 + j * 2, m)?;
        }
        rom.set_garc_file(path, false, usize::from(n), d)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_move_reads_its_fields() {
        let mut d = vec![0u8; 0x24];
        d[0] = 9; // Fire
        d[1] = 4; // damage and status
        d[2] = 2;
        d[3] = 90;
        d[4] = 100;
        d[5] = 15;
        d[8] = 4; // burn
        d[10] = 10;
        d[14] = 1;
        d[16] = 4;
        d[18] = 0xF0; // 16% recoil
        d[32] = 0x01;
        let mv = read_move(53, "Flamethrower", &d).expect("reads");
        assert_eq!(mv.move_type, Some(ppr_model::Type::Fire));
        assert_eq!(mv.category, MoveCategory::Special);
        assert_eq!((mv.status, mv.status_percent_chance), (Some(StatusType::Burn), 10));
        assert_eq!(mv.status_move_type, StatusMoveType::Damage);
        assert_eq!(mv.critical_chance, CriticalChance::Increased);
        assert_eq!(mv.recoil_percent, 16);
        assert!(mv.makes_contact && !mv.is_trap);
    }
}
