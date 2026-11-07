//! Move data randomization: random power, accuracy, PP, types and categories. Struggle keeps its own.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - A very inaccurate move's 10% chance of 50% more accuracy rounds down to a multiple of 5 (say 52.5% becomes
//!   50%), so accuracies stay multiples of 5.

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};
use ppr_model::{GameData, MoveCategory, STRUGGLE};

const LABEL: &str = "Move Data";

/// Randomizes move data according to [`ppr_settings::MoveDataSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MoveDataRandomizer;

impl Step<RunData> for MoveDataRandomizer {
    fn id(&self) -> u64 {
        step_ids::MOVE_DATA
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.move_data;
        (s.power || s.accuracy || s.pp || s.types || s.category) && !data.game.moves.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.move_data.clone();
        let game = &mut data.game;
        if s.power {
            powers(game, &mut rng);
        }
        if s.accuracy {
            accuracies(game, &mut rng);
        }
        if s.pp {
            pps(game, &mut rng);
        }
        if s.types {
            types(game, &mut rng);
        }
        // Moves have their own physical/special category from Gen 4 on.
        if s.category && game.identity.generation.number() >= 4 {
            categories(game, &mut rng);
        }
        Ok(())
    }
}

/// A random number below `n`, as a `u16`.
fn below_u16(rng: &mut StepRng, n: u16) -> u16 {
    u16::try_from(below(rng, usize::from(n))).unwrap_or_default()
}

/// Random powers: damaging moves (power 10 or more) get 50–100, or a third of the time 20–150, in
/// steps of 5, with two 1% chances of 50 more; moves hitting more than once divide it by their hits.
fn powers(game: &mut GameData, rng: &mut StepRng) {
    for mv in game.moves.iter_mut().filter(|m| m.number != STRUGGLE && m.power >= 10) {
        // Two thirds of the time 50–100.
        let mut power = if below(rng, 3) == 2 {
            below_u16(rng, 27) * 5 + 20
        } else {
            below_u16(rng, 11) * 5 + 50
        };
        for _ in 0..2 {
            if below(rng, 100) == 0 {
                power += 50;
            }
        }
        if mv.hit_count_hundredths != 100 {
            power = per_hit(power, mv.hit_count_hundredths);
        }
        mv.power = power;
    }
}

/// The power per hit of a move hitting `hits_hundredths` / 100 times: power / hits / 5 rounded (halves up),
/// times 5, and at least 5.
fn per_hit(power: u16, hits_hundredths: u16) -> u16 {
    let hits = f64::from(hits_hundredths) / 100.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "power is at most 250, hits at least one"
    )]
    let fifths = (f64::from(power) / hits / 5.0 + 0.5).floor() as u16;
    (fifths * 5).max(5)
}

/// Random PP: 15–25, or a third of the time 5–40, in steps of 5.
fn pps(game: &mut GameData, rng: &mut StepRng) {
    for mv in game.moves.iter_mut().filter(|m| m.number != STRUGGLE) {
        mv.pp = if below(rng, 3) == 2 {
            u8::try_from(below(rng, 8) * 5 + 5).unwrap_or(5)
        } else {
            u8::try_from(below(rng, 3) * 5 + 15).unwrap_or(15)
        };
    }
}

/// Random accuracies, for moves that can miss (accuracy 5 or more), in three tiers by their
/// accuracy: up to 50% gets 20–50% with a 10% chance of half again; below 90% counts down from 100% in steps of
/// 5, stopping each step with 20% chance; the rest likewise with 40% chance. Never below 20%.
fn accuracies(game: &mut GameData, rng: &mut StepRng) {
    for mv in game
        .moves
        .iter_mut()
        .filter(|m| m.number != STRUGGLE && m.accuracy >= 5)
    {
        mv.accuracy = if mv.accuracy <= 50 {
            let mut acc = u8::try_from(below(rng, 7) * 5 + 20).unwrap_or(20);
            if below(rng, 10) == 0 {
                acc = acc * 3 / 2 / 5 * 5;
            }
            acc
        } else {
            let stop_in_ten = if mv.accuracy < 90 { 2 } else { 4 };
            let mut acc = 100;
            while acc > 20 {
                if below(rng, 10) < stop_in_ten {
                    break;
                }
                acc -= 5;
            }
            acc
        };
    }
}

/// Random types: any type of the game, but not for typeless moves.
fn types(game: &mut GameData, rng: &mut StepRng) {
    let all = game.types.clone();
    if all.is_empty() {
        return;
    }
    for mv in game
        .moves
        .iter_mut()
        .filter(|m| m.number != STRUGGLE && m.move_type.is_some())
    {
        mv.move_type = Some(all[below(rng, all.len())]);
    }
}

/// Random categories: each physical or special move swaps with 50% chance.
fn categories(game: &mut GameData, rng: &mut StepRng) {
    for mv in game
        .moves
        .iter_mut()
        .filter(|m| m.number != STRUGGLE && m.category != MoveCategory::Status)
    {
        if below(rng, 2) == 0 {
            mv.category = if mv.category == MoveCategory::Physical {
                MoveCategory::Special
            } else {
                MoveCategory::Physical
            };
        }
    }
}

#[cfg(test)]
mod tests;
