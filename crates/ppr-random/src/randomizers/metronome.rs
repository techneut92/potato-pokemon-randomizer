//! Metronome Only mode: every species learns only Metronome at level 1, trainers' Pokémon know their level-up
//! moves (so Metronome), every TM and move tutor teaches Metronome, Metronome has 40 PP and HM moves none, so they
//! only work outside battle. Egg moves stay. It runs after the trainer steps, so those still choose from the real
//! moves.

use ppr_model::{LevelUpMove, MoveId};
use ppr_settings::MovesetMode;

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

/// Metronome's move number.
const METRONOME: MoveId = 118;

/// Applies Metronome Only Mode ([`MovesetMode::MetronomeOnly`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct MetronomeOnlyMode;

impl Step<RunData> for MetronomeOnlyMode {
    fn id(&self) -> u64 {
        step_ids::METRONOME_ONLY
    }

    fn label(&self) -> &'static str {
        "Metronome Only"
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.movesets.mode == MovesetMode::MetronomeOnly && !data.game.learnsets.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let game = &mut ctx.data.game;
        for learnset in game.learnsets.values_mut().filter(|l| !l.is_empty()) {
            *learnset = vec![LevelUpMove::new(1, METRONOME)];
        }
        for tp in game.trainers.iter_mut().flat_map(|t| &mut t.pokemon) {
            tp.reset_moves = true;
            tp.moves = [0; 4];
        }
        game.tm_moves.fill(METRONOME);
        game.tutor_moves.fill(METRONOME);
        let hms = game.hm_moves.clone();
        for mv in &mut game.moves {
            if mv.number == METRONOME {
                mv.pp = 40;
            } else if hms.contains(&mv.number) {
                mv.pp = 0;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{Move, MoveCategory, Trainer, TrainerPokemon, Type};
    use ppr_settings::{MovesetSettings, Settings, TmSettings};

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    #[test]
    fn everything_teaches_metronome() {
        let mut g = emerald(vec![
            species(1, Type::Normal, None, 50),
            species(2, Type::Normal, None, 50),
        ]);
        g.moves = (1..=354)
            .map(|n| Move::basic(n, "M", 40, Type::Normal, MoveCategory::Physical))
            .collect();
        g.learnsets
            .insert(1, vec![LevelUpMove::new(1, 33), LevelUpMove::new(9, 45)]);
        g.learnsets.insert(2, Vec::new());
        g.egg_moves.insert(1, vec![44]);
        g.tm_moves = vec![1, 2, 3];
        g.tutor_moves = vec![4, 5];
        g.hm_moves = vec![15, 19];
        let mut tp = TrainerPokemon::new(1, 10);
        tp.moves = [33, 45, 0, 0];
        g.trainers = vec![Trainer::new(1, "T", 0, vec![tp])];
        let mut data = RunData {
            game: g,
            settings: Settings {
                movesets: MovesetSettings {
                    mode: MovesetMode::MetronomeOnly,
                    ..MovesetSettings::default()
                },
                tms: TmSettings {
                    random: true,
                    ..TmSettings::default()
                },
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        let g = data.game;
        assert_eq!(g.learnsets[&1], [LevelUpMove::new(1, METRONOME)]);
        assert!(g.learnsets[&2].is_empty(), "species without moves keep none");
        assert_eq!(g.egg_moves[&1], [44]);
        assert_eq!(g.tm_moves, [METRONOME; 3]);
        assert_eq!(g.tutor_moves, [METRONOME; 2]);
        assert!(g.trainers[0].pokemon[0].reset_moves);
        assert_eq!(g.trainers[0].pokemon[0].moves, [0; 4]);
        assert_eq!(g.moves[usize::from(METRONOME) - 1].pp, 40);
        assert_eq!((g.moves[14].pp, g.moves[18].pp, g.moves[15].pp), (0, 0, 20));
    }
}
