use ppr_model::{Move, Type};
use ppr_settings::{MoveDataSettings, Settings};

use super::*;
use crate::randomizers::testing::emerald;
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const TRIPLE_KICK: u16 = 167;

fn game() -> GameData {
    let mut g = emerald(Vec::new());
    g.types = vec![Type::Normal, Type::Fire, Type::Water, Type::Grass];
    g.moves = (1..=354)
        .map(|n| Move::basic(n, &format!("M{n}"), 40, Type::Normal, MoveCategory::Physical))
        .collect();
    let at = |n: u16| usize::from(n) - 1;
    g.moves[at(1)].power = 0;
    g.moves[at(1)].category = MoveCategory::Status;
    g.moves[at(2)].accuracy = 30;
    g.moves[at(3)].accuracy = 70;
    g.moves[at(4)].accuracy = 0;
    g.moves[at(5)].move_type = None;
    g.moves[at(TRIPLE_KICK)].hit_count_hundredths = 271;
    g.moves[at(STRUGGLE)].power = 50;
    g
}

fn run_on(game: GameData, move_data: MoveDataSettings) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            move_data,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(7), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn all() -> MoveDataSettings {
    MoveDataSettings {
        power: true,
        accuracy: true,
        pp: true,
        types: true,
        category: true,
        names: false,
        update_to_generation: None,
    }
}

#[test]
fn values_stay_in_their_ranges() {
    let before = game();
    let g = run_on(game(), all());
    for (old, new) in before.moves.iter().zip(&g.moves) {
        if old.number == STRUGGLE {
            assert_eq!(old, new, "Struggle keeps its data");
            continue;
        }
        if old.power < 10 {
            assert_eq!(new.power, old.power, "{}", old.number);
        } else if old.hit_count_hundredths == 100 {
            assert!(
                (20..=250).contains(&new.power) && new.power.is_multiple_of(5),
                "{}: {}",
                old.number,
                new.power
            );
        }
        assert!(
            (5..=40).contains(&new.pp) && new.pp.is_multiple_of(5),
            "{}: {}",
            old.number,
            new.pp
        );
        match old.accuracy {
            0..=4 => assert_eq!(new.accuracy, old.accuracy),
            5..=50 => assert!((20..=75).contains(&new.accuracy) && new.accuracy.is_multiple_of(5)),
            _ => assert!((20..=100).contains(&new.accuracy) && new.accuracy.is_multiple_of(5)),
        }
        assert_eq!(new.move_type.is_none(), old.move_type.is_none(), "{}", old.number);
        assert_eq!(new.category, old.category, "no split in Gen 3");
    }
    assert!(
        g.moves
            .iter()
            .any(|m| m.move_type != Some(Type::Normal) && m.move_type.is_some())
    );
    assert_ne!(g.moves, before.moves);
}

#[test]
fn multi_hit_moves_get_less_power_per_hit() {
    let g = run_on(
        game(),
        MoveDataSettings {
            power: true,
            ..MoveDataSettings::default()
        },
    );
    let kick = g.moves[usize::from(TRIPLE_KICK) - 1].power;
    assert!((5..=95).contains(&kick) && kick.is_multiple_of(5), "{kick}");
}

#[test]
fn categories_swap_only_with_the_split() {
    let mut gen4 = game();
    gen4.identity.generation = ppr_model::Generation::new(4).expect("Gen 4");
    let settings = MoveDataSettings {
        category: true,
        ..MoveDataSettings::default()
    };
    let g = run_on(gen4, settings);
    assert!(g.moves.iter().any(|m| m.category == MoveCategory::Special));
    assert_eq!(g.moves[0].category, MoveCategory::Status);
}

#[test]
fn powers_per_hit_round_half_up() {
    assert_eq!(per_hit(75, 271), 30, "5.535 rounds to 6");
    assert_eq!(per_hit(100, 200), 50);
    assert_eq!(per_hit(25, 200), 15, "2.5 rounds up to 3");
    assert_eq!(per_hit(20, 500), 5, "never 0");
}
