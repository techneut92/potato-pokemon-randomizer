//! Random moves learnt by level and as eggs, and damaging moves ordered by power.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - "Evolution Moves for All Pokémon" only adds evolution moves (moves at level 0) in Gen 7 and later, the games
//!   that have them; the original adds them in whatever game the setting reaches.
//!
//! Alternate formes get learnsets of their own; cosmetic formes learn their base species' new moves.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, GameData, LevelUpMove, Move, MoveCategory, MoveId, STRUGGLE, Species, Type};
use ppr_settings::{MovesetMode, MovesetSettings};

use super::common::{BANNED_FOR_DAMAGING, is_good_damaging, perfect_accuracy, strength};
use crate::choice::{below, shuffle, unit};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Pokémon Movesets";

/// How far the average power of a type's moves may be from that of all types.
const POWER_SPREAD: f64 = 0.25;
/// The limit on extra moves added to balance a type.
const MAX_BALANCING: usize = 10_000;

/// Randomizes movesets according to [`MovesetSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MovesetRandomizer;

impl Step<RunData> for MovesetRandomizer {
    fn id(&self) -> u64 {
        step_ids::MOVESETS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.movesets;
        (randomizes(s.mode) || s.reorder_damaging) && !data.game.learnsets.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.movesets.clone();
        let game = &mut data.game;
        if randomizes(s.mode) {
            let pools = Pools::new(game, s.no_game_breaking, &mut rng);
            let learnsets = randomize_learnsets(game, &s, &pools, &mut rng);
            let egg_moves = randomize_egg_moves(game, &s, &pools, &mut rng);
            game.learnsets = learnsets;
            game.egg_moves = egg_moves;
        }
        if s.reorder_damaging {
            order_damaging_moves(game, &mut rng);
        }
        Ok(())
    }
}

/// Random movesets: not with Metronome Only, which [`super::MetronomeOnlyMode`] applies after the trainers.
fn randomizes(mode: MovesetMode) -> bool {
    matches!(mode, MovesetMode::RandomPreferSameType | MovesetMode::RandomCompletely)
}

/// The moves to pick from, all and damaging, overall and by type. A move may be in a
/// type's list more than once, to bring the type's average power near that of all types.
struct Pools<'a> {
    all: Vec<&'a Move>,
    damaging: Vec<&'a Move>,
    by_type: BTreeMap<Type, Vec<&'a Move>>,
    damaging_by_type: BTreeMap<Type, Vec<&'a Move>>,
}

impl<'a> Pools<'a> {
    fn new(game: &'a GameData, no_game_breaking: bool, rng: &mut StepRng) -> Self {
        let perfect_accuracy = perfect_accuracy(game);
        let banned = |n: MoveId| {
            n == STRUGGLE
                || game.hm_moves.contains(&n)
                || game.banned_level_up_moves.contains(&n)
                || (no_game_breaking && game.game_breaking_moves.contains(&n))
        };
        let mut pools = Self {
            all: Vec::new(),
            damaging: Vec::new(),
            by_type: BTreeMap::new(),
            damaging_by_type: BTreeMap::new(),
        };
        for mv in game.moves.iter().filter(|m| !banned(m.number)) {
            pools.all.push(mv);
            if let Some(t) = mv.move_type {
                pools.by_type.entry(t).or_default().push(mv);
            }
            if !BANNED_FOR_DAMAGING.contains(&mv.number) && is_good_damaging(mv, perfect_accuracy) {
                pools.damaging.push(mv);
                if let Some(t) = mv.move_type {
                    pools.damaging_by_type.entry(t).or_default().push(mv);
                }
            }
        }
        pools.balance(rng);
        pools
    }

    /// Adds moves to the types whose average power is too far from that of all types.
    fn balance(&mut self, rng: &mut StepRng) {
        if self.by_type.is_empty() {
            return;
        }
        let averages: BTreeMap<Type, f64> = self
            .by_type
            .iter()
            .map(|(&t, moves)| {
                // An integer sum, dropping fractions at each step.
                let sum = moves
                    .iter()
                    .filter(|m| m.power > 0)
                    .fold(0.0, |sum: f64, m| (sum + strength(m)).trunc());
                #[expect(clippy::cast_precision_loss, reason = "a few hundred moves")]
                let average = sum / moves.len() as f64;
                (t, average)
            })
            .collect();
        #[expect(clippy::cast_precision_loss, reason = "at most 18 types")]
        let overall = averages.values().sum::<f64>() / averages.len() as f64;
        let (min, max) = (overall * (1.0 - POWER_SPREAD), overall * (1.0 + POWER_SPREAD));
        for (t, mut average) in averages {
            let Some(moves) = self.by_type.get_mut(&t) else {
                continue;
            };
            for too_weak in [true, false] {
                let mut picked: Vec<MoveId> = Vec::new();
                for _ in 0..MAX_BALANCING {
                    if (too_weak && average >= min) || (!too_weak && average <= max) {
                        break;
                    }
                    let mut candidates: Vec<&Move> = moves
                        .iter()
                        .copied()
                        .filter(|m| {
                            if too_weak {
                                strength(m) > average
                            } else {
                                strength(m) < average
                            }
                        })
                        .collect();
                    if candidates.is_empty() {
                        break;
                    }
                    if candidates.iter().all(|m| picked.contains(&m.number)) {
                        picked.clear();
                    } else {
                        candidates.retain(|m| !picked.contains(&m.number));
                    }
                    let extra = candidates[below(rng, candidates.len())];
                    #[expect(clippy::cast_precision_loss, reason = "a few hundred moves")]
                    let n = moves.len() as f64;
                    average = (average * n + strength(extra)) / (n + 1.0);
                    moves.push(extra);
                    picked.push(extra.number);
                }
            }
        }
    }
}

/// Whether `list` has a move not in `used`.
fn has_unused(list: &[&Move], used: &[MoveId]) -> bool {
    list.iter().any(|m| !used.contains(&m.number))
}

/// The type-themed choice: a type to prefer for the next move, or `None` for any.
fn preferred_type(sp: &Species, rng: &mut StepRng) -> Option<Type> {
    let picked = unit(rng);
    match (sp.primary_type, sp.secondary_type) {
        // Normal and another type: 10% Normal, 30% the other.
        (Type::Normal, Some(other)) | (other, Some(Type::Normal)) => {
            if picked < 0.1 {
                Some(Type::Normal)
            } else if picked < 0.4 {
                Some(other)
            } else {
                None
            }
        }
        (primary, Some(secondary)) => {
            if picked < 0.2 {
                Some(primary)
            } else if picked < 0.4 {
                Some(secondary)
            } else {
                None
            }
        }
        (primary, None) => (picked < 0.4).then_some(primary),
    }
}

/// The species' moves so far, and how the next one is chosen.
struct Picker<'p, 'a> {
    pools: &'p Pools<'a>,
    species: &'p Species,
    type_themed: bool,
    /// Attack over the sum of Attack and Special Attack: the chance of a physical damaging move.
    attack_ratio: f64,
    learnt: Vec<MoveId>,
}

impl<'p, 'a> Picker<'p, 'a> {
    fn new(pools: &'p Pools<'a>, species: &'p Species, type_themed: bool) -> Self {
        let (attack, special) = (f64::from(species.stats.attack), f64::from(species.stats.sp_attack));
        Self {
            pools,
            species,
            type_themed,
            attack_ratio: attack / (attack + special),
            learnt: Vec::new(),
        }
    }

    /// The next move: a damaging one if asked (of the species' stronger attacking category, when
    /// possible), of the preferred type if any, and not learnt yet.
    fn next(&mut self, damaging: bool, rng: &mut StepRng) -> MoveId {
        let preferred = if self.type_themed {
            preferred_type(self.species, rng)
        } else {
            None
        };
        let free = |list: &[&Move]| has_unused(list, &self.learnt);
        let mut list: &[&Move] = &self.pools.all;
        let filtered: Vec<&Move>;
        if damaging {
            let typed = preferred.and_then(|t| self.pools.damaging_by_type.get(&t));
            if let Some(typed) = typed.filter(|l| free(l)) {
                list = typed;
            } else if free(&self.pools.damaging) {
                list = &self.pools.damaging;
            }
            let category = if unit(rng) < self.attack_ratio {
                MoveCategory::Physical
            } else {
                MoveCategory::Special
            };
            filtered = list.iter().copied().filter(|m| m.category == category).collect();
            if !filtered.is_empty() && free(&filtered) {
                list = &filtered;
            }
        } else if let Some(typed) = preferred.and_then(|t| self.pools.by_type.get(&t)).filter(|l| free(l)) {
            list = typed;
        }
        let mut mv = list[below(rng, list.len())].number;
        while self.learnt.contains(&mv) {
            mv = list[below(rng, list.len())].number;
        }
        self.learnt.push(mv);
        mv
    }
}

/// `percent` of `count`, rounded half up.
fn good_damaging_count(s: &MovesetSettings, count: usize) -> usize {
    let share = f64::from(s.good_damaging_percent.unwrap_or(0)) / 100.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "at most a moveset's length"
    )]
    let n = (share * count as f64 + 0.5).floor() as usize;
    n
}

/// New level-up learnsets.
fn randomize_learnsets(
    game: &GameData,
    s: &MovesetSettings,
    pools: &Pools<'_>,
    rng: &mut StepRng,
) -> BTreeMap<DexNumber, Vec<LevelUpMove>> {
    let type_themed = s.mode == MovesetMode::RandomPreferSameType;
    let starting = s.guaranteed_level_one_moves.filter(|_| game.four_starting_moves);
    let evolution_moves = s.evolution_moves_for_all && game.identity.generation.number() >= 7;
    let mut learnsets = game.learnsets.clone();
    for (&n, moves) in &mut learnsets {
        let Some(species) = game.species(n) else { continue };
        if let Some(count) = starting {
            let at_one = moves.iter().filter(|m| m.level == 1).count();
            for _ in at_one..usize::from(count) {
                moves.insert(0, LevelUpMove::new(1, 0));
            }
        }
        if moves.is_empty() || species.forme.cosmetic {
            continue;
        }
        // Every species gets a move on evolving, to be randomized with the rest.
        if evolution_moves && moves[0].level != 0 {
            moves.insert(0, LevelUpMove::new(0, 0));
        }
        // The last move at level 1, which is to be a damaging one: index 0 if there is none.
        let mut first_attack = usize::from(moves[0].level != 1);
        while first_attack < moves.len() && moves[first_attack].level == 1 {
            first_attack += 1;
        }
        first_attack = first_attack.saturating_sub(1);

        let mut good_left = good_damaging_count(s, moves.len());
        let mut picker = Picker::new(pools, species, type_themed);
        let mut attack = 0;
        for i in 0..moves.len() {
            let mv = picker.next(i == first_attack || good_left > 0, rng);
            if i == first_attack {
                attack = mv;
            } else {
                good_left = good_left.saturating_sub(1);
            }
        }
        let mut learnt = picker.learnt;
        shuffle(rng, &mut learnt);
        if let Some(at) = learnt.iter().position(|&m| m == attack) {
            learnt.swap(at, first_attack);
        }
        for (i, (slot, mv)) in moves.iter_mut().zip(learnt).enumerate() {
            slot.move_id = mv;
            if i == first_attack {
                slot.level = 1;
            }
        }
    }
    // Cosmetic formes learn their base species' moves, at their own levels.
    for (forme, base) in cosmetic_formes(game) {
        let base_moves: Vec<MoveId> = learnsets
            .get(&base)
            .map(|m| m.iter().map(|m| m.move_id).collect())
            .unwrap_or_default();
        if let Some(moves) = learnsets.get_mut(&forme) {
            for (slot, mv) in moves.iter_mut().zip(base_moves) {
                slot.move_id = mv;
            }
        }
    }
    learnsets
}

/// The cosmetic formes and their base species.
fn cosmetic_formes(game: &GameData) -> Vec<(DexNumber, DexNumber)> {
    game.alt_formes()
        .filter(|sp| sp.forme.cosmetic)
        .filter_map(|sp| Some((sp.number, sp.conceptual_base()?)))
        .collect()
}

/// New egg moves.
fn randomize_egg_moves(
    game: &GameData,
    s: &MovesetSettings,
    pools: &Pools<'_>,
    rng: &mut StepRng,
) -> BTreeMap<DexNumber, Vec<MoveId>> {
    let type_themed = s.mode == MovesetMode::RandomPreferSameType;
    let mut egg_moves = game.egg_moves.clone();
    for (&n, moves) in &mut egg_moves {
        let Some(species) = game.species(n) else { continue };
        if species.forme.cosmetic {
            continue;
        }
        let mut good_left = good_damaging_count(s, moves.len());
        let mut picker = Picker::new(pools, species, type_themed);
        for _ in 0..moves.len() {
            picker.next(good_left > 0, rng);
            good_left = good_left.saturating_sub(1);
        }
        let mut learnt = picker.learnt;
        shuffle(rng, &mut learnt);
        *moves = learnt;
    }
    for (forme, base) in cosmetic_formes(game) {
        if let (Some(base_moves), true) = (egg_moves.get(&base).cloned(), egg_moves.contains_key(&forme)) {
            egg_moves.insert(forme, base_moves);
        }
    }
    egg_moves
}

/// The damaging moves (power above 1) of each learnset swap places so they
/// come from weakest to strongest, ties in random order; moves on evolving stay.
fn order_damaging_moves(game: &mut GameData, rng: &mut StepRng) {
    let by_number: BTreeMap<MoveId, &Move> = game.moves.iter().map(|m| (m.number, m)).collect();
    for moves in game.learnsets.values_mut() {
        let slots: Vec<usize> = (0..moves.len())
            .filter(|&i| moves[i].level != 0 && by_number.get(&moves[i].move_id).is_some_and(|m| m.power > 1))
            .collect();
        let mut damaging: Vec<&Move> = slots.iter().map(|&i| by_number[&moves[i].move_id]).collect();
        shuffle(rng, &mut damaging);
        damaging.sort_by(|a, b| strength(a).total_cmp(&strength(b)));
        for (&i, mv) in slots.iter().zip(damaging) {
            moves[i].move_id = mv.number;
        }
    }
}

#[cfg(test)]
mod tests;
