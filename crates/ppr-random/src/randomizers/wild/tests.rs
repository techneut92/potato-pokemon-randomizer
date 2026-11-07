use std::collections::BTreeSet;

use ppr_model::{Evolution, EvolutionType, TypeChart, WildEncounter};
use ppr_settings::Settings;
use proptest::prelude::*;

use super::*;
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const GRASS_LINE: [DexNumber; 3] = [1, 2, 3];
const BUG_LINE: [DexNumber; 3] = [10, 11, 12];
/// Banned for wild encounters by the test game, like Unown in `FireRed`.
const BANNED: DexNumber = 59;

/// Three-stage lines, single-stage species of rotating types and strengths, two legendaries, and four
/// encounter sets on two routes.
fn game() -> GameData {
    let mut species = Vec::new();
    let mut add = |number: DexNumber, primary: Type, secondary: Option<Type>, each: u8| {
        species.push(testing::species(number, primary, secondary, each));
    };
    let mut evolutions = Vec::new();
    for (base, t, second) in [
        (1, Type::Grass, None),
        (4, Type::Fire, None),
        (7, Type::Water, None),
        (10, Type::Bug, Some(Type::Flying)),
    ] {
        for i in 0..3 {
            add(base + i, t, second, 40 + 15 * u8::try_from(i).expect("small"));
        }
        evolutions.push(Evolution::new(base, base + 1, EvolutionType::Level, 16));
        evolutions.push(Evolution::new(base + 1, base + 2, EvolutionType::Level, 32));
    }
    let singles = [
        Type::Normal,
        Type::Rock,
        Type::Electric,
        Type::Psychic,
        Type::Ghost,
        Type::Water,
    ];
    for n in 20..60 {
        let i = usize::from(n - 20);
        add(
            n,
            singles[i % singles.len()],
            None,
            30 + u8::try_from(i * 2).expect("small"),
        );
    }
    add(144, Type::Ice, Some(Type::Flying), 96);
    add(150, Type::Psychic, None, 106);
    species.sort_by_key(|s| s.number);
    GameData {
        type_chart: TypeChart::default(),
        evolutions,
        starter_held_items: Vec::new(),
        encounters: encounter_sets(),
        wild_banned: vec![BANNED],
        ..emerald(species)
    }
}

/// Route A: walking with two evolution lines, and surfing. Route B: walking (with the banned species) and
/// fishing, where #22 is banned.
fn encounter_sets() -> Vec<EncounterSet> {
    let set = |kind, location: &str, species: &[DexNumber], level| EncounterSet {
        kind,
        map: None,
        name: format!("{location} {kind:?}"),
        location: Some(location.into()),
        post_game: false,
        post_game_from: None,
        rate: 20,
        banned: Vec::new(),
        encounters: species
            .iter()
            .map(|&s| WildEncounter {
                forme: 0,
                species: s,
                min_level: level,
                max_level: level + 2,
            })
            .collect(),
    };
    let mut fishing = set(
        EncounterKind::Fishing,
        "ROUTE B",
        &[7, 23, 7, 23, 24, 7, 23, 24, 25, 26],
        5,
    );
    fishing.banned = vec![22];
    vec![
        set(
            EncounterKind::Walking,
            "ROUTE A",
            &[10, 11, 20, 21, 20, 21, 1, 2, 20, 21, 22, 22],
            3,
        ),
        set(EncounterKind::Surfing, "ROUTE A", &[7, 8, 22, 22, 22], 20),
        set(
            EncounterKind::Walking,
            "ROUTE B",
            &[10, 20, 23, 24, 25, 26, 20, 23, 24, 25, 26, BANNED],
            8,
        ),
        fishing,
    ]
}

fn random(f: impl FnOnce(&mut WildSettings)) -> Settings {
    let mut settings = Settings::default();
    settings.wild.mode = WildMode::Random;
    f(&mut settings.wild);
    settings.validate().expect("valid settings");
    settings
}

fn run(settings: &Settings, seed: u64) -> GameData {
    let mut data = RunData {
        game: game(),
        settings: settings.clone(),
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

/// Every (set, original, replacement), slot by slot.
fn pairs(after: &GameData) -> Vec<(usize, DexNumber, DexNumber)> {
    let before = game();
    before
        .encounters
        .iter()
        .zip(&after.encounters)
        .enumerate()
        .flat_map(|(i, (b, a))| {
            b.encounters
                .iter()
                .zip(&a.encounters)
                .map(move |(x, y)| (i, x.species, y.species))
        })
        .collect()
}

fn consistent_within(after: &GameData, zone_of: impl Fn(usize) -> usize) -> bool {
    let mut seen: HashMap<(usize, DexNumber), DexNumber> = HashMap::new();
    pairs(after)
        .into_iter()
        .all(|(set, old, new)| *seen.entry((zone_of(set), old)).or_insert(new) == new)
}

fn one_to_one_within(after: &GameData, zone: &[usize]) -> bool {
    let distinct: BTreeSet<_> = pairs(after)
        .into_iter()
        .filter(|p| zone.contains(&p.0))
        .map(|p| (p.1, p.2))
        .collect();
    let news: BTreeSet<_> = distinct.iter().map(|p| p.1).collect();
    news.len() == distinct.len()
}

#[test]
fn unchanged_settings_do_nothing() {
    assert!(!WildRandomizer.applies(&RunData {
        game: game(),
        settings: Settings::default(),
    }));
    assert_eq!(run(&Settings::default(), 1), game());
}

#[test]
fn keeps_slots_and_levels() {
    let after = run(&random(|_| {}), 1);
    let before = game();
    for (b, a) in before.encounters.iter().zip(&after.encounters) {
        assert_eq!(b.encounters.len(), a.encounters.len());
        assert!(
            b.encounters
                .iter()
                .zip(&a.encounters)
                .all(|(x, y)| (x.min_level, x.max_level) == (y.min_level, y.max_level))
        );
    }
    assert_ne!(after.encounters, before.encounters);
}

#[test]
fn the_same_seed_gives_the_same_game() {
    let s = random(|w| w.zones = WildZones::Location);
    assert_eq!(run(&s, 9), run(&s, 9));
    assert_ne!(run(&s, 9), run(&s, 10));
}

#[test]
fn zones_replace_consistently_and_one_to_one() {
    let game_wide = run(&random(|_| {}), 3);
    assert!(consistent_within(&game_wide, |_| 0));
    assert!(one_to_one_within(&game_wide, &[0, 1, 2, 3]));
    let per_set = run(&random(|w| w.zones = WildZones::Set), 3);
    assert!(consistent_within(&per_set, |set| set));
    let per_location = run(&random(|w| w.zones = WildZones::Location), 3);
    assert!(
        consistent_within(&per_location, |set| set / 2),
        "sets 0–1 are route A, 2–3 route B"
    );
    assert!(one_to_one_within(&per_location, &[0, 1]));
}

#[test]
fn split_zones_are_per_encounter_type() {
    let s = random(|w| {
        w.split_by_kind = true;
        w.types = WildTypes::ZoneThemes;
    });
    for seed in 0..10 {
        let after = run(&s, seed);
        assert!(consistent_within(&after, |set| [0, 1, 0, 2][set]));
        // Walking (sets 0 and 2) is one zone with one type throughout.
        let walking: Vec<_> = [0, 2].iter().flat_map(|&i| after.encounters[i].species()).collect();
        assert!(shared_type(&after, &walking).is_some(), "seed {seed}");
    }
}

#[test]
fn keep_primary_type() {
    for zones in [WildZones::Slot, WildZones::Location] {
        let after = run(
            &random(|w| {
                w.zones = zones;
                w.types = WildTypes::KeepPrimary;
            }),
            4,
        );
        for (_, old, new) in pairs(&after) {
            let t = after.species(old).expect("old").primary_type;
            assert!(after.species(new).expect("new").has_type(t), "#{old} → #{new}");
        }
    }
}

#[test]
fn keep_themes_keeps_a_shared_type() {
    // Route B's fishing is all Water (#7) and Normal/… mixes; give it a pure theme first.
    let mut g = game();
    g.encounters[1].encounters.iter_mut().for_each(|e| e.species = 7);
    let mut data = RunData {
        game: g,
        settings: random(|w| {
            w.zones = WildZones::Set;
            w.keep_themes = true;
        }),
    };
    standard_pipeline()
        .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    assert!(
        data.game.encounters[1].encounters.iter().all(|e| data
            .game
            .species(e.species)
            .expect("species")
            .has_type(Type::Water))
    );
}

#[test]
fn evolution_rules() {
    let basic = run(&random(|w| w.evolution = WildEvolution::BasicOnly), 6);
    assert!(pairs(&basic).iter().all(|&(_, _, new)| basic.stages_before(new) == 0));
    let same = run(&random(|w| w.evolution = WildEvolution::SameStage), 6);
    assert!(
        pairs(&same)
            .iter()
            .all(|&(_, old, new)| same.stages_before(old) == same.stages_before(new))
    );
}

#[test]
fn keep_relations_maps_a_line_to_a_line() {
    for seed in 0..20 {
        let after = run(&random(|w| w.keep_relations = true), seed);
        let new_of = |old| {
            pairs(&after)
                .into_iter()
                .find(|p| p.1 == old)
                .map(|p| p.2)
                .expect("present")
        };
        for line in [BUG_LINE, GRASS_LINE] {
            let (a, b) = (new_of(line[0]), new_of(line[1]));
            assert!(
                after.evolved_species(a).contains(&b),
                "seed {seed}: #{a} evolves into #{b}"
            );
        }
    }
}

#[test]
fn banned_species_never_appear_and_area_bans_hold() {
    for seed in 0..20 {
        let after = run(&random(|w| w.zones = WildZones::Slot), seed);
        assert!(pairs(&after).iter().all(|p| p.2 != BANNED), "seed {seed}");
        assert!(
            after.encounters[3].encounters.iter().all(|e| e.species != 22),
            "seed {seed}"
        );
    }
}

#[test]
fn catch_em_all_uses_every_species_and_keeps_banned_ones() {
    let s = random(|w| {
        w.zones = WildZones::Set;
        w.no_legendaries = true;
        w.catch_em_all = true;
    });
    let after = run(&s, 8);
    let used: Vec<_> = pairs(&after)
        .iter()
        .filter(|p| p.1 != BANNED)
        .map(|p| (p.0, p.1, p.2))
        .collect();
    let news: BTreeSet<_> = used.iter().map(|p| p.2).collect();
    let originals: usize = game()
        .encounters
        .iter()
        .map(|s| s.species().into_iter().filter(|&n| n != BANNED).count())
        .sum();
    assert_eq!(
        news.len(),
        originals,
        "no species is used twice while unused ones remain"
    );
    assert!(news.iter().all(|&n| !after.species(n).expect("species").is_legendary()));
    assert!(
        pairs(&after).iter().any(|p| p.1 == BANNED && p.2 == BANNED),
        "the banned species stays"
    );
}

#[test]
fn similar_strength_uses_bst_windows() {
    let game = game();
    let s = random(|_| {});
    let mut rng = Seed(1).rng_for(0);
    let cancel = CancelToken::default();
    let allowed: SpeciesSet = game.species.iter().map(|sp| sp.number).collect();
    let mut inner = Inner::new(&game, &s.wild, allowed.clone(), SpeciesSet::new(), &mut rng, &cancel).expect("inner");
    // 46 species: the pool must reach min(5, 46 / 4) = 5 species, starting within 10% of 300.
    for _ in 0..50 {
        let n = inner.similar_strength(&allowed, 300);
        let bst = game.species(n).expect("species").bst();
        assert!((240..=360).contains(&bst), "{bst}");
    }
    let tiny: SpeciesSet = [20, 21].into();
    assert!(
        tiny.contains(&inner.similar_strength(&tiny, 600)),
        "small sets: any member"
    );
}

#[test]
fn balance_low_levels_lowers_the_target_bst() {
    let after = run(
        &random(|w| {
            w.zones = WildZones::Set;
            w.similar_strength = true;
            w.balance_low_levels = true;
        }),
        2,
    );
    // Route A's walking is levels 3–5: average 4, so at most BST 290, widened to 5 species.
    for e in &after.encounters[0].encounters {
        assert!(after.species(e.species).expect("species").bst() <= 360);
    }
}

#[test]
fn level_modifier_and_minimum_catch_rate_apply() {
    let mut settings = Settings::default();
    settings.wild.level_modifier = 50;
    settings.wild.min_catch_rate = 2;
    let after = run(&settings, 1);
    let first = &after.encounters[0].encounters[0];
    assert_eq!(
        (first.species, first.min_level, first.max_level),
        (10, 5, 8),
        "4.5 and 7.5 round up"
    );
    assert_eq!(after.species(20).expect("species").catch_rate, 128);
    assert_eq!(after.species(150).expect("legendary").catch_rate, 64);
    assert!(!after.guaranteed_catching);
    settings.wild.min_catch_rate = 5;
    settings.wild.level_modifier = -100;
    let all = run(&settings, 1);
    assert!(all.guaranteed_catching, "level 5 patches the game");
    assert_eq!(
        all.species(20).expect("species").catch_rate,
        45,
        "and leaves catch rates alone"
    );
    assert!(
        all.encounters
            .iter()
            .flat_map(|s| &s.encounters)
            .all(|e| e.min_level == 1)
    );
}

#[test]
fn shared_type_prefers_the_primary_and_not_normal() {
    let g = game();
    assert_eq!(shared_type(&g, &[10, 11]), Some(Type::Bug));
    assert_eq!(shared_type(&g, &[20, 26]), Some(Type::Normal));
    assert_eq!(shared_type(&g, &[20, 21]), None);
    assert_eq!(
        shared_types(&g, &[10, 12]),
        [Type::Flying, Type::Bug],
        "the fixed type order"
    );
}

proptest! {
    #[test]
    fn any_valid_settings_run(
        zones in 0..5usize, split in any::<bool>(), types in 0..3usize, themes in any::<bool>(),
        evolution in 0..3usize, relations in any::<bool>(), similar in any::<bool>(), balance in any::<bool>(),
        all in any::<bool>(), legendaries in any::<bool>(), seed in any::<u64>(),
    ) {
        let mut settings = Settings::default();
        let w = &mut settings.wild;
        w.mode = WildMode::Random;
        w.zones = [WildZones::Slot, WildZones::Set, WildZones::Map, WildZones::Location, WildZones::Game][zones];
        w.split_by_kind = split;
        w.types = [WildTypes::None, WildTypes::ZoneThemes, WildTypes::KeepPrimary][types];
        w.keep_themes = themes;
        w.evolution = [WildEvolution::None, WildEvolution::BasicOnly, WildEvolution::SameStage][evolution];
        w.keep_relations = relations;
        w.similar_strength = similar;
        w.balance_low_levels = balance;
        w.catch_em_all = all;
        w.no_legendaries = legendaries;
        prop_assume!(settings.validate().is_ok());
        let mut data = RunData { game: game(), settings };
        // Strict combinations may find no replacement in this small game; that must be an error, not a
        // panic or a wrong result.
        match standard_pipeline().run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {}) {
            Ok(()) => {}
            Err(RandomizeError::Impossible { .. }) => return Ok(()),
            Err(e) => prop_assert!(false, "{e}"),
        }
        let after = data.game;
        prop_assert_eq!(after.encounters.len(), 4);
        for (_, old, new) in pairs(&after) {
            prop_assert!(after.species(new).is_some());
            prop_assert!(new != BANNED || old == BANNED);
            prop_assert!(!legendaries || !after.species(new).expect("species").is_legendary());
        }
    }
}

#[test]
fn no_premature_evolutions_in_the_wild() {
    for zones in [WildZones::Slot, WildZones::Location] {
        let mut settings = random(|w| w.zones = zones);
        settings.general.no_premature_evolutions = true;
        for seed in 0..10 {
            let after = run(&settings, seed);
            for e in after.encounters.iter().flat_map(|s| &s.encounters) {
                assert!(
                    crate::randomizers::common::is_legal_evolution_at(&after, e.species, e.min_level, 1.0),
                    "#{} at level {}",
                    e.species,
                    e.min_level
                );
            }
        }
    }
}
