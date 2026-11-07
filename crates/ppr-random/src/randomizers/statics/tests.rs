use ppr_model::{DexNumber, Type, is_legendary, is_ultra_beast};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const LEGENDARIES: [DexNumber; 5] = [144, 145, 146, 150, 151];

/// Species 1–200, each stat `n % 100 + 20`, except the legendaries, whose stats are all 100 (BST 600).
fn game() -> GameData {
    let species = (1..=200)
        .map(|number: DexNumber| {
            let each = if LEGENDARIES.contains(&number) {
                100
            } else {
                u8::try_from(number % 100 + 20).expect("small")
            };
            testing::species(number, Type::Normal, None, each)
        })
        .collect();
    let egg = StaticEncounter {
        egg: true,
        ..StaticEncounter::new(30, 1)
    };
    let roaming = StaticEncounter {
        linked_levels: vec![40],
        ..StaticEncounter::new(145, 50)
    };
    GameData {
        statics: vec![
            StaticEncounter::new(144, 50),
            StaticEncounter::new(10, 20),
            StaticEncounter::new(150, 70),
            egg,
            roaming,
            StaticEncounter::new(60, 30),
        ],
        main_game_legendaries: vec![150],
        special_music_statics: vec![144, 150],
        ..emerald(species)
    }
}

fn run_on(game: GameData, settings: StaticSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            statics: settings,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn with_mode(mode: StaticMode) -> StaticSettings {
    StaticSettings {
        mode,
        ..StaticSettings::default()
    }
}

fn species(game: &GameData) -> Vec<DexNumber> {
    game.statics.iter().map(|s| s.species).collect()
}

#[test]
fn matching_keeps_legendaries_legendary() {
    let before = game();
    for seed in 0..50 {
        let after = run_on(game(), with_mode(StaticMode::RandomMatching), seed);
        for (old, new) in before.statics.iter().zip(&after.statics) {
            assert_eq!(
                is_legendary(old.species),
                is_legendary(new.species),
                "seed {seed}: {old:?} -> {new:?}"
            );
        }
        // Three legendaries, five to choose from: no repeats.
        let mut legendaries: Vec<DexNumber> = species(&after).into_iter().filter(|&n| is_legendary(n)).collect();
        legendaries.sort_unstable();
        legendaries.dedup();
        assert_eq!(legendaries.len(), 3, "seed {seed}");
    }
}

#[test]
fn random_takes_every_species_once_before_repeating() {
    let after = run_on(game(), with_mode(StaticMode::Random), 7);
    let mut all = species(&after);
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 6);
    assert_ne!(species(&after), species(&game()));
}

#[test]
fn similar_strength_stays_close_and_changes_the_species() {
    let before = game();
    for seed in 0..50 {
        let after = run_on(game(), with_mode(StaticMode::SimilarStrength), seed);
        for (old, new) in before.statics.iter().zip(&after.statics) {
            assert_ne!(old.species, new.species, "seed {seed}");
            let (a, b) = (bst(&before, old.species), bst(&before, new.species));
            assert!(a.abs_diff(b) * 10 <= a * 3, "seed {seed}: {a} -> {b}");
        }
    }
}

#[test]
fn main_game_legendaries_can_be_limited() {
    let settings = StaticSettings {
        limit_main_game_legendaries: true,
        ..with_mode(StaticMode::SimilarStrength)
    };
    let before = game();
    for seed in 0..50 {
        let after = run_on(game(), settings.clone(), seed);
        // Static 2 is Mewtwo, the main-game legendary.
        assert!(bst(&before, after.statics[2].species) <= 600, "seed {seed}");
    }
}

#[test]
fn strong_statics_can_become_anything() {
    let settings = StaticSettings {
        randomize_600_bst: true,
        ..with_mode(StaticMode::SimilarStrength)
    };
    let before = game();
    let weak = (0..50)
        .map(|seed| run_on(game(), settings.clone(), seed))
        .filter(|after| bst(&before, after.statics[0].species) < 400)
        .count();
    assert!(weak > 0, "Articuno (BST 600) became a weak species at least once");
}

#[test]
fn levels_change_except_for_eggs() {
    let settings = StaticSettings {
        level_modifier: 50,
        ..StaticSettings::default()
    };
    let after = run_on(game(), settings, 1);
    assert_eq!(species(&after), species(&game()), "levels only");
    let levels: Vec<u8> = after.statics.iter().map(|s| s.level).collect();
    assert_eq!(levels, [75, 30, 100, 1, 75, 45]);
    assert_eq!(after.statics[4].linked_levels, [60]);
}

#[test]
fn music_follows_only_when_asked_and_possible() {
    let fix = StaticSettings {
        fix_music: true,
        ..with_mode(StaticMode::Random)
    };
    assert!(run_on(game(), fix.clone(), 1).correct_static_music);
    assert!(!run_on(game(), with_mode(StaticMode::Random), 1).correct_static_music);
    let mut no_fix = game();
    no_fix.special_music_statics.clear();
    assert!(!run_on(no_fix, fix, 1).correct_static_music);
}

#[test]
fn games_without_statics_are_left_alone() {
    let mut empty = game();
    empty.statics.clear();
    let data = RunData {
        game: empty,
        settings: Settings {
            statics: with_mode(StaticMode::Random),
            ..Settings::default()
        },
    };
    assert!(!StaticRandomizer.applies(&data));
}

#[test]
fn the_same_seed_gives_the_same_statics() {
    let a = run_on(game(), with_mode(StaticMode::SimilarStrength), 42);
    let b = run_on(game(), with_mode(StaticMode::SimilarStrength), 42);
    assert_eq!(a.statics, b.statics);
}

/// The test game with Mega Evolutions for species 50 (stone 700) and 60 (stones 701 and 702), and species 50
/// holding its stone as a static.
fn game_with_megas(force: bool) -> GameData {
    let mut game = game();
    for (number, base, forme, stone) in [(201, 50, 1, 700), (202, 60, 1, 701), (203, 60, 2, 702)] {
        let mut sp = testing::species(number, Type::Normal, None, 120);
        sp.forme.base = Some(base);
        sp.forme.number = forme;
        sp.forme.mega = true;
        sp.forme.mega_item = Some(stone);
        sp.forme.irregular = true;
        game.species.push(sp);
    }
    game.statics[1] = StaticEncounter {
        held_item: Some(700),
        ..StaticEncounter::new(50, 32)
    };
    game.formes.force_swap_static_megas = force;
    game
}

#[test]
fn statics_holding_their_mega_stone_swap_to_species_that_can_mega_evolve() {
    let stones = |n: DexNumber| match n {
        50 => vec![700],
        60 => vec![701, 702],
        _ => Vec::new(),
    };
    for mode in [
        StaticMode::RandomMatching,
        StaticMode::Random,
        StaticMode::SimilarStrength,
    ] {
        for seed in 0..20 {
            let forced = run_on(game_with_megas(true), with_mode(mode), seed);
            let st = &forced.statics[1];
            let stone = st.held_item.expect("a Mega Stone");
            assert!(stones(st.species).contains(&stone), "{mode:?}: {st:?}");

            let asked = run_on(
                game_with_megas(false),
                StaticSettings {
                    swap_mega_evos: true,
                    ..with_mode(mode)
                },
                seed,
            );
            assert_eq!(asked.statics[1], forced.statics[1]);
        }
    }
    // Without the option the stone stays with whatever replaces the species; other statics keep holding nothing.
    let plain = run_on(game_with_megas(false), with_mode(StaticMode::Random), 3);
    assert_eq!(plain.statics[1].held_item, Some(700));
    assert!(
        plain
            .statics
            .iter()
            .enumerate()
            .all(|(i, st)| i == 1 || st.held_item.is_none())
    );
}

/// Species 1–810 (with the Ultra Beasts, 793–799 and 803–806), and as statics the Ultra Beasts `ultra_beasts`,
/// Mewtwo and two others.
fn game_with_ultra_beasts(ultra_beasts: &[DexNumber]) -> GameData {
    let species = (1..=810)
        .map(|number: DexNumber| {
            testing::species(
                number,
                Type::Normal,
                None,
                u8::try_from(number % 100 + 20).expect("small"),
            )
        })
        .collect();
    let mut statics: Vec<StaticEncounter> = ultra_beasts.iter().map(|&n| StaticEncounter::new(n, 60)).collect();
    statics.extend([
        StaticEncounter::new(150, 70),
        StaticEncounter::new(10, 20),
        StaticEncounter::new(700, 30),
    ]);
    GameData {
        statics,
        ..emerald(species)
    }
}

#[test]
fn matching_keeps_ultra_beasts_apart() {
    let before = game_with_ultra_beasts(&[793, 794, 803]);
    let ultra_beasts = before.ultra_beasts();
    assert_eq!(ultra_beasts.len(), 11);
    for seed in 0..50 {
        let after = run_on(before.clone(), with_mode(StaticMode::RandomMatching), seed);
        for (old, new) in before.statics.iter().zip(&after.statics) {
            assert_eq!(
                ultra_beasts.contains(&old.species),
                ultra_beasts.contains(&new.species),
                "seed {seed}: {old:?} -> {new:?}"
            );
            assert_eq!(is_legendary(old.species), is_legendary(new.species), "seed {seed}");
        }
        let mut taken: Vec<DexNumber> = species(&after)[..3].to_vec();
        taken.sort_unstable();
        taken.dedup();
        assert_eq!(
            taken.len(),
            3,
            "seed {seed}: no Ultra Beast twice while others are left"
        );
    }
}

#[test]
fn ultra_beasts_refill_when_used_up() {
    let statics: Vec<DexNumber> = [793, 794, 795, 796, 797, 798, 799, 803, 804, 805, 806, 793, 794].to_vec();
    let after = run_on(
        game_with_ultra_beasts(&statics),
        with_mode(StaticMode::RandomMatching),
        3,
    );
    let new = species(&after);
    let mut first: Vec<DexNumber> = new[..11].to_vec();
    first.sort_unstable();
    assert_eq!(
        first,
        [793, 794, 795, 796, 797, 798, 799, 803, 804, 805, 806],
        "each once"
    );
    assert!(new[11..13].iter().all(|&n| is_ultra_beast(n)));
}

#[test]
fn ultra_beasts_are_like_any_other_species_outside_matching() {
    let before = game_with_ultra_beasts(&[793]);
    let became_other = (0..20)
        .map(|seed| run_on(before.clone(), with_mode(StaticMode::Random), seed))
        .any(|after| !is_ultra_beast(after.statics[0].species));
    assert!(became_other);
}

/// The test game with statics 1 and 5 (species 10 and 60) limited to `allowed`.
fn game_with_allowed(allowed: &[DexNumber]) -> GameData {
    let mut game = game();
    for i in [1, 5] {
        game.statics[i].allowed_species = Some(allowed.to_vec());
    }
    game
}

#[test]
fn limited_statics_get_only_allowed_species_in_every_mode() {
    let allowed = [11, 12, 13, 14, 15, 70, 71, 72];
    let settings = [
        with_mode(StaticMode::RandomMatching),
        with_mode(StaticMode::Random),
        with_mode(StaticMode::SimilarStrength),
        StaticSettings {
            randomize_600_bst: true,
            ..with_mode(StaticMode::SimilarStrength)
        },
    ];
    for s in settings {
        for seed in 0..30 {
            let after = run_on(game_with_allowed(&allowed), s.clone(), seed);
            for i in [1, 5] {
                assert!(
                    allowed.contains(&after.statics[i].species),
                    "{:?} seed {seed}: {:?}",
                    s.mode,
                    after.statics[i]
                );
                assert_eq!(after.statics[i].allowed_species.as_deref(), Some(&allowed[..]));
            }
            if s.mode == StaticMode::SimilarStrength && !s.randomize_600_bst {
                // Of similar strength to species 10 (BST 180) and 60 (BST 480) among those allowed.
                assert!(after.statics[1].species < 20, "seed {seed}");
                assert!(after.statics[5].species >= 70, "seed {seed}");
            }
        }
    }
}

#[test]
fn limited_statics_fall_back_to_the_full_pool() {
    // One species allowed: the first static takes it from the pool, the second gets it again from the full pool.
    for mode in [StaticMode::RandomMatching, StaticMode::Random] {
        let after = run_on(game_with_allowed(&[40]), with_mode(mode), 5);
        assert_eq!(
            (after.statics[1].species, after.statics[5].species),
            (40, 40),
            "{mode:?}"
        );
    }
    // With similar strength a limited static may keep its species when that is all that is allowed.
    let mut game = game();
    game.statics[1].allowed_species = Some(vec![10]);
    let after = run_on(game, with_mode(StaticMode::SimilarStrength), 5);
    assert_eq!(after.statics[1].species, 10);
}

#[test]
fn limited_statics_fail_when_nothing_allowed_exists() {
    let mut data = RunData {
        game: game_with_allowed(&[999]),
        settings: Settings {
            statics: with_mode(StaticMode::Random),
            ..Settings::default()
        },
    };
    let result = standard_pipeline().run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {});
    assert!(result.is_err());
}
