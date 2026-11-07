use ppr_model::{Evolution, EvolutionType, GameData, LevelUpMove, Move, MoveCategory, Type};
use ppr_settings::{CompatMode, Settings, TmHmCompatSettings};

use super::*;
use crate::randomizers::testing::{emerald, species};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Three species, 1 evolving into 2; TMs 1–4 (TM 1 Normal, TM 2 Fire, others Water) and HM 5 (Rock Smash's
/// number stands in for the early HM).
fn game() -> GameData {
    let mut g = emerald(vec![
        species(1, Type::Fire, None, 50),
        species(2, Type::Fire, Some(Type::Water), 60),
        species(3, Type::Grass, None, 50),
    ]);
    g.evolutions = vec![Evolution::new(1, 2, EvolutionType::Level, 16)];
    g.moves = vec![
        Move::basic(1, "N", 40, Type::Normal, MoveCategory::Physical),
        Move::basic(2, "F", 40, Type::Fire, MoveCategory::Special),
        Move::basic(3, "W", 40, Type::Water, MoveCategory::Special),
        Move::basic(4, "W2", 40, Type::Water, MoveCategory::Special),
        Move::basic(5, "H", 40, Type::Fighting, MoveCategory::Physical),
    ];
    g.tm_moves = vec![1, 2, 3, 4];
    g.hm_moves = vec![5];
    g.early_required_hm_moves = vec![5];
    g.tm_hm_compat = (1..=3).map(|n| (n, vec![false; 5])).collect();
    g.learnsets = (1..=3).map(|n| (n, vec![LevelUpMove::new(1, 3)])).collect();
    g
}

fn run(game: GameData, s: TmHmCompatSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            tm_hm_compat: s,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn full_compatibility_teaches_everything() {
    let g = run(
        game(),
        TmHmCompatSettings {
            mode: CompatMode::Full,
            ..TmHmCompatSettings::default()
        },
        1,
    );
    assert!(g.tm_hm_compat.values().all(|f| f.iter().all(|&b| b)));
}

#[test]
fn full_hm_and_level_up_sanity_only_touch_their_flags() {
    let g = run(
        game(),
        TmHmCompatSettings {
            level_up_sanity: true,
            full_hm: true,
            ..TmHmCompatSettings::default()
        },
        1,
    );
    for flags in g.tm_hm_compat.values() {
        assert_eq!(flags, &[false, false, true, false, true]);
    }
}

#[test]
fn evolutions_keep_what_their_pre_evolutions_learn() {
    for seed in 0..20 {
        let g = run(
            game(),
            TmHmCompatSettings {
                mode: CompatMode::RandomCompletely,
                follow_evolutions: true,
                ..TmHmCompatSettings::default()
            },
            seed,
        );
        let (from, to) = (&g.tm_hm_compat[&1], &g.tm_hm_compat[&2]);
        assert!(from.iter().zip(to).all(|(&f, &t)| !f || t), "seed {seed}");
    }
}

#[test]
fn same_type_moves_are_likelier() {
    let options = Options {
        prefer_same_type: true,
        follow_evolutions: false,
        copy_split_evos: false,
    };
    let p = |m: Option<Type>, early| probability(Type::Fire, Some(Type::Water), m, early, options);
    assert!((p(Some(Type::Water), false) - 0.9).abs() < 1e-9);
    assert!((p(Some(Type::Normal), false) - 0.5).abs() < 1e-9);
    assert!((p(Some(Type::Grass), false) - 0.25).abs() < 1e-9);
    assert!((p(Some(Type::Grass), true) - 0.45).abs() < 1e-9);
    assert!((p(Some(Type::Fire), true) - 1.0).abs() < 1e-9);
    let random = Options {
        prefer_same_type: false,
        ..options
    };
    assert!((probability(Type::Fire, None, Some(Type::Fire), false, random) - 0.5).abs() < 1e-9);
}

#[test]
fn random_compatibility_changes_flags() {
    let g = run(
        game(),
        TmHmCompatSettings {
            mode: CompatMode::RandomPreferSameType,
            ..TmHmCompatSettings::default()
        },
        3,
    );
    assert!(g.tm_hm_compat.values().flatten().any(|&b| b));
    assert!(g.tm_hm_compat.values().all(|f| f.len() == 5));
}

#[test]
fn tutor_compatibility_follows_level_up_moves_and_evolutions() {
    let mut g = game();
    g.tutor_moves = vec![2, 3, 4];
    g.tutor_compat = (1..=3).map(|n| (n, vec![false; 3])).collect();
    let mut data = RunData {
        game: g,
        settings: Settings {
            tutor_compat: ppr_settings::TutorCompatSettings {
                mode: CompatMode::RandomCompletely,
                level_up_sanity: true,
                follow_evolutions: true,
            },
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(5), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    let c = &data.game.tutor_compat;
    assert!(
        c.values().all(|f| f.len() == 3 && f[1]),
        "everyone learns move 3 by level"
    );
    assert!(c[&1].iter().zip(&c[&2]).all(|(&f, &t)| !f || t));
    assert_eq!(data.game.tm_hm_compat, game().tm_hm_compat, "TM/HM flags stay");
}
