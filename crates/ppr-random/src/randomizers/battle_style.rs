//! Battle styles: trainers battle in a random or chosen style, with copies of their first Pokémon added until they have
//! enough for it. Trainers who always battle alongside another, and those who get no buffs (the first rival battle),
//! keep theirs.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - Gen 1 and 2 have only single battles, so their trainers keep their style and teams.
//! - A chosen style the game lacks (triple and rotation battles outside Gen 5 and 6) becomes double battles, so no
//!   trainer ends up with three Pokémon in a single battle.

use ppr_model::{BattleStyle, GameData};
use ppr_settings::BattleStyleMode;

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Battle Style";

/// Changes trainers' battle styles according to [`ppr_settings::BattleStyleSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct BattleStyleRandomizer;

impl Step<RunData> for BattleStyleRandomizer {
    fn id(&self) -> u64 {
        step_ids::BATTLE_STYLE
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.battle_style.mode != BattleStyleMode::Unchanged
            && !data.game.trainers.is_empty()
            && data.game.identity.generation.number() >= 3
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let settings = data.settings.battle_style.clone();
        modify(&mut data.game, settings.mode, settings.style, &mut rng);
        Ok(())
    }
}

/// The styles the game has: triple and rotation battles only in Gen 5 and 6.
fn styles(game: &GameData) -> &'static [BattleStyle] {
    const ALL: [BattleStyle; 4] = [
        BattleStyle::Single,
        BattleStyle::Double,
        BattleStyle::Triple,
        BattleStyle::Rotation,
    ];
    if (5..=6).contains(&game.identity.generation.number()) {
        &ALL
    } else {
        &ALL[..2]
    }
}

/// Gives every trainer that may change its new battle style, and Pokémon enough for it.
fn modify(game: &mut GameData, mode: BattleStyleMode, chosen: BattleStyle, rng: &mut StepRng) {
    let styles = styles(game);
    let chosen = if styles.contains(&chosen) {
        chosen
    } else {
        BattleStyle::Double
    };
    for trainer in &mut game.trainers {
        if trainer.multi_battle == ppr_model::MultiBattleStatus::Always || trainer.should_not_get_buffs() {
            continue;
        }
        trainer.battle_style = match mode {
            BattleStyleMode::Random => styles[below(rng, styles.len())],
            BattleStyleMode::Unchanged | BattleStyleMode::SingleStyle => chosen,
        };
        if let Some(first) = trainer.pokemon.first().cloned() {
            let needed = trainer.battle_style.required_pokemon();
            if trainer.pokemon.len() < needed {
                trainer.pokemon.resize(needed, first);
            }
        }
        trainer.forced_battle_style = true;
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{MultiBattleStatus, Trainer, TrainerPokemon};
    use ppr_settings::{BattleStyleSettings, Settings};

    use super::*;
    use crate::randomizers::testing::emerald;
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn run(mode: BattleStyleMode, style: BattleStyle, seed: u64) -> Vec<Trainer> {
        run_on(emerald(Vec::new()), mode, style, seed)
    }

    fn run_on(mut g: GameData, mode: BattleStyleMode, style: BattleStyle, seed: u64) -> Vec<Trainer> {
        g.trainers = (1..=40)
            .map(|i| Trainer::new(i, "T", 0, vec![TrainerPokemon::new(i, 5)]))
            .collect();
        g.trainers[0].multi_battle = MultiBattleStatus::Always;
        g.trainers[1].tag = Some("RIVAL1-0".into());
        g.trainers[2].multi_battle = MultiBattleStatus::Potential;
        let mut data = RunData {
            game: g,
            settings: Settings {
                battle_style: BattleStyleSettings { mode, style },
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game.trainers
    }

    #[test]
    fn gen1_has_only_single_battles() {
        let red = crate::randomizers::testing::gen1(Vec::new(), false);
        let trainers = run_on(red, BattleStyleMode::SingleStyle, BattleStyle::Double, 1);
        assert!(
            trainers
                .iter()
                .all(|t| t.battle_style == BattleStyle::Single && t.pokemon.len() == 1)
        );
    }

    #[test]
    fn single_style_doubles_everyone_who_may_change() {
        let trainers = run(BattleStyleMode::SingleStyle, BattleStyle::Double, 1);
        for t in &trainers[..2] {
            assert_eq!(t.battle_style, BattleStyle::Single);
            assert!(!t.forced_battle_style);
            assert_eq!(t.pokemon.len(), 1);
        }
        for t in &trainers[2..] {
            assert_eq!(t.battle_style, BattleStyle::Double);
            assert!(t.forced_battle_style);
            assert_eq!(t.pokemon, vec![TrainerPokemon::new(t.index, 5); 2]);
        }
    }

    #[test]
    fn random_styles_are_those_of_the_game() {
        let trainers = run(BattleStyleMode::Random, BattleStyle::Triple, 2);
        let doubles = trainers[2..]
            .iter()
            .filter(|t| t.battle_style == BattleStyle::Double)
            .count();
        assert!(doubles > 5 && doubles < 33, "{doubles}");
        assert!(trainers.iter().all(|t| t.battle_style <= BattleStyle::Double));
        // Gen 3 has no triple battles.
        let triples = run(BattleStyleMode::SingleStyle, BattleStyle::Triple, 3);
        assert!(triples[2..].iter().all(|t| t.battle_style == BattleStyle::Double));
        assert!(triples[2..].iter().all(|t| t.pokemon.len() == 2));
    }
}
