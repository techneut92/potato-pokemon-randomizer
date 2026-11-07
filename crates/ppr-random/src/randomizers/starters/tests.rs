use ppr_model::{Effectiveness, Evolution, EvolutionType, Generation, Item, TypeChart};
use ppr_settings::Settings;
use proptest::prelude::*;

use super::*;
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// A small game: three full evolution lines per standard type, single-stage species, legendaries and a
/// dual-type line.
fn game() -> GameData {
    let mut species = Vec::new();
    let mut evolutions = Vec::new();
    let mut add = |number: DexNumber, primary: Type, secondary: Option<Type>, bst_each: u8| {
        species.push(testing::species(number, primary, secondary, bst_each));
    };
    // Three-stage lines: 1-3 Grass, 4-6 Fire, 7-9 Water, 10-12 Normal/Flying.
    for (base, t, second) in [
        (1, Type::Grass, None),
        (4, Type::Fire, None),
        (7, Type::Water, None),
        (10, Type::Normal, Some(Type::Flying)),
    ] {
        for i in 0..3 {
            add(base + i, t, second, 50 + 10 * u8::try_from(i).expect("small"));
        }
        evolutions.push(Evolution::new(base, base + 1, EvolutionType::Level, 16));
        evolutions.push(Evolution::new(base + 1, base + 2, EvolutionType::Level, 36));
    }
    // Single-stage species of other types.
    for (n, t) in [
        (20, Type::Electric),
        (21, Type::Rock),
        (22, Type::Psychic),
        (23, Type::Bug),
        (24, Type::Ghost),
    ] {
        add(n, t, None, 70);
    }
    // Legendaries (by number).
    add(144, Type::Ice, Some(Type::Flying), 96);
    add(151, Type::Psychic, None, 100);
    species.sort_by_key(|s| s.number);
    let types = Type::in_generation(Generation::new(3).expect("valid"));
    let mut type_chart = TypeChart::neutral(&types);
    // Two triangles: Fire, Water, Grass and Rock, Bug, Psychic (made up). Electric beats Water, but nothing
    // closes a triangle with it.
    for (attacker, defender) in [
        (Type::Water, Type::Fire),
        (Type::Grass, Type::Water),
        (Type::Fire, Type::Grass),
        (Type::Bug, Type::Rock),
        (Type::Psychic, Type::Bug),
        (Type::Rock, Type::Psychic),
        (Type::Electric, Type::Water),
    ] {
        type_chart.set(attacker, defender, Effectiveness::Double);
    }
    GameData {
        type_chart,
        evolutions,
        items: [(1, true, false), (2, true, true), (3, false, false)]
            .into_iter()
            .map(|(id, allowed, bad)| Item {
                id,
                name: format!("Item{id}"),
                allowed,
                bad,
                tm: false,
            })
            .collect(),
        ..emerald(species)
    }
}

fn run_on(game: GameData, settings: StarterSettings, seed: u64) -> Result<GameData, RandomizeError> {
    let mut data = RunData {
        game,
        settings: Settings {
            starters: settings,
            ..Settings::default()
        },
    };
    standard_pipeline().run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})?;
    Ok(data.game)
}

fn run(settings: StarterSettings, seed: u64) -> Result<Vec<DexNumber>, RandomizeError> {
    Ok(run_on(game(), settings, seed)?.starters)
}

fn settings(mode: StarterMode) -> StarterSettings {
    StarterSettings {
        mode,
        ..StarterSettings::default()
    }
}

#[test]
fn unchanged_keeps_the_starters() {
    assert_eq!(run(settings(StarterMode::Unchanged), 1).expect("runs"), [1, 4, 7]);
}

#[test]
fn custom_starters_come_first_and_random_fills_the_rest() {
    let s = StarterSettings {
        custom: vec![None, Some(20), None],
        ..settings(StarterMode::Custom)
    };
    let starters = run(s, 5).expect("runs");
    assert_eq!(starters[0], 20);
    assert_eq!(starters.len(), 3);
    let full = StarterSettings {
        custom: vec![Some(21), Some(22), Some(23)],
        ..settings(StarterMode::Custom)
    };
    assert_eq!(run(full, 5).expect("runs"), [21, 22, 23]);
}

#[test]
fn custom_starters_must_exist() {
    let s = StarterSettings {
        custom: vec![Some(999)],
        ..settings(StarterMode::Custom)
    };
    assert!(matches!(run(s, 1), Err(RandomizeError::Impossible { .. })));
}

#[test]
fn two_evolution_mode_only_picks_line_starts() {
    for seed in 0..50 {
        let starters = run(settings(StarterMode::RandomWithTwoEvolutions), seed).expect("runs");
        assert!(starters.iter().all(|n| [1, 4, 7, 10].contains(n)), "{starters:?}");
    }
}

#[test]
fn fire_water_grass_gives_one_of_each_in_gen3_order() {
    let s = StarterSettings {
        types: StarterTypes::FireWaterGrass,
        ..settings(StarterMode::Random)
    };
    for seed in 0..50 {
        let st = run(s.clone(), seed).expect("runs");
        let g = game();
        let types: Vec<_> = st.iter().map(|&n| g.species(n).expect("valid").primary_type).collect();
        assert_eq!(types, [Type::Grass, Type::Fire, Type::Water]);
    }
}

#[test]
fn single_type_with_too_few_species_fails_cleanly() {
    let s = StarterSettings {
        types: StarterTypes::SingleType(Some(Type::Electric)),
        ..settings(StarterMode::Random)
    };
    assert!(matches!(run(s, 1), Err(RandomizeError::Impossible { .. })));
}

#[test]
fn not_enough_candidates_is_an_error_not_a_panic() {
    let s = StarterSettings {
        bst_min: 590,
        bst_max: 600,
        ..settings(StarterMode::Random)
    };
    assert!(matches!(run(s, 1), Err(RandomizeError::Impossible { .. })));
}

#[test]
fn same_seed_same_starters() {
    assert_eq!(
        run(settings(StarterMode::Random), 42).expect("runs"),
        run(settings(StarterMode::Random), 42).expect("runs")
    );
}

#[test]
fn type_triangles_are_found_once_per_starting_type() {
    let triangles = find_type_triangles(&game());
    // Each of the two triangles once per starting type, each type beaten by the next.
    assert_eq!(triangles.len(), 6);
    assert!(triangles.contains(&[Type::Fire, Type::Water, Type::Grass]));
    assert!(triangles.contains(&[Type::Rock, Type::Bug, Type::Psychic]));
    let g = game();
    for t in &triangles {
        for i in 0..3 {
            assert_eq!(
                g.type_chart.get(t[(i + 1) % 3], t[i]),
                Some(Effectiveness::Double),
                "{t:?}"
            );
        }
    }
}

#[test]
fn type_triangle_starters_follow_a_triangle_and_record_it() {
    let s = StarterSettings {
        types: StarterTypes::TypeTriangle,
        ..settings(StarterMode::Random)
    };
    let mut seen = BTreeSet::new();
    for seed in 0..40 {
        let g = run_on(game(), s.clone(), seed).expect("runs");
        let triangle = g.starter_type_triangle.expect("recorded");
        for (n, t) in g.starters.iter().zip(triangle) {
            let sp = g.species(*n).expect("valid");
            assert!(sp.primary_type == t || sp.secondary_type == Some(t), "{n} is not {t:?}");
        }
        seen.insert(triangle);
    }
    // The rotations of both triangles get chosen.
    assert!(seen.len() > 3, "{seen:?}");
}

#[test]
fn without_valid_starters_the_triangle_rule_fails_cleanly() {
    let s = StarterSettings {
        types: StarterTypes::TypeTriangle,
        ..settings(StarterMode::Random)
    };
    // Each triangle loses a type, so none has valid starters; the draw must still end.
    let mut g = game();
    g.species
        .retain(|sp| sp.primary_type != Type::Grass && sp.primary_type != Type::Rock);
    let result = run_on(g, s, 3);
    assert!(matches!(result, Err(RandomizeError::Impossible { .. })), "{result:?}");
}

fn any_mode() -> impl Strategy<Value = StarterMode> {
    prop_oneof![
        Just(StarterMode::Random),
        Just(StarterMode::RandomBasic),
        Just(StarterMode::RandomWithTwoEvolutions)
    ]
}

proptest! {
    #[test]
    fn random_starters_respect_every_restriction(
        seed: u64,
        mode in any_mode(),
        no_legendaries: bool,
        no_dual_types: bool,
        unique: bool,
    ) {
        let types = if unique { StarterTypes::Unique } else { StarterTypes::None };
        let s = StarterSettings { mode, types, no_legendaries, no_dual_types, ..StarterSettings::default() };
        let g = game();
        // Some combinations have too few candidates; those must fail cleanly, never panic.
        if let Ok(st) = run(s, seed) {
            prop_assert_eq!(st.len(), 3);
            let distinct: BTreeSet<_> = st.iter().collect();
            prop_assert_eq!(distinct.len(), 3);
            for &n in &st {
                let sp = g.species(n).expect("a species of the game");
                prop_assert!(!(no_legendaries && sp.is_legendary()));
                prop_assert!(!(no_dual_types && sp.secondary_type.is_some()));
                if mode != StarterMode::Random {
                    prop_assert!(g.pre_evolved_species(n).is_empty());
                }
            }
            if unique {
                for (i, &a) in st.iter().enumerate() {
                    for &b in &st[i + 1..] {
                        prop_assert!(!shares_type(&g, a, b));
                    }
                }
            }
        }
    }
}

#[test]
fn held_items_are_random_allowed_items_whatever_the_mode() {
    let mut seen = BTreeSet::new();
    for seed in 0..40 {
        let s = StarterSettings {
            random_held_items: true,
            ..settings(StarterMode::Unchanged)
        };
        let g = run_on(game(), s, seed).expect("runs");
        assert_eq!(g.starters, [1, 4, 7], "starters unchanged");
        seen.insert(g.starter_held_items[0].expect("an item"));
    }
    assert_eq!(seen, BTreeSet::from([1, 2]), "never the item that is not allowed");
}

#[test]
fn banning_bad_items_leaves_only_good_ones() {
    for seed in 0..20 {
        let s = StarterSettings {
            random_held_items: true,
            ban_bad_items: true,
            ..settings(StarterMode::Random)
        };
        assert_eq!(run_on(game(), s, seed).expect("runs").starter_held_items, [Some(1)]);
    }
    let mut g = game();
    g.items.retain(|i| i.bad);
    let s = StarterSettings {
        random_held_items: true,
        ban_bad_items: true,
        ..settings(StarterMode::Unchanged)
    };
    assert!(matches!(run_on(g, s, 1), Err(RandomizeError::Impossible { .. })));
}

/// [`game`] with alternate formes: 301 of species 1, the irregular 302 of species 4 and the cosmetic 303 of
/// species 7. Starters can be alternate formes when `starters` is set.
fn game_with_formes(starters: bool) -> GameData {
    let mut g = game();
    for (n, base, irregular, cosmetic) in [(301, 1, false, false), (302, 4, true, false), (303, 7, false, true)] {
        g.species.push(testing::alt_forme(n, base, 1, 50, cosmetic, irregular));
    }
    g.formes.starters = starters;
    g
}

fn chosen(game: &GameData, s: &StarterSettings, no_irregular: bool, seeds: u64) -> BTreeSet<DexNumber> {
    (0..seeds)
        .flat_map(|seed| {
            let mut settings = Settings {
                starters: s.clone(),
                ..Settings::default()
            };
            settings.general.no_irregular_alt_formes = no_irregular;
            let mut data = RunData {
                game: game.clone(),
                settings,
            };
            standard_pipeline()
                .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
                .expect("runs");
            data.game.starters
        })
        .collect()
}

#[test]
fn alternate_formes_are_starters_only_when_allowed_and_possible() {
    let random = settings(StarterMode::Random);
    assert!(
        chosen(&game_with_formes(true), &random, false, 60)
            .iter()
            .all(|&n| n < 300)
    );
    let allowed = StarterSettings {
        allow_alt_formes: true,
        ..random
    };
    assert!(
        chosen(&game_with_formes(false), &allowed, false, 60)
            .iter()
            .all(|&n| n < 300),
        "the game's starters can't be alternate formes"
    );
    let with = chosen(&game_with_formes(true), &allowed, false, 100);
    assert!(with.contains(&301) && with.contains(&302), "{with:?}");
    assert!(!with.contains(&303), "cosmetic formes are never chosen");
    assert!(
        !chosen(&game_with_formes(true), &allowed, true, 100).contains(&302),
        "No Irregular Alt Formes"
    );
}
