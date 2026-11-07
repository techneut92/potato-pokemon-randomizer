//! TM moves: TMs teach random moves.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).

use ppr_model::{GameData, MoveId, STRUGGLE};

use super::common::{BANNED_FOR_DAMAGING, is_good_damaging, perfect_accuracy};
use crate::choice::{below, shuffle};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "TM Moves";

/// Randomizes TM moves according to [`ppr_settings::TmSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TmMoveRandomizer;

impl Step<RunData> for TmMoveRandomizer {
    fn id(&self) -> u64 {
        step_ids::TM_MOVES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.tms.random
            && data.settings.movesets.mode != ppr_settings::MovesetMode::MetronomeOnly
            && !data.game.tm_moves.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = &data.settings.tms;
        let options = Options {
            keep_field_moves: s.keep_field_moves,
            no_game_breaking: s.no_game_breaking,
            good_damaging_percent: s.good_damaging_percent,
        };
        let game = &data.game;
        let new = random_moves(game, &game.tm_moves, &[], options, &mut rng).ok_or(RandomizeError::Impossible {
            step: LABEL,
            message: "not enough moves to choose from".into(),
        })?;
        data.game.tm_moves = new;
        Ok(())
    }
}

/// The options TM and move tutor moves share.
#[derive(Debug, Clone, Copy)]
pub(super) struct Options {
    pub(super) keep_field_moves: bool,
    pub(super) no_game_breaking: bool,
    pub(super) good_damaging_percent: Option<u8>,
}

/// Shared by TM and move tutor moves: new moves for `old`, never one of
/// `excluded`, an HM move or Struggle, and each move once; field moves stay if asked. `None` when there are
/// too few moves.
pub(super) fn random_moves(
    game: &GameData,
    old: &[MoveId],
    excluded: &[MoveId],
    options: Options,
    rng: &mut StepRng,
) -> Option<Vec<MoveId>> {
    let kept = |m: &MoveId| options.keep_field_moves && game.field_moves.contains(m);
    let banned = |m: MoveId| {
        m == STRUGGLE
            || game.hm_moves.contains(&m)
            || game.banned_level_up_moves.contains(&m)
            || excluded.contains(&m)
            || (options.no_game_breaking && game.game_breaking_moves.contains(&m))
            || (options.keep_field_moves && old.contains(&m) && game.field_moves.contains(&m))
    };
    let perfect = perfect_accuracy(game);
    let mut usable: Vec<MoveId> = game.moves.iter().map(|m| m.number).filter(|&m| !banned(m)).collect();
    let mut damaging: Vec<MoveId> = game
        .moves
        .iter()
        .filter(|m| !banned(m.number) && !BANNED_FOR_DAMAGING.contains(&m.number) && is_good_damaging(m, perfect))
        .map(|m| m.number)
        .collect();

    let count = old.iter().filter(|m| !kept(m)).count();
    let share = f64::from(options.good_damaging_percent.unwrap_or(0)) / 100.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "at most a few hundred"
    )]
    let mut good_left = (share * count as f64 + 0.5).floor() as usize;
    let mut picked = Vec::with_capacity(count);
    for _ in 0..count {
        let from = if good_left > 0 && !damaging.is_empty() {
            &damaging
        } else {
            &usable
        };
        if from.is_empty() {
            return None;
        }
        let mv = from[below(rng, from.len())];
        picked.push(mv);
        usable.retain(|&m| m != mv);
        damaging.retain(|&m| m != mv);
        good_left = good_left.saturating_sub(1);
    }
    // Forced damaging moves would otherwise come first.
    shuffle(rng, &mut picked);
    let mut picked = picked.into_iter();
    old.iter()
        .map(|m| if kept(m) { Some(*m) } else { picked.next() })
        .collect()
}

#[cfg(test)]
mod tests;
