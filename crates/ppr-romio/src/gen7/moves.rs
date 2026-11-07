//! Moves (`MoveData`, file 0: a `WD` pack of one entry per move), level-up learnsets (`PokemonMovesets`) and egg
//! moves (`EggMoves`).

use std::collections::BTreeMap;

use ppr_formats::ctr::{Decompression, mini};
use ppr_model::{
    CriticalChance, DexNumber, LevelUpMove, Move, MoveCategory, MoveId, StatChange, StatChangeMoveType, StatChangeType,
    StatusMoveType, StatusType,
};

use super::constants::{self as c, Family};
use super::files::Files;
use super::personal::{Personal, type_byte, type_of};
use super::{put_u16, word};
use crate::RomError;

const WD: [u8; 2] = *b"WD";

fn i8_at(d: &[u8], at: usize) -> i8 {
    i8::from_le_bytes([d.get(at).copied().unwrap_or_default()])
}

fn stat_change_type(b: u8) -> Option<StatChangeType> {
    use StatChangeType as S;
    Some(match b {
        1 => S::Attack,
        2 => S::Defense,
        3 => S::SpecialAttack,
        4 => S::SpecialDefense,
        5 => S::Speed,
        6 => S::Accuracy,
        7 => S::Evasion,
        8 => S::All,
        9 => S::Special,
        _ => return None,
    })
}

fn status_type(v: u16) -> Option<StatusType> {
    use StatusType as S;
    Some(match v {
        1 => S::Paralyze,
        2 => S::Sleep,
        3 => S::Freeze,
        4 => S::Burn,
        5 => S::Poison,
        6 => S::Confusion,
        _ => return None,
    })
}

fn category_of(b: u8) -> MoveCategory {
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

/// One move from its entry.
fn parse(number: MoveId, name: &str, d: &[u8]) -> Result<Move, RomError> {
    if d.len() < 40 {
        return Err(RomError::Malformed(format!("move {number} is too short")));
    }
    let effect = word(d, 16)?;
    let status = word(d, 8)?;
    let flags = u32::from_le_bytes([d[36], d[37], d[38], d[39]]);
    let quality = d[1];
    let target = d[20];
    let recoil_or_absorb = i8_at(d, 18);
    let (absorb, recoil) = if quality == c::DAMAGE_ABSORB_QUALITY {
        (u8::try_from(recoil_or_absorb).unwrap_or_default(), 0)
    } else {
        (0, u8::try_from(-i16::from(recoil_or_absorb)).unwrap_or_default())
    };
    let crit = d[14];
    let hit_count_hundredths = if c::NORMAL_MULTIHIT_MOVES.contains(&number) {
        // 19/6 hits on average.
        317
    } else if c::DOUBLE_HIT_MOVES.contains(&number) {
        200
    } else if number == c::TRIPLE_KICK {
        // Assuming the first hit lands.
        271
    } else {
        100
    };
    let stat_change_move_type = match quality {
        c::NO_DAMAGE_STAT_CHANGE_QUALITY | c::NO_DAMAGE_STATUS_AND_STAT_CHANGE_QUALITY => match target {
            6 | 7 => StatChangeMoveType::NoDamageUser,
            2 => StatChangeMoveType::NoDamageAlly,
            8 => StatChangeMoveType::NoDamageAll,
            _ => StatChangeMoveType::NoDamageTarget,
        },
        c::DAMAGE_TARGET_DEBUFF_QUALITY => StatChangeMoveType::DamageTarget,
        c::DAMAGE_USER_BUFF_QUALITY => StatChangeMoveType::DamageUser,
        _ => StatChangeMoveType::NoneOrUnknown,
    };
    let stat_changes = (0..3)
        .filter_map(|i| {
            Some(StatChange {
                stat: stat_change_type(d[21 + i])?,
                stages: i8_at(d, 24 + i),
                percent_chance: d[27 + i],
            })
        })
        .collect();
    // Statuses from 7 on are not modelled; below that, the chance and how the move inflicts it are read even
    // without a status.
    let (status_kind, status_percent_chance, status_move_type) = if status < 7 {
        let mut kind = status_type(status);
        if kind == Some(StatusType::Poison) && (number == c::TOXIC || number == c::POISON_FANG) {
            kind = Some(StatusType::ToxicPoison);
        }
        let how = match quality {
            c::NO_DAMAGE_STATUS_QUALITY | c::NO_DAMAGE_STATUS_AND_STAT_CHANGE_QUALITY => StatusMoveType::NoDamage,
            c::DAMAGE_STATUS_QUALITY => StatusMoveType::Damage,
            _ => StatusMoveType::NoneOrUnknown,
        };
        (kind, d[10], how)
    } else {
        (None, 0, StatusMoveType::NoneOrUnknown)
    };
    Ok(Move {
        number,
        name: name.to_owned(),
        effect,
        power: u16::from(d[3]),
        pp: d[5],
        accuracy: d[4],
        move_type: type_of(d[0]),
        category: category_of(d[2]),
        target,
        priority: i8_at(d, 6),
        hit_count_hundredths,
        makes_contact: flags & 0x001 != 0,
        stat_change_move_type,
        stat_changes,
        status_move_type,
        status: status_kind,
        status_percent_chance,
        critical_chance: match crit {
            0 => CriticalChance::Normal,
            6 => CriticalChance::Guaranteed,
            _ => CriticalChance::Increased,
        },
        flinch_percent_chance: d[15],
        recoil_percent: recoil,
        absorb_percent: absorb,
        is_charge: flags & 0x002 != 0,
        is_recharge: flags & 0x004 != 0,
        is_punch: flags & 0x080 != 0,
        is_sound: flags & 0x100 != 0,
        is_trap: status == 8 || c::TRAPPING_EFFECTS.contains(&effect),
    })
}

fn entries(files: &mut Files, path: &str) -> Result<Vec<Vec<u8>>, RomError> {
    let pack = files.garc(path, &Decompression::None)?.file(0)?;
    mini::unpack(&pack, WD)?.ok_or_else(|| RomError::Malformed("move data is no WD pack".into()))
}

/// The moves, from 1.
pub(super) fn read(files: &mut Files, path: &str, family: Family, names: &[String]) -> Result<Vec<Move>, RomError> {
    let data = entries(files, path)?;
    (1..=family.move_count())
        .map(|i| {
            let d = data
                .get(i)
                .ok_or_else(|| RomError::Malformed(format!("no data for move {i}")))?;
            parse(
                MoveId::try_from(i).unwrap_or_default(),
                names.get(i).map_or("", String::as_str),
                d,
            )
        })
        .collect()
}

/// Writes the category, power, type, accuracy and PP of the moves that changed.
pub(super) fn write(files: &mut Files, path: &str, loaded: &[Move], new: &[Move]) -> Result<(), RomError> {
    let mut data = entries(files, path)?;
    for (old, m) in loaded.iter().zip(new) {
        if old == m {
            continue;
        }
        let Some(d) = data.get_mut(usize::from(m.number)) else {
            continue;
        };
        d[2] = category_byte(m.category);
        d[3] = u8::try_from(m.power).unwrap_or(u8::MAX);
        d[0] = m.move_type.map_or(d[0], type_byte);
        // As the original: up to 101 is kept, more becomes 100.
        d[4] = if m.accuracy > 101 { 100 } else { m.accuracy };
        d[5] = m.pp;
    }
    let pack = mini::pack(&data, WD)?;
    files.garc_mut(path, &Decompression::None)?.set_file(0, pack)?;
    Ok(())
}

/// The level-up moves of each species.
pub(super) fn read_learnsets(
    files: &mut Files,
    path: &str,
    family: Family,
) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let garc = files.garc(path, &Decompression::None)?;
    let mut out = BTreeMap::new();
    for n in 1..=family.pokemon_count() + family.forme_count() {
        let d = garc.file(n)?;
        let mut moves = Vec::new();
        let mut at = 0;
        while at + 4 <= d.len() {
            let (mv, level) = (word(&d, at)?, word(&d, at + 2)?);
            if mv == 0xFFFF && level == 0xFFFF {
                break;
            }
            moves.push(LevelUpMove::new(u8::try_from(level).unwrap_or(u8::MAX), mv));
            at += 4;
        }
        out.insert(DexNumber::try_from(n).unwrap_or_default(), moves);
    }
    Ok(out)
}

/// Writes the learnsets that changed.
pub(super) fn write_learnsets(
    files: &mut Files,
    path: &str,
    loaded: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
    new: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
) -> Result<(), RomError> {
    let changed: Vec<(&DexNumber, &Vec<LevelUpMove>)> = new.iter().filter(|(n, l)| loaded.get(n) != Some(l)).collect();
    if changed.is_empty() {
        return Ok(());
    }
    let garc = files.garc_mut(path, &Decompression::None)?;
    for (&n, learnset) in changed {
        let mut d = vec![0u8; learnset.len() * 4 + 4];
        for (i, m) in learnset.iter().enumerate() {
            put_u16(&mut d, i * 4, m.move_id);
            put_u16(&mut d, i * 4 + 2, u16::from(m.level));
        }
        let end = learnset.len() * 4;
        put_u16(&mut d, end, 0xFFFF);
        put_u16(&mut d, end + 2, 0xFFFF);
        garc.set_file(usize::from(n), d)?;
    }
    Ok(())
}

/// The egg move file of each species and alternate forme that has one. A species' file starts with a forme
/// reference: its own number, or the file of its first alternate forme, whose other formes follow.
fn egg_move_files(garc: &ppr_formats::ctr::Garc, personal: &Personal) -> Result<Vec<(DexNumber, usize)>, RomError> {
    let mut out = Vec::new();
    let mut formes = Vec::new();
    for n in 1..=personal.family.pokemon_count() {
        let number = DexNumber::try_from(n).unwrap_or_default();
        out.push((number, n));
        let reference = usize::from(word(&garc.file(n)?, 0)?);
        if reference != n {
            for (&i, f) in personal.formes.iter().filter(|(_, f)| f.species == number) {
                formes.push((
                    DexNumber::try_from(i).unwrap_or_default(),
                    reference + usize::from(f.forme) - 1,
                ));
            }
        }
    }
    out.extend(formes);
    Ok(out)
}

/// The egg moves of each species, and of the alternate formes with files of their own: a forme reference (u16),
/// the count (u16) and the moves.
pub(super) fn read_egg_moves(
    files: &mut Files,
    path: &str,
    personal: &Personal,
) -> Result<BTreeMap<DexNumber, Vec<MoveId>>, RomError> {
    let garc = files.garc(path, &Decompression::None)?;
    let mut out = BTreeMap::new();
    for (n, file) in egg_move_files(garc, personal)? {
        let d = garc.file(file)?;
        let count = usize::from(word(&d, 2)?);
        let moves = (0..count).map(|j| word(&d, 4 + j * 2)).collect::<Result<Vec<_>, _>>()?;
        out.insert(n, moves);
    }
    Ok(out)
}

/// Writes the egg moves that changed over the old ones; a species keeps its number of egg moves, so moves past it
/// are dropped and a shorter list leaves the old moves after it.
pub(super) fn write_egg_moves(
    files: &mut Files,
    path: &str,
    personal: &Personal,
    loaded: &BTreeMap<DexNumber, Vec<MoveId>>,
    new: &BTreeMap<DexNumber, Vec<MoveId>>,
) -> Result<(), RomError> {
    if new == loaded {
        return Ok(());
    }
    let file_of: BTreeMap<DexNumber, usize> = egg_move_files(files.garc(path, &Decompression::None)?, personal)?
        .into_iter()
        .collect();
    let garc = files.garc_mut(path, &Decompression::None)?;
    for (n, moves) in new.iter().filter(|(n, l)| loaded.get(n) != Some(l)) {
        let Some(&file) = file_of.get(n) else { continue };
        let mut d = garc.file(file)?;
        let count = usize::from(word(&d, 2)?);
        for (j, &m) in moves.iter().enumerate().take(count) {
            put_u16(&mut d, 4 + j * 2, m);
        }
        garc.set_file(file, d)?;
    }
    Ok(())
}
