//! Species ability randomization.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005). An ability is drawn once from the allowed ones
//!   rather than redrawn until one is allowed; the same distribution.
//! - Not implemented yet: the battle style of the Misc. Tweaks (double-battle abilities count as bad unless only multi
//!   battles are fought).
//!
//! An alternate forme gets random abilities of its own, a cosmetic forme its base species'. With "Follow Mega
//! Evolutions" and "Follow Evolutions", a Mega Evolution counts as an evolution of its base species and keeps its
//! abilities; Charizard's and Mewtwo's two Mega Evolutions are split evolutions, which get random abilities of their
//! own.

use std::collections::BTreeSet;

use ppr_model::{DexNumber, GameData};
use ppr_settings::{AbilityMode, AbilitySettings};

use super::copy_up::{self, CopyUp};
use crate::choice::{below, unit};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Abilities";

/// Ability numbers, the same in every generation but Air Lock (77 in Gen 3).
const WONDER_GUARD: u16 = 25;
/// Battle-trapping abilities: Shadow Tag, Magnet Pull, Arena Trap.
const TRAPPING: &[u16] = &[23, 42, 71];
/// Negative abilities: Defeatist, Slow Start, Truant, Klutz, Stall.
const NEGATIVE: &[u16] = &[129, 112, 54, 103, 100];
/// Bad abilities: Minus, Plus, Anticipation, Forewarn, Frisk, Honey Gather, Aura Break, Receiver,
/// Power of Alchemy.
const BAD: &[u16] = &[58, 57, 107, 108, 119, 118, 188, 222, 223];
/// Double-battle abilities: Friend Guard, Healer, Telepathy, Symbiosis, Battery.
const DOUBLE_BATTLE: &[u16] = &[132, 131, 140, 180, 217];
/// Duplicate abilities: the twins of the abilities in a game's variations.
const DUPLICATES: &[u16] = &[
    72, 73, 74, 75, 76, 116, 160, 163, 164, 194, 219, 221, 223, 230, 231, 232, 236, 242,
];
/// Gen 3's Air Lock, which Gen 3 also leaves out when combining duplicates.
const GEN3_AIR_LOCK: u16 = 77;

/// The chance of a second ability.
const SECOND_ABILITY_CHANCE: f64 = 0.5;

/// Randomizes species' abilities according to [`AbilitySettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct AbilityRandomizer;

impl Step<RunData> for AbilityRandomizer {
    fn id(&self) -> u64 {
        step_ids::ABILITIES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        // "Abilities don't exist in some games..."
        data.settings.abilities.mode != AbilityMode::Unchanged && !data.game.ability_names.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.abilities.clone();
        randomize(&mut data.game, &s, &mut rng)
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// The abilities [`randomize`] may give out.
fn allowed(game: &GameData, s: &AbilitySettings) -> Vec<u16> {
    let mut banned: BTreeSet<u16> = game.useless_abilities.iter().copied().collect();
    if !s.allow_wonder_guard {
        banned.insert(WONDER_GUARD);
    }
    let lists: [(bool, &[u16]); 4] = [
        (s.ban_trapping, TRAPPING),
        (s.ban_negative, NEGATIVE),
        (s.ban_bad, BAD),
        // Not only multi battles, as the battle style isn't known here.
        (s.ban_bad, DOUBLE_BATTLE),
    ];
    for (ban, list) in lists {
        if ban {
            banned.extend(list);
        }
    }
    if s.combine_duplicates {
        banned.extend(DUPLICATES);
        if game.identity.generation.number() == 3 {
            banned.insert(GEN3_AIR_LOCK);
        }
    }
    let highest = u16::try_from(game.ability_names.len().saturating_sub(1)).unwrap_or(u16::MAX);
    (1..=highest).filter(|a| !banned.contains(a)).collect()
}

/// A random ability: an allowed ability not in `taken`; with `combine_duplicates`, an ability with twins
/// becomes one of them at random.
fn pick(
    rng: &mut StepRng,
    game: &GameData,
    allowed: &[u16],
    combine_duplicates: bool,
    taken: &[u16],
) -> Result<u16, RandomizeError> {
    let left: Vec<u16> = allowed.iter().copied().filter(|a| !taken.contains(a)).collect();
    if left.is_empty() {
        return Err(impossible("no ability left to give"));
    }
    let ability = left[below(rng, left.len())];
    if !combine_duplicates {
        return Ok(ability);
    }
    // One of the ability's variations at random.
    match game.ability_variations.get(&ability) {
        Some(twins) => {
            let twins: Vec<u16> = twins.iter().copied().filter(|a| !taken.contains(a)).collect();
            Ok(if twins.is_empty() {
                ability
            } else {
                twins[below(rng, twins.len())]
            })
        }
        None => Ok(ability),
    }
}

fn abilities(game: &GameData, n: DexNumber) -> Result<&[u16], RandomizeError> {
    Ok(&game
        .species(n)
        .ok_or_else(|| impossible(format!("no species {n}")))?
        .abilities)
}

fn set_abilities(game: &mut GameData, n: DexNumber, abilities: Vec<u16>) -> Result<(), RandomizeError> {
    let sp = game
        .species
        .iter_mut()
        .find(|sp| sp.number == n)
        .ok_or_else(|| impossible(format!("no species {n}")))?;
    sp.abilities = abilities;
    Ok(())
}

/// Randomizes every species' abilities; species with Wonder Guard keep their abilities.
fn randomize(game: &mut GameData, s: &AbilitySettings, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let allowed = allowed(game, s);
    let all = game.species.iter().map(|sp| sp.number).collect();
    let options = copy_up::Options {
        evolution_sanity: s.follow_evolutions,
        copy_split_evos: false,
    };
    for step in copy_up::plan_with_megas(game, &all, options, s.follow_megas) {
        let (n, new) = match step {
            CopyUp::Basic(n) | CopyUp::AltForme { forme: n, .. } => {
                let old = abilities(game, n)?;
                if old.contains(&WONDER_GUARD) {
                    continue;
                }
                let slots = old.len();
                let mut new = Vec::with_capacity(slots);
                new.push(pick(rng, game, &allowed, s.combine_duplicates, &[])?);
                if slots >= 2 {
                    let second = if s.ensure_two || unit(rng) < SECOND_ABILITY_CHANCE {
                        pick(rng, game, &allowed, s.combine_duplicates, &new)?
                    } else {
                        0
                    };
                    new.push(second);
                }
                if slots >= 3 {
                    // The hidden ability.
                    let hidden = pick(rng, game, &allowed, s.combine_duplicates, &new)?;
                    new.push(hidden);
                }
                (n, new)
            }
            CopyUp::Cosmetic { base, forme } => (forme, abilities(game, base)?.to_vec()),
            CopyUp::Evolved { from, to, .. } => {
                if abilities(game, to)?.contains(&WONDER_GUARD) {
                    continue;
                }
                (to, abilities(game, from)?.to_vec())
            }
        };
        set_abilities(game, n, new)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
