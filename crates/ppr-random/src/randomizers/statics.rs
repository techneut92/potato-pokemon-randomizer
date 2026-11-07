//! New static Pokémon, or only new levels for them.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005); species are taken in national order.
//! - Encounters linked to a static (Emerald's roaming Latios and Latias) always get its replacement.
//! - "Limit Main-Game Legendaries" falls back to the whole pool when no species is weak enough, instead of
//!   failing.
//!
//! Where static Pokémon have a forme, "Allow Alternate Formes" lets replacements be alternate formes (but those
//! that revert for the player, those that need their species' ability while abilities stay, and with "No Irregular
//! Alt Formes" the irregular ones), and a base species with cosmetic formes gets one at random.
//!
//! In "legendary ↔ legendary" mode the Ultra Beasts are a group of their own: an Ultra Beast becomes another one
//! (alternate formes never), and the others never become one.
//!
//! A static that may only become some species (the hidden hollows of Black 2 and White 2) gets one of those left in
//! its pool, else one of those in its full pool, in every mode; with similar strength it may keep its species.
//!
//! With "Swap Mega Evolvables", and always in X and Y (whose Lucario comes with its Mega Stone), a static that holds
//! the Mega Stone of its species becomes a species that can Mega Evolve with a stone, holding one of its stones.

use ppr_model::{DexNumber, GameData, ItemId, StaticEncounter};
use ppr_settings::{StaticMode, StaticSettings};

use super::common::{
    Pool, SpeciesSet, banned_formes, bst, forme_for, modified_level, pool_with_formes, power_level_replacement,
};
use crate::choice::pick;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Static Pokémon";
const NO_SPECIES: &str = "no species to choose from";

/// The base stat total from which "Randomize 600+ BST" lets a static become anything.
const STRONG_BST: u32 = 600;

/// Replaces static Pokémon and changes their levels according to [`StaticSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct StaticRandomizer;

impl Step<RunData> for StaticRandomizer {
    fn id(&self) -> u64 {
        step_ids::STATICS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.statics;
        // Games without supported statics have none to change.
        !data.game.statics.is_empty() && (s.mode != StaticMode::Unchanged || s.level_modifier != 0)
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.statics.clone();
        if s.mode != StaticMode::Unchanged {
            let banned = banned_formes(
                &data.game,
                true,
                data.settings.abilities.mode != ppr_settings::AbilityMode::Unchanged,
                data.settings.general.no_irregular_alt_formes,
            );
            data.game.statics = randomize(&data.game, &s, &banned, &mut rng)?;
            if s.fix_music && !data.game.special_music_statics.is_empty() {
                data.game.correct_static_music = true;
            }
        }
        apply_level_modifier(&mut data.game.statics, s.level_modifier);
        Ok(())
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// The level modifier of statics: eggs keep theirs, linked encounters change too.
fn apply_level_modifier(statics: &mut [StaticEncounter], percent: i16) {
    if percent == 0 {
        return;
    }
    for s in statics.iter_mut().filter(|s| !s.egg) {
        s.level = modified_level(s.level, percent);
        for l in &mut s.linked_levels {
            *l = modified_level(*l, percent);
        }
    }
}

/// Whether a static holds the Mega Stone of one of its species' Mega Evolutions.
fn can_mega_evolve(game: &GameData, st: &StaticEncounter) -> bool {
    st.held_item.is_some()
        && game
            .mega_evolutions()
            .any(|m| m.forme.base == Some(st.species) && m.forme.mega_item == st.held_item)
}

/// The species with a Mega Evolution that needs a Mega Stone.
fn mega_evolvables(game: &GameData) -> SpeciesSet {
    game.mega_evolutions()
        .filter(|m| m.forme.mega_item.is_some())
        .filter_map(|m| m.forme.base)
        .collect()
}

/// The Mega Stone of a random Mega Evolution of `n`.
fn mega_stone(rng: &mut StepRng, game: &GameData, n: DexNumber) -> Option<ItemId> {
    let megas: SpeciesSet = game
        .mega_evolutions()
        .filter(|m| m.forme.base == Some(n))
        .map(|m| m.number)
        .collect();
    pick(rng, &megas)
        .and_then(|m| game.species(m))
        .and_then(|m| m.forme.mega_item)
}

/// The species that can Mega Evolve left in the pool, else those of `fallback`.
fn mega_candidates(evolvables: &SpeciesSet, fallback: &SpeciesSet, pool: &Pool) -> SpeciesSet {
    let left: SpeciesSet = evolvables.intersection(&pool.left).copied().collect();
    if left.is_empty() {
        evolvables.intersection(fallback).copied().collect()
    } else {
        left
    }
}

/// A species that can Mega Evolve for a static that could, taken from the pool ([`mega_candidates`]).
fn mega_evolvable(
    rng: &mut StepRng,
    evolvables: &SpeciesSet,
    fallback: &SpeciesSet,
    pool: &mut Pool,
) -> Result<DexNumber, RandomizeError> {
    let n = pick(rng, &mega_candidates(evolvables, fallback, pool))
        .ok_or_else(|| impossible("no species that can Mega Evolve"))?;
    pool.remove(n);
    Ok(n)
}

/// A random species from the pool for a static, removed from it: with `allowed` (a static that may only become
/// some species) one of those left, else one of those in the full pool.
fn take_for(rng: &mut StepRng, pool: &mut Pool, allowed: Option<&SpeciesSet>) -> Result<DexNumber, RandomizeError> {
    let n = match allowed {
        Some(allowed) => pick(rng, &pool.filtered(|n| allowed.contains(&n))),
        None => pick(rng, &pool.left),
    }
    .ok_or_else(|| impossible(NO_SPECIES))?;
    pool.remove(n);
    Ok(n)
}

/// New species for the statics, without the level modifier.
fn randomize(
    game: &GameData,
    s: &StaticSettings,
    banned: &SpeciesSet,
    rng: &mut StepRng,
) -> Result<Vec<StaticEncounter>, RandomizeError> {
    let mut all = pool_with_formes(game, s.allow_alt_formes && game.formes.statics);
    all.retain(|n| !banned.contains(n));
    let mut replacements = game.statics.clone();
    let swap_megas = s.swap_mega_evos || game.formes.force_swap_static_megas;
    let evolvables = mega_evolvables(game);
    let base_species = pool_with_formes(game, false);
    let megas: Vec<bool> = game
        .statics
        .iter()
        .map(|r| swap_megas && can_mega_evolve(game, r))
        .collect();
    let allowed: Vec<Option<SpeciesSet>> = game
        .statics
        .iter()
        .map(|r| r.allowed_species.as_ref().map(|l| l.iter().copied().collect()))
        .collect();
    let statics = replacements.iter_mut().zip(megas.iter().copied().zip(&allowed));
    match s.mode {
        StaticMode::Unchanged => {}
        StaticMode::RandomMatching => {
            let ultra_beasts = game.ultra_beasts();
            let (legendaries, others): (SpeciesSet, SpeciesSet) = all
                .iter()
                .filter(|n| !ultra_beasts.contains(n))
                .partition(|&&n| game.is_legendary(n));
            let (base_legendaries, base_others): (SpeciesSet, SpeciesSet) =
                base_species.iter().partition(|&&n| game.is_legendary(n));
            let mut legendaries = Pool::new(legendaries);
            let mut others = Pool::new(others);
            let mut ultra_beast_pool = Pool::new(ultra_beasts.difference(banned).copied().collect());
            for (r, (mega, allowed)) in statics {
                let allowed = allowed.as_ref();
                if ultra_beasts.contains(&r.species) && !game.is_legendary(r.species) {
                    r.species = take_for(rng, &mut ultra_beast_pool, allowed)?;
                    continue;
                }
                let (pool, fallback) = if game.is_legendary(r.species) {
                    (&mut legendaries, &base_legendaries)
                } else {
                    (&mut others, &base_others)
                };
                if mega {
                    r.species = mega_evolvable(rng, &evolvables, fallback, pool)?;
                    r.held_item = mega_stone(rng, game, r.species);
                } else {
                    r.species = take_for(rng, pool, allowed)?;
                }
            }
        }
        StaticMode::Random => {
            let mut pool = Pool::new(all);
            for (r, (mega, allowed)) in statics {
                if mega {
                    r.species = mega_evolvable(rng, &evolvables, &base_species, &mut pool)?;
                    r.held_item = mega_stone(rng, game, r.species);
                } else {
                    r.species = take_for(rng, &mut pool, allowed.as_ref())?;
                }
            }
        }
        StaticMode::SimilarStrength => {
            let mut pool = Pool::new(all);
            for (r, (mega, allowed)) in statics {
                let old = r.species;
                r.species = if s.randomize_600_bst && bst(game, old) >= STRONG_BST {
                    if mega {
                        mega_evolvable(rng, &evolvables, &base_species, &mut pool)?
                    } else {
                        take_for(rng, &mut pool, allowed.as_ref())?
                    }
                } else {
                    let limit = s.limit_main_game_legendaries && game.main_game_legendaries.contains(&old);
                    // A static that may only become some species may stay the same, in case nothing else fits.
                    let (candidates, ban_current) = match allowed {
                        _ if mega => (mega_candidates(&evolvables, &base_species, &pool), true),
                        Some(allowed) => (pool.filtered(|n| allowed.contains(&n)), false),
                        None => (pool.left.clone(), true),
                    };
                    let new = power_level_replacement(rng, game, &candidates, old, limit, ban_current)
                        .ok_or_else(|| impossible(NO_SPECIES))?;
                    pool.remove(new);
                    new
                };
                if mega {
                    r.held_item = mega_stone(rng, game, r.species);
                }
            }
        }
    }
    if s.mode != StaticMode::Unchanged {
        for r in &mut replacements {
            r.forme = forme_for(rng, game, r.species, game.formes.statics);
        }
    }
    Ok(replacements)
}

#[cfg(test)]
mod tests;
