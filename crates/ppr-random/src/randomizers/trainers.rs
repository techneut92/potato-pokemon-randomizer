//! Trainer Pokémon: trainers' Pokémon levels, added Pokémon, rivals carrying the starter, and the species, from
//! random to type themed ([`pick`]). With random shiny trainer Pokémon, every trainer Pokémon becomes shiny with a
//! chance of 1 in 256 when the species are chosen, rolled after its species; only Gen 7 stores the flag, so the
//! option is offered there only.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - Random group types go to the groups in a random order, so the same gyms don't always get the leftover types.
//! - Random group types are only types with species to choose from, and "Force diverse types" repeats a type when
//!   none is left, rather than failing the run.
//! - "Random (even distribution, main-game)" is the same as "Random" in Gen 3, which has no main-game trainer list.
//! - "No Premature Evolutions" comes with the general options.
//! - With "Swap Mega Evolvables", a Pokémon holding a Mega Stone of its species whose replacement has no Mega
//!   Evolution (only possible through a type theme without species that can Mega Evolve) holds nothing, where the
//!   original fails the run.
//!
//! Where trainers' Pokémon have a forme, "Allow Alternate Formes" lets replacements be alternate formes (but those
//! the game bans for trainers, those that need their species' ability while abilities stay, and with "No
//! Irregular Alt Formes" the irregular ones), and a base species with cosmetic formes gets one at random.
//!
//! With "Swap Mega Evolvables", as in the original, a Pokémon holding the Mega Stone of its species becomes a species
//! that can Mega Evolve with a stone, holding one of its stones. Those species come from the whole pool ("Limit
//! Pokémon"), legendaries and species from outside the local ones included; with a type theme, those of the theme,
//! or any species of the theme when none can. The rival's starter and the original members of unchanged teams keep
//! their species but get one of its Mega Stones at random.

use std::collections::BTreeSet;

use ppr_model::{DexNumber, GameData, ItemId, TrainerPokemon};
use ppr_settings::TrainerMode;

use super::common::{forme_for, modified_level};
use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

mod pick;
mod rival;

const LABEL: &str = "Trainer Pokémon";

/// Adds Pokémon to trainers' teams by [`ppr_settings::TrainerSettings::additional_pokemon`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerPokemonAdder;

impl Step<RunData> for TrainerPokemonAdder {
    fn id(&self) -> u64 {
        step_ids::TRAINER_ADDITIONS
    }

    fn label(&self) -> &'static str {
        "Added Trainer Pokémon"
    }

    fn applies(&self, data: &RunData) -> bool {
        added(data) && !data.game.trainers.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.trainers.clone();
        rival::add_pokemon(&mut data.game, &s, &mut rng);
        Ok(())
    }
}

/// Changes trainer Pokémon levels by [`ppr_settings::TrainerSettings::level_modifier`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerLevelModifier;

impl Step<RunData> for TrainerLevelModifier {
    fn id(&self) -> u64 {
        step_ids::TRAINER_LEVELS
    }

    fn label(&self) -> &'static str {
        "Trainer Levels"
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.trainers.level_modifier.is_some_and(|m| m != 0) && !data.game.trainers.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, .. } = ctx;
        let percent = data.settings.trainers.level_modifier.unwrap_or_default();
        // The level modifier: the first rival battle only gets easier.
        for trainer in &mut data.game.trainers {
            if trainer.is_first_rival_or_friend() && percent > 0 {
                continue;
            }
            for tp in &mut trainer.pokemon {
                tp.level = modified_level(tp.level, percent);
            }
        }
        Ok(())
    }
}

/// Chooses trainers' Pokémon according to [`ppr_settings::TrainerSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerPokemonRandomizer;

impl Step<RunData> for TrainerPokemonRandomizer {
    fn id(&self) -> u64 {
        step_ids::TRAINER_POKEMON
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.trainers;
        let changed = s.mode != TrainerMode::Unchanged || added(data);
        (changed || s.evolve.is_some() || carries_starter(data)) && !data.game.trainers.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.trainers.clone();
        if carries_starter(data) {
            rival::carry_starters(&mut data.game, &s, &mut rng);
        }
        if s.mode != TrainerMode::Unchanged || added(data) {
            let rules = pick::Rules {
                no_premature_evolutions: data.settings.general.no_premature_evolutions,
                no_irregular_alt_formes: data.settings.general.no_irregular_alt_formes,
                abilities_random: data.settings.abilities.mode != ppr_settings::AbilityMode::Unchanged,
            };
            pick::randomize(&mut data.game, &s, rules, &mut rng)?;
        } else if let Some(percent) = s.evolve {
            evolve_as_far_as_legal(&mut data.game, percent, &mut rng);
        }
        Ok(())
    }
}

/// Pokémon were added to teams, so they are to be chosen: the game can take more for some trainers.
fn added(data: &RunData) -> bool {
    let s = &data.settings.trainers;
    (0..3).any(|i| s.additional_pokemon[i] > 0 && data.game.trainer_pokemon_additions[i])
}

/// Whether rivals carry the starter: the option, and new trainer Pokémon or starters.
fn carries_starter(data: &RunData) -> bool {
    let s = &data.settings;
    s.trainers.rival_carries_starter
        && (s.trainers.mode != TrainerMode::Unchanged || s.starters.mode != ppr_settings::StarterMode::Unchanged)
}

/// Evolves every trainer Pokémon as far as its level, scaled by `percent`, allows.
fn evolve_as_far_as_legal(game: &mut GameData, percent: i16, rng: &mut StepRng) {
    let factor = 1.0 + f64::from(percent) / 100.0;
    let mut trainers = std::mem::take(&mut game.trainers);
    for tp in trainers.iter_mut().flat_map(|t| t.pokemon.iter_mut()) {
        let evolved = evolved_as_far_as_legal(game, tp.species, tp.level, factor, rng);
        if evolved != tp.species {
            tp.species = evolved;
            tp.forme = forme_for(rng, game, evolved, game.formes.trainers);
            keep_valid_ability_slot(game, tp);
        }
    }
    game.trainers = trainers;
}

/// Random evolutions whose estimated level, times `factor`, the level reaches.
pub(super) fn evolved_as_far_as_legal(
    game: &GameData,
    mut species: DexNumber,
    level: u8,
    factor: f64,
    rng: &mut StepRng,
) -> DexNumber {
    loop {
        let possible: Vec<DexNumber> = game
            .evolutions
            .iter()
            .filter(|e| e.from == species && f64::from(level) >= factor * f64::from(e.estimated_level))
            .map(|e| e.to)
            .collect();
        if possible.is_empty() {
            return species;
        }
        species = possible[below(rng, possible.len())];
    }
}

/// The Mega Stones: the items the game's Mega Evolutions need.
pub(super) fn mega_stones(game: &GameData) -> BTreeSet<ItemId> {
    game.mega_evolutions().filter_map(|m| m.forme.mega_item).collect()
}

/// Whether `tp` held a Mega Stone as the game was loaded: it holds one, as only Pokémon that held one get one when
/// Mega Evolvables are swapped, or it was added as a copy of one that did.
pub(super) fn held_mega_stone(stones: &BTreeSet<ItemId>, tp: &TrainerPokemon) -> bool {
    tp.added_from_mega_stone || tp.held_item.is_some_and(|item| stones.contains(&item))
}

/// Slot 2 becomes 1 when the new species has no second ability (an alternate forme's base species', where the
/// game gives trainers' formes those).
pub(super) fn keep_valid_ability_slot(game: &GameData, tp: &mut TrainerPokemon) {
    let second = game
        .trainer_pokemon_abilities(tp.species)
        .get(1)
        .copied()
        .unwrap_or_default();
    if tp.ability_slot == 2 && second == 0 {
        tp.ability_slot = 1;
    }
}

#[cfg(test)]
mod tests;
