//! Random evolutions, and adjusting evolution levels to new base stat totals.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//!
//! The base species get random evolutions; the links of Burmy to Wormadam's formes go with the old evolutions.
//! Where alternate formes have evolutions of their own (Gen 7), the alternate formes that are not only cosmetic
//! join them, as in the original whatever "Allow Alternate Formes" says (it only matters to Follow Mega
//! Evolutions). No species evolves into an alternate forme that reverts for the player, one that needs its
//! species' ability while abilities stay or, with "No Irregular Alt Formes", an irregular one. Species outside the
//! pool (cosmetic formes such as the Own Tempo Rockruff) keep their evolutions.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, Evolution, EvolutionType, GameData, Species};
use ppr_settings::{EvolutionMode, EvolutionSettings};

use super::common::{SpeciesSet, banned_formes, bst, pool_with_formes, similar_strength};
use crate::choice::pick;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Evolutions";
const ADJUST_LABEL: &str = "Evolution Levels";

/// How many times a full set of evolutions is tried before giving up.
const MAX_TRIES: usize = 1000;
/// The stage limit without "Limit Evolutions to Three Stages", and the limit of "Limit Evolutions to Three Stages".
const DEFAULT_STAGE_LIMIT: usize = 10;
const THREE_STAGES: usize = 3;
/// Eevee: with Same Typing, its evolutions share a type with the old ones instead.
const EEVEE: DexNumber = 133;

/// Randomizes evolutions according to [`EvolutionSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct EvolutionRandomizer;

impl Step<RunData> for EvolutionRandomizer {
    fn id(&self) -> u64 {
        step_ids::EVOLUTIONS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.evolutions.mode != EvolutionMode::Unchanged
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.evolutions.clone();
        let hint = data.settings.exp_curves.standardize.is_none();
        let banned = banned_formes(
            &data.game,
            true,
            data.settings.abilities.mode != ppr_settings::AbilityMode::Unchanged,
            data.settings.general.no_irregular_alt_formes,
        );
        randomize(&mut data.game, &s, &banned, &mut rng, hint)
    }
}

/// Sets evolution levels to fit the new base stat totals ("Adjust Evolution Levels").
#[derive(Debug, Clone, Copy, Default)]
pub struct EvolutionLevelAdjuster;

impl Step<RunData> for EvolutionLevelAdjuster {
    fn id(&self) -> u64 {
        step_ids::EVOLUTION_LEVELS
    }

    fn label(&self) -> &'static str {
        ADJUST_LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.evolutions.adjust_levels
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        ctx.data.game.adjust_evolution_levels();
        Ok(())
    }
}

/// One randomization: the options, and the evolutions chosen so far.
struct Inner<'a> {
    game: &'a GameData,
    s: &'a EvolutionSettings,
    every_level: bool,
    stage_limit: usize,
    pool: SpeciesSet,
    /// The species nothing evolves into.
    banned: &'a SpeciesSet,
    original: BTreeMap<DexNumber, Vec<Evolution>>,
    chosen: Vec<Evolution>,
    /// The chosen evolutions by species, both ways.
    next: BTreeMap<DexNumber, Vec<DexNumber>>,
    previous: BTreeMap<DexNumber, Vec<DexNumber>>,
}

/// Random evolutions: up to [`MAX_TRIES`] tries at a full set of evolutions.
fn randomize(
    game: &mut GameData,
    s: &EvolutionSettings,
    banned: &SpeciesSet,
    rng: &mut StepRng,
    suggest_exp_curves: bool,
) -> Result<(), RandomizeError> {
    let every_level = s.mode == EvolutionMode::RandomEveryLevel;
    let pool: SpeciesSet = pool_with_formes(game, game.formes.evolutions);
    let mut original: BTreeMap<DexNumber, Vec<Evolution>> = pool.iter().map(|&n| (n, Vec::new())).collect();
    for e in &game.evolutions {
        if let Some(list) = original.get_mut(&e.from) {
            list.push(*e);
        }
    }
    let mut inner = Inner {
        game,
        s,
        every_level,
        // These options do not combine with every level; the UI disables them.
        stage_limit: if s.limit_three_stages && !every_level {
            THREE_STAGES
        } else {
            DEFAULT_STAGE_LIMIT
        },
        pool,
        banned,
        original,
        chosen: Vec::new(),
        next: BTreeMap::new(),
        previous: BTreeMap::new(),
    };
    for _ in 0..MAX_TRIES {
        if inner.try_once(rng) {
            let chosen = std::mem::take(&mut inner.chosen);
            let pool = std::mem::take(&mut inner.pool);
            game.evolutions.retain(|e| !pool.contains(&e.from));
            game.evolutions.extend(chosen);
            return Ok(());
        }
    }
    let mut message = format!("could not randomize evolutions in {MAX_TRIES} tries");
    if suggest_exp_curves {
        message.push_str("; try \"Standardize EXP Curves\"");
    }
    Err(RandomizeError::Impossible { step: LABEL, message })
}

impl Inner<'_> {
    fn species(&self, n: DexNumber) -> Option<&Species> {
        self.game.species(n)
    }

    fn next_of(&self, n: DexNumber) -> &[DexNumber] {
        self.next.get(&n).map_or(&[], Vec::as_slice)
    }

    fn previous_of(&self, n: DexNumber) -> &[DexNumber] {
        self.previous.get(&n).map_or(&[], Vec::as_slice)
    }

    /// One try: every species' evolutions in national order, or `false` when one has
    /// nothing left to evolve into.
    fn try_once(&mut self, rng: &mut StepRng) -> bool {
        self.chosen.clear();
        self.next.clear();
        self.previous.clear();
        let pool: Vec<DexNumber> = self.pool.iter().copied().collect();
        for from in pool {
            let olds: Vec<Evolution> = if self.every_level {
                // A placeholder: every species once.
                vec![Evolution::new(from, from, EvolutionType::Level, 0)]
            } else {
                self.original[&from].clone()
            };
            for old in olds {
                let possible = self.possible(from, &old);
                if possible.is_empty() {
                    return false;
                }
                let picked = if self.s.similar_strength && !self.every_level {
                    similar_strength(rng, self.game, &possible, bst(self.game, old.to))
                } else {
                    pick(rng, &possible).unwrap_or(from)
                };
                let new = if self.every_level {
                    Evolution::new(from, picked, EvolutionType::Level, 1)
                } else {
                    Evolution { to: picked, ..old }
                };
                self.chosen.push(new);
                self.next.entry(from).or_default().push(picked);
                self.previous.entry(picked).or_default().push(from);
            }
        }
        true
    }

    /// The species `from` may evolve into in place of `old`.
    fn possible(&self, from: DexNumber, old: &Evolution) -> SpeciesSet {
        let Some(from_sp) = self.species(from) else {
            return SpeciesSet::new();
        };
        let type_match = if self.s.same_typing {
            if from == EEVEE && !self.every_level {
                self.species(old.to)
            } else {
                Some(from_sp)
            }
        } else {
            None
        };
        self.pool
            .iter()
            .copied()
            .filter(|&to| {
                let Some(to_sp) = self.species(to) else { return false };
                to != from
                    && !self.banned.contains(&to)
                    && to_sp.growth_curve == from_sp.growth_curve
                    && !self.next_of(from).contains(&to)
                    && (!self.s.no_convergence || self.previous_of(to).is_empty())
                    && (!self.s.force_change || !self.original[&from].iter().any(|e| e.to == to))
                    && (!self.s.force_growth || self.every_level || to_sp.bst() > from_sp.bst())
                    && type_match.is_none_or(|m| shares_type(to_sp, m))
                    && (self.every_level || (!self.creates_cycle(from, to) && !self.breaks_stage_limit(from, to)))
            })
            .collect()
    }

    /// Whether `from` evolving into `to` closes a loop. Without every level the chosen
    /// evolutions never loop, so that is `to` leading to `from`.
    fn creates_cycle(&self, from: DexNumber, to: DexNumber) -> bool {
        let mut seen = BTreeSet::new();
        let mut stack = vec![to];
        while let Some(n) = stack.pop() {
            if n == from {
                return true;
            }
            if seen.insert(n) {
                stack.extend(self.next_of(n));
            }
        }
        false
    }

    /// Whether `from` evolving into `to` makes a line longer than the stage limit.
    fn breaks_stage_limit(&self, from: DexNumber, to: DexNumber) -> bool {
        let before = self.pre_evolutions(from, 0);
        let after = self.evolutions(to, 0);
        before + after + 2 > self.stage_limit
    }

    /// How many stages lead to `n`, counted up to the stage limit.
    fn pre_evolutions(&self, n: DexNumber, depth: usize) -> usize {
        let pre = self.previous_of(n);
        if pre.is_empty() {
            return 0;
        }
        if depth + 1 == self.stage_limit {
            return 1;
        }
        pre.iter()
            .map(|&p| self.pre_evolutions(p, depth + 1) + 1)
            .max()
            .unwrap_or_default()
    }

    /// How many stages follow `n`, counted up to the stage limit; a species that will get
    /// an evolution later counts one.
    fn evolutions(&self, n: DexNumber, depth: usize) -> usize {
        let next = self.next_of(n);
        if next.is_empty() {
            return usize::from(self.original.get(&n).is_some_and(|o| !o.is_empty()));
        }
        if depth + 1 == self.stage_limit {
            return 1;
        }
        next.iter()
            .map(|&t| self.evolutions(t, depth + 1) + 1)
            .max()
            .unwrap_or_default()
    }
}

/// Whether `a` and `b` share a type.
fn shares_type(a: &Species, b: &Species) -> bool {
    a.has_type(b.primary_type) || b.secondary_type.is_some_and(|t| a.has_type(t))
}

#[cfg(test)]
mod tests;
