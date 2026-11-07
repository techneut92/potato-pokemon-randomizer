use std::collections::BTreeSet;

use ppr_model::Type;
use ppr_settings::Settings;

use super::*;
use crate::randomizers::testing::{emerald, species};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const SONIC_BOOM: MoveId = 49;
const CUT: MoveId = 15;

/// 354 moves: odd ones damaging (Fire, physical, 60–120 power), even ones status (Normal); Cut is an HM.
fn game() -> GameData {
    let mut g = emerald((1..=30).map(|n| species(n, Type::Fire, None, 50)).collect());
    g.moves = (1..=354)
        .map(|n| {
            if n % 2 == 1 {
                Move::basic(n, "D", 60 + n % 61, Type::Fire, MoveCategory::Physical)
            } else {
                Move::basic(n, "S", 0, Type::Normal, MoveCategory::Status)
            }
        })
        .collect();
    g.hm_moves = vec![CUT];
    g.game_breaking_moves = vec![SONIC_BOOM, 82];
    g.four_starting_moves = true;
    for n in 1..=30 {
        g.learnsets.insert(
            n,
            vec![
                LevelUpMove::new(1, 2),
                LevelUpMove::new(5, 4),
                LevelUpMove::new(10, 6),
                LevelUpMove::new(20, 8),
                LevelUpMove::new(30, 10),
            ],
        );
    }
    g.egg_moves.insert(1, vec![2, 4, 6]);
    g
}

fn run_on(game: GameData, movesets: MovesetSettings) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            movesets,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(5), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn random(mode: MovesetMode) -> MovesetSettings {
    MovesetSettings {
        mode,
        no_game_breaking: true,
        ..MovesetSettings::default()
    }
}

fn is_damaging(n: MoveId) -> bool {
    n % 2 == 1
}

#[test]
fn learnsets_get_new_distinct_moves_with_an_attack_at_level_one() {
    let before = game();
    let g = run_on(game(), random(MovesetMode::RandomCompletely));
    assert_ne!(g.learnsets, before.learnsets);
    for (n, moves) in &g.learnsets {
        let levels: Vec<u8> = moves.iter().map(|m| m.level).collect();
        assert_eq!(levels, [1, 5, 10, 20, 30], "{n}: levels stay");
        let ids: BTreeSet<MoveId> = moves.iter().map(|m| m.move_id).collect();
        assert_eq!(ids.len(), moves.len(), "{n}: no repeats");
        assert!(is_damaging(moves[0].move_id), "{n}: level 1 attack");
        for &id in &ids {
            assert!(![STRUGGLE, CUT, SONIC_BOOM, 82, 0].contains(&id), "{n}: {id}");
        }
    }
    assert_eq!(g.egg_moves[&1].len(), 3);
    assert_ne!(g.egg_moves, before.egg_moves);
}

#[test]
fn guaranteed_level_one_moves_add_moves_at_level_one() {
    let s = MovesetSettings {
        guaranteed_level_one_moves: Some(4),
        ..random(MovesetMode::RandomPreferSameType)
    };
    let g = run_on(game(), s);
    for moves in g.learnsets.values() {
        assert_eq!(moves.len(), 8);
        assert_eq!(moves.iter().filter(|m| m.level == 1).count(), 4);
        assert!(is_damaging(moves[3].move_id), "the last level 1 move attacks");
        assert!(moves.iter().all(|m| m.move_id != 0));
    }
}

#[test]
fn good_damaging_moves_fill_the_share() {
    let s = MovesetSettings {
        good_damaging_percent: Some(100),
        ..random(MovesetMode::RandomCompletely)
    };
    let g = run_on(game(), s);
    for moves in g.learnsets.values() {
        assert!(moves.iter().all(|m| is_damaging(m.move_id)), "{moves:?}");
    }
    assert!(g.egg_moves[&1].iter().all(|&m| is_damaging(m)));
}

#[test]
fn damaging_moves_are_reordered_by_power() {
    let mut g = game();
    let learnset = vec![
        LevelUpMove::new(1, 59),  // 119 power
        LevelUpMove::new(5, 2),   // status
        LevelUpMove::new(10, 1),  // 61 power
        LevelUpMove::new(20, 31), // 91 power
    ];
    g.learnsets.insert(1, learnset);
    let s = MovesetSettings {
        reorder_damaging: true,
        ..MovesetSettings::default()
    };
    let g = run_on(g, s);
    let ids: Vec<MoveId> = g.learnsets[&1].iter().map(|m| m.move_id).collect();
    assert_eq!(ids, [1, 2, 31, 59]);
}

#[test]
fn pools_leave_out_banned_moves_and_balance_types() {
    let g = game();
    let mut rng = Seed(1).rng_for(step_ids::MOVESETS);
    let pools = Pools::new(&g, true, &mut rng);
    let numbers: BTreeSet<MoveId> = pools.all.iter().map(|m| m.number).collect();
    assert!(!numbers.contains(&STRUGGLE) && !numbers.contains(&CUT) && !numbers.contains(&SONIC_BOOM));
    assert!(
        pools
            .damaging
            .iter()
            .all(|m| m.power > 0 && !BANNED_FOR_DAMAGING.contains(&m.number))
    );
    // Normal has only status moves (average 0), Fire only damaging ones: Normal can't be balanced, Fire gets
    // weaker copies until its average is within 25% of the overall one... which it can't reach with
    // damaging moves only, so its list just grows.
    assert_eq!(
        pools.by_type[&Type::Normal].len(),
        numbers.iter().filter(|&&n| n % 2 == 0).count()
    );
    assert!(pools.by_type[&Type::Fire].len() > pools.damaging_by_type[&Type::Fire].len());
}

#[test]
fn preferred_types_follow_their_shares() {
    let mut rng = Seed(9).rng_for(step_ids::MOVESETS);
    let normal_fire = species(1, Type::Normal, Some(Type::Fire), 50);
    let mut counts: BTreeMap<Option<Type>, usize> = BTreeMap::new();
    for _ in 0..10_000 {
        *counts.entry(preferred_type(&normal_fire, &mut rng)).or_default() += 1;
    }
    assert!((800..1200).contains(&counts[&Some(Type::Normal)]), "{counts:?}");
    assert!((2700..3300).contains(&counts[&Some(Type::Fire)]), "{counts:?}");
}

#[test]
fn good_damaging_counts_round_half_up() {
    let s = |p| MovesetSettings {
        good_damaging_percent: Some(p),
        ..MovesetSettings::default()
    };
    assert_eq!(good_damaging_count(&s(50), 5), 3, "2.5 rounds up");
    assert_eq!(good_damaging_count(&s(20), 4), 1);
    assert_eq!(good_damaging_count(&MovesetSettings::default(), 9), 0);
}

#[test]
fn evolution_moves_for_all_come_with_gen_7() {
    let s = MovesetSettings {
        evolution_moves_for_all: true,
        ..random(MovesetMode::RandomCompletely)
    };
    let g = run_on(game(), s.clone());
    assert!(
        g.learnsets.values().all(|m| m[0].level == 1),
        "Gen 3 has no evolution moves"
    );

    let mut gen7 = game();
    gen7.identity.generation = ppr_model::Generation::new(7).expect("generation 7");
    // One species already learns a move on evolving: it keeps one.
    gen7.learnsets
        .get_mut(&2)
        .expect("there")
        .insert(0, LevelUpMove::new(0, 12));
    let g = run_on(gen7, s);
    for (n, moves) in &g.learnsets {
        let levels: Vec<u8> = moves.iter().map(|m| m.level).collect();
        assert_eq!(levels, [0, 1, 5, 10, 20, 30], "{n}");
        assert!(
            moves.iter().all(|m| m.move_id != 0),
            "{n}: the evolution move is a real one"
        );
    }
}
