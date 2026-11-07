//! Trainers (`TrainerData`, `TrainerPokemon`), the Driftveil tournament teams of Black 2 and White 2
//! (`DriftveilPokemon`), trainer names and classes (with the tournament's), the mugshot names, tags and multi
//! battles, and the patch that lets double battles start against a single trainer.
//!
//! Deliberate choices:
//! - A trainer Pokémon's species is its alternate forme's own number (from 650) for a forme with data of its own. It
//!   keeps its forced gender while its slot keeps its species, and its difficulty byte while its IVs don't change.
//! - The Driftveil tournament teams keep their sizes: Pokémon beyond them are not written, and missing ones keep
//!   what the game has. They store no forme, so they get their base species and its moves, as in the original.

use ppr_model::{BattleStyle, ItemId, MultiBattleStatus, Trainer, TrainerPokemon};

use super::constants as c;
use super::entry::{Family, need};
use super::store::{Store, Text, byte, find_after, put_byte, put_word, word};
use crate::RomError;
use crate::entries::RomEntry;

/// Regular trainer names and classes before the tournament's, as the original counts them.
const NORMAL_TRAINER_NAMES: usize = 813;
const NORMAL_TRAINER_CLASSES: usize = 236;
/// How many Pokémon the first tournament trainers have (the others have 6).
const TOURNAMENT_SMALL_TEAMS: usize = 2;
const TOURNAMENT_LEVEL: u8 = 25;
/// Bytes of a tournament Pokémon: species, four moves, an unknown word, the item.
const TOURNAMENT_ITEM: usize = 12;

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// What the game data of a trainer Pokémon doesn't hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Slot {
    species: u16,
    ivs: u8,
    difficulty: u8,
    forme: u8,
    /// The forme word as the game has it.
    stored_forme: u16,
    gender: u8,
}

/// What the trainers were read with.
#[derive(Debug, Clone)]
pub(super) struct Trainers {
    /// By trainer, the hidden data of each Pokémon.
    slots: Vec<Vec<Slot>>,
    /// The trainers that battled in doubles as loaded, by index.
    doubles: Vec<u16>,
}

impl Trainers {
    /// How many trainers are regular ones, before the Driftveil tournament's.
    pub(super) fn regular(&self) -> usize {
        self.slots.len()
    }
}

fn item(id: u16) -> Option<ItemId> {
    (id != 0).then_some(id)
}

fn style(b: u8) -> BattleStyle {
    match b {
        1 => BattleStyle::Double,
        2 => BattleStyle::Triple,
        3 => BattleStyle::Rotation,
        _ => BattleStyle::Single,
    }
}

/// Trainer names: the regular ones without the empty first, then (Black 2 and White 2) the tournament's.
fn names(store: &mut Store, entry: &RomEntry, family: Family) -> Result<Vec<String>, RomError> {
    let mut names = store
        .strings(Text::Strings, need(entry, "TrainerNamesTextOffset")?)?
        .iter()
        .skip(1)
        .cloned()
        .collect::<Vec<_>>();
    if family == Family::Bw2 {
        names.extend_from_slice(store.strings(Text::Strings, need(entry, "PWTTrainerNamesTextOffset")?)?);
    }
    Ok(names)
}

/// Trainer class names, with the tournament's in Black 2 and White 2.
pub(super) fn class_names(store: &mut Store, entry: &RomEntry, family: Family) -> Result<Vec<String>, RomError> {
    let mut names = store
        .strings(Text::Strings, need(entry, "TrainerClassesTextOffset")?)?
        .to_vec();
    if family == Family::Bw2 {
        names.extend_from_slice(store.strings(Text::Strings, need(entry, "PWTTrainerClassesTextOffset")?)?);
    }
    Ok(names)
}

/// Every trainer, tagged.
#[expect(clippy::too_many_lines, reason = "the regular trainers, then the tournament's")]
pub(super) fn read(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    paths: &super::Paths,
) -> Result<(Vec<Trainer>, Trainers), RomError> {
    let names = names(store, entry, family)?;
    let count = store.narc(&paths.trainer_data)?.files.len();
    let mut trainers = Vec::new();
    let mut slots = Vec::new();
    let mut doubles = Vec::new();
    for i in 1..count {
        let data = store.narc_file(&paths.trainer_data, i)?.to_vec();
        let pokes = store.narc_file(&paths.trainer_pokemon, i)?.to_vec();
        let index = u16::try_from(i).map_err(|_| malformed("too many trainers"))?;
        let moves = data[0] & 1 != 0;
        let items = data[0] & 2 != 0;
        let mut trainer = Trainer::new(
            index,
            names.get(i - 1).cloned().unwrap_or_default(),
            data[1],
            Vec::new(),
        );
        trainer.battle_style = style(byte(&data, 2)?);
        if trainer.battle_style == BattleStyle::Double {
            doubles.push(index);
        }
        let mut own = Vec::new();
        let mut at = 0;
        for _ in 0..byte(&data, 3)? {
            let difficulty = byte(&pokes, at)?;
            let flags = byte(&pokes, at + 1)?;
            let level = word(&pokes, at + 2)?;
            let species = word(&pokes, at + 4)?;
            let stored_forme = word(&pokes, at + 6)?;
            let forme = u8::try_from(stored_forme).unwrap_or_default();
            let species = super::forme_species(family, species, forme);
            let ivs = u8::try_from(u16::from(difficulty) * 31 / 255).unwrap_or_default();
            let mut p = TrainerPokemon::new(species, u8::try_from(level).unwrap_or(u8::MAX));
            p.forme = forme;
            p.ivs = ivs;
            p.ability_slot = flags >> 4;
            at += 8;
            if items {
                p.held_item = item(word(&pokes, at)?);
                at += 2;
            }
            if moves {
                for m in 0..4 {
                    p.moves[m] = word(&pokes, at + m * 2)?;
                }
                at += 8;
            }
            own.push(Slot {
                species,
                ivs,
                difficulty,
                forme,
                stored_forme,
                gender: flags & 0xF,
            });
            trainer.pokemon.push(p);
        }
        trainers.push(trainer);
        slots.push(own);
    }
    if family == Family::Bw2
        && let Some(path) = &paths.driftveil
    {
        let mut file = 1;
        for (t, &offset) in c::BW2_DRIFTVEIL_TRAINER_OFFSETS.iter().enumerate() {
            let index = u16::try_from(trainers.len() + 1).map_err(|_| malformed("too many trainers"))?;
            let class = u16::try_from(NORMAL_TRAINER_CLASSES + offset).map_err(|_| malformed("tournament class"))?;
            let name = names.get(NORMAL_TRAINER_NAMES + offset).cloned().unwrap_or_default();
            let mut trainer = Trainer::new(index, name, 0, Vec::new());
            trainer.class = class;
            let size = if t < TOURNAMENT_SMALL_TEAMS { 3 } else { 6 };
            for _ in 0..size {
                let data = store.narc_file(path, file)?;
                let mut p = TrainerPokemon::new(word(data, 0)?, TOURNAMENT_LEVEL);
                p.ivs = 31;
                p.held_item = item(word(data, TOURNAMENT_ITEM)?);
                for m in 0..4 {
                    p.moves[m] = word(data, 2 + m * 2)?;
                }
                trainer.pokemon.push(p);
                file += 1;
            }
            trainers.push(trainer);
        }
    }
    match family {
        Family::Bw1 => {
            tag_bw1(&mut trainers);
            multi(&mut trainers, MultiBattleStatus::Always, &[62, 63, 401, 402]);
            for i in [287, 288, 289] {
                if let Some(t) = trainers.get_mut(i - 1) {
                    t.force_starter_position = Some(0);
                }
            }
        }
        Family::Bw2 => {
            tag_bw2(&mut trainers);
            multi_bw2(&mut trainers, entry.code.starts_with("IRE"));
        }
    }
    Ok((trainers, Trainers { slots, doubles }))
}

/// The bytes of a trainer's Pokémon.
fn pokemon_bytes(trainer: &Trainer, slots: &[Slot], moves_at: impl Fn(&TrainerPokemon) -> [u16; 4]) -> Vec<u8> {
    let custom_moves = trainer.poke_type() & 1 != 0;
    let items = trainer.poke_type() & 2 != 0;
    let mut out = Vec::new();
    for (j, p) in trainer.pokemon.iter().enumerate() {
        let slot = slots.get(j).filter(|s| s.species == p.species);
        let difficulty = match slots.get(j) {
            Some(s) if s.ivs == p.ivs => s.difficulty,
            // As the original: one more, against the rounding down when reading.
            _ => u8::try_from((1 + u16::from(p.ivs) * 255 / 31).min(255)).unwrap_or(u8::MAX),
        };
        let gender = slot.map_or(0, |s| s.gender);
        let forme = slot
            .filter(|s| s.forme == p.forme)
            .map_or(u16::from(p.forme), |s| s.stored_forme);
        out.push(difficulty);
        out.push((p.ability_slot << 4) | gender);
        out.extend_from_slice(&u16::from(p.level).to_le_bytes());
        out.extend_from_slice(&super::base_species(p.species).to_le_bytes());
        out.extend_from_slice(&forme.to_le_bytes());
        if items {
            out.extend_from_slice(&p.held_item.unwrap_or_default().to_le_bytes());
        }
        if custom_moves {
            let moves = if p.reset_moves { moves_at(p) } else { p.moves };
            for m in moves {
                out.extend_from_slice(&m.to_le_bytes());
            }
        }
    }
    out
}

/// Writes the trainers that changed, the tournament teams and the names.
#[expect(
    clippy::too_many_arguments,
    reason = "the parts of the game the trainers are spread over"
)]
pub(super) fn write(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    paths: &super::Paths,
    hidden: &Trainers,
    old: &[Trainer],
    new: &[Trainer],
    moves_at: impl Fn(&TrainerPokemon) -> [u16; 4],
) -> Result<(), RomError> {
    if new.len() != old.len() {
        return Err(malformed("the trainers must keep their number"));
    }
    let regular = hidden.slots.len();
    for (i, (t, before)) in new.iter().zip(old).enumerate().take(regular) {
        if t.pokemon == before.pokemon
            && t.battle_style == before.battle_style
            && t.forced_battle_style == before.forced_battle_style
        {
            continue;
        }
        let count = u8::try_from(t.pokemon.len()).map_err(|_| malformed("too many trainer Pokémon"))?;
        let data = store.narc_file_mut(&paths.trainer_data, i + 1)?;
        put_byte(data, 0, t.poke_type())?;
        put_byte(data, 3, count)?;
        if t.forced_battle_style {
            let (value, ai) = match t.battle_style {
                BattleStyle::Single => (0, None),
                BattleStyle::Double => (1, Some(true)),
                BattleStyle::Triple => (2, Some(true)),
                // As the original: rotation battles keep the AI's flags, with every bit but the doubles one set.
                BattleStyle::Rotation => (3, Some(false)),
            };
            if byte(data, 2)? != value {
                put_byte(data, 2, value)?;
                let flags = byte(data, 12)?;
                let flags = match ai {
                    None => flags & 0x7F,
                    Some(true) => flags | 0x80,
                    Some(false) => flags | 0x7F,
                };
                put_byte(data, 12, flags)?;
            }
        }
        let bytes = pokemon_bytes(t, &hidden.slots[i], &moves_at);
        *store.narc_file_mut(&paths.trainer_pokemon, i + 1)? = bytes;
    }
    if family == Family::Bw2
        && let Some(path) = &paths.driftveil
    {
        let mut file = 1;
        for (k, (t, before)) in new.iter().zip(old).skip(regular).enumerate() {
            let size = if k < TOURNAMENT_SMALL_TEAMS { 3 } else { 6 };
            for j in 0..size {
                let (Some(p), Some(b)) = (t.pokemon.get(j), before.pokemon.get(j)) else {
                    file += 1;
                    continue;
                };
                if p != b {
                    let base = TrainerPokemon {
                        species: super::base_species(p.species),
                        forme: 0,
                        ..p.clone()
                    };
                    let moves = if p.reset_moves { moves_at(&base) } else { p.moves };
                    let data = store.narc_file_mut(path, file)?;
                    put_word(data, 0, base.species)?;
                    put_word(data, TOURNAMENT_ITEM, p.held_item.unwrap_or_default())?;
                    for (m, &mv) in moves.iter().enumerate() {
                        put_word(data, 2 + m * 2, mv)?;
                    }
                }
                file += 1;
            }
        }
    }
    if new.iter().zip(old).any(|(t, b)| t.name != b.name) {
        write_names(store, entry, family, new)?;
    }
    Ok(())
}

/// Writes trainer names, the tournament's at their places and the mugshot names from the same list.
fn write_names(store: &mut Store, entry: &RomEntry, family: Family, trainers: &[Trainer]) -> Result<(), RomError> {
    let names_text = need(entry, "TrainerNamesTextOffset")?;
    let old = store.strings(Text::Strings, names_text)?.to_vec();
    let names: Vec<String> = std::iter::once(old.first().cloned().unwrap_or_default())
        .chain(trainers.iter().map(|t| t.name.clone()))
        .collect();
    if family == Family::Bw2 {
        let regular = old.len().min(names.len());
        let tournament = names[regular..].to_vec();
        let pwt = store.strings_mut(Text::Strings, need(entry, "PWTTrainerNamesTextOffset")?)?;
        for (name, &at) in tournament.iter().zip(&c::BW2_DRIFTVEIL_TRAINER_OFFSETS) {
            if let Some(s) = pwt.get_mut(at) {
                name.clone_into(s);
            }
        }
        *store.strings_mut(Text::Strings, names_text)? = names[..regular].to_vec();
    } else {
        store.strings_mut(Text::Strings, names_text)?.clone_from(&names);
    }
    // The mugshot names come from the same list, the tournament's included.
    write_mugshots(store, entry, family, &names)
}

fn write_mugshots(store: &mut Store, entry: &RomEntry, family: Family, names: &[String]) -> Result<(), RomError> {
    let mugshots = store.strings_mut(Text::Strings, need(entry, "TrainerMugshotsTextOffset")?)?;
    let map: &[(usize, usize)] = family.pick(&c::BW1_MUGSHOTS, &c::BW2_MUGSHOTS);
    for &(mugshot, trainer) in map {
        if let (Some(s), Some(name)) = (mugshots.get_mut(mugshot), names.get(trainer)) {
            name.clone_into(s);
        }
    }
    Ok(())
}

/// Writes the class names, the tournament's to their own text.
pub(super) fn write_class_names(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    names: &[String],
) -> Result<(), RomError> {
    let text = need(entry, "TrainerClassesTextOffset")?;
    if family == Family::Bw2 {
        let regular = store.strings(Text::Strings, text)?.len().min(names.len());
        *store.strings_mut(Text::Strings, text)? = names[..regular].to_vec();
        *store.strings_mut(Text::Strings, need(entry, "PWTTrainerClassesTextOffset")?)? = names[regular..].to_vec();
    } else {
        *store.strings_mut(Text::Strings, text)? = names.to_vec();
    }
    Ok(())
}

/// Lets double battles start against a single trainer: the trainers who battled in doubles get single battle text
/// boxes, and the field code no longer needs a pair of trainers or two Pokémon.
pub(super) fn allow_double_battles(
    store: &mut Store,
    entry: &RomEntry,
    paths: &super::Paths,
    hidden: &Trainers,
) -> Result<(), RomError> {
    let Some(text_boxes) = &paths.trainer_text_boxes else {
        return Ok(());
    };
    let data = store.narc_file_mut(text_boxes, 0)?;
    for at in (0..data.len().saturating_sub(3)).step_by(4) {
        let trainer = word(data, at)?;
        if hidden.doubles.contains(&trainer) {
            let replacement = match word(data, at + 2)? {
                3 => Some(0),
                5 => Some(2),
                6 => Some(0x18),
                _ => None,
            };
            if let Some(r) = replacement {
                put_word(data, at + 2, r)?;
            }
        }
    }
    let field = need(entry, "FieldOvlNumber")?;
    let text_box_prefix = entry
        .value("TrainerOverworldTextBoxPrefix")
        .unwrap_or_default()
        .to_owned();
    // As the original, the offsets after the other two prefixes are counted with this prefix's length.
    let prefix_length = text_box_prefix.len() / 2;
    let unsupported = || malformed("double battle mode is not supported for this game");
    let overlay = store.overlay_mut(field)?;
    let at = find_after(overlay, &text_box_prefix)?.ok_or_else(unsupported)?;
    put_byte(overlay, at - 2, 0)?;
    put_byte(overlay, at, 2)?;
    put_byte(overlay, at + 2, 0x18)?;
    let limit = entry.value("DoubleBattleLimitPrefix").unwrap_or_default();
    let at = super::store::find(overlay, limit)?.ok_or_else(unsupported)? + prefix_length;
    put_word(overlay, at, 0x46C0)?;
    put_word(overlay, at + 2, 0x46C0)?;
    let pointer = entry.value("DoubleBattleGetPointerPrefix").unwrap_or_default();
    let beq = u16::try_from(need(entry, "BeqToSingleTrainerNumber")?).map_err(|_| unsupported())?;
    let at = super::store::find(overlay, pointer)?.ok_or_else(unsupported)? + prefix_length;
    for (to, from) in [(0x10, 0xE), (0xE, 0xC), (0xC, 0xA)] {
        let value = word(overlay, at + from)?;
        put_word(overlay, at + to, value)?;
    }
    put_word(overlay, at + 0xA, beq)?;
    put_word(overlay, at + 8, 0x2800)?;
    let choice = entry.value("TextBoxChoicePrefix").unwrap_or_default().to_owned();
    let arm9 = store.arm9_mut()?;
    let at = find_after(arm9, &choice)?.ok_or_else(unsupported)?;
    put_byte(arm9, at - 4, 2)?;
    Ok(())
}

fn tag(trainers: &mut [Trainer], tag: &str, numbers: &[usize]) {
    for &n in numbers {
        if let Some(t) = n.checked_sub(1).and_then(|i| trainers.get_mut(i)) {
            t.tag = Some(tag.to_owned());
        }
    }
}

/// A rival's three battles, one per starter: the trainer at index `index` (counted from 0) and those around it.
fn tag_rival(trainers: &mut [Trainer], tag: &str, index: usize) {
    for (k, i) in (index - 1..=index + 1).enumerate() {
        if let Some(t) = trainers.get_mut(i) {
            t.tag = Some(format!("{tag}-{k}"));
        }
    }
}

fn multi(trainers: &mut [Trainer], status: MultiBattleStatus, numbers: &[usize]) {
    for &n in numbers {
        if let Some(t) = n.checked_sub(1).and_then(|i| trainers.get_mut(i)) {
            t.multi_battle = status;
        }
    }
}

fn multi_bw2(trainers: &mut [Trainer], black: bool) {
    use MultiBattleStatus::{Always, Potential};
    multi(
        trainers,
        Always,
        &[
            342, 347, 356, 374, 375, 376, 377, 494, 495, 496, 614, 615, 643, 644, 704, 705, 797, 798, 799, 807, 809,
        ],
    );
    multi(trainers, Potential, &[513, 522, 602, 603, 788, 790, 791, 792, 793]);
    if black {
        multi(trainers, Always, &[521, 789]);
        multi(trainers, Potential, &[520, 786]);
    } else {
        multi(trainers, Potential, &[514, 521]);
        multi(trainers, Always, &[519, 787]);
    }
}

/// The trainer groups of Black and White. Cilan, Chili and Cress are gyms 1, 9 and 10, their trainers gym 11.
fn tag_bw1(t: &mut [Trainer]) {
    tag(t, "GYM11", &[0x09, 0x0A]);
    tag(t, "GYM2", &[0x56, 0x57, 0x58]);
    tag(t, "GYM3", &[0xC4, 0xC6, 0xC7, 0xC8]);
    tag(t, "GYM4", &[0x42, 0x43, 0x44, 0x45]);
    tag(t, "GYM5", &[0xC9, 0xCA, 0xCB, 0x5F, 0xA8]);
    tag(t, "GYM6", &[0x7D, 0x7F, 0x80, 0x46, 0x47]);
    tag(t, "GYM7", &[0xD7, 0xD8, 0xD9, 0xD4, 0xD5, 0xD6]);
    tag(t, "GYM8", &[0x109, 0x10A, 0x10F, 0x10E, 0x110, 0x10B, 0x113, 0x112]);
    tag(t, "GYM1-LEADER", &[0x0C]);
    tag(t, "GYM9-LEADER", &[0x0B]);
    tag(t, "GYM10-LEADER", &[0x0D]);
    tag(t, "GYM2-LEADER", &[0x15]);
    tag(t, "GYM3-LEADER", &[0x16]);
    tag(t, "GYM4-LEADER", &[0x17]);
    tag(t, "GYM5-LEADER", &[0x18]);
    tag(t, "GYM6-LEADER", &[0x19]);
    tag(t, "GYM7-LEADER", &[0x83]);
    tag(t, "GYM8-LEADER", &[0x84, 0x85]);
    tag(t, "ELITE1", &[0xE4, 0x233]);
    tag(t, "ELITE2", &[0xE6, 0x235]);
    tag(t, "ELITE3", &[0xE7, 0x236]);
    tag(t, "ELITE4", &[0xE5, 0x234]);
    tag(t, "CHAMPION", &[0x197]);
    tag(t, "UBER", &[0x21E, 0x237, 0xE8, 0x24A, 0x24B]);
    for (name, n) in [
        ("RIVAL1", 0x35),
        ("RIVAL2", 0x11F),
        ("RIVAL3", 0x38),
        ("RIVAL4", 0x193),
        ("RIVAL5", 0x5A),
        ("RIVAL6", 0x21B),
        ("RIVAL7", 0x24C),
        ("RIVAL8", 0x24F),
        ("FRIEND1", 0x3B),
        ("FRIEND2", 0x1F2),
        ("FRIEND3", 0x1FB),
        ("FRIEND4", 0x1EB),
        ("FRIEND5", 0x1EE),
        ("FRIEND6", 0x252),
    ] {
        tag_rival(t, name, n);
    }
    tag(t, "NOTSTRONG", &[64]);
    tag(t, "STRONG", &[65, 89, 218]);
}

/// The trainer groups of Black 2 and White 2. The challenge mode teams of the leaders, Elite Four and Champion are
/// tagged as their regular ones.
fn tag_bw2(t: &mut [Trainer]) {
    tag(t, "GYM1", &[0xab, 0xac]);
    tag(t, "GYM2", &[0xb2, 0xb3]);
    tag(t, "GYM3", &[0x2de, 0x2df, 0x2e0, 0x2e1]);
    tag(t, "GYM4", &[0x26d, 0x94, 0xcf, 0xd0, 0xd1]);
    tag(t, "GYM5", &[0x13f, 0x140, 0x141, 0x142, 0x143, 0x144, 0x145]);
    tag(t, "GYM6", &[0x95, 0x96, 0x97, 0x98, 0x14c]);
    tag(t, "GYM7", &[0x17d, 0x17e, 0x17f, 0x180, 0x181]);
    tag(t, "GYM8", &[0x15e, 0x15f, 0x160, 0x161, 0x162, 0x163]);
    tag(t, "GYM1-LEADER", &[0x9c, 0x2fc]);
    tag(t, "GYM2-LEADER", &[0x9d, 0x2fd]);
    tag(t, "GYM3-LEADER", &[0x9a, 0x2fe]);
    tag(t, "GYM4-LEADER", &[0x99, 0x2ff]);
    tag(t, "GYM5-LEADER", &[0x9e, 0x300]);
    tag(t, "GYM6-LEADER", &[0x9b, 0x301]);
    tag(t, "GYM7-LEADER", &[0x9f, 0x302]);
    tag(t, "GYM8-LEADER", &[0xa0, 0x303]);
    tag(t, "ELITE1", &[0x26, 0x304, 0x8f, 0x309]);
    tag(t, "ELITE2", &[0x28, 0x305, 0x91, 0x30a]);
    tag(t, "ELITE3", &[0x29, 0x307, 0x92, 0x30c]);
    tag(t, "ELITE4", &[0x27, 0x306, 0x90, 0x30b]);
    tag(t, "CHAMPION", &[0x155, 0x308, 0x218, 0x30d]);
    for (name, n) in [
        ("RIVAL1", 0xa1),
        ("RIVAL2", 0xa6),
        ("RIVAL3", 0x24c),
        ("RIVAL4", 0x170),
        ("RIVAL5", 0x17a),
        ("RIVAL6", 0x2bd),
        ("RIVAL7", 0x31a),
        ("RIVAL8", 0x2ac),
        ("RIVAL9", 0x2b5),
        ("RIVAL10", 0x2b8),
        ("FRIEND2", 0x168),
        ("FRIEND2", 0x16b),
    ] {
        tag_rival(t, name, n);
    }
    tag(t, "GYM1", &[0x173, 0x278]);
    tag(t, "GYM1-NOTSTRONG", &[0x32E]);
    tag(t, "GYM9-LEADER", &[0x1f0]);
    tag(t, "GYM10-LEADER", &[0x1ee]);
    tag(t, "GYM11-LEADER", &[0x1ef]);
    tag(t, "THEMED:ZINZOLIN-STRONG", &[0x2c0, 0x248, 0x15b, 0x1f1]);
    tag(t, "THEMED:COLRESS-STRONG", &[0x166, 0x158, 0x32d]);
    tag(t, "THEMED:COLRESS-STRONG-NOTSTRONG", &[0x32f]);
    tag(t, "THEMED:SHADOW1", &[0x247, 0x15c, 0x2af]);
    tag(t, "THEMED:SHADOW2", &[0x1f2, 0x2b0]);
    tag(t, "THEMED:SHADOW3", &[0x1f3, 0x2b1]);
    tag(
        t,
        "UBER",
        &[
            0x246, 0x1c8, 0xca, 0xc9, 0x5, 0x6, 0x30e, 0x30f, 0x310, 0x311, 0x159, 0x8c, 0x24f,
        ],
    );
}
