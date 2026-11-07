use ppr_model::{EvolutionType as E, ExpCurve, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::testing::{self, emerald, species};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const TYPES: [Type; 4] = [Type::Normal, Type::Fire, Type::Water, Type::Grass];

/// Species 1–60 of four types and rising totals; 1 → 2 → 3, 4 → 5 → 6, 7 → 8, Eevee-like 10 into 11 and 12 by
/// stones, and 13 → 14 by trade.
fn game() -> GameData {
    let species = (1..=60u16)
        .map(|n| {
            let t = TYPES[usize::from(n % 4)];
            species(n, t, None, 20 + u8::try_from(n).expect("small"))
        })
        .collect();
    let mut g = emerald(species);
    g.evolutions = vec![
        Evolution::new(1, 2, E::Level, 16),
        Evolution::new(2, 3, E::Level, 32),
        Evolution::new(4, 5, E::Level, 16),
        Evolution::new(5, 6, E::Level, 36),
        Evolution::new(7, 8, E::Level, 20),
        Evolution::new(10, 11, E::Stone, 82),
        Evolution::new(10, 12, E::Stone, 84),
        Evolution::new(13, 14, E::Trade, 0),
    ];
    g.estimate_evolution_levels();
    g
}

fn try_run(game: GameData, evolutions: EvolutionSettings, seed: u64) -> Result<GameData, RandomizeError> {
    let mut data = RunData {
        game,
        settings: Settings {
            evolutions,
            ..Settings::default()
        },
    };
    standard_pipeline().run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})?;
    Ok(data.game)
}

fn run_on(game: GameData, evolutions: EvolutionSettings, seed: u64) -> GameData {
    try_run(game, evolutions, seed).expect("runs")
}

fn random() -> EvolutionSettings {
    EvolutionSettings {
        mode: EvolutionMode::Random,
        ..EvolutionSettings::default()
    }
}

fn targets(g: &GameData, from: DexNumber) -> Vec<DexNumber> {
    g.evolutions.iter().filter(|e| e.from == from).map(|e| e.to).collect()
}

#[test]
fn random_evolutions_keep_their_methods_and_make_no_loops() {
    let before = game();
    for seed in 0..20 {
        let g = run_on(game(), random(), seed);
        assert_eq!(g.evolutions.len(), before.evolutions.len());
        for (old, new) in before.evolutions.iter().zip(&g.evolutions) {
            assert_eq!((old.from, old.kind, old.parameter), (new.from, new.kind, new.parameter));
            assert_ne!(new.to, new.from);
        }
        let mut split = targets(&g, 10);
        split.dedup();
        assert_eq!(split.len(), 2, "split evolutions differ");
        for sp in &g.species {
            assert!(g.stages_after(sp.number) < 10, "{seed}: no loop through {}", sp.number);
        }
    }
    assert_ne!(run_on(game(), random(), 1).evolutions, before.evolutions);
}

#[test]
fn options_restrict_the_new_evolutions() {
    let before = game();
    for seed in 0..10 {
        let s = EvolutionSettings {
            force_change: true,
            no_convergence: true,
            force_growth: true,
            same_typing: true,
            limit_three_stages: true,
            ..random()
        };
        let g = run_on(game(), s, seed);
        let mut all_targets: Vec<DexNumber> = g.evolutions.iter().map(|e| e.to).collect();
        all_targets.sort_unstable();
        let count = all_targets.len();
        all_targets.dedup();
        assert_eq!(all_targets.len(), count, "{seed}: no convergence");
        for e in &g.evolutions {
            assert!(!targets(&before, e.from).contains(&e.to), "{seed}: force change");
            let (from, to) = (g.species(e.from).expect("there"), g.species(e.to).expect("there"));
            assert!(to.bst() > from.bst(), "{seed}: force growth");
            assert!(shares_type(to, from), "{seed}: same typing");
            assert!(g.stages_before(e.to) + g.stages_after(e.to) < 3, "{seed}: three stages");
        }
    }
}

#[test]
fn same_typing_follows_the_old_targets_of_eevee() {
    let mut g = game();
    g.species.push(species(EEVEE, Type::Fire, None, 40));
    for e in &mut g.evolutions {
        if e.from == 10 {
            e.from = EEVEE;
        }
    }
    let s = EvolutionSettings {
        same_typing: true,
        ..random()
    };
    for seed in 0..10 {
        let r = run_on(g.clone(), s.clone(), seed);
        for (old, new) in targets(&g, EEVEE).into_iter().zip(targets(&r, EEVEE)) {
            let (old_to, new_to) = (r.species(old).expect("there"), r.species(new).expect("there"));
            assert!(shares_type(new_to, old_to), "{seed}");
        }
    }
}

#[test]
fn similar_strength_stays_near_the_old_totals() {
    let s = EvolutionSettings {
        similar_strength: true,
        ..random()
    };
    let before = game();
    let g = run_on(game(), s, 3);
    for (old, new) in before.evolutions.iter().zip(&g.evolutions) {
        let (a, b) = (i64::from(bst(&g, old.to)), i64::from(bst(&g, new.to)));
        assert!((a - b).abs() <= a / 3, "{a} vs {b}");
    }
}

#[test]
fn every_level_gives_every_species_one_evolution() {
    let s = EvolutionSettings {
        mode: EvolutionMode::RandomEveryLevel,
        ..EvolutionSettings::default()
    };
    let g = run_on(game(), s, 1);
    assert_eq!(g.evolutions.len(), g.species.len());
    for sp in &g.species {
        let own: Vec<_> = g.evolutions.iter().filter(|e| e.from == sp.number).collect();
        assert_eq!(own.len(), 1);
        assert_eq!((own[0].kind, own[0].parameter), (E::Level, 1));
        assert_ne!(own[0].to, sp.number);
    }
}

#[test]
fn evolutions_only_lead_to_species_with_the_same_curve() {
    let mut g = game();
    for sp in &mut g.species {
        if sp.number != 1 && sp.number != 2 {
            sp.growth_curve = ExpCurve::Slow;
        }
    }
    g.evolutions = vec![Evolution::new(1, 2, E::Level, 16), Evolution::new(2, 3, E::Level, 32)];
    // 1 can only become 2, and 2 nothing.
    let err = try_run(g, random(), 1).expect_err("impossible");
    assert!(err.to_string().contains("Standardize EXP Curves"), "{err}");
}

#[test]
fn adjusted_levels_follow_the_totals() {
    let mut g = game();
    // 7 → 8 gets much weaker species at both ends, so a lower level.
    for sp in &mut g.species {
        if sp.number == 7 || sp.number == 8 {
            sp.stats = ppr_model::BaseStats {
                hp: 10,
                attack: 10,
                defense: 10,
                speed: 10,
                sp_attack: 10,
                sp_defense: 10,
            };
        }
    }
    let s = EvolutionSettings {
        adjust_levels: true,
        ..EvolutionSettings::default()
    };
    let r = run_on(g, s, 1);
    let e = r.evolutions.iter().find(|e| e.from == 7).expect("there");
    assert!(e.parameter < 20, "{}", e.parameter);
    assert_eq!(e.parameter, e.estimated_level);
    let stone = r.evolutions.iter().find(|e| e.from == 10).expect("there");
    assert_eq!((stone.kind, stone.parameter), (E::Stone, 82), "only levels change");
}

#[test]
fn unchanged_changes_nothing() {
    assert_eq!(run_on(game(), EvolutionSettings::default(), 1), game());
}

/// [`game`] with Giratina (487) and alternate formes: 501 of species 1 evolving into 502, the irregular forme of
/// species 2, the cosmetic 503 of species 7 evolving into 8, and Giratina's Origin Forme 504. Alternate formes have
/// evolutions of their own when `own` is set.
fn game_with_formes(own: bool) -> GameData {
    let mut g = game();
    g.species.push(species(487, Type::Ghost, None, 100));
    g.species.push(testing::alt_forme(501, 1, 1, 30, false, false));
    g.species.push(testing::alt_forme(502, 2, 1, 40, false, true));
    g.species.push(testing::alt_forme(503, 7, 1, 40, true, false));
    g.species.push(testing::alt_forme(504, 487, 1, 100, false, false));
    g.evolutions.push(Evolution::new(501, 502, E::Level, 16));
    g.evolutions.push(Evolution::new(503, 8, E::Level, 20));
    g.formes.evolutions = own;
    g.estimate_evolution_levels();
    g
}

fn run_with(game: GameData, no_irregular: bool, seed: u64) -> GameData {
    let mut settings = Settings {
        evolutions: random(),
        ..Settings::default()
    };
    settings.general.no_irregular_alt_formes = no_irregular;
    let mut data = RunData { game, settings };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn alternate_formes_with_their_own_evolutions_join_the_pool() {
    for seed in 0..20 {
        // Without evolutions of their own, alternate formes are left alone and never evolved into.
        let g = run_with(game_with_formes(false), false, seed);
        assert_eq!(targets(&g, 501), [502], "seed {seed}");
        assert!(g.evolutions.iter().filter(|e| e.from != 501).all(|e| e.to < 500));
    }
    let mut into = BTreeSet::new();
    for seed in 0..60 {
        let g = run_with(game_with_formes(true), false, seed);
        assert_eq!(
            targets(&g, 503),
            [8],
            "seed {seed}: a cosmetic forme keeps its evolution"
        );
        assert_eq!(
            targets(&g, 501).len(),
            1,
            "seed {seed}: the forme gets a random evolution"
        );
        assert!(
            !g.evolutions.iter().any(|e| e.to == 503 || e.to == 504),
            "seed {seed}: {:?}",
            g.evolutions
        );
        into.extend(g.evolutions.iter().map(|e| e.to).filter(|&n| n > 500));
    }
    assert_eq!(into, BTreeSet::from([501, 502]), "alternate formes are evolved into");
    for seed in 0..30 {
        let g = run_with(game_with_formes(true), true, seed);
        assert!(
            !g.evolutions.iter().any(|e| e.to == 502),
            "seed {seed}: No Irregular Alt Formes"
        );
    }
}
