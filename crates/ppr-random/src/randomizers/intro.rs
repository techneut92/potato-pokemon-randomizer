//! Intro Pokémon: the game's intro shows a random species, one of those the ROM can show there. Alternate formes and
//! Mega Evolutions may be chosen where the ROM can show them (X and Y).

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

/// Picks the intro Pokémon by [`ppr_settings::GeneralSettings::random_intro_pokemon`].
#[derive(Debug, Clone, Copy, Default)]
pub struct IntroPokemonRandomizer;

impl Step<RunData> for IntroPokemonRandomizer {
    fn id(&self) -> u64 {
        step_ids::INTRO_POKEMON
    }

    fn label(&self) -> &'static str {
        "Intro Pokémon"
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.general.random_intro_pokemon && !data.game.intro_candidates.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let pool = data.game.species_pool_with_formes();
        let candidates: Vec<_> = data
            .game
            .intro_candidates
            .iter()
            .copied()
            .filter(|n| pool.contains(n))
            .collect();
        // With no species that fits, the intro stays as it is.
        if !candidates.is_empty() {
            data.game.intro_pokemon = Some(candidates[below(&mut rng, candidates.len())]);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::Type;
    use ppr_settings::{GeneralSettings, Settings};

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    #[test]
    fn the_intro_shows_a_candidate() {
        let mut game = emerald([4, 7, 9].map(|n| species(n, Type::Fire, None, 50)).to_vec());
        game.intro_candidates = vec![4, 7, 9];
        for (random, seed) in [(true, 1), (false, 2)] {
            let mut data = RunData {
                game: game.clone(),
                settings: Settings {
                    general: GeneralSettings {
                        random_intro_pokemon: random,
                        ..GeneralSettings::default()
                    },
                    ..Settings::default()
                },
            };
            standard_pipeline()
                .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
                .expect("runs");
            match data.game.intro_pokemon {
                Some(n) => assert!(random && [4, 7, 9].contains(&n)),
                None => assert!(!random),
            }
        }
    }

    #[test]
    fn the_intro_may_show_a_forme_the_rom_can_show() {
        let mut list: Vec<_> = [4, 7].map(|n| species(n, Type::Fire, None, 50)).to_vec();
        let mut mega = species(8, Type::Fire, None, 60);
        mega.forme.base = Some(4);
        mega.forme.number = 1;
        mega.forme.mega = true;
        list.push(mega);
        let mut game = emerald(list);
        game.intro_candidates = vec![4, 7, 8];
        let mut seen = std::collections::BTreeSet::new();
        for seed in 0..64 {
            let mut data = RunData {
                game: game.clone(),
                settings: Settings {
                    general: GeneralSettings {
                        random_intro_pokemon: true,
                        ..GeneralSettings::default()
                    },
                    ..Settings::default()
                },
            };
            standard_pipeline()
                .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
                .expect("runs");
            seen.extend(data.game.intro_pokemon);
        }
        assert_eq!(seen, [4, 7, 8].into());

        // Where the ROM cannot show formes, they are no candidates.
        game.intro_candidates = vec![4, 7];
        for seed in 0..32 {
            let mut data = RunData {
                game: game.clone(),
                settings: Settings {
                    general: GeneralSettings {
                        random_intro_pokemon: true,
                        ..GeneralSettings::default()
                    },
                    ..Settings::default()
                },
            };
            standard_pipeline()
                .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
                .expect("runs");
            assert_ne!(data.game.intro_pokemon, Some(8));
        }
    }
}
