//! Trainers: their data (Pokémon flags, class, battle style, count), their Pokémon, names and classes, tags and
//! multi battles.
//!
//! A trainer's Pokémon are 8 bytes each (strength, ability and gender, level, species, forme), followed by a held
//! item when the trainer's flags have bit 1 and by four moves when they have bit 0.
//!
//! Deliberate choices:
//! - Only trainers that changed are written, so an unchanged trainer keeps flags the game set without using them.
//! - A Pokémon's species is its alternate forme's own number (from 722) for a forme with data of its own; a forme
//!   the species doesn't have is read as forme 0. A Pokémon keeps the forme word as stored while its forme stays,
//!   and its strength and gender flags when its species and IVs did not change. A new strength
//!   is the lowest one that gives the IVs: the IVs themselves in X and Y (the top three bits kept), IVs × 255 / 31
//!   rounded up in Omega Ruby and Alpha Sapphire.
//! - Names and classes are written to every language's text when one of them changed.

use std::collections::BTreeMap;

use ppr_model::{BattleStyle, DexNumber, LevelUpMove, MultiBattleStatus, Trainer, TrainerPokemon};

use super::constants::Family;
use super::rom::{Bytes, BytesMut, Rom};
use super::species::FormeTable;
use crate::RomError;

/// What the game data does not hold about a trainer's Pokémon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PokemonExtra {
    species: DexNumber,
    /// The forme in the game data, and the forme word as stored.
    forme: u8,
    stored_forme: u16,
    strength: u8,
    gender: u8,
    ivs: u8,
}

/// Per trainer (from 1), its Pokémon's extras.
pub(crate) type Extras = Vec<Vec<PokemonExtra>>;

/// The files the trainers are in.
pub(crate) struct Files<'a> {
    pub(crate) data: &'a str,
    pub(crate) pokemon: &'a str,
    pub(crate) family: Family,
    pub(crate) formes: &'a FormeTable,
}

fn ivs_of(strength: u8, family: Family) -> u8 {
    match family {
        Family::Xy => strength & 0x1F,
        Family::Oras => u8::try_from(u16::from(strength) * 31 / 255).unwrap_or(31),
    }
}

fn strength_for(ivs: u8, old: u8, family: Family) -> u8 {
    match family {
        Family::Xy => (old & 0xE0) | (ivs & 0x1F),
        Family::Oras => u8::try_from((u16::from(ivs.min(31)) * 255).div_ceil(31)).unwrap_or(u8::MAX),
    }
}

/// Reads every trainer; `names` are the trainer name texts without the first, empty one.
pub(crate) fn read(rom: &mut Rom, files: &Files<'_>, names: &[String]) -> Result<(Vec<Trainer>, Extras), RomError> {
    let oras = files.family == Family::Oras;
    let count = rom.garc(files.data, false)?.len();
    let mut trainers = Vec::new();
    let mut extras = vec![Vec::new()];
    for i in 1..count {
        let t = rom.garc_file(files.data, false, i)?;
        let p = rom.garc_file(files.pokemon, false, i)?;
        let poke_type = if oras { t.u16_at(0)? } else { u16::from(t.u8_at(0)?) };
        let class = if oras { t.u16_at(2)? } else { u16::from(t.u8_at(1)?) };
        let at = if oras { 6 } else { 2 };
        let battle_style = match t.u8_at(at)? {
            1 => BattleStyle::Double,
            2 => BattleStyle::Triple,
            3 => BattleStyle::Rotation,
            _ => BattleStyle::Single,
        };
        let count = usize::from(t.u8_at(at + 1)?);
        let mut pokemon = Vec::new();
        let mut extra = Vec::new();
        let mut o = 0;
        for _ in 0..count {
            let strength = p.u8_at(o)?;
            let ability_and_gender = p.u8_at(o + 1)?;
            let base = p.u16_at(o + 4)?;
            let stored_forme = p.u16_at(o + 6)?;
            let forme = files.formes.holder_forme(base, u32::from(stored_forme));
            let species = files.formes.species(base, forme);
            let ivs = ivs_of(strength, files.family);
            let mut tp = TrainerPokemon {
                forme,
                species,
                level: u8::try_from(p.u16_at(o + 2)?).unwrap_or(u8::MAX),
                ivs,
                held_item: None,
                moves: [0; 4],
                reset_moves: false,
                ability_slot: (ability_and_gender >> 4) & 0xF,
                added: false,
                shiny: false,
                added_from_mega_stone: false,
            };
            extra.push(PokemonExtra {
                species,
                forme,
                stored_forme,
                strength,
                gender: ability_and_gender & 0xF,
                ivs,
            });
            o += 8;
            if poke_type & 2 != 0 {
                tp.held_item = Some(p.u16_at(o)?).filter(|&id| id != 0);
                o += 2;
            }
            if poke_type & 1 != 0 {
                for m in 0..4 {
                    tp.moves[m] = p.u16_at(o + m * 2)?;
                }
                o += 8;
            }
            pokemon.push(tp);
        }
        trainers.push(Trainer {
            index: u16::try_from(i).unwrap_or(u16::MAX),
            name: names.get(i - 1).cloned().unwrap_or_else(|| "UNKNOWN".to_owned()),
            class,
            tag: None,
            multi_battle: MultiBattleStatus::Never,
            battle_style,
            forced_battle_style: false,
            force_starter_position: None,
            pokemon,
        });
        extras.push(extra);
    }
    match files.family {
        Family::Xy => tag_xy(&mut trainers),
        Family::Oras => tag_oras(&mut trainers),
    }
    Ok((trainers, extras))
}

fn tag(trainers: &mut [Trainer], tag: &str, numbers: &[usize]) {
    for &n in numbers {
        if let Some(t) = n.checked_sub(1).and_then(|i| trainers.get_mut(i)) {
            t.tag = Some(tag.to_owned());
        }
    }
}

/// Tags the trainers at list positions `offset - 1`, `offset` and `offset + 1` with `tag` and the given suffixes.
fn tag_rival(trainers: &mut [Trainer], tag: &str, offset: usize, suffixes: [u8; 3]) {
    for (i, suffix) in suffixes.into_iter().enumerate() {
        if let Some(t) = (offset + i).checked_sub(1).and_then(|at| trainers.get_mut(at)) {
            t.tag = Some(format!("{tag}-{suffix}"));
        }
    }
}

fn multi(trainers: &mut [Trainer], numbers: &[usize]) {
    for &n in numbers {
        if let Some(t) = n.checked_sub(1).and_then(|i| trainers.get_mut(i)) {
            t.multi_battle = MultiBattleStatus::Always;
        }
    }
}

fn tag_xy(trs: &mut [Trainer]) {
    tag(trs, "GYM1", &[39, 40, 48]);
    tag(trs, "GYM2", &[64, 63, 106, 105]);
    tag(trs, "GYM3", &[83, 84, 146, 147]);
    tag(trs, "GYM4", &[121, 122, 123, 124]);
    tag(trs, "GYM5", &[461, 462, 463, 464, 465, 466, 467, 468, 469, 28, 29, 30]);
    tag(trs, "GYM6", &[245, 250, 248, 243]);
    tag(trs, "GYM7", &[170, 171, 172, 365, 366]);
    tag(trs, "GYM8", &[168, 169, 31, 32]);
    for (g, n) in [6, 76, 21, 22, 23, 24, 25, 26].into_iter().enumerate() {
        tag(trs, &format!("GYM{}-LEADER", g + 1), &[n]);
    }
    tag(trs, "NOTSTRONG", &[188]);
    tag(trs, "ELITE1", &[269]);
    tag(trs, "ELITE2", &[271]);
    tag(trs, "ELITE3", &[187]);
    tag(trs, "ELITE4", &[270]);
    tag(trs, "CHAMPION", &[276]);
    tag(trs, "THEMED:LYSANDRE-LEADER", &[303, 525, 526]);
    tag(
        trs,
        "STRONG",
        &[
            174, 175, 304, 344, 345, 346, 347, 348, 349, 350, 351, 470, 471, 472, 473, 474, 475, 476, 477, 478, 479,
        ],
    );
    tag(trs, "STRONG", &[324, 325, 438, 439, 573]);
    tag(trs, "STRONG", &[327, 328]);
    let rival = [0, 1, 2];
    for (n, o) in [596, 575, 581, 578, 584, 607, 587, 590, 593, 599]
        .into_iter()
        .enumerate()
    {
        tag_rival(trs, &format!("RIVAL{}", n + 2), o, rival);
    }
    for (n, o) in [435, 130, 329, 184, 332, 604, 335, 338, 341, 519]
        .into_iter()
        .enumerate()
    {
        tag_rival(trs, &format!("RIVAL{}", n + 2), o, rival);
    }
    tag_rival(trs, "FRIEND1", 137, rival);
    tag_rival(trs, "FRIEND2", 321, rival);
    multi(trs, &[108, 111, 348, 350, 438, 439, 470, 472, 476, 610, 611, 612]);
}

fn tag_oras(trs: &mut [Trainer]) {
    tag(trs, "GYM1", &[562, 22, 667]);
    tag(trs, "GYM2", &[60, 56, 59]);
    tag(trs, "GYM3", &[34, 568, 614, 35]);
    tag(trs, "GYM4", &[81, 824, 83, 615, 823, 613, 85]);
    tag(trs, "GYM5", &[63, 64, 65, 66, 67, 68, 69]);
    tag(trs, "GYM6", &[115, 517, 516, 118, 730]);
    tag(trs, "GYM7", &[157, 158, 159, 226, 320, 225]);
    tag(trs, "GYM8", &[647, 342, 594, 646, 338, 339, 340, 341]);
    tag(trs, "GYM1-LEADER", &[561]);
    tag(trs, "GYM2-LEADER", &[563]);
    tag(trs, "GYM3-LEADER", &[567]);
    tag(trs, "GYM4-LEADER", &[569]);
    tag(trs, "GYM5-LEADER", &[570]);
    tag(trs, "GYM6-LEADER", &[571]);
    tag(trs, "GYM7-LEADER", &[552]);
    tag(trs, "GYM8-LEADER", &[572, 943]);
    tag(trs, "ELITE1", &[553, 909]);
    tag(trs, "ELITE2", &[554, 910]);
    tag(trs, "ELITE3", &[555, 911]);
    tag(trs, "ELITE4", &[556, 912]);
    tag(trs, "CHAMPION", &[557, 913, 680, 942]);
    tag(trs, "THEMED:MAXIE-LEADER", &[235, 236, 271]);
    tag(trs, "THEMED:ARCHIE-LEADER", &[178, 231, 266]);
    tag(trs, "THEMED:MATT-STRONG", &[683, 684, 685, 686, 687]);
    tag(trs, "THEMED:SHELLY-STRONG", &[688, 689, 690]);
    tag(trs, "THEMED:TABITHA-STRONG", &[691, 692, 693]);
    tag(trs, "THEMED:COURTNEY-STRONG", &[694, 695, 696, 697, 698]);
    tag(trs, "THEMED:WALLY-STRONG", &[518, 583, 944, 946]);
    let rival = [2, 0, 1];
    for (n, o) in [1, 289, 674, 292, 527, 699].into_iter().enumerate() {
        tag_rival(trs, &format!("RIVAL{}", n + 1), o, rival);
    }
    for (n, o) in [4, 295, 677, 298, 530, 906].into_iter().enumerate() {
        tag_rival(trs, &format!("RIVAL{}", n + 1), o, rival);
    }
    multi(trs, &[683, 687, 688, 691, 694, 698, 900, 901, 902, 903, 904, 905]);
}

/// The last four moves a species learns by `level`, as the game picks a wild Pokémon's moves.
pub(crate) fn moves_at_level(learnset: &[LevelUpMove], level: u8) -> [u16; 4] {
    let mut moves = [0u16; 4];
    let mut count = 0;
    for m in learnset {
        if m.level > level {
            break;
        }
        if moves[..count].contains(&m.move_id) {
            continue;
        }
        if count == 4 {
            moves.rotate_left(1);
            moves[3] = m.move_id;
        } else {
            moves[count] = m.move_id;
            count += 1;
        }
    }
    moves
}

/// Writes the trainers that changed.
pub(crate) fn write(
    rom: &mut Rom,
    files: &Files<'_>,
    extras: &Extras,
    old: &[Trainer],
    new: &[Trainer],
    learnsets: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
) -> Result<(), RomError> {
    let oras = files.family == Family::Oras;
    for (t, o) in new.iter().zip(old) {
        if t == o {
            continue;
        }
        let i = usize::from(t.index);
        let extra = extras.get(i).map_or(&[][..], Vec::as_slice);
        let mut data = rom.garc_file(files.data, false, i)?;
        let custom_moves = t
            .pokemon
            .iter()
            .any(|p| !p.reset_moves && p.moves.iter().any(|&m| m != 0));
        let items = t.pokemon.iter().any(|p| p.held_item.is_some());
        let poke_type = u8::from(custom_moves) | (u8::from(items) << 1);
        let base = if oras {
            data.set_u16(0, u16::from(poke_type))?;
            4
        } else {
            data.set_u8(0, poke_type)?;
            0
        };
        data.set_u8(base + 3, u8::try_from(t.pokemon.len()).unwrap_or(u8::MAX))?;
        if t.forced_battle_style {
            let style = data.u8_at(base + 2)?;
            let ai = data.u8_at(base + 12)?;
            let (new_style, new_ai) = match t.battle_style {
                BattleStyle::Single => (0, ai & 0x7F),
                // Doubles and triples need the flag that keeps trainers from attacking their own Pokémon.
                BattleStyle::Double => (1, ai | 0x80),
                BattleStyle::Triple => (2, ai | 0x80),
                BattleStyle::Rotation => (3, ai | 0x7F),
            };
            if style != new_style {
                data.set_u8(base + 2, new_style)?;
                data.set_u8(base + 12, new_ai)?;
            }
        }
        let mut pokes = Vec::new();
        for (k, p) in t.pokemon.iter().enumerate() {
            let stored_forme = extra
                .get(k)
                .filter(|e| e.species == p.species && e.forme == p.forme)
                .map_or(u16::from(p.forme), |e| e.stored_forme);
            let strength = match extra.get(k) {
                Some(e) if e.ivs == p.ivs => e.strength,
                Some(e) => strength_for(p.ivs, e.strength, files.family),
                None => strength_for(p.ivs, 0, files.family),
            };
            let gender = extra.get(k).map_or(0, |e| e.gender);
            pokes.push(strength);
            pokes.push((p.ability_slot << 4) | (gender & 0xF));
            pokes.extend_from_slice(&u16::from(p.level).to_le_bytes());
            pokes.extend_from_slice(&files.formes.base(p.species).to_le_bytes());
            pokes.extend_from_slice(&stored_forme.to_le_bytes());
            if items {
                pokes.extend_from_slice(&p.held_item.unwrap_or(0).to_le_bytes());
            }
            if custom_moves {
                let moves = if p.reset_moves {
                    moves_at_level(learnsets.get(&p.species).map_or(&[][..], Vec::as_slice), p.level)
                } else {
                    p.moves
                };
                for m in moves {
                    pokes.extend_from_slice(&m.to_le_bytes());
                }
            }
        }
        rom.set_garc_file(files.data, false, i, data)?;
        rom.set_garc_file(files.pokemon, false, i, pokes)?;
    }
    Ok(())
}

/// The text GARCs of the other languages, which get the trainer names and classes too.
pub(crate) const OTHER_LANGUAGES: [&str; 7] = ["JaKana", "JaKanji", "Fr", "It", "De", "Es", "Ko"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_and_ivs() {
        assert_eq!(ivs_of(255, Family::Oras), 31);
        assert_eq!(ivs_of(0xE5, Family::Xy), 5);
        for ivs in 0..=31 {
            assert_eq!(ivs_of(strength_for(ivs, 0xE0, Family::Oras), Family::Oras), ivs);
            assert_eq!(ivs_of(strength_for(ivs, 0xE0, Family::Xy), Family::Xy), ivs);
        }
        assert_eq!(strength_for(3, 0xE0, Family::Xy), 0xE3);
    }

    #[test]
    fn moves_at_level_keep_the_last_four() {
        let set: Vec<LevelUpMove> = [(1, 10), (1, 11), (5, 12), (7, 13), (9, 14), (9, 10), (20, 15)]
            .into_iter()
            .map(|(level, move_id)| LevelUpMove { level, move_id })
            .collect();
        // Move 10 is forgotten by level 9 and learnt again.
        assert_eq!(moves_at_level(&set, 9), [12, 13, 14, 10]);
        assert_eq!(moves_at_level(&set, 7), [10, 11, 12, 13]);
        assert_eq!(moves_at_level(&set, 1), [10, 11, 0, 0]);
    }
}
