use ppr_model::{Effectiveness, GameData, Type, TypeChart};
use ppr_settings::{Settings, TypeEffectivenessMode, TypeEffectivenessSettings};

use super::*;
use crate::randomizers::testing::emerald;
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// A small chart: Fire > Grass, Water > Fire, Grass > Water, Normal ✗ Ghost, Ghost ✗ Normal, Fire ½ Water,
/// Ghost ½ Steel, Dark ½ Steel.
fn game() -> GameData {
    let mut g = emerald(Vec::new());
    let types = Type::in_generation(g.identity.generation);
    let mut chart = TypeChart::neutral(&types);
    for (a, d, e) in [
        (Type::Fire, Type::Grass, Effectiveness::Double),
        (Type::Water, Type::Fire, Effectiveness::Double),
        (Type::Grass, Type::Water, Effectiveness::Double),
        (Type::Normal, Type::Ghost, Effectiveness::Zero),
        (Type::Ghost, Type::Normal, Effectiveness::Zero),
        (Type::Fire, Type::Water, Effectiveness::Half),
        (Type::Ghost, Type::Steel, Effectiveness::Half),
        (Type::Dark, Type::Steel, Effectiveness::Half),
    ] {
        chart.set(a, d, e);
    }
    g.type_chart = chart;
    g
}

fn run(s: TypeEffectivenessSettings, seed: u64) -> TypeChart {
    let mut data = RunData {
        game: game(),
        settings: Settings {
            type_effectiveness: s,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game.type_chart
}

fn counts(chart: &TypeChart) -> [usize; 4] {
    let mut c = [0; 4];
    for &a in chart.types() {
        for &d in chart.types() {
            c[slot(chart.get(a, d))] += 1;
        }
    }
    c
}

#[test]
fn update_makes_steel_take_ghost_and_dark_neutrally() {
    let chart = run(
        TypeEffectivenessSettings {
            update: true,
            ..TypeEffectivenessSettings::default()
        },
        1,
    );
    assert_eq!(chart.get(Type::Ghost, Type::Steel), Some(Effectiveness::Neutral));
    assert_eq!(chart.get(Type::Dark, Type::Steel), Some(Effectiveness::Neutral));
    assert_eq!(chart.get(Type::Fire, Type::Grass), Some(Effectiveness::Double));
}

#[test]
fn random_charts_keep_the_counts() {
    for (mode, seed) in [
        (TypeEffectivenessMode::Random, 1),
        (TypeEffectivenessMode::RandomBalanced, 2),
        (TypeEffectivenessMode::KeepIdentities, 3),
    ] {
        let chart = run(
            TypeEffectivenessSettings {
                mode,
                ..TypeEffectivenessSettings::default()
            },
            seed,
        );
        assert_eq!(counts(&chart), counts(&game().type_chart), "{mode:?}");
    }
}

#[test]
fn balanced_charts_give_no_type_more_than_the_most_before() {
    for seed in 0..10 {
        let chart = run(
            TypeEffectivenessSettings {
                mode: TypeEffectivenessMode::RandomBalanced,
                ..TypeEffectivenessSettings::default()
            },
            seed,
        );
        for &t in chart.types() {
            assert!(chart.when_attacking(t, Effectiveness::Zero).len() <= 1, "seed {seed}");
            assert!(chart.when_defending(t, Effectiveness::Half).len() <= 2, "seed {seed}");
        }
    }
}

#[test]
fn inverse_swaps_weaknesses_and_resistances() {
    let chart = run(
        TypeEffectivenessSettings {
            mode: TypeEffectivenessMode::Inverse,
            ..TypeEffectivenessSettings::default()
        },
        1,
    );
    assert_eq!(chart.get(Type::Fire, Type::Grass), Some(Effectiveness::Half));
    assert_eq!(chart.get(Type::Normal, Type::Ghost), Some(Effectiveness::Double));
    assert_eq!(chart.get(Type::Fire, Type::Water), Some(Effectiveness::Double));
    let with_immunities = run(
        TypeEffectivenessSettings {
            mode: TypeEffectivenessMode::Inverse,
            random_immunities: true,
            ..TypeEffectivenessSettings::default()
        },
        2,
    );
    let former = [
        (Type::Fire, Type::Grass),
        (Type::Water, Type::Fire),
        (Type::Grass, Type::Water),
    ];
    let zeros = former
        .iter()
        .filter(|&&(a, d)| with_immunities.get(a, d) == Some(Effectiveness::Zero))
        .count();
    assert_eq!(zeros, 2, "two immunities before, two former weaknesses now");
}
