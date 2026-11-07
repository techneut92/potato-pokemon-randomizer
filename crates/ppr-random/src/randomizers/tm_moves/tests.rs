use std::collections::BTreeSet;

use ppr_model::{Move, MoveCategory, Type};
use ppr_settings::{Settings, TmSettings};

use super::*;
use crate::randomizers::testing::emerald;
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const CUT: MoveId = 15;
const FLY: MoveId = 19;
const DIG: MoveId = 91;

/// 354 moves, odd ones good damaging; TM01 to TM50 teach Dig, then moves 1 to 49.
fn game() -> GameData {
    let mut g = emerald(Vec::new());
    g.moves = (1..=354)
        .map(|n| {
            let power = if n % 2 == 1 { 80 } else { 0 };
            Move::basic(n, "M", power, Type::Normal, MoveCategory::Physical)
        })
        .collect();
    g.hm_moves = vec![CUT, FLY];
    g.field_moves = vec![CUT, FLY, DIG];
    g.game_breaking_moves = vec![49, 82];
    g.tm_moves = std::iter::once(DIG).chain(1..=49).collect();
    g
}

fn run_on(game: GameData, tms: TmSettings) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            tms,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(2), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn tms_get_distinct_allowed_moves() {
    let s = TmSettings {
        random: true,
        no_game_breaking: true,
        ..TmSettings::default()
    };
    let g = run_on(game(), s);
    assert_eq!(g.tm_moves.len(), 50);
    assert_ne!(g.tm_moves, game().tm_moves);
    let distinct: BTreeSet<MoveId> = g.tm_moves.iter().copied().collect();
    assert_eq!(distinct.len(), 50);
    for m in &g.tm_moves {
        assert!(![CUT, FLY, STRUGGLE, 49, 82].contains(m), "{m}");
    }
}

#[test]
fn field_move_tms_can_stay() {
    let s = TmSettings {
        random: true,
        keep_field_moves: true,
        ..TmSettings::default()
    };
    let g = run_on(game(), s);
    assert_eq!(g.tm_moves[0], DIG);
    assert_eq!(g.tm_moves.iter().filter(|&&m| m == DIG).count(), 1);
}

#[test]
fn good_damaging_moves_fill_the_share() {
    let s = TmSettings {
        random: true,
        good_damaging_percent: Some(100),
        ..TmSettings::default()
    };
    let g = run_on(game(), s);
    assert!(g.tm_moves.iter().all(|m| m % 2 == 1), "{:?}", g.tm_moves);
}

#[test]
fn unchanged_keeps_the_tms() {
    assert_eq!(run_on(game(), TmSettings::default()).tm_moves, game().tm_moves);
}
