//! Species type randomization.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - A second type is drawn once from the types other than the first, rather than redrawn until it differs; the
//!   same distribution.
//!
//! An alternate forme gets its base species' types, and with a chance of [`GSTC_ALT_FORME`] a new second type; a
//! cosmetic forme gets its base species' types. With "Follow Mega Evolutions" and types that follow evolutions, a
//! Mega Evolution counts as an evolution of its base species: it keeps its types, and gets a second type with a
//! chance of [`GSTC_MEGA_EVO`] when it has none. Charizard's and Mewtwo's two Mega Evolutions are split evolutions,
//! which get random types of their own.

use ppr_model::{DexNumber, GameData, Type};
use ppr_settings::{TypeMode, TypeSettings};

use super::copy_up::{self, CopyUp};
use crate::choice::{below, unit};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Types";

/// The chances of a second type: for species without an evolution, and for those with one.
const GSTC_NO_EVO: f64 = 0.5;
const GSTC_HAS_EVO: f64 = 0.35;
const GSTC_MIDDLE_EVO: f64 = 0.15;
const GSTC_FINAL_EVO: f64 = 0.25;
const GSTC_MEGA_EVO: f64 = 0.25;
const GSTC_ALT_FORME: f64 = 0.5;

/// Randomizes species' types according to [`TypeSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TypeRandomizer;

impl Step<RunData> for TypeRandomizer {
    fn id(&self) -> u64 {
        step_ids::TYPES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.types.mode != TypeMode::Unchanged
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.types.clone();
        data.game.keep_original_types();
        randomize(&mut data.game, &s, &mut rng)
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// Random types, following evolutions as chosen.
fn randomize(game: &mut GameData, s: &TypeSettings, rng: &mut StepRng) -> Result<(), RandomizeError> {
    let types = game.types.clone();
    if types.is_empty() {
        return Err(impossible("the game has no types"));
    }
    let evolution_sanity = s.mode == TypeMode::FollowEvolutions;
    let all = game.species.iter().map(|sp| sp.number).collect();
    let options = copy_up::Options {
        evolution_sanity,
        copy_split_evos: false,
    };
    for step in copy_up::plan_with_megas(game, &all, options, s.follow_megas) {
        let (n, primary, secondary) = match step {
            CopyUp::Basic(n) => {
                let chance = if evolution_sanity && game.evolutions.iter().filter(|e| e.from == n).count() == 1 {
                    GSTC_HAS_EVO
                } else {
                    GSTC_NO_EVO
                };
                let primary = types[below(rng, types.len())];
                (n, primary, second_type(rng, &types, primary, chance, s.force_dual))
            }
            CopyUp::Evolved {
                from, to, final_evo, ..
            } => {
                let pre = game
                    .species(from)
                    .ok_or_else(|| impossible(format!("no species {from}")))?;
                let (primary, mut secondary) = (pre.primary_type, pre.secondary_type);
                if secondary.is_none() {
                    let chance = if game.species(to).is_some_and(ppr_model::Species::is_mega) {
                        GSTC_MEGA_EVO
                    } else if final_evo {
                        GSTC_FINAL_EVO
                    } else {
                        GSTC_MIDDLE_EVO
                    };
                    secondary = second_type(rng, &types, primary, chance, false);
                }
                (to, primary, secondary)
            }
            CopyUp::AltForme { base, forme } => {
                let (primary, secondary) = types_of(game, base)?;
                let new = second_type(rng, &types, primary, GSTC_ALT_FORME, false);
                (forme, primary, new.or(secondary))
            }
            CopyUp::Cosmetic { base, forme } => {
                let (primary, secondary) = types_of(game, base)?;
                (forme, primary, secondary)
            }
        };
        set_types(game, n, primary, secondary)?;
    }
    Ok(())
}

/// A random second type: with `chance`, or always when `force`, a type other than `primary`.
fn second_type(rng: &mut StepRng, types: &[Type], primary: Type, chance: f64, force: bool) -> Option<Type> {
    if unit(rng) < chance || force {
        let others: Vec<Type> = types.iter().copied().filter(|&t| t != primary).collect();
        (!others.is_empty()).then(|| others[below(rng, others.len())])
    } else {
        None
    }
}

fn types_of(game: &GameData, n: DexNumber) -> Result<(Type, Option<Type>), RandomizeError> {
    let sp = game.species(n).ok_or_else(|| impossible(format!("no species {n}")))?;
    Ok((sp.primary_type, sp.secondary_type))
}

fn set_types(game: &mut GameData, n: DexNumber, primary: Type, secondary: Option<Type>) -> Result<(), RandomizeError> {
    let sp = game
        .species
        .iter_mut()
        .find(|sp| sp.number == n)
        .ok_or_else(|| impossible(format!("no species {n}")))?;
    sp.primary_type = primary;
    sp.secondary_type = secondary;
    Ok(())
}

#[cfg(test)]
mod tests {
    use ppr_model::{Evolution, EvolutionType};
    use ppr_settings::Settings;

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    /// Species 1–40, all Normal; lines 1 → 2 → 3 and 4 → 5, and 6 evolving into 7 and 8.
    fn game() -> GameData {
        let mut g = emerald((1..=40).map(|n| species(n, Type::Normal, None, 50)).collect());
        g.evolutions = [(1, 2), (2, 3), (4, 5), (6, 7), (6, 8)]
            .into_iter()
            .map(|(from, to)| Evolution::new(from, to, EvolutionType::Level, 20))
            .collect();
        g
    }

    fn run_on(game: GameData, types: TypeSettings, seed: u64) -> GameData {
        let mut data = RunData {
            game,
            settings: Settings {
                types,
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game
    }

    fn types_of(g: &GameData, n: DexNumber) -> (Type, Option<Type>) {
        let sp = g.species(n).expect("there");
        (sp.primary_type, sp.secondary_type)
    }

    #[test]
    fn evolutions_keep_their_pre_evolutions_types() {
        let s = TypeSettings {
            mode: TypeMode::FollowEvolutions,
            force_dual: false,
            follow_megas: false,
        };
        for seed in 0..20 {
            let g = run_on(game(), s.clone(), seed);
            for (from, to) in [(1, 2), (2, 3), (4, 5)] {
                let (a, b) = (types_of(&g, from), types_of(&g, to));
                assert_eq!(a.0, b.0, "{seed}: {from} -> {to}");
                if a.1.is_some() {
                    assert_eq!(a.1, b.1, "{seed}: {from} -> {to}");
                }
            }
            for sp in &g.species {
                assert_ne!(Some(sp.primary_type), sp.secondary_type, "{}", sp.name);
                assert!(g.types.contains(&sp.primary_type));
            }
        }
    }

    #[test]
    fn random_types_vary_and_can_be_forced_dual() {
        let g = run_on(
            game(),
            TypeSettings {
                mode: TypeMode::Random,
                force_dual: false,
                follow_megas: false,
            },
            1,
        );
        let distinct: std::collections::BTreeSet<Type> = g.species.iter().map(|sp| sp.primary_type).collect();
        assert!(distinct.len() > 5, "{distinct:?}");
        let singles = g.species.iter().filter(|sp| sp.secondary_type.is_none()).count();
        assert!((5..35).contains(&singles), "about half single-typed: {singles}");

        let dual = run_on(
            game(),
            TypeSettings {
                mode: TypeMode::Random,
                force_dual: true,
                follow_megas: false,
            },
            1,
        );
        assert!(dual.species.iter().all(|sp| sp.secondary_type.is_some()));
    }

    /// [`game`] with Mega Evolutions: 900 of species 3, and 901 and 902 of species 40.
    fn mega_game() -> GameData {
        let mut g = game();
        for (n, base, number) in [(900, 3, 1), (901, 40, 1), (902, 40, 2)] {
            let mut sp = species(n, Type::Normal, None, 50);
            sp.forme = ppr_model::Forme {
                base: Some(base),
                number,
                mega: true,
                ..ppr_model::Forme::default()
            };
            g.species.push(sp);
        }
        g
    }

    #[test]
    fn mega_evolutions_keep_their_base_species_types_when_they_follow() {
        let s = TypeSettings {
            mode: TypeMode::FollowEvolutions,
            force_dual: false,
            follow_megas: true,
        };
        let (mut second, mut own) = (0, false);
        for seed in 0..40 {
            let g = run_on(mega_game(), s.clone(), seed);
            let (base, mega) = (types_of(&g, 3), types_of(&g, 900));
            assert_eq!(base.0, mega.0, "{seed}");
            if base.1.is_some() {
                assert_eq!(base.1, mega.1, "{seed}");
            } else if mega.1.is_some() {
                second += 1;
            }
            // Two Mega Evolutions of one species split, so they get types of their own.
            own |= [901, 902].iter().any(|&n| types_of(&g, n).0 != types_of(&g, 40).0);
        }
        assert!(second > 0, "a Mega Evolution may add a second type");
        assert!(own);
    }

    #[test]
    fn mega_evolutions_are_alternate_formes_unless_they_follow() {
        // As alternate formes they keep the primary type; split ones too.
        for (mode, follow_megas) in [(TypeMode::FollowEvolutions, false), (TypeMode::Random, true)] {
            let s = TypeSettings {
                mode,
                force_dual: false,
                follow_megas,
            };
            for seed in 0..10 {
                let g = run_on(mega_game(), s.clone(), seed);
                for (base, n) in [(3, 900), (40, 901), (40, 902)] {
                    assert_eq!(types_of(&g, base).0, types_of(&g, n).0, "{seed}");
                }
            }
        }
    }

    #[test]
    fn unchanged_changes_nothing() {
        assert_eq!(run_on(game(), TypeSettings::default(), 1), game());
    }
}
