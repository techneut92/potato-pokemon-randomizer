//! Rivals carrying the starter, and Pokémon added to trainers' teams.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, GameData, MultiBattleStatus, Trainer};
use ppr_settings::TrainerSettings;

use super::mega_stones;
use super::pick::random_ability_slot;
use crate::StepRng;
use crate::choice::below;

/// The level of a team's best Pokémon, and its position: the highest level, the last one counting 2 levels more
/// (so it wins ties), earlier ones winning other ties.
fn best(t: &Trainer) -> Option<usize> {
    let last = t.pokemon.len().checked_sub(1)?;
    let mut best = 0;
    for i in 1..=last {
        let bonus = if i == last { 2 } else { 0 };
        if u16::from(t.pokemon[i].level) + bonus > u16::from(t.pokemon[best].level) {
            best = i;
        }
    }
    Some(best)
}

/// Random evolutions until none is left or one leads back.
fn fully_evolve(game: &GameData, mut species: DexNumber, rng: &mut StepRng) -> DexNumber {
    let mut seen = BTreeSet::from([species]);
    loop {
        let next: Vec<DexNumber> = game
            .evolutions
            .iter()
            .filter(|e| e.from == species)
            .map(|e| e.to)
            .collect();
        if next.is_empty() || next.iter().any(|n| seen.contains(n)) {
            return species;
        }
        species = next[below(rng, next.len())];
        seen.insert(species);
    }
}

/// `base` from `initial`, then random evolutions at their estimated levels below
/// `max`; with "Trainers Evolve their Pokémon" fully evolved from the highest evolution level, scaled.
fn evolutions_by_level(
    game: &GameData,
    s: &TrainerSettings,
    base: DexNumber,
    initial: u16,
    max: u16,
    rng: &mut StepRng,
) -> BTreeMap<u16, DexNumber> {
    let fully_evolved_level = s.evolve.map(|percent| {
        let level = (1.0 + f64::from(percent) / 100.0) * f64::from(game.highest_evolution_level());
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a level of at most 255"
        )]
        let level = level.ceil().max(0.0) as u16;
        level
    });
    let max = fully_evolved_level.map_or(max, |f| max.max(f));
    let mut out = BTreeMap::from([(initial, base)]);
    let (mut level, mut species) = (initial, base);
    while level < max {
        let next: Vec<_> = game.evolutions.iter().filter(|e| e.from == species).collect();
        if next.is_empty() {
            break;
        }
        let chosen = next[below(rng, next.len())];
        if chosen.estimated_level >= max {
            break;
        }
        level = chosen.estimated_level;
        species = chosen.to;
        out.insert(level, species);
    }
    if let Some(f) = fully_evolved_level {
        out.insert(f, fully_evolve(game, species, rng));
    }
    out
}

/// The team position of the rival's starter: the one the game fixes, else the best Pokémon.
fn starter_position(t: &Trainer) -> Option<usize> {
    match t.force_starter_position.map(usize::from) {
        Some(p) if p < t.pokemon.len() => Some(p),
        _ => best(t),
    }
}

/// Gives every trainer tagged `tag` the evolution of the starter that fits its level, in the place of its
/// starter. A level below the first evolution's gets the first.
fn change_starter_with_tag(trainers: &mut [Trainer], tag: &str, by_level: &BTreeMap<u16, DexNumber>, ability: u8) {
    for t in trainers.iter_mut().filter(|t| t.tag.as_deref() == Some(tag)) {
        let Some(p) = starter_position(t) else { continue };
        let tp = &mut t.pokemon[p];
        let found = by_level
            .range(..=u16::from(tp.level))
            .next_back()
            .or_else(|| by_level.iter().next());
        if let Some((_, &species)) = found {
            tp.species = species;
            tp.forme = 0;
            tp.reset_moves = true;
            tp.ability_slot = ability;
        }
    }
}

/// The level of the starter in the first battle tagged `tag`: its best Pokémon's level; 0 when there is none.
fn level_of_starter(trainers: &[Trainer], tag: &str) -> u16 {
    trainers
        .iter()
        .find(|t| t.tag.as_deref() == Some(tag))
        .and_then(|t| best(t).map(|b| u16::from(t.pokemon[b].level)))
        .unwrap_or_default()
}

/// Yellow: the rival always has the second starter (Eevee). It evolves alike in battles 1 to 4, up to the level
/// of battle 3; from battle 5, where the rival's team depends on the player's choices, each variant evolves on
/// its own, so a branching species like Eevee can branch differently.
fn carry_starter_yellow(
    game: &GameData,
    trainers: &mut [Trainer],
    s: &TrainerSettings,
    prefix: &str,
    rng: &mut StepRng,
) {
    let Some(&starter) = game.starters.get(1) else { return };
    let highest_pre_branch = level_of_starter(trainers, &format!("{prefix}3-0"));
    let by_level = evolutions_by_level(game, s, starter, 1, highest_pre_branch, rng);
    // No abilities in Gen 1.
    let ability = 0;
    for encounter in 1..=4 {
        change_starter_with_tag(trainers, &format!("{prefix}{encounter}-0"), &by_level, ability);
    }
    let (&last_level, &last_species) = by_level
        .range(..=highest_pre_branch)
        .next_back()
        .or_else(|| by_level.iter().next())
        .unwrap_or((&1, &starter));
    for variant in 0..3 {
        let by_level = evolutions_by_level(game, s, last_species, last_level, 100, rng);
        for encounter in 5..=8 {
            change_starter_with_tag(trainers, &format!("{prefix}{encounter}-{variant}"), &by_level, ability);
        }
    }
}

/// In battle `n` of variant `v`, the trainers tagged `<prefix><n>-<v>` get starter `(v + offset) % 3`, evolved as
/// fits their starter's level, in its place; Yellow as [`carry_starter_yellow`].
fn carry_starter(
    game: &GameData,
    trainers: &mut [Trainer],
    s: &TrainerSettings,
    prefix: &str,
    offset: usize,
    rng: &mut StepRng,
) {
    let battle = |tag: &str| -> Option<u32> { tag.strip_prefix(prefix)?.split('-').next()?.parse().ok() };
    let last_battle = trainers
        .iter()
        .filter_map(|t| t.tag.as_deref().and_then(battle))
        .max()
        .unwrap_or_default();
    if last_battle == 0 {
        return;
    }
    if game.is_yellow() {
        carry_starter_yellow(game, trainers, s, prefix, rng);
        return;
    }
    if game.starters.len() < 3 {
        return;
    }
    for variant in 0..3 {
        let starter = game.starters[(variant + offset) % 3];
        let by_level = evolutions_by_level(game, s, starter, 1, 100, rng);
        let ability = random_ability_slot(game, starter, rng);
        for encounter in 0..=last_battle {
            change_starter_with_tag(trainers, &format!("{prefix}{encounter}-{variant}"), &by_level, ability);
        }
    }
}

/// Rivals carry the starter strong against the player's, friends the weak one.
pub(super) fn carry_starters(game: &mut GameData, s: &TrainerSettings, rng: &mut StepRng) {
    let mut trainers = std::mem::take(&mut game.trainers);
    carry_starter(game, &mut trainers, s, "RIVAL", 1, rng);
    carry_starter(game, &mut trainers, s, "FRIEND", 2, rng);
    game.trainers = trainers;
}

/// Adds copies of random team members, at random levels from the team's lowest to below
/// its single strongest, before the last one, up to 6 Pokémon (3 for trainers battling with a partner). They
/// hold nothing and are chosen anew later; copies of a Pokémon holding a Mega Stone are marked as such.
pub(super) fn add_pokemon(game: &mut GameData, s: &TrainerSettings, rng: &mut StepRng) {
    let [boss, important, regular] = std::array::from_fn(|i| {
        if game.trainer_pokemon_additions[i] {
            s.additional_pokemon[i]
        } else {
            0
        }
    });
    let stones = mega_stones(game);
    for t in &mut game.trainers {
        let additional = if t.is_boss() {
            boss
        } else if t.is_important() {
            if t.should_not_get_buffs() {
                continue;
            }
            important
        } else {
            regular
        };
        if additional == 0 || t.pokemon.is_empty() {
            continue;
        }
        let original = t.pokemon.clone();
        let lowest = original.iter().map(|p| p.level).min().unwrap_or_default();
        let highest = original.iter().map(|p| p.level).max().unwrap_or_default();
        let shared_highest = original.iter().filter(|p| p.level == highest).count() > 1;
        // Keep the trainer's ace the strongest.
        let upper = if shared_highest {
            highest
        } else {
            highest.saturating_sub(1)
        };
        let max = if t.multi_battle == MultiBattleStatus::Never {
            6
        } else {
            3
        };
        for _ in 0..additional {
            if t.pokemon.len() >= max {
                break;
            }
            let mut new = original[below(rng, original.len())].clone();
            let span = usize::from(upper.max(lowest) - lowest) + 1;
            new.level = lowest + u8::try_from(below(rng, span)).unwrap_or_default();
            new.added_from_mega_stone = new.held_item.is_some_and(|item| stones.contains(&item));
            new.held_item = None;
            new.added = true;
            let before_last = t.pokemon.len() - 1;
            t.pokemon.insert(before_last, new);
        }
        // A starter fixed in the last place stays last.
        if t.force_starter_position.map(usize::from) == Some(original.len() - 1) {
            t.force_starter_position = u8::try_from(t.pokemon.len() - 1).ok();
        }
    }
}
