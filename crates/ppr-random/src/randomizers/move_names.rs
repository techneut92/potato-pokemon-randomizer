//! Every move but Struggle gets a name made of a word for its type and one for what it does, from the word lists
//! in `data/move_names`.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - Names are chosen after all other move data, so a move whose category changed gets words for its new
//!   category.
//! - The name a move gets when 50 tries find no unused one fits the game: "Attack Strike" would be 13 letters,
//!   one more than Gen 3 holds, so the game would show it without its end mark.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::LazyLock;

use ppr_model::{GameData, Move, MoveCategory, MoveId, STRUGGLE, StatChangeMoveType, StatusType, Type};

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Move Names";

/// How many names are tried for a move before falling back.
const MAX_TRIES: usize = 50;

/// A word list file: `[KEY] (Word) (Word) …` lines; other lines are comments.
type Lists = BTreeMap<&'static str, Vec<&'static str>>;

static TYPE_WORDS: LazyLock<Lists> = LazyLock::new(|| parse(include_str!("../../data/move_names/TypeMoveNames.txt")));
static CATEGORY_WORDS: LazyLock<Lists> =
    LazyLock::new(|| parse(include_str!("../../data/move_names/CatMoveNames.txt")));
static EXTRA_WORDS: LazyLock<Lists> =
    LazyLock::new(|| parse(include_str!("../../data/move_names/SubCatMoveNames.txt")));

/// Moves whose main effect is healing, which nothing in the move data tells.
const HEAL_MOVES: [MoveId; 25] = [
    105, 135, 156, 208, 234, 235, 236, 256, 273, 275, 287, 303, 355, 361, 392, 456, 461, 215, 312, 505, 666, 685, 659,
    791, 816,
];

/// Randomizes move names when [`ppr_settings::MoveDataSettings::names`] is on.
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveNameRandomizer;

impl Step<RunData> for MoveNameRandomizer {
    fn id(&self) -> u64 {
        step_ids::MOVE_NAMES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.move_data.names && !data.game.moves.is_empty() && data.game.max_move_name_length > 0
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        randomize(&mut data.game, &mut rng);
        Ok(())
    }
}

/// Parses a word list file.
fn parse(text: &'static str) -> Lists {
    let mut lists = Lists::new();
    for line in text.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, rest)) = line
            .split_once('[')
            .and_then(|(_, after)| after.split_once(']'))
            .filter(|(key, _)| !key.is_empty())
        else {
            continue;
        };
        let words: Vec<&str> = rest
            .split('(')
            .skip(1)
            .filter_map(|part| part.split_once(')').map(|(word, _)| word))
            .filter(|word| !word.is_empty())
            .collect();
        if !words.is_empty() {
            lists.insert(key, words);
        }
    }
    lists
}

/// Gives every move but Struggle a new name.
fn randomize(game: &mut GameData, rng: &mut StepRng) {
    let max_len = game.max_move_name_length;
    let upper_case = upper_case_names(game.moves.iter().map(|m| m.name.as_str()));
    let mut used = BTreeSet::new();
    for mv in game.moves.iter_mut().filter(|m| m.number != STRUGGLE) {
        let name = random_name(mv, max_len, &mut used, rng);
        mv.name = if upper_case { name.to_uppercase() } else { name };
    }
}

/// Whether the game's names are upper case: no lowercase letters, or more than ten times as many uppercase ones.
fn upper_case_names<'a>(names: impl Iterator<Item = &'a str>) -> bool {
    let (mut upper, mut lower) = (0usize, 0usize);
    for c in names.flat_map(str::chars) {
        if c.is_uppercase() {
            upper += 1;
        } else if c.is_lowercase() {
            lower += 1;
        }
    }
    lower == 0 || (upper > 0 && lower * 10 < upper)
}

/// A random name: a type word and an action word, without the space when that is too long, and not
/// used before.
fn random_name(mv: &Move, max_len: usize, used: &mut BTreeSet<String>, rng: &mut StepRng) -> String {
    let type_words = mv
        .move_type
        .and_then(|t| TYPE_WORDS.get(type_key(t)))
        .map_or(&["Attack"][..], Vec::as_slice);
    let action_words = action_words(mv);
    for _ in 0..MAX_TRIES {
        let type_word = type_words[below(rng, type_words.len())];
        let action_word = action_words[below(rng, action_words.len())];
        let mut name = format!("{type_word} {action_word}");
        if name.chars().count() > max_len {
            name = format!("{type_word}{action_word}");
            if name.chars().count() > max_len {
                continue;
            }
        }
        if used.insert(name.clone()) {
            return name;
        }
    }
    ["Attack Strike", "AttackStrike", "Attack"]
        .into_iter()
        .find(|n| n.len() <= max_len)
        .unwrap_or("Atk")
        .to_owned()
}

/// The key of a type in `TypeMoveNames.txt`: its name in upper case.
fn type_key(t: Type) -> &'static str {
    match t {
        Type::Normal => "NORMAL",
        Type::Fighting => "FIGHTING",
        Type::Flying => "FLYING",
        Type::Poison => "POISON",
        Type::Ground => "GROUND",
        Type::Rock => "ROCK",
        Type::Bug => "BUG",
        Type::Ghost => "GHOST",
        Type::Steel => "STEEL",
        Type::Fire => "FIRE",
        Type::Water => "WATER",
        Type::Grass => "GRASS",
        Type::Electric => "ELECTRIC",
        Type::Psychic => "PSYCHIC",
        Type::Ice => "ICE",
        Type::Dragon => "DRAGON",
        Type::Dark => "DARK",
        Type::Fairy => "FAIRY",
    }
}

/// A list of `SubCatMoveNames.txt`, if it has words.
fn extra(key: &str) -> Option<&'static [&'static str]> {
    EXTRA_WORDS.get(key).map(Vec::as_slice).filter(|w| !w.is_empty())
}

/// The words for what a move does: punch, sound, draining, status and trapping moves have their own words; the rest
/// get their category's.
fn action_words(mv: &Move) -> &'static [&'static str] {
    let special = [
        (mv.is_punch, "PUNCH"),
        (mv.is_sound, "SOUND"),
        (mv.absorb_percent > 0 && mv.power > 0, "DRAIN"),
    ];
    if let Some(words) = special.iter().filter(|(on, _)| *on).find_map(|(_, key)| extra(key)) {
        return words;
    }
    if mv.category == MoveCategory::Status
        && let Some(words) = status_words(mv)
    {
        return words;
    }
    if mv.is_trap
        && let Some(words) = extra("STATUS_TRAP")
    {
        return words;
    }
    let key = match mv.category {
        MoveCategory::Physical => "PHYSICAL",
        MoveCategory::Special => "SPECIAL",
        MoveCategory::Status => "STATUS",
    };
    CATEGORY_WORDS.get(key).map_or(&["Strike"][..], Vec::as_slice)
}

/// The words for a status move's subcategory: healing, causing a status, trapping, or raising or lowering stats; `None`
/// for moves like Splash, which get the plain status words.
fn status_words(mv: &Move) -> Option<&'static [&'static str]> {
    if HEAL_MOVES.contains(&mv.number) {
        return extra("STATUS_HEAL");
    }
    if let Some(words) = mv.status.and_then(|s| extra(status_key(s))) {
        return Some(words);
    }
    if mv.is_trap {
        return extra("STATUS_TRAP");
    }
    match mv.stat_change_move_type {
        StatChangeMoveType::NoDamageUser => {
            let raises = mv.stat_changes.iter().any(|c| c.stages > 0);
            extra(if raises { "STATUS_BUFF" } else { "STATUS_DEBUFF" })
        }
        StatChangeMoveType::NoDamageTarget => extra("STATUS_DEBUFF"),
        StatChangeMoveType::NoDamageAll | StatChangeMoveType::NoDamageAlly => extra("STATUS_BUFF"),
        StatChangeMoveType::NoneOrUnknown | StatChangeMoveType::DamageTarget | StatChangeMoveType::DamageUser => None,
    }
}

/// The key of a status in `SubCatMoveNames.txt`.
fn status_key(s: StatusType) -> &'static str {
    match s {
        StatusType::Poison | StatusType::ToxicPoison => "INFLICT_POISON",
        StatusType::Burn => "INFLICT_BURN",
        StatusType::Freeze => "INFLICT_FREEZE",
        StatusType::Paralyze => "INFLICT_PARALYZE",
        StatusType::Sleep => "INFLICT_SLEEP",
        StatusType::Confusion => "INFLICT_CONFUSION",
    }
}

#[cfg(test)]
mod tests;
