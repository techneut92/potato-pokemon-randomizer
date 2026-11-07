//! Species restrictions: with "Limit Pokémon", fixes the species the randomizers may choose from and drops the
//! evolutions that leave them, before any other step.
//!
//! Deliberate choice: the chosen generations apply only while "Limit Pokémon" is ticked, not also after it is
//! unticked again.
//!
//! Gen 1 games have only one generation to choose from, so the limit does not apply there.

use std::collections::BTreeSet;

use ppr_model::{DexNumber, GameData};
use ppr_settings::GenerationLimit;

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

/// Applies [`ppr_settings::GeneralSettings::limit_pokemon`].
#[derive(Debug, Clone, Copy, Default)]
pub struct SpeciesRestrictions;

impl Step<RunData> for SpeciesRestrictions {
    fn id(&self) -> u64 {
        step_ids::SPECIES_RESTRICTIONS
    }

    fn label(&self) -> &'static str {
        "Limit Pokémon"
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.general.limit_pokemon.is_some() && data.game.identity.generation.number() > 1
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let data = ctx.data;
        let Some(limit) = data.settings.general.limit_pokemon else {
            return Ok(());
        };
        let pool = allowed(&data.game, limit);
        if pool.is_empty() {
            return Err(RandomizeError::Impossible {
                step: self.label(),
                message: "the chosen generations hold no species of this game".into(),
            });
        }
        data.game
            .evolutions
            .retain(|e| pool.contains(&e.from) && pool.contains(&e.to));
        data.game.restricted_species = Some(pool);
        Ok(())
    }
}

/// The allowed species: the species of the allowed generations up to the game's own (alternate formes by their base
/// species), and with relatives allowed, their whole evolutionary families.
fn allowed(game: &GameData, limit: GenerationLimit) -> BTreeSet<DexNumber> {
    let own = game.identity.generation.number();
    let mut pool: BTreeSet<DexNumber> = game
        .species
        .iter()
        .map(|sp| sp.number)
        .filter(|&n| {
            let generation = game.generation_of(n);
            generation <= own && limit.allows(generation)
        })
        .collect();
    if limit.allow_evolutionary_relatives {
        let families: Vec<DexNumber> = pool.iter().flat_map(|&n| game.family(n)).collect();
        pool.extend(families);
        // The alternate formes of the relatives too.
        let formes: Vec<DexNumber> = game
            .alt_formes()
            .filter(|sp| pool.contains(&sp.base_number()))
            .map(|sp| sp.number)
            .collect();
        pool.extend(formes);
    }
    pool
}

#[cfg(test)]
mod tests {
    use ppr_model::{Evolution, EvolutionType, Type};
    use ppr_settings::{GeneralSettings, Settings};

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    /// Pichu (172) → Pikachu (25) → Raichu (26), Azurill (298) → Marill (183), and Bulbasaur (1).
    fn game() -> GameData {
        let mut g = emerald(
            [1, 25, 26, 172, 183, 298]
                .into_iter()
                .map(|n| species(n, Type::Normal, None, 60))
                .collect(),
        );
        g.evolutions = [(172, 25), (25, 26), (298, 183)]
            .into_iter()
            .map(|(from, to)| Evolution::new(from, to, EvolutionType::Level, 16))
            .collect();
        g
    }

    fn run(generations: [bool; 7], relatives: bool) -> GameData {
        run_on(game(), generations, relatives)
    }

    fn run_on(game: GameData, generations: [bool; 7], relatives: bool) -> GameData {
        let mut data = RunData {
            game,
            settings: Settings {
                general: GeneralSettings {
                    limit_pokemon: Some(GenerationLimit {
                        generations,
                        allow_evolutionary_relatives: relatives,
                    }),
                    ..GeneralSettings::default()
                },
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game
    }

    const GEN_1: [bool; 7] = [true, false, false, false, false, false, false];

    #[test]
    fn only_allowed_generations_remain_with_their_evolutions() {
        let g = run(GEN_1, false);
        assert_eq!(g.species_pool(), BTreeSet::from([1, 25, 26]));
        let evolutions: Vec<_> = g.evolutions.iter().map(|e| (e.from, e.to)).collect();
        assert_eq!(evolutions, [(25, 26)], "Pichu and Azurill's evolutions leave the pool");
    }

    #[test]
    fn relatives_join_from_any_generation() {
        let g = run(GEN_1, true);
        assert_eq!(g.species_pool(), BTreeSet::from([1, 25, 26, 172]));
        assert_eq!(g.evolutions.len(), 2);
        // Generations past the game's own count for nothing.
        let later = GenerationLimit {
            generations: [false, false, false, true, true, true, true],
            allow_evolutionary_relatives: true,
        };
        assert!(allowed(&game(), later).is_empty());
    }

    #[test]
    fn gen1_games_are_not_limited() {
        let red = crate::randomizers::testing::gen1(game().species, false);
        let g = run_on(red.clone(), [false, true, false, false, false, false, false], false);
        assert_eq!(g, red);
    }

    #[test]
    fn without_a_limit_every_species_is_allowed() {
        let mut data = RunData {
            game: game(),
            settings: Settings::default(),
        };
        standard_pipeline()
            .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        assert!(data.game.restricted_species.is_none());
        assert_eq!(data.game.species_pool().len(), 6);
        assert_eq!(data.game.evolutions.len(), 3);
    }
}
