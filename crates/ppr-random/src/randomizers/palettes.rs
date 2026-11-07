//! New colors for every Pokémon, made by the `populator` from base colors (random, or following the species'
//! types), part by part as the palette descriptions (`data/palettes`) divide each palette. Evolutions can keep
//! their pre-evolution's colors, and the shiny palette can become the original normal one. Gen 1 and Gen 2 have
//! palettes of their own kind, randomized by `gen1` and `gen2`.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - Palettes are filled in national order.
//! - A species gets 16 base colors (palettes use at most 5).
//! - Random colors are drawn against their exact highest weight, not an estimate from samples, and when no color
//!   is found in the number of tries the last one is used instead of failing; likewise a type color when all of
//!   the type's colors are taken.
//! - Species past the end of the description file keep their colors.
//! - Gen 2's random bright and dark colors are drawn like the others: against their exact highest weight, the last
//!   try taken when none is kept.

mod base_colors;
mod description;
mod gen1;
mod gen2;
mod populator;

use std::collections::BTreeMap;

use ppr_model::{DexNumber, GameData};

use self::base_colors::{BaseColorMap, TypeBaseColorList, Types};
use self::description::{PartDescription, body};
use self::populator::{Populator, populate_average};
use super::copy_up::{self, CopyUp};
use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Pokémon Palettes";

/// The palette descriptions `pokePalettes<ID>.txt`, one line per species in national order.
fn descriptions(files: &str) -> Option<&'static str> {
    match files {
        "E" => Some(include_str!("../../data/palettes/pokePalettesE.txt")),
        "FRLG" => Some(include_str!("../../data/palettes/pokePalettesFRLG.txt")),
        "DP" => Some(include_str!("../../data/palettes/pokePalettesDP.txt")),
        "BW" => Some(include_str!("../../data/palettes/pokePalettesBW.txt")),
        _ => None,
    }
}

/// Randomizes Pokémon palettes according to [`ppr_settings::PaletteSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PaletteRandomizer;

impl Step<RunData> for PaletteRandomizer {
    fn id(&self) -> u64 {
        step_ids::PALETTES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.palettes.random && !data.game.palettes.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let settings = &data.settings.palettes;
        let game = &mut data.game;
        match game.identity.generation.number() {
            1 => {
                gen1::randomize(&mut rng, game, settings);
                return Ok(());
            }
            2 => {
                gen2::randomize(&mut rng, game, settings);
                return Ok(());
            }
            _ => {}
        }
        let file = game
            .palette_files
            .as_deref()
            .and_then(descriptions)
            .ok_or_else(|| RandomizeError::Impossible {
                step: LABEL,
                message: "this game's palettes are not described".into(),
            })?;
        let lines: Vec<&str> = file.lines().collect();
        let types =
            |g: &GameData, n: DexNumber| -> Option<Types> { g.species(n).map(|s| (s.primary_type, s.secondary_type)) };

        // Copy-up actions: base colors per species, evolutions from their pre-evolutions.
        let options = copy_up::Options {
            evolution_sanity: settings.follow_evolutions,
            copy_split_evos: true,
        };
        let set = game.palettes.keys().copied().collect();
        let mut lists: BTreeMap<DexNumber, TypeBaseColorList> = BTreeMap::new();
        let mut maps: Vec<BaseColorMap> = Vec::new();
        for step in copy_up::plan(game, &set, options) {
            let (n, list) = match step {
                // As in the original, alternate formes' palettes are left as they are.
                CopyUp::AltForme { .. } | CopyUp::Cosmetic { .. } => continue,
                CopyUp::Basic(n) => {
                    let Some(t) = types(game, n) else { continue };
                    maps.push(BaseColorMap::default());
                    (
                        n,
                        TypeBaseColorList::basic(&mut rng, t, settings.follow_types, maps.len() - 1),
                    )
                }
                CopyUp::Evolved { from, to, .. } => {
                    let (Some(prevo), Some(from_types), Some(to_types)) =
                        (lists.get(&from), types(game, from), types(game, to))
                    else {
                        continue;
                    };
                    (
                        to,
                        TypeBaseColorList::evolved(&mut rng, prevo, from_types, to_types, settings.follow_types),
                    )
                }
            };
            if settings.shiny_from_normal
                && let Some(p) = game.palettes.get_mut(&n)
            {
                p.shiny.clone_from(&p.normal);
            }
            lists.insert(n, list);
        }

        // Fill every species' palettes from its base colors.
        for (n, list) in &lists {
            let Some(line) = lines.get(usize::from(*n) - 1) else {
                continue;
            };
            let Some(palette) = game.palettes.get_mut(n) else {
                continue;
            };
            for (i, part) in PartDescription::all_from(&body(line)).iter().enumerate() {
                if part.is_average() {
                    populate_average(&mut palette.normal, part);
                } else if !part.blank
                    && let Some(&key) = list.colors.get(i)
                {
                    let (base, mode) = maps[list.map].get(&mut rng, key);
                    Populator::populate(&mut rng, &mut palette.normal, part, base, mode);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{Color, SpeciesPalettes, Type};
    use ppr_settings::{PaletteSettings, Settings};

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn game() -> GameData {
        let mut g = emerald(vec![
            species(1, Type::Grass, Some(Type::Poison), 45),
            species(2, Type::Grass, Some(Type::Poison), 60),
            species(4, Type::Fire, None, 39),
        ]);
        g.palette_files = Some("E".into());
        let palette: Vec<Color> = (0..16u8).map(|i| Color::rgb(i * 8, i * 8, i * 8)).collect();
        for n in [1, 2, 4] {
            g.palettes.insert(
                n,
                SpeciesPalettes {
                    normal: palette.clone(),
                    shiny: vec![Color::WHITE; 16],
                    id: None,
                },
            );
        }
        g
    }

    fn run(game: GameData, palettes: PaletteSettings, seed: u64) -> GameData {
        let mut data = RunData {
            game,
            settings: Settings {
                palettes,
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game
    }

    #[test]
    fn described_slots_change_and_the_rest_stays() {
        let before = game();
        let g = run(
            game(),
            PaletteSettings {
                random: true,
                shiny_from_normal: true,
                ..PaletteSettings::default()
            },
            1,
        );
        // Bulbasaur [3,4,5,6/12,13,14,15]: the background and undescribed slots keep their colors.
        let (old, new) = (&before.palettes[&1].normal, &g.palettes[&1].normal);
        assert_eq!(new[0], old[0]);
        assert_eq!(new[1], old[1]);
        assert_ne!(new[2..6], old[2..6]);
        assert_ne!(new[11..15], old[11..15]);
        assert_eq!(g.palettes[&1].shiny, *old, "shiny from the original normal palette");
    }

    #[test]
    fn each_family_describes_all_its_species() {
        for (files, species) in [("E", 386), ("FRLG", 386), ("DP", 493), ("BW", 649)] {
            let lines = descriptions(files).expect("described").lines().count();
            assert!(lines >= species, "{files}: {lines} lines");
        }
        assert_eq!(descriptions("XY"), None);
    }

    #[test]
    fn evolutions_follow_their_pre_evolutions() {
        let settings = PaletteSettings {
            random: true,
            follow_types: true,
            follow_evolutions: true,
            shiny_from_normal: false,
        };
        // Ivysaur's main part is its first description part, like Bulbasaur's; with following evolutions both
        // come from the same base color, so their hues are close.
        for seed in 0..5 {
            let g = run(game(), settings.clone(), seed);
            assert_eq!(g.palettes[&4].shiny, vec![Color::WHITE; 16]);
            assert_ne!(g.palettes[&2].normal, game().palettes[&2].normal);
        }
        let off = run(game(), PaletteSettings::default(), 1);
        assert_eq!(off.palettes, game().palettes);
    }
}
