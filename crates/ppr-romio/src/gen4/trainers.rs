//! Trainers, their names and classes, read and written, and the patch that lets any trainer fight a double
//! battle.
//!
//! A trainer is 20 bytes in the trainer data NARC: the team format (bit 0: custom moves, bit 1: held items), the
//! class, the number of Pokémon, items, AI flags and the battle mode (0 for single battles). Its team is a file of
//! the trainer Pokémon NARC: per Pokémon a difficulty byte (0 to 255 for 0 to 31 IVs), a byte with the ability
//! slot in its high nibble (0 for the first ability) and the gender in its low one, the level, the species with
//! the forme in the bits above it, then the held item and the four moves when the format has them, and in
//! Platinum, `HeartGold` and `SoulSilver` two bytes of ball seals.
//!
//! Deliberate choices:
//! - Only trainers that changed are written; their team file is made anew in its new size.
//! - A Pokémon that keeps its place and species keeps its gender and difficulty byte (when its IVs are unchanged);
//!   every Pokémon keeps the ball seals of its place. A new species gets gender 0.
//! - In `HeartGold` and `SoulSilver`, where one Pokémon with its second ability makes all after it use theirs too,
//!   every Pokémon of a trainer written gets the ability slot of the last.
//! - Names and class names are written only when they changed.

use ppr_model::{BattleStyle, GameData, ItemId, Trainer, TrainerPokemon, Type};

use super::constants::{self as c, Family};
use super::files::{byte, find_after, put, put_word, word};
use super::{Rom, trainer_tags};
use crate::RomError;
use crate::nds_text::gen4 as text;

/// The trainer data: format, class, number of Pokémon and battle mode.
const TR_FORMAT: usize = 0;
const TR_CLASS: usize = 1;
const TR_COUNT: usize = 3;
const TR_BATTLE_MODE: usize = 16;
/// The battle mode of a trainer made to fight double battles alone.
const FORCED_DOUBLE: u8 = 3;
/// The partners of the player (Cheryl and the others) in Diamond, Pearl and Platinum: a battle mode for them would
/// turn their wild double battles into glitched trainer battles.
const PARTNER_TRAINERS: [u16; 5] = [608, 609, 610, 611, 612];
/// The highest difficulty byte, which means 31 IVs.
const MAX_DIFFICULTY: u16 = 255;
const MAX_IVS: u16 = 31;
/// How the double battle patch finds the special double battle intro's branch.
const DP_DOUBLE_BATTLE_FIX_PREFIX: &str = "022912D90221214201";
const PT_DOUBLE_BATTLE_FIX_PREFIX: &str = "022919D90221214205";
const HGSS_DOUBLE_BATTLE_FIX_PREFIX: &str = "2C2815D00221214201";
/// "&" in trainer names, as the text holds it.
const AND: &str = "\\and";

/// The size of a team member in a team file of `format`.
fn member_size(family: Family, format: u8) -> usize {
    let mut size = 6;
    if format & 2 != 0 {
        size += 2;
    }
    if format & 1 != 0 {
        size += 8;
    }
    if family != Family::DiamondPearl {
        size += 2;
    }
    size
}

/// What the game data doesn't hold of a team member: gender bits, difficulty byte and ball seals, with the species
/// and IVs they were read with.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Member {
    species: u16,
    ivs: u8,
    difficulty: u8,
    gender: u8,
    seals: Option<[u8; 2]>,
}

/// What a trainer's team holds beyond the game data.
#[derive(Debug, Clone, Default)]
pub(super) struct Teams {
    members: Vec<Vec<Member>>,
}

/// The trainer class names.
pub(super) fn class_names(rom: &mut Rom) -> Result<Vec<String>, RomError> {
    rom.strings_of("TrainerClassesTextOffset")
}

/// The trainer names without the empty first one, with `&` for its control code.
fn names(rom: &mut Rom) -> Result<Vec<String>, RomError> {
    let names = rom.strings_of("TrainerNamesTextOffset")?;
    Ok(names.into_iter().skip(1).map(|n| n.replace(AND, "&")).collect())
}

/// The trainers from number 1, tagged.
pub(super) fn read(rom: &mut Rom) -> Result<(Vec<Trainer>, Teams), RomError> {
    let family = rom.family();
    let names = names(rom)?;
    let data_path = rom.entry.file("TrainerData")?;
    let team_path = rom.entry.file("TrainerPokemon")?;
    let data = rom.files.narc(&data_path)?.files.clone();
    let teams = rom.files.narc(&team_path)?.files.clone();
    let mut trainers = Vec::new();
    let mut members = Vec::new();
    for (i, tr) in data.iter().enumerate().skip(1) {
        let team = teams
            .get(i)
            .ok_or_else(|| RomError::Malformed(format!("trainer {i} has no team")))?;
        let format = byte(tr, TR_FORMAT)?;
        let count = usize::from(byte(tr, TR_COUNT)?);
        let size = member_size(family, format);
        let mut pokemon = Vec::with_capacity(count);
        let mut own = Vec::with_capacity(count);
        for p in 0..count {
            let at = p * size;
            let difficulty = byte(team, at)?;
            let flags = byte(team, at + 1)?;
            let raw = word(team, at + 4)?;
            let forme = u8::try_from(raw >> 10).unwrap_or_default();
            let species = c::forme_species(family, raw & 0x1FF, forme);
            let mut tp = TrainerPokemon::new(species, byte(team, at + 2)?);
            tp.forme = forme;
            tp.ivs = u8::try_from(u16::from(difficulty) * MAX_IVS / MAX_DIFFICULTY).unwrap_or_default();
            // The first ability is 0 in every Gen 4 game.
            tp.ability_slot = match flags >> 4 {
                0 => 1,
                slot => slot,
            };
            let mut next = at + 6;
            if format & 2 != 0 {
                let item = word(team, next)?;
                tp.held_item = (item != 0).then_some(item);
                next += 2;
            }
            if format & 1 != 0 {
                for (m, slot) in tp.moves.iter_mut().enumerate() {
                    *slot = word(team, next + m * 2)?;
                }
                next += 8;
            }
            let seals = (family != Family::DiamondPearl)
                .then(|| team.get(next..next + 2).map(|s| [s[0], s[1]]))
                .flatten();
            own.push(Member {
                species,
                ivs: tp.ivs,
                difficulty,
                gender: flags & 0x0F,
                seals,
            });
            pokemon.push(tp);
        }
        let index = u16::try_from(i).map_err(|_| RomError::Malformed("too many trainers".into()))?;
        let name = names.get(i - 1).cloned().unwrap_or_default();
        let mut trainer = Trainer::new(index, name, byte(tr, TR_CLASS)?, pokemon);
        if byte(tr, TR_BATTLE_MODE)? != 0 {
            trainer.battle_style = BattleStyle::Double;
        }
        trainers.push(trainer);
        members.push(own);
    }
    trainer_tags::tag(&mut trainers, family);
    Ok((trainers, Teams { members }))
}

/// The type theme of each gym leader and Elite Four member, by tag.
pub(super) fn gym_and_elite_themes(family: Family) -> std::collections::BTreeMap<String, Type> {
    let themes: &[(&str, Type)] = match family {
        Family::DiamondPearl => &c::DP_GYM_AND_ELITE_THEMES,
        Family::Platinum => &c::PT_GYM_AND_ELITE_THEMES,
        Family::HeartGoldSoulSilver => &c::HGSS_GYM_AND_ELITE_THEMES,
    };
    themes.iter().map(|&(g, t)| (g.to_owned(), t)).collect()
}

/// Writes the trainers, names and class names that changed, after the double battle patch when trainers are
/// made to fight in a battle style of their own for the first time.
pub(super) fn write(rom: &mut Rom, teams: &Teams, loaded: &GameData, data: &GameData) -> Result<(), RomError> {
    if data.trainer_class_names != loaded.trainer_class_names {
        let index = rom.entry.need("TrainerClassesTextOffset")?;
        rom.files.set_strings(index, data.trainer_class_names.clone())?;
    }
    if data.trainers == loaded.trainers {
        return Ok(());
    }
    if data.trainers.len() != loaded.trainers.len() {
        return Err(RomError::Malformed("the trainers must keep their number".into()));
    }
    if data.trainers.iter().any(|t| t.forced_battle_style) && !loaded.trainers.iter().any(|t| t.forced_battle_style) {
        allow_double_battles(rom)?;
    }
    let family = rom.family();
    let data_path = rom.entry.file("TrainerData")?;
    let team_path = rom.entry.file("TrainerPokemon")?;
    for (i, (trainer, before)) in data.trainers.iter().zip(&loaded.trainers).enumerate() {
        if trainer.pokemon != before.pokemon
            || trainer.class != before.class
            || trainer.forced_battle_style != before.forced_battle_style
        {
            let mut trainer = trainer.clone();
            if family == Family::HeartGoldSoulSilver
                && let Some(last) = trainer.pokemon.last().map(|p| p.ability_slot)
            {
                for p in &mut trainer.pokemon {
                    p.ability_slot = last;
                }
            }
            let own = teams.members.get(i).map(Vec::as_slice).unwrap_or_default();
            let team = team_bytes(&trainer, own, family, data)?;
            *rom.files.narc_file(&team_path, i + 1)? = team;
            let tr = rom.files.narc_file(&data_path, i + 1)?;
            let class = u8::try_from(trainer.class)
                .map_err(|_| RomError::Malformed(format!("trainer {} has class {}", trainer.index, trainer.class)))?;
            put(tr, TR_FORMAT, &[trainer.poke_type(), class])?;
            let count = u8::try_from(trainer.pokemon.len())
                .map_err(|_| RomError::Malformed(format!("trainer {} has too many Pokémon", trainer.index)))?;
            put(tr, TR_COUNT, &[count])?;
            let partner = family != Family::HeartGoldSoulSilver && PARTNER_TRAINERS.contains(&trainer.index);
            if trainer.forced_battle_style && !partner {
                let mode = byte(tr, TR_BATTLE_MODE)?;
                if trainer.battle_style == BattleStyle::Double {
                    if mode == 0 {
                        put(tr, TR_BATTLE_MODE, &[FORCED_DOUBLE])?;
                    }
                } else if mode == FORCED_DOUBLE {
                    put(tr, TR_BATTLE_MODE, &[0])?;
                }
            }
        }
    }
    if data
        .trainers
        .iter()
        .zip(&loaded.trainers)
        .any(|(a, b)| a.name != b.name)
    {
        write_names(rom, &data.trainers)?;
    }
    Ok(())
}

/// A trainer's team file.
fn team_bytes(trainer: &Trainer, own: &[Member], family: Family, data: &GameData) -> Result<Vec<u8>, RomError> {
    let format = trainer.poke_type();
    let size = member_size(family, format);
    let mut bytes = vec![0u8; trainer.pokemon.len() * size];
    for (p, (tp, chunk)) in trainer.pokemon.iter().zip(bytes.chunks_mut(size)).enumerate() {
        let old = own.get(p);
        let same = old.filter(|m| m.species == tp.species);
        let difficulty = match same {
            Some(m) if m.ivs == tp.ivs => m.difficulty,
            // 1 is added against the truncation of reading.
            _ => {
                u8::try_from((1 + u16::from(tp.ivs) * MAX_DIFFICULTY / MAX_IVS).min(MAX_DIFFICULTY)).unwrap_or(u8::MAX)
            }
        };
        let ability = if tp.ability_slot == 1 { 0 } else { tp.ability_slot << 4 };
        let gender = same.map_or(0, |m| m.gender);
        // The species is stored as its base species and forme.
        let species = c::base_species(tp.species);
        if species > 0x1FF || tp.forme > 0x3F {
            return Err(RomError::Malformed(format!(
                "Gen 4 can't store species {} with forme {}",
                tp.species, tp.forme
            )));
        }
        put(chunk, 0, &[difficulty, ability | gender])?;
        put_word(chunk, 2, u16::from(tp.level))?;
        put_word(chunk, 4, species | (u16::from(tp.forme) << 10))?;
        let mut next = 6;
        if format & 2 != 0 {
            let item: ItemId = tp.held_item.unwrap_or(0);
            put_word(chunk, next, item)?;
            next += 2;
        }
        if format & 1 != 0 {
            let moves = if tp.reset_moves {
                data.moves_at_level(tp.species, tp.level)
            } else {
                tp.moves
            };
            for (m, &mv) in moves.iter().enumerate() {
                put_word(chunk, next + m * 2, mv)?;
            }
            next += 8;
        }
        if family != Family::DiamondPearl
            && let Some(seals) = old.and_then(|m| m.seals)
        {
            put(chunk, next, &seals)?;
        }
    }
    Ok(bytes)
}

/// The trainer names, after the empty first one, packed if they were.
fn write_names(rom: &mut Rom, trainers: &[Trainer]) -> Result<(), RomError> {
    let index = rom.entry.need("TrainerNamesTextOffset")?;
    let path = rom.entry.file("Text")?;
    let compressed = text::decode(rom.files.narc_file(&path, index)?)?.compressed;
    let old = rom.files.strings(index)?;
    let mut names = vec![old.first().cloned().unwrap_or_default()];
    names.extend(trainers.iter().map(|t| t.name.replace('&', AND)));
    rom.files.set_strings_packed(index, names, compressed)
}

/// Lets any trainer fight a double battle. The game prefers the special double battle intro, with its music,
/// over a trainer's own; its branch becomes unconditional, which slightly breaks the games' own double battles.
/// The "trainer eye" routines and text boxes check for a battle mode of exactly 2, so trainers given mode 3 fight
/// a double battle alone; the end-of-battle text box data checks that too.
fn allow_double_battles(rom: &mut Rom) -> Result<(), RomError> {
    let unsupported = || RomError::Malformed("this game can't be patched for double battles".into());
    let prefix = match rom.family() {
        Family::DiamondPearl => DP_DOUBLE_BATTLE_FIX_PREFIX,
        Family::Platinum => PT_DOUBLE_BATTLE_FIX_PREFIX,
        Family::HeartGoldSoulSilver => HGSS_DOUBLE_BATTLE_FIX_PREFIX,
    };
    let flag_return = rom.entry.string("DoubleBattleFlagReturnPrefix").to_owned();
    let walking1 = rom.entry.string("DoubleBattleWalkingPrefix1").to_owned();
    let walking2 = rom.entry.string("DoubleBattleWalkingPrefix2").to_owned();
    let text_box = rom.entry.string("DoubleBattleTextBoxPrefix").to_owned();
    let arm9 = rom.files.arm9();
    let at = find_after(arm9, prefix).ok_or_else(unsupported)?;
    // b (unconditional) instead of beq.
    put(arm9, at, &[0xE0])?;
    // Return the double battle flag as it is.
    let at = find_after(arm9, &flag_return).ok_or_else(unsupported)?;
    put_word(arm9, at, 0xBD08)?;
    // The double trainer walk only for 2: cmp r0, #2; beq.
    let at = find_after(arm9, &walking1).ok_or_else(unsupported)?;
    put(arm9, at, &[0x02])?;
    put(arm9, at + 3, &[0xD0])?;
    let at = find_after(arm9, &walking2).ok_or_else(unsupported)?;
    put(arm9, at, &[0x02])?;
    // nop; cmp r0, #2; beq.
    let at = find_after(arm9, &text_box).ok_or_else(unsupported)?;
    put_word(arm9, at, 0x46C0)?;
    put_word(arm9, at + 2, 0x2802)?;
    put(arm9, at + 5, &[0xD0])?;
    let path = rom.entry.file("BattleSkillSubSeq")?;
    let file = rom.entry.need("TrainerEndFileNumber")?;
    let offset = rom.entry.need("TrainerEndTextBoxOffset")?;
    put(rom.files.narc_file(&path, file)?, offset, &[0])
}

#[cfg(test)]
mod tests {
    use super::super::tests::{load, real_rom, save};
    use super::*;
    use crate::GameHandler;

    #[test]
    fn member_sizes_follow_the_format() {
        assert_eq!(member_size(Family::DiamondPearl, 0), 6);
        assert_eq!(member_size(Family::DiamondPearl, 3), 16);
        assert_eq!(member_size(Family::Platinum, 0), 8);
        assert_eq!(member_size(Family::HeartGoldSoulSilver, 1), 16);
    }

    /// With `PPR_TEST_ROMS`: changed trainers, names and class names read back from Platinum.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_platinum_trainers_read_back() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes);
        let mut d = h.game_data().clone();
        assert_ne!(d.trainers.len(), 0);
        let roark = d
            .trainers
            .iter()
            .position(|t| t.tag.as_deref() == Some("GYM1-LEADER"))
            .expect("Roark");
        {
            let t = &mut d.trainers[roark];
            t.name = "Rocky".into();
            t.pokemon[0].species = 25;
            t.pokemon[0].level = 42;
            t.pokemon[0].held_item = Some(234);
            t.pokemon[0].moves = [85, 98, 86, 9];
            let mut extra = TrainerPokemon::new(6, 30);
            extra.ivs = 31;
            t.pokemon.push(extra);
        }
        d.trainers[0].pokemon[0].level = 9;
        d.trainer_class_names[2] = "Kid".into();
        h.apply(&d).expect("applies");
        let back = load(save(&h));
        let b = back.game_data();
        assert_eq!(b.trainers[roark].name, "Rocky");
        assert_eq!(b.trainers[roark].pokemon.len(), d.trainers[roark].pokemon.len());
        let p = &b.trainers[roark].pokemon;
        assert_eq!(
            (p[0].species, p[0].level, p[0].held_item, p[0].moves),
            (25, 42, Some(234), [85, 98, 86, 9])
        );
        assert_eq!(
            (p.last().map(|p| p.species), p.last().map(|p| p.ivs)),
            (Some(6), Some(31))
        );
        assert_eq!(b.trainers[0].pokemon[0].level, 9);
        assert_eq!(b.trainer_class_names[2], "Kid");
        let untouched = 1;
        assert_eq!(b.trainers[untouched], h.game_data().trainers[untouched]);
    }
}
