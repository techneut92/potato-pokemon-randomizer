//! TM, HM and move tutor compatibility: which species can learn the moves of TMs, HMs and move tutors.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//!
//! As in the original, alternate formes keep their compatibility when it follows evolutions (they are random
//! otherwise), and cosmetic formes get their base species' compatibility once anything changed.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, GameData, MoveId, Type};
use ppr_settings::CompatMode;

use super::copy_up::{self, CopyUp};
use crate::choice::unit;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "TM/HM Compatibility";

/// Compatibility flags by species, one per move of a list.
pub(super) type Compat = BTreeMap<DexNumber, Vec<bool>>;

/// Changes TM/HM compatibility according to [`ppr_settings::TmHmCompatSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TmHmCompatibilityRandomizer;

impl Step<RunData> for TmHmCompatibilityRandomizer {
    fn id(&self) -> u64 {
        step_ids::TM_HM_COMPAT
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.tm_hm_compat;
        (s.mode != CompatMode::Unchanged || s.level_up_sanity || s.full_hm) && !data.game.tm_hm_compat.is_empty()
    }

    /// Randomizes or fills TM/HM compatibility as chosen, then applies the level-up and evolution sanity rules.
    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.tm_hm_compat.clone();
        let game = &data.game;
        let moves: Vec<MoveId> = game.tm_moves.iter().chain(&game.hm_moves).copied().collect();
        let mut compat = game.tm_hm_compat.clone();
        match s.mode {
            CompatMode::Unchanged => {}
            CompatMode::RandomPreferSameType | CompatMode::RandomCompletely => {
                let options = Options {
                    prefer_same_type: s.mode == CompatMode::RandomPreferSameType,
                    follow_evolutions: s.follow_evolutions,
                    copy_split_evos: false,
                };
                randomize(
                    game,
                    &mut compat,
                    &moves,
                    &game.early_required_hm_moves,
                    options,
                    &mut rng,
                );
            }
            CompatMode::Full => full(&mut compat, 0),
        }
        if s.level_up_sanity {
            level_up_sanity(game, &mut compat, &game.tm_moves);
            if s.follow_evolutions {
                evolution_sanity(game, &mut compat);
            }
        }
        if s.full_hm {
            full(&mut compat, game.tm_moves.len());
        }
        copy_to_cosmetic_formes(game, &mut compat);
        data.game.tm_hm_compat = compat;
        Ok(())
    }
}

const TUTOR_LABEL: &str = "Move Tutor Compatibility";

/// Changes move tutor compatibility according to [`ppr_settings::TutorCompatSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TutorCompatibilityRandomizer;

impl Step<RunData> for TutorCompatibilityRandomizer {
    fn id(&self) -> u64 {
        step_ids::TUTOR_COMPAT
    }

    fn label(&self) -> &'static str {
        TUTOR_LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.tutor_compat;
        (s.mode != CompatMode::Unchanged || s.level_up_sanity) && !data.game.tutor_compat.is_empty()
    }

    /// Randomizes move tutor compatibility as chosen, then applies the level-up and evolution sanity rules.
    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.tutor_compat.clone();
        let game = &data.game;
        let mut compat = game.tutor_compat.clone();
        match s.mode {
            CompatMode::Unchanged => {}
            CompatMode::RandomPreferSameType | CompatMode::RandomCompletely => {
                let options = Options {
                    prefer_same_type: s.mode == CompatMode::RandomPreferSameType,
                    follow_evolutions: s.follow_evolutions,
                    copy_split_evos: true,
                };
                randomize(game, &mut compat, &game.tutor_moves, &[], options, &mut rng);
            }
            CompatMode::Full => full(&mut compat, 0),
        }
        if s.level_up_sanity {
            level_up_sanity(game, &mut compat, &game.tutor_moves);
            if s.follow_evolutions {
                evolution_sanity(game, &mut compat);
            }
        }
        copy_to_cosmetic_formes(game, &mut compat);
        data.game.tutor_compat = compat;
        Ok(())
    }
}

/// How [`randomize`] works.
#[derive(Debug, Clone, Copy)]
pub(super) struct Options {
    pub(super) prefer_same_type: bool,
    pub(super) follow_evolutions: bool,
    /// Split evolutions copy from their pre-evolutions too (tutors); otherwise they are random (TMs).
    pub(super) copy_split_evos: bool,
}

/// Random compatibility: every species' flags for `moves`
/// at random, or with Follow Evolutions mostly copied from the pre-evolution.
pub(super) fn randomize(
    game: &GameData,
    compat: &mut Compat,
    moves: &[MoveId],
    prioritized: &[MoveId],
    options: Options,
    rng: &mut StepRng,
) {
    let types: Vec<Option<Type>> = moves
        .iter()
        .map(|&m| game.moves.iter().find(|mv| mv.number == m).and_then(|mv| mv.move_type))
        .collect();
    let all = compat.keys().copied().collect();
    let plan_options = copy_up::Options {
        evolution_sanity: options.follow_evolutions,
        copy_split_evos: options.copy_split_evos,
    };
    for step in copy_up::plan(game, &all, plan_options) {
        match step {
            CopyUp::AltForme { .. } | CopyUp::Cosmetic { .. } if options.follow_evolutions => {}
            CopyUp::Basic(n) | CopyUp::AltForme { forme: n, .. } | CopyUp::Cosmetic { forme: n, .. } => {
                let Some(sp) = game.species(n) else { continue };
                let new: Vec<bool> = moves
                    .iter()
                    .zip(&types)
                    .map(|(m, &t)| {
                        let p = probability(sp.primary_type, sp.secondary_type, t, prioritized.contains(m), options);
                        unit(rng) < p
                    })
                    .collect();
                compat.insert(n, new);
            }
            CopyUp::Evolved { from, to, .. } => {
                let (Some(f), Some(t)) = (game.species(from), game.species(to)) else {
                    continue;
                };
                let previous = compat.get(&from).cloned().unwrap_or_default();
                let new: Vec<bool> = types
                    .iter()
                    .enumerate()
                    .map(|(i, &move_type)| {
                        if previous.get(i).copied().unwrap_or(false) {
                            return true;
                        }
                        // A slight chance to learn what the pre-evolution could not: 25%, or with Prefer Same
                        // Type 10%, and 90% for a type new to this evolution.
                        let mut p = 0.25;
                        if options.prefer_same_type {
                            p = 0.1;
                            let new_type = |own: Option<Type>| {
                                own.is_some()
                                    && own == move_type
                                    && own != Some(f.primary_type)
                                    && own != f.secondary_type
                            };
                            if new_type(Some(t.primary_type)) || new_type(t.secondary_type) {
                                p = 0.9;
                            }
                        }
                        unit(rng) < p
                    })
                    .collect();
                compat.insert(to, new);
            }
        }
    }
}

/// Cosmetic formes learn what their base species learns.
pub(super) fn copy_to_cosmetic_formes(game: &GameData, compat: &mut Compat) {
    for sp in game.species.iter().filter(|sp| sp.forme.cosmetic) {
        if let Some(flags) = sp.conceptual_base().and_then(|b| compat.get(&b)).cloned() {
            compat.insert(sp.number, flags);
        }
    }
}

/// The chance a species learns a move: 50%, or with Prefer Same Type 90% for its own types, 50% for Normal and 25%
/// otherwise; an early required HM's move 1.8 times that, at most 100%.
fn probability(primary: Type, secondary: Option<Type>, move_type: Option<Type>, early: bool, options: Options) -> f64 {
    let mut p = 0.5;
    if options.prefer_same_type {
        p = if move_type == Some(primary) || (secondary.is_some() && move_type == secondary) {
            0.9
        } else if move_type == Some(Type::Normal) {
            0.5
        } else {
            0.25
        };
    }
    if early {
        p = f64::min(1.0, p * 1.8);
    }
    p
}

/// Full compatibility for every move from index `from` on: 0 for all TMs and HMs, the number of TMs for HMs only.
pub(super) fn full(compat: &mut Compat, from: usize) {
    for flags in compat.values_mut() {
        flags.iter_mut().skip(from).for_each(|f| *f = true);
    }
}

/// Level-up sanity: species can learn the moves of `moves` they learn by level.
pub(super) fn level_up_sanity(game: &GameData, compat: &mut Compat, moves: &[MoveId]) {
    for (n, flags) in compat.iter_mut() {
        let Some(learnset) = game.learnsets.get(n) else {
            continue;
        };
        for learnt in learnset {
            if let Some(i) = moves.iter().position(|&m| m == learnt.move_id)
                && let Some(flag) = flags.get_mut(i)
            {
                *flag = true;
            }
        }
    }
}

/// Evolution sanity: evolutions can learn whatever their pre-evolutions can.
pub(super) fn evolution_sanity(game: &GameData, compat: &mut Compat) {
    let all = compat.keys().copied().collect();
    let options = copy_up::Options {
        evolution_sanity: true,
        copy_split_evos: true,
    };
    for step in copy_up::plan(game, &all, options) {
        if let CopyUp::Evolved { from, to, .. } = step {
            let from_flags = compat.get(&from).cloned().unwrap_or_default();
            if let Some(to_flags) = compat.get_mut(&to) {
                for (t, f) in to_flags.iter_mut().zip(from_flags) {
                    *t |= f;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
