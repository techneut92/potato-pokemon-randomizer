//! Gen 1: a species has no colors of its own, only the number of one of the game's ten Pokémon palettes (Super Game
//! Boy palettes). Each species gets a random one, or one that fits its primary type; evolutions can keep their
//! pre-evolution's.

use ppr_model::{DexNumber, GameData, Type};
use ppr_settings::PaletteSettings;
use rand_core::Rng;

use crate::choice::below;
use crate::randomizers::copy_up::{self, CopyUp};

// The Pokémon palettes among the game's Super Game Boy palettes, by number.
const MEWMON: u8 = 16;
const BLUEMON: u8 = 17;
const REDMON: u8 = 18;
const CYANMON: u8 = 19;
const PURPLEMON: u8 = 20;
const BROWNMON: u8 = 21;
const GREENMON: u8 = 22;
const PINKMON: u8 = 23;
const YELLOWMON: u8 = 24;
const GREYMON: u8 = 25;

/// The palettes a random species may get.
const POKEMON_PALETTES: [u8; 10] = [
    MEWMON, BLUEMON, REDMON, CYANMON, PURPLEMON, BROWNMON, GREENMON, PINKMON, YELLOWMON, GREYMON,
];

/// The palette of a type without palettes of its own.
const DEFAULT_PALETTE: u8 = MEWMON;

/// The palettes that fit a type; empty for the types Gen 1 doesn't have.
fn type_palettes(t: Type) -> &'static [u8] {
    match t {
        Type::Normal => &[BROWNMON, PINKMON, GREYMON],
        Type::Fighting => &[BROWNMON, GREYMON],
        Type::Flying => &[BLUEMON],
        Type::Grass => &[GREENMON],
        Type::Water | Type::Ice => &[BLUEMON, CYANMON],
        Type::Fire => &[REDMON],
        Type::Rock => &[GREYMON],
        Type::Ground => &[BROWNMON],
        Type::Psychic => &[MEWMON, YELLOWMON],
        Type::Bug => &[GREENMON, YELLOWMON],
        Type::Dragon => &[BLUEMON, BROWNMON, GREYMON],
        Type::Electric => &[YELLOWMON],
        Type::Ghost | Type::Poison => &[PURPLEMON],
        Type::Steel | Type::Dark | Type::Fairy => &[],
    }
}

fn random_palette(rng: &mut impl Rng) -> u8 {
    POKEMON_PALETTES[below(rng, POKEMON_PALETTES.len())]
}

fn random_type_palette(rng: &mut impl Rng, t: Type) -> u8 {
    let ids = type_palettes(t);
    if ids.is_empty() {
        DEFAULT_PALETTE
    } else {
        ids[below(rng, ids.len())]
    }
}

/// New palette numbers for every species with one.
pub(super) fn randomize(rng: &mut impl Rng, game: &mut GameData, settings: &PaletteSettings) {
    let options = copy_up::Options {
        evolution_sanity: settings.follow_evolutions,
        copy_split_evos: true,
    };
    let set = game.palettes.keys().copied().collect();
    let primary = |g: &GameData, n: DexNumber| g.species(n).map(|s| s.primary_type);
    for step in copy_up::plan(game, &set, options) {
        let (n, id) = match step {
            // Gen 1 and 2 have no alternate formes.
            CopyUp::AltForme { .. } | CopyUp::Cosmetic { .. } => continue,
            CopyUp::Basic(n) => {
                let Some(t) = primary(game, n) else { continue };
                let id = if settings.follow_types {
                    random_type_palette(rng, t)
                } else {
                    random_palette(rng)
                };
                (n, id)
            }
            CopyUp::Evolved { from, to, .. } => {
                let (Some(from_type), Some(to_type), Some(from_id)) = (
                    primary(game, from),
                    primary(game, to),
                    game.palettes.get(&from).and_then(|p| p.id),
                ) else {
                    continue;
                };
                let id = if settings.follow_types && to_type != from_type {
                    if type_palettes(to_type).contains(&from_id) {
                        from_id
                    } else {
                        random_type_palette(rng, to_type)
                    }
                } else {
                    from_id
                };
                (to, id)
            }
        };
        if let Some(p) = game.palettes.get_mut(&n) {
            p.id = Some(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{Evolution, EvolutionType, SpeciesPalettes};
    use rand_core::SeedableRng;

    use super::*;
    use crate::randomizers::testing::{gen1, species};

    fn game() -> GameData {
        let mut g = gen1(
            vec![
                species(1, Type::Grass, Some(Type::Poison), 45),
                species(2, Type::Grass, Some(Type::Poison), 60),
                species(4, Type::Fire, None, 39),
                species(5, Type::Water, None, 58),
            ],
            true,
        );
        g.evolutions = vec![
            Evolution::new(1, 2, EvolutionType::Level, 16),
            Evolution::new(4, 5, EvolutionType::Level, 16),
        ];
        for n in [1, 2, 4, 5] {
            g.palettes.insert(
                n,
                SpeciesPalettes {
                    id: Some(0),
                    ..SpeciesPalettes::default()
                },
            );
        }
        g
    }

    fn run(settings: &PaletteSettings, seed: u64) -> GameData {
        let mut g = game();
        randomize(&mut rand_chacha::ChaCha8Rng::seed_from_u64(seed), &mut g, settings);
        g
    }

    #[test]
    fn species_get_pokemon_palettes_that_follow_types_and_evolutions() {
        let id = |g: &GameData, n: DexNumber| g.palettes[&n].id.expect("has a palette");
        let settings = PaletteSettings {
            random: true,
            follow_types: true,
            follow_evolutions: true,
            shiny_from_normal: false,
        };
        for seed in 0..20 {
            let g = run(&settings, seed);
            assert_eq!(id(&g, 1), GREENMON);
            assert_eq!(id(&g, 2), GREENMON, "same type: the pre-evolution's palette");
            assert_eq!(id(&g, 4), REDMON);
            // The Fire palette doesn't fit Water, so the evolution gets a Water one.
            assert!([BLUEMON, CYANMON].contains(&id(&g, 5)));
        }
        let random = PaletteSettings {
            random: true,
            ..PaletteSettings::default()
        };
        let all: Vec<u8> = (0..20).map(|seed| id(&run(&random, seed), 4)).collect();
        assert!(all.iter().all(|i| POKEMON_PALETTES.contains(i)));
        assert!(all.iter().any(|&i| i != all[0]), "random palettes vary");
    }
}
