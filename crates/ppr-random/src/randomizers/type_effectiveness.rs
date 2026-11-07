//! Type effectiveness: the type chart gets Gen 6's changes, or is randomized, kept by type identity, or inverted.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).

use std::collections::BTreeSet;

use ppr_model::{Effectiveness, Type, TypeChart};
use ppr_settings::{TypeEffectivenessMode, TypeEffectivenessSettings};

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Type Effectiveness";
const UPDATE_LABEL: &str = "Type Effectiveness Updates";

/// How often placing an effectiveness may fail before randomizing gives up.
const MAX_PLACEMENT_TRIES: usize = 10_000;
/// The number of swaps when keeping type identities.
const KEEP_IDENTITIES_SWAPS: usize = 10_000;
/// The effectivenesses that are balanced, also the order effectivenesses are placed in.
const PLACED: [Effectiveness; 3] = [Effectiveness::Zero, Effectiveness::Half, Effectiveness::Double];

/// Gives the type chart Gen 6's changes when [`TypeEffectivenessSettings::update`] is on.
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeEffectivenessUpdater;

impl Step<RunData> for TypeEffectivenessUpdater {
    fn id(&self) -> u64 {
        step_ids::TYPE_EFFECTIVENESS_UPDATES
    }

    fn label(&self) -> &'static str {
        UPDATE_LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.type_effectiveness.update && !data.game.type_chart.types().is_empty()
    }

    /// Gen 6's changes: Gen 1's Poison, Bug, Ghost and Ice fixes, or later Steel's lost Ghost and Dark resistances.
    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let game = &mut ctx.data.game;
        let changes: &[(Type, Type, Effectiveness)] = if game.identity.generation.number() == 1 {
            &[
                (Type::Poison, Type::Bug, Effectiveness::Neutral),
                (Type::Bug, Type::Poison, Effectiveness::Half),
                (Type::Ghost, Type::Psychic, Effectiveness::Double),
                (Type::Ice, Type::Fire, Effectiveness::Half),
            ]
        } else {
            &[
                (Type::Ghost, Type::Steel, Effectiveness::Neutral),
                (Type::Dark, Type::Steel, Effectiveness::Neutral),
            ]
        };
        for &(attacker, defender, e) in changes {
            game.type_chart.set(attacker, defender, e);
        }
        Ok(())
    }
}

/// Changes the type chart according to [`TypeEffectivenessSettings::mode`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeEffectivenessRandomizer;

impl Step<RunData> for TypeEffectivenessRandomizer {
    fn id(&self) -> u64 {
        step_ids::TYPE_EFFECTIVENESS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.type_effectiveness.mode != TypeEffectivenessMode::Unchanged
            && !data.game.type_chart.types().is_empty()
    }

    /// Randomizes, keeps identities of, or inverts the chart as chosen.
    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s: TypeEffectivenessSettings = data.settings.type_effectiveness.clone();
        let chart = &mut data.game.type_chart;
        match s.mode {
            TypeEffectivenessMode::Unchanged => {}
            TypeEffectivenessMode::Random => *chart = randomize(chart, false, &mut rng)?,
            TypeEffectivenessMode::RandomBalanced => *chart = randomize(chart, true, &mut rng)?,
            TypeEffectivenessMode::KeepIdentities => keep_identities(chart, &mut rng),
            TypeEffectivenessMode::Inverse => invert(chart, s.random_immunities, &mut rng)?,
        }
        Ok(())
    }
}

fn random_type(types: &[Type], rng: &mut StepRng) -> Type {
    types[below(rng, types.len())]
}

/// A random chart: as many of each effectiveness as before, placed on random neutral pairs;
/// balanced, no type gets more of one as attacker or defender than the most any type had.
fn randomize(old: &TypeChart, balanced: bool, rng: &mut StepRng) -> Result<TypeChart, RandomizeError> {
    let types = old.types().to_vec();
    let mut chart = TypeChart::neutral(&types);
    let count = |e| types.iter().map(|&a| old.when_attacking(a, e).len()).sum::<usize>();
    let most = |f: &dyn Fn(Type, Effectiveness) -> usize, e| types.iter().map(|&t| f(t, e)).max().unwrap_or(0);
    let mut tries = 0;
    for e in PLACED {
        let (most_attacking, most_defending) = (
            most(&|t, e| old.when_attacking(t, e).len(), e),
            most(&|t, e| old.when_defending(t, e).len(), e),
        );
        let mut left = count(e);
        while left > 0 && tries <= MAX_PLACEMENT_TRIES {
            let (attacker, defender) = (random_type(&types, rng), random_type(&types, rng));
            let valid = chart.get(attacker, defender) == Some(Effectiveness::Neutral)
                && (!balanced
                    || (chart.when_attacking(attacker, e).len() != most_attacking
                        && chart.when_defending(defender, e).len() != most_defending));
            if valid {
                chart.set(attacker, defender, e);
                left -= 1;
            }
            tries += 1;
        }
        if tries > MAX_PLACEMENT_TRIES {
            return Err(RandomizeError::Impossible {
                step: LABEL,
                message: "couldn't randomize type effectiveness".into(),
            });
        }
    }
    Ok(chart)
}

/// Keeping type identities: 10 000 swaps of part of two defending types' columns, each
/// only when both parts hold the same effectivenesses.
fn keep_identities(chart: &mut TypeChart, rng: &mut StepRng) {
    let types = chart.types().to_vec();
    let mut swaps = 0;
    while swaps < KEEP_IDENTITIES_SWAPS {
        let (a, b) = (random_type(&types, rng), random_type(&types, rng));
        let size = below(rng, types.len());
        let mut chunk = BTreeSet::new();
        while chunk.len() < size {
            chunk.insert(random_type(&types, rng));
        }
        let counts = |column: Type| {
            let mut c = [0usize; 4];
            for &t in &chunk {
                c[slot(chart.get(t, column))] += 1;
            }
            c
        };
        if counts(a) == counts(b) {
            for &t in &chunk {
                let (ea, eb) = (chart.get(t, a), chart.get(t, b));
                if let (Some(ea), Some(eb)) = (ea, eb) {
                    chart.set(t, a, eb);
                    chart.set(t, b, ea);
                }
            }
            swaps += 1;
        }
    }
}

fn slot(e: Option<Effectiveness>) -> usize {
    match e {
        Some(Effectiveness::Zero) => 0,
        Some(Effectiveness::Half) => 1,
        Some(Effectiveness::Neutral) | None => 2,
        Some(Effectiveness::Double) => 3,
    }
}

/// The inverse chart: immunities and resistances become weaknesses, weaknesses resistances; with
/// random immunities as many former weaknesses become immunities as there were immunities.
fn invert(chart: &mut TypeChart, random_immunities: bool, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let types = chart.types().to_vec();
    let mut immunities = 0;
    let mut former_weaknesses = Vec::new();
    for &attacker in &types {
        for &defender in &types {
            match chart.get(attacker, defender) {
                Some(Effectiveness::Zero) => {
                    chart.set(attacker, defender, Effectiveness::Double);
                    immunities += 1;
                }
                Some(Effectiveness::Half) => {
                    chart.set(attacker, defender, Effectiveness::Double);
                }
                Some(Effectiveness::Double) => {
                    chart.set(attacker, defender, Effectiveness::Half);
                    former_weaknesses.push((attacker, defender));
                }
                Some(Effectiveness::Neutral) | None => {}
            }
        }
    }
    if random_immunities {
        if former_weaknesses.len() < immunities {
            return Err(RandomizeError::Impossible {
                step: LABEL,
                message: "too few weaknesses to turn into immunities".into(),
            });
        }
        for _ in 0..immunities {
            let (a, d) = former_weaknesses.remove(below(rng, former_weaknesses.len()));
            chart.set(a, d, Effectiveness::Zero);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
