//! Trainers (`TrainerData`, `TrainerPokemon`): a file per trainer from 1, its class (byte 0), battle type (2: 0
//! single, 1 double), team size (3), AI flags (12) and team layout (13); and its team, 32 bytes per Pokémon:
//! ability slot and gender (0), nature (1), EVs (2–7), IVs (8, u32: 5 bits per stat, bit 30 shiny), level (14),
//! species (16), forme (18), item (20) and four moves (24).
//!
//! A Pokémon's species is its alternate forme's own entry for a forme with an entry; a forme the species doesn't
//! have is read as forme 0. It keeps the forme as stored while its species and forme stay.

use ppr_formats::ctr::Decompression;
use ppr_model::{BattleStyle, DexNumber, GameData, MultiBattleStatus, Trainer, TrainerPokemon};

use super::constants::{self as c, Family};
use super::files::Files;
use super::personal::Personal;
use super::{put_u16, put_u32, word};
use crate::RomError;

const MEMBER: usize = 32;
/// The bit of a trainer Pokémon's IVs that makes it shiny.
const SHINY: u32 = 1 << 30;

/// What a trainer Pokémon has that the game data leaves out: the bytes around its level, species, item and moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Extra {
    raw: [u8; MEMBER],
    /// The species and forme in the game data.
    species: DexNumber,
    forme: u8,
}

impl Extra {
    fn ivs(&self) -> u32 {
        u32::from_le_bytes([self.raw[8], self.raw[9], self.raw[10], self.raw[11]])
    }
}

/// The trainers' data the game data leaves out, by trainer.
#[derive(Debug, Clone)]
pub(super) struct Teams {
    extras: Vec<Vec<Extra>>,
}

/// The trainer tags (tag, trainers) and the rival tags (tag, first trainer) of a family.
type Tags = (
    &'static [(&'static str, &'static [usize])],
    &'static [(&'static str, usize)],
);

fn tags(family: Family) -> Tags {
    match family {
        Family::SunMoon => (c::TRAINER_TAGS_SM, c::RIVAL_TAGS_SM),
        Family::UltraSunMoon => (c::TRAINER_TAGS_USUM, c::RIVAL_TAGS_USUM),
    }
}

/// Reads the trainers. `names` are the trainer names after the empty first line.
pub(super) fn read(
    files: &mut Files,
    paths: [&str; 2],
    family: Family,
    names: &[String],
    personal: &Personal,
) -> Result<(Teams, Vec<Trainer>), RomError> {
    let [data_path, team_path] = paths;
    let count = files.garc(data_path, &Decompression::None)?.len();
    let mut trainers = Vec::new();
    let mut extras = Vec::new();
    for i in 1..count {
        let t = files.garc(data_path, &Decompression::None)?.file(i)?;
        let team = files.garc(team_path, &Decompression::None)?.file(i)?;
        if t.len() < 16 {
            return Err(RomError::Malformed(format!("trainer {i} is too short")));
        }
        let mut trainer = Trainer::new(
            u16::try_from(i).unwrap_or_default(),
            names.get(i - 1).map_or("UNKNOWN", String::as_str),
            t[0],
            Vec::new(),
        );
        if t[2] == 1 {
            trainer.battle_style = BattleStyle::Double;
        }
        let mut team_extras = Vec::new();
        for p in 0..usize::from(t[3]) {
            let raw: [u8; MEMBER] = team
                .get(p * MEMBER..(p + 1) * MEMBER)
                .and_then(|r| r.try_into().ok())
                .ok_or_else(|| RomError::Malformed(format!("the team of trainer {i} is cut off")))?;
            let base = u16::from_le_bytes([raw[16], raw[17]]);
            let forme = personal.holder_forme(base, u16::from_le_bytes([raw[18], raw[19]]));
            let extra = Extra {
                raw,
                species: personal.species_of(base, forme),
                forme,
            };
            let mut level = u8::try_from(word(&raw, 14)?).unwrap_or(u8::MAX);
            // One of Ultra Sun and Ultra Moon's trainers has a Pokémon two levels above the rest of its team.
            if family == Family::UltraSunMoon
                && i == 78
                && p == 3
                && level == 16
                && trainer.pokemon.first().is_some_and(|f: &TrainerPokemon| f.level == 16)
            {
                level = 14;
            }
            let mut tp = TrainerPokemon::new(extra.species, level);
            tp.forme = forme;
            tp.ability_slot = raw[0] >> 4;
            tp.ivs = u8::try_from(extra.ivs() & 0x1F).unwrap_or_default();
            tp.shiny = extra.ivs() & SHINY != 0;
            tp.held_item = Some(word(&raw, 20)?).filter(|&id| id != 0);
            tp.moves = [word(&raw, 24)?, word(&raw, 26)?, word(&raw, 28)?, word(&raw, 30)?];
            trainer.pokemon.push(tp);
            team_extras.push(extra);
        }
        trainers.push(trainer);
        extras.push(team_extras);
    }
    let (plain, rivals) = tags(family);
    for &(tag, numbers) in plain {
        for &n in numbers {
            if let Some(t) = trainers.get_mut(n - 1) {
                t.tag = Some(tag.to_owned());
            }
        }
    }
    for &(tag, n) in rivals {
        for (k, at) in [n - 1, n, n + 1].into_iter().enumerate() {
            if let Some(t) = trainers.get_mut(at) {
                t.tag = Some(format!("{tag}-{k}"));
            }
        }
    }
    let multi = match family {
        Family::SunMoon => c::MULTI_BATTLES_SM,
        Family::UltraSunMoon => c::MULTI_BATTLES_USUM,
    };
    for &n in multi {
        if let Some(t) = trainers.get_mut(n - 1) {
            t.multi_battle = MultiBattleStatus::Always;
        }
    }
    if family == Family::UltraSunMoon {
        for (at, position) in c::FORCED_STARTER_POSITIONS_USUM {
            if let Some(t) = trainers.get_mut(at) {
                t.force_starter_position = Some(position);
            }
        }
    }
    Ok((Teams { extras }, trainers))
}

impl Teams {
    /// The extra data for each Pokémon of a written team: the loaded Pokémon in order for those not added by the
    /// randomizer, the first loaded Pokémon's for added ones. A Pokémon keeps its stored forme while its species and
    /// forme stay; otherwise it gets the forme of the game data.
    fn team_extras(&self, trainer: usize, team: &[TrainerPokemon], personal: &Personal) -> Vec<Extra> {
        let loaded = self.extras.get(trainer).map_or(&[][..], Vec::as_slice);
        let mut next = loaded.iter();
        team.iter()
            .map(|tp| {
                let base = if tp.added { None } else { next.next() };
                let mut extra = base.or_else(|| loaded.first()).cloned().unwrap_or(Extra {
                    raw: [0; MEMBER],
                    species: 0,
                    forme: 0,
                });
                if extra.species != tp.species || extra.forme != tp.forme || tp.added {
                    let forme = match personal.forme_number(tp.species) {
                        0 => tp.forme,
                        f => f,
                    };
                    put_u16(&mut extra.raw, 18, u16::from(forme));
                }
                extra
            })
            .collect()
    }

    /// Writes every trainer when any changed, as the original does.
    pub(super) fn write(
        &self,
        files: &mut Files,
        paths: [&str; 2],
        data: &GameData,
        personal: &Personal,
    ) -> Result<(), RomError> {
        let [data_path, team_path] = paths;
        for (i, tr) in data.trainers.iter().enumerate() {
            let index = i + 1;
            let mut t = files.garc(data_path, &Decompression::None)?.file(index)?;
            t[13] = tr.poke_type();
            t[3] = u8::try_from(tr.pokemon.len()).unwrap_or(u8::MAX);
            if tr.forced_battle_style {
                if tr.battle_style == BattleStyle::Double {
                    if t[2] == 0 {
                        t[2] = 1;
                        // Keeps trainers from attacking their own Pokémon.
                        t[12] |= 0x8;
                    }
                } else if t[2] == 1 {
                    t[2] = 0;
                    t[12] &= 0x7F;
                }
            }
            let extras = self.team_extras(i, &tr.pokemon, personal);
            let mut team = vec![0u8; MEMBER * tr.pokemon.len()];
            for (p, (tp, extra)) in tr.pokemon.iter().zip(extras).enumerate() {
                let m = &mut team[p * MEMBER..(p + 1) * MEMBER];
                m.copy_from_slice(&extra.raw);
                m[0] = (tp.ability_slot << 4) | (extra.raw[0] & 0xF);
                let mut ivs = extra.ivs();
                if u8::try_from(ivs & 0x1F).unwrap_or_default() != tp.ivs {
                    let iv = u32::from(tp.ivs & 0x1F);
                    ivs = (0..6).fold(ivs & !0x3FFF_FFFF, |acc, s| acc | iv << (5 * s));
                }
                ivs = if tp.shiny { ivs | SHINY } else { ivs & !SHINY };
                put_u32(m, 8, ivs);
                put_u16(m, 14, u16::from(tp.level));
                put_u16(m, 16, personal.base(tp.species));
                put_u16(m, 20, tp.held_item.unwrap_or(0));
                let moves = if tp.reset_moves {
                    data.moves_at_level(tp.species, tp.level)
                } else {
                    tp.moves
                };
                for (k, mv) in moves.into_iter().enumerate() {
                    put_u16(m, 24 + k * 2, mv);
                }
            }
            files.garc_mut(data_path, &Decompression::None)?.set_file(index, t)?;
            files.garc_mut(team_path, &Decompression::None)?.set_file(index, team)?;
        }
        Ok(())
    }
}

/// Beast Lusamine's Pokémon in Sun and Moon get aura boosts by species, in the battle module: room for five
/// species, each with the stat it boosts (its highest). Formes count as their base species.
pub(super) fn write_beast_lusamine(battle: &mut [u8], data: &GameData) -> Result<(), RomError> {
    let Some(lusamine) = data.trainers.get(c::BEAST_LUSAMINE_TRAINER) else {
        return Ok(());
    };
    let Some(at) = super::find(battle, c::BEAST_LUSAMINE_BOOSTS_PREFIX)? else {
        return Ok(());
    };
    let mut unique = Vec::new();
    for p in &lusamine.pokemon {
        let base = data
            .species(p.species)
            .and_then(|sp| sp.forme.base)
            .unwrap_or(p.species);
        if !unique.contains(&base) {
            unique.push(base);
        }
    }
    for (i, &n) in unique.iter().take(5).enumerate() {
        let Some(sp) = data.species(n) else { continue };
        put_u16(battle, at + i * 0x10, n);
        battle[at + i * 0x10 + 2] = super::personal::aura_for_highest_stat(sp);
    }
    Ok(())
}
