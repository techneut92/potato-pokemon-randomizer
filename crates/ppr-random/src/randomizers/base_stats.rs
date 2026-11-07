//! Shuffles or randomizes species' base stats, keeping each one's total. Gen 1 has five stats, Special being one.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - Copying stats up to Shedinja copies all but HP, rather than failing (no setting combination reaches it).
//! - A random spread whose total is below the minimums gives the stats nothing beyond them, rather than failing.
//! - Fixed: in Gen 1, random stats for an evolution with a lower total than its pre-evolution keep each stat at
//!   least at its minimum, as in the other generations, instead of failing when a stat would go below 0.
//!
//! Shuffled stat orders go to alternate and cosmetic formes too. Random stats leave alternate formes as they are,
//! as in the original, and copy the spread to cosmetic formes.
//!
//! With "Follow Mega Evolutions" and "Follow Evolutions", Mega Evolutions count as evolutions of their base species
//! (`copy_up::plan_with_megas`): random stats copy the base species' spread to them, or add their extra total at
//! random with "Randomize Added Stats on Evolution" and for Charizard's and Mewtwo's two Mega Evolutions, which are
//! split evolutions; shuffled stats give those two an order of their own, as split evolutions are not copied there.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, GameData, Species};
use ppr_settings::{BaseStatMode, BaseStatSettings};

use super::common::SpeciesSet;
use super::copy_up::{self, CopyUp};
use super::stat_points::{SHEDINJA, current_ratios, set_ratios, stats_of};
use crate::choice::{shuffle, unit};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Base Statistics";

/// The least a random spread gives a stat.
const MIN_HP: f64 = 20.0;
const MIN_NON_HP_STAT: f64 = 10.0;

/// Shuffles or randomizes base stats according to [`BaseStatSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BaseStatRandomizer;

impl Step<RunData> for BaseStatRandomizer {
    fn id(&self) -> u64 {
        step_ids::BASE_STATS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.base_stats.mode != BaseStatMode::Unchanged
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.base_stats.clone();
        match s.mode {
            BaseStatMode::Unchanged => Ok(()),
            BaseStatMode::Shuffle => shuffle_stats(&mut data.game, &s, &mut rng),
            BaseStatMode::Random => randomize_stats(&mut data.game, &s, &mut rng),
        }
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

fn species_mut(game: &mut GameData, n: DexNumber) -> Result<&mut Species, RandomizeError> {
    let i = game
        .species
        .binary_search_by_key(&n, |s| s.number)
        .map_err(|_| impossible(format!("no species {n}")))?;
    Ok(&mut game.species[i])
}

fn species(game: &GameData, n: DexNumber) -> Result<&Species, RandomizeError> {
    game.species(n).ok_or_else(|| impossible(format!("no species {n}")))
}

/// The game has Gen 1's five stats.
fn is_gen1(game: &GameData) -> bool {
    game.identity.generation.number() == 1
}

/// Spreads the species' stats by `ratios`: keeps the species' total.
fn apply_ratios(game: &mut GameData, n: DexNumber, ratios: &[f64]) -> Result<(), RandomizeError> {
    let gen1 = is_gen1(game);
    let sp = species_mut(game, n)?;
    if set_ratios(sp, ratios, gen1) {
        Ok(())
    } else {
        Err(impossible(format!("the stats of {} don't fit its total", sp.name)))
    }
}

fn all_species(game: &GameData) -> SpeciesSet {
    game.species.iter().map(|s| s.number).collect()
}

/// Each basic species gets a random order of its stats (five in Gen 1); evolutions copy theirs.
fn shuffle_stats(game: &mut GameData, s: &BaseStatSettings, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let gen1 = is_gen1(game);
    let count = if gen1 { 5 } else { 6 };
    let options = copy_up::Options {
        evolution_sanity: s.follow_evolutions,
        copy_split_evos: false,
    };
    let mut orders: BTreeMap<DexNumber, Vec<usize>> = BTreeMap::new();
    for step in copy_up::plan_with_megas(game, &all_species(game), options, s.follow_megas) {
        match step {
            CopyUp::Basic(n) => {
                let mut order: Vec<usize> = (0..count).collect();
                shuffle(rng, &mut order);
                orders.insert(n, order);
            }
            CopyUp::Evolved { from, to, .. }
            | CopyUp::AltForme { base: from, forme: to }
            | CopyUp::Cosmetic { base: from, forme: to } => {
                let order = orders[&from].clone();
                orders.insert(to, order);
            }
        }
    }
    for (n, mut order) in orders {
        let stats: Vec<f64> = stats_of(species(game, n)?.stats, gen1)
            .into_iter()
            .map(f64::from)
            .collect();
        let ratios: Vec<f64> = if n == SHEDINJA && !gen1 {
            // Shedinja keeps HP 1: whatever would go to HP goes where HP would have come from.
            let first = order[0];
            for o in &mut order[1..] {
                if *o == 0 {
                    *o = first;
                }
            }
            order[1..].iter().map(|&o| stats[o]).collect()
        } else {
            order.iter().map(|&o| stats[o]).collect()
        };
        apply_ratios(game, n, &ratios)?;
    }
    Ok(())
}

/// Random base stats, following evolutions as chosen.
fn randomize_stats(game: &mut GameData, s: &BaseStatSettings, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let options = copy_up::Options {
        evolution_sanity: s.follow_evolutions,
        copy_split_evos: true,
    };
    for step in copy_up::plan_with_megas(game, &all_species(game), options, s.follow_megas) {
        match step {
            CopyUp::Basic(n) => randomize_within_bst(game, n, rng)?,
            CopyUp::Evolved { from, to, split, .. } => {
                if split || s.random_evolution_stats {
                    assign_for_evolution(game, from, to, rng)?;
                } else {
                    copy_up_evolution(game, from, to)?;
                }
            }
            // As in the original, alternate formes keep their stats.
            CopyUp::AltForme { .. } => {}
            CopyUp::Cosmetic { base, forme } => copy_up_evolution(game, base, forme)?,
        }
    }
    Ok(())
}

/// Random weights for `count` stats, summing to 1.
fn weights(rng: &mut StepRng, count: usize) -> Vec<f64> {
    let w: Vec<f64> = (0..count).map(|_| unit(rng)).collect();
    let total: f64 = w.iter().sum();
    w.into_iter().map(|x| x / total).collect()
}

/// The total above the minimums spread by random weights; five stats in Gen 1.
fn randomize_within_bst(game: &mut GameData, n: DexNumber, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let gen1 = is_gen1(game);
    let bst = f64::from(species(game, n)?.bst());
    let ratios: Vec<f64> = if n == SHEDINJA && !gen1 {
        let left = (bst - MIN_NON_HP_STAT * 5.0).max(0.0);
        weights(rng, 5)
            .into_iter()
            .map(|w| w * left + MIN_NON_HP_STAT)
            .collect()
    } else {
        let count: u8 = if gen1 { 5 } else { 6 };
        let left = (bst - (MIN_HP + MIN_NON_HP_STAT * f64::from(count - 1))).max(0.0);
        let mut ratios: Vec<f64> = weights(rng, usize::from(count))
            .into_iter()
            .map(|w| w * left + MIN_NON_HP_STAT)
            .collect();
        ratios[0] += MIN_HP - MIN_NON_HP_STAT;
        ratios
    };
    apply_ratios(game, n, &ratios)
}

/// Rounds to the nearest whole number, halves up.
fn round_half_up(x: f64) -> f64 {
    (x + 0.5).floor()
}

/// The evolution's stats are its pre-evolution's plus its extra total, spread by random weights; five stats in
/// Gen 1.
fn assign_for_evolution(
    game: &mut GameData,
    from: DexNumber,
    to: DexNumber,
    rng: &mut StepRng,
) -> Result<(), RandomizeError> {
    let from_sp = species(game, from)?;
    let to_sp = species(game, to)?;
    let gen1 = is_gen1(game);
    let diff = f64::from(to_sp.bst()) - f64::from(from_sp.bst());
    let from_stats: Vec<f64> = stats_of(from_sp.stats, gen1).into_iter().map(f64::from).collect();
    let ratios: Vec<f64> = if to == SHEDINJA && !gen1 {
        weights(rng, 5)
            .into_iter()
            .zip(&from_stats[1..])
            .map(|(w, &stat)| (stat + round_half_up(w * diff)).max(0.0))
            .collect()
    } else {
        weights(rng, from_stats.len())
            .into_iter()
            .zip(from_stats)
            .enumerate()
            .map(|(i, (w, stat))| {
                // Stats can get too low when the total goes down.
                let min = if i == 0 { MIN_HP } else { MIN_NON_HP_STAT };
                (stat + round_half_up(w * diff)).max(min)
            })
            .collect()
    };
    apply_ratios(game, to, &ratios)
}

/// Copies random stats up an evolution: the evolution keeps its total, spread like its pre-evolution's stats.
fn copy_up_evolution(game: &mut GameData, from: DexNumber, to: DexNumber) -> Result<(), RandomizeError> {
    let gen1 = is_gen1(game);
    let from_sp = species(game, from)?;
    let mut ratios = current_ratios(from_sp, gen1);
    if to == SHEDINJA && from_sp.number != SHEDINJA && !gen1 {
        ratios.remove(0);
    }
    apply_ratios(game, to, &ratios)
}

#[cfg(test)]
mod tests;
