//! Move tutor moves: move tutors teach random moves that no TM teaches.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).

use super::tm_moves::{Options, random_moves};
use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Move Tutor Moves";

/// Randomizes move tutor moves according to [`ppr_settings::TutorSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TutorMoveRandomizer;

impl Step<RunData> for TutorMoveRandomizer {
    fn id(&self) -> u64 {
        step_ids::TUTOR_MOVES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.tutors.random
            && data.settings.movesets.mode != ppr_settings::MovesetMode::MetronomeOnly
            && !data.game.tutor_moves.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = &data.settings.tutors;
        let options = Options {
            keep_field_moves: s.keep_field_moves,
            no_game_breaking: s.no_game_breaking,
            good_damaging_percent: s.good_damaging_percent,
        };
        let game = &data.game;
        let new = random_moves(game, &game.tutor_moves, &game.tm_moves, options, &mut rng).ok_or(
            RandomizeError::Impossible {
                step: LABEL,
                message: "not enough moves to choose from".into(),
            },
        )?;
        data.game.tutor_moves = new;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ppr_model::{GameData, Move, MoveCategory, MoveId, Type};
    use ppr_settings::{Settings, TmSettings, TutorSettings};

    use super::*;
    use crate::randomizers::testing::emerald;
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn game() -> GameData {
        let mut g = emerald(Vec::new());
        g.moves = (1..=354)
            .map(|n| Move::basic(n, "M", 80, Type::Normal, MoveCategory::Physical))
            .collect();
        g.hm_moves = vec![15];
        g.field_moves = vec![15, 91];
        g.tm_moves = (1..=50).collect();
        g.tutor_moves = std::iter::once(91).chain(100..129).collect();
        g
    }

    #[test]
    fn tutors_get_moves_no_tm_teaches() {
        let mut data = RunData {
            game: game(),
            settings: Settings {
                tms: TmSettings {
                    random: true,
                    ..TmSettings::default()
                },
                tutors: TutorSettings {
                    random: true,
                    keep_field_moves: true,
                    ..TutorSettings::default()
                },
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(4), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        let g = data.game;
        assert_eq!(g.tutor_moves.len(), 30);
        assert_eq!(g.tutor_moves[0], 91, "Dig stays");
        let tms: BTreeSet<MoveId> = g.tm_moves.iter().copied().collect();
        let tutors: BTreeSet<MoveId> = g.tutor_moves.iter().copied().collect();
        assert_eq!(tutors.len(), 30);
        assert!(tms.is_disjoint(&tutors), "the new TMs are left out");
        assert!(!tutors.contains(&15));
    }
}
