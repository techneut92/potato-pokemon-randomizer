//! New base stat totals, the stats keeping their proportions.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005); species and shuffle groups are walked in national
//!   order.
//! - Shuffled totals are the ones at the start of this step, so after "Update Base Stats to Generation", not the
//!   totals as loaded.
//! - A buff or nerf of at most 0% changes nothing.
//! - Shuffling following evolutions keeps a species' own total when its line has no match for it (an evolution
//!   whose donor does not evolve, or a species basic in its group but not in the game), instead of failing.
//!
//! Alternate formes: buffs and nerfs give a forme its base species' factor. Shuffling shuffles the base species,
//! then gives each alternate forme its base species' new total times its old ratio to it (Wishiwashi's School Form
//! 1.8 times, as its own ratio would nearly always reach the highest total). Fully random totals skip cosmetic
//! formes. Cosmetic formes always get their base species' new total. A Mega Evolution then gets its base species'
//! new total plus 100, whatever the mode.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, GameData};
use ppr_settings::{BstMode, BstSettings};

use super::common::{SpeciesSet, bst};
use super::copy_up::{self, CopyUp};
use super::stat_points::{max_bst, set_bst};
use crate::choice::{below, shuffle, unit};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Base Stat Totals";

/// Wishiwashi, whose School Form gets this ratio to it when totals are shuffled.
const WISHIWASHI: DexNumber = 746;
const WISHIWASHI_SCHOOL_MULTIPLIER: f64 = 1.8;

/// Sunkern's and Arceus's totals: random totals are at least the first and below the second.
const SUNKERN_BST: u32 = 180;
const ARCEUS_BST: u32 = 720;

/// What a Mega Evolution's total is above its base species'.
const MEGA_BST_BOOST: u32 = 100;

/// Randomizes base stat totals according to [`BstSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BstRandomizer;

impl Step<RunData> for BstRandomizer {
    fn id(&self) -> u64 {
        step_ids::BSTS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.bsts.mode != BstMode::Unchanged
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.bsts.clone();
        let game = &mut data.game;
        match s.mode {
            BstMode::Unchanged => Ok(()),
            BstMode::BuffNerf => buff_nerf(game, &s, &mut rng),
            BstMode::Shuffle => shuffle_bsts(game, &s, &mut rng),
            BstMode::Random => random_bsts(game, &mut rng),
        }
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// Sets the total of species `n`, the stats keeping their proportions.
fn apply_bst(game: &mut GameData, n: DexNumber, total: u32) -> Result<(), RandomizeError> {
    let i = game
        .species
        .binary_search_by_key(&n, |s| s.number)
        .map_err(|_| impossible(format!("no species {n}")))?;
    let gen1 = game.identity.generation.number() == 1;
    let sp = &mut game.species[i];
    if set_bst(sp, total, gen1) {
        Ok(())
    } else {
        Err(impossible(format!("the stats of {} can't total {total}", sp.name)))
    }
}

/// Gives a Mega Evolution its base species' total plus [`MEGA_BST_BOOST`]; other formes stay.
fn mega_boost(game: &mut GameData, base: DexNumber, forme: DexNumber) -> Result<(), RandomizeError> {
    if game.species(forme).is_some_and(|sp| sp.forme.mega) {
        apply_bst(game, forme, bst(game, base) + MEGA_BST_BOOST)?;
    }
    Ok(())
}

fn all_species(game: &GameData) -> SpeciesSet {
    game.species.iter().map(|s| s.number).collect()
}

/// Buff or nerf by percentage: each basic species' total times a random factor; evolutions take
/// their pre-evolution's factor when following evolutions.
fn buff_nerf(game: &mut GameData, s: &BstSettings, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let max = f64::from(s.max_buff_nerf_percent) / 100.0;
    let options = copy_up::Options {
        evolution_sanity: s.follow_evolutions,
        copy_split_evos: true,
    };
    let max_total = max_bst(game.identity.generation.number() == 1);
    let mut factors: BTreeMap<DexNumber, f64> = BTreeMap::new();
    for step in copy_up::plan(game, &all_species(game), options) {
        let (n, factor) = match step {
            CopyUp::Basic(n) => (n, 1.0 - max + unit(rng) * 2.0 * max),
            CopyUp::Evolved { from, to, .. } | CopyUp::AltForme { base: from, forme: to } => (to, factors[&from]),
            CopyUp::Cosmetic { base, forme } => {
                factors.insert(forme, factors[&base]);
                apply_bst(game, forme, bst(game, base))?;
                continue;
            }
        };
        factors.insert(n, factor);
        // Truncated, at most the highest total.
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive, and capped below"
        )]
        let total = ((f64::from(bst(game, n)) * factor) as u32).min(max_total);
        apply_bst(game, n, total)?;
        if let CopyUp::AltForme { base, forme } = step {
            mega_boost(game, base, forme)?;
        }
    }
    Ok(())
}

/// Species of `group` that no species of the game evolves into.
fn basics(game: &GameData, group: &SpeciesSet) -> SpeciesSet {
    group
        .iter()
        .copied()
        .filter(|&n| !game.evolutions.iter().any(|e| e.to == n))
        .collect()
}

/// The shuffle groups: all species, split by the length of their evolution lines and by being
/// legendary.
fn shuffle_groups(game: &GameData, s: &BstSettings) -> Vec<SpeciesSet> {
    let all: SpeciesSet = game.base_species().map(|sp| sp.number).collect();
    let mut groups = vec![all.clone()];
    if s.follow_evolutions {
        // Each basic species' family, by how many stages follow it.
        let mut by_length: BTreeMap<usize, SpeciesSet> = BTreeMap::new();
        for n in basics(game, &all) {
            by_length
                .entry(game.stages_after(n))
                .or_default()
                .extend(game.family(n).into_iter().filter(|m| all.contains(m)));
        }
        groups = by_length.into_values().collect();
    }
    if s.separate_legendaries {
        groups = groups
            .into_iter()
            .flat_map(|g| {
                let (legendary, other): (SpeciesSet, SpeciesSet) = g.iter().partition(|&&n| game.is_legendary(n));
                [legendary, other]
            })
            .collect();
    }
    groups
}

/// Shuffles the totals within each shuffle group.
fn shuffle_bsts(game: &mut GameData, s: &BstSettings, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let before: BTreeMap<DexNumber, u32> = game.species.iter().map(|sp| (sp.number, u32::from(sp.bst()))).collect();
    for group in shuffle_groups(game, s) {
        shuffle_group(game, &group, s.follow_evolutions, &before, rng)?;
    }
    copy_shuffled_to_alt_formes(game, &before)
}

/// Gives the alternate formes their base species' shuffled total times their old ratio to it, cosmetic formes the
/// same total.
fn copy_shuffled_to_alt_formes(game: &mut GameData, before: &BTreeMap<DexNumber, u32>) -> Result<(), RandomizeError> {
    let options = copy_up::Options {
        evolution_sanity: true,
        copy_split_evos: true,
    };
    let max_total = max_bst(game.identity.generation.number() == 1);
    for step in copy_up::plan(game, &all_species(game), options) {
        let (base, forme) = match step {
            CopyUp::Basic(_) => continue,
            CopyUp::Evolved { to, .. } => match game.species(to).and_then(ppr_model::Species::conceptual_base) {
                Some(base) => (base, to),
                None => continue,
            },
            CopyUp::AltForme { base, forme } => (base, forme),
            CopyUp::Cosmetic { base, forme } => {
                apply_bst(game, forme, bst(game, base))?;
                continue;
            }
        };
        let multiplier = if base == WISHIWASHI {
            WISHIWASHI_SCHOOL_MULTIPLIER
        } else {
            f64::from(before[&forme]) / f64::from(before[&base].max(1))
        };
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "positive, and capped below"
        )]
        let total = ((f64::from(bst(game, base)) * multiplier) as u32).min(max_total);
        apply_bst(game, forme, total)?;
        mega_boost(game, base, forme)?;
    }
    Ok(())
}

/// Shuffles one group: basic species take the total of a random basic species of the group, and their
/// evolutions the totals of that species' evolutions.
fn shuffle_group(
    game: &mut GameData,
    group: &SpeciesSet,
    follow_evolutions: bool,
    before: &BTreeMap<DexNumber, u32>,
    rng: &mut StepRng,
) -> Result<(), RandomizeError> {
    let keys: Vec<DexNumber> = if follow_evolutions {
        basics(game, group).into_iter().collect()
    } else {
        group.iter().copied().collect()
    };
    let mut values = keys.clone();
    shuffle(rng, &mut values);
    let mut donors: BTreeMap<DexNumber, DexNumber> = keys.into_iter().zip(values).collect();
    let options = copy_up::Options {
        evolution_sanity: follow_evolutions,
        copy_split_evos: true,
    };
    for step in copy_up::plan(game, group, options) {
        let (n, donor) = match step {
            // The groups hold no alternate formes.
            CopyUp::AltForme { .. } | CopyUp::Cosmetic { .. } => continue,
            CopyUp::Basic(n) => (n, donors.get(&n).copied().unwrap_or(n)),
            CopyUp::Evolved { from, to, .. } => {
                // "Assumes lines are even": the first evolution of the pre-evolution's donor.
                let from_donor = donors.get(&from).copied().unwrap_or(from);
                let donor = game
                    .evolutions
                    .iter()
                    .find(|e| e.from == from_donor)
                    .map_or(to, |e| e.to);
                (to, donor)
            }
        };
        donors.insert(n, donor);
        apply_bst(game, n, before[&donor])?;
    }
    Ok(())
}

/// Fully random totals: every species gets a total from Sunkern's up to below Arceus's.
fn random_bsts(game: &mut GameData, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let range = usize::try_from(ARCEUS_BST - SUNKERN_BST).unwrap_or(1);
    for n in all_species(game) {
        if game.species(n).is_some_and(|sp| sp.forme.cosmetic) {
            continue;
        }
        let total = SUNKERN_BST + u32::try_from(below(rng, range)).unwrap_or_default();
        apply_bst(game, n, total)?;
    }
    // Cosmetic formes copy their base species' new total, Mega Evolutions add to it.
    let options = copy_up::Options {
        evolution_sanity: false,
        copy_split_evos: false,
    };
    for step in copy_up::plan(game, &all_species(game), options) {
        match step {
            CopyUp::Cosmetic { base, forme } => apply_bst(game, forme, bst(game, base))?,
            CopyUp::AltForme { base, forme } => mega_boost(game, base, forme)?,
            CopyUp::Basic(_) | CopyUp::Evolved { .. } => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
