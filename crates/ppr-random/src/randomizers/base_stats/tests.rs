use ppr_model::{BaseStats, Evolution, EvolutionType, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::stat_points::{from_stat_order, stat_order, stats_of};
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Species 1–20 with distinct stats, 1 → 2 → 3, Nincada (290) → Ninjask (291) and Shedinja (292).
fn game() -> GameData {
    let mut numbers: Vec<DexNumber> = (1..=20).collect();
    numbers.extend([290, 291, 292]);
    let species = numbers
        .into_iter()
        .map(|n| {
            let mut sp = testing::species(n, Type::Normal, None, 0);
            let b = u8::try_from(n % 50).expect("small");
            sp.stats = from_stat_order([30 + b, 40 + b, 50 + b, 60 + b, 70 + b, 80 + b]);
            sp
        })
        .collect();
    let mut g = emerald(species);
    let shedinja = species_mut(&mut g, SHEDINJA).expect("there");
    shedinja.stats = from_stat_order([1, 90, 45, 30, 30, 40]);
    g.evolutions = [(1, 2), (2, 3), (290, 291), (290, 292)]
        .into_iter()
        .map(|(from, to)| Evolution::new(from, to, EvolutionType::Level, 20))
        .collect();
    g
}

fn run_on(game: GameData, settings: BaseStatSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            base_stats: settings,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn settings(mode: BaseStatMode, follow_evolutions: bool) -> BaseStatSettings {
    BaseStatSettings {
        mode,
        follow_evolutions,
        follow_megas: false,
        random_evolution_stats: false,
        update_to_generation: None,
    }
}

fn stats(g: &GameData, n: DexNumber) -> [u8; 6] {
    stat_order(g.species(n).expect("there").stats)
}

/// Which old stat each new stat came from, for distinct stats.
fn order(old: [u8; 6], new: [u8; 6]) -> Vec<usize> {
    new.iter()
        .map(|v| old.iter().position(|o| o == v).expect("shuffled"))
        .collect()
}

#[test]
fn unchanged_changes_nothing() {
    let g = game();
    assert_eq!(run_on(g.clone(), BaseStatSettings::default(), 1), g);
}

#[test]
fn shuffle_permutes_each_species_stats() {
    let old = game();
    let new = run_on(old.clone(), settings(BaseStatMode::Shuffle, false), 2);
    assert_ne!(new, old);
    for sp in &old.species {
        let mut a = stats(&old, sp.number);
        let mut b = stats(&new, sp.number);
        if sp.number == SHEDINJA {
            assert_eq!(b[0], 1, "Shedinja keeps its HP");
            let rest = |s: [u8; 6]| s[1..].iter().map(|&v| u32::from(v)).sum::<u32>();
            assert_eq!(rest(a), rest(b));
            continue;
        }
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "{}", sp.name);
    }
}

#[test]
fn shuffle_following_evolutions_shares_the_order() {
    for seed in 0..10 {
        let old = game();
        let new = run_on(old.clone(), settings(BaseStatMode::Shuffle, true), seed);
        let first = order(stats(&old, 1), stats(&new, 1));
        assert_eq!(order(stats(&old, 2), stats(&new, 2)), first);
        assert_eq!(order(stats(&old, 3), stats(&new, 3)), first);
        // Ninjask and Shedinja split from Nincada; split evolutions get their own order when shuffling.
    }
}

#[test]
fn random_keeps_totals_and_minimums() {
    let old = game();
    let new = run_on(old.clone(), settings(BaseStatMode::Random, false), 3);
    assert_ne!(new, old);
    for sp in &old.species {
        let after = new.species(sp.number).expect("there");
        assert_eq!(after.stats.total(), sp.stats.total(), "{}", sp.name);
        let s = stat_order(after.stats);
        if sp.number == SHEDINJA {
            assert_eq!(s[0], 1);
        } else {
            assert!(s[0] >= 20 && s[1..].iter().all(|&v| v >= 10), "{s:?}");
        }
    }
}

#[test]
fn random_following_evolutions_copies_the_spread() {
    let new = run_on(game(), settings(BaseStatMode::Random, true), 4);
    let mut copy = new.species(1).expect("there").clone();
    copy.stats = game().species(2).expect("there").stats;
    assert!(set_ratios(
        &mut copy,
        &current_ratios(new.species(1).expect("there"), false),
        false
    ));
    assert_eq!(stats(&new, 2), stat_order(copy.stats), "Mon2 is spread like Mon1");
}

#[test]
fn random_evolution_stats_add_to_the_pre_evolution() {
    let s = BaseStatSettings {
        random_evolution_stats: true,
        ..settings(BaseStatMode::Random, true)
    };
    for seed in 0..10 {
        let new = run_on(game(), s.clone(), seed);
        let (a, b) = (stats(&new, 1), stats(&new, 2));
        // Each stat gets about a sixth of the 6 extra points or more, rounded; never much less than before.
        assert!(a.iter().zip(b).all(|(&x, y)| y + 1 >= x), "{a:?} {b:?}");
        assert_eq!(stats(&new, 292)[0], 1);
    }
}

#[test]
fn halves_round_up() {
    assert!((round_half_up(-2.5) - -2.0).abs() < f64::EPSILON);
    assert!((round_half_up(2.5) - 3.0).abs() < f64::EPSILON);
}

/// Red with species 1–10 of distinct stats and 1 → 2 → 3, Mon3 having a lower total than Mon2.
fn gen1_game() -> GameData {
    let species = (1..=10)
        .map(|n| {
            let mut sp = testing::species(n, Type::Normal, None, 0);
            let b = u8::try_from(n).expect("small");
            sp.stats = BaseStats {
                hp: 30 + b,
                attack: 40 + b,
                defense: 50 + b,
                speed: 60 + b,
                sp_attack: 70 + b,
                sp_defense: 0,
            };
            sp
        })
        .collect();
    let mut g = testing::gen1(species, false);
    species_mut(&mut g, 3).expect("there").stats.attack = 10;
    g.evolutions = [(1, 2), (2, 3)]
        .into_iter()
        .map(|(from, to)| Evolution::new(from, to, EvolutionType::Level, 20))
        .collect();
    g
}

fn gen1_stats(g: &GameData, n: DexNumber) -> Vec<u8> {
    stats_of(g.species(n).expect("there").stats, true)
}

#[test]
fn gen1_shuffle_permutes_five_stats() {
    let old = gen1_game();
    let new = run_on(old.clone(), settings(BaseStatMode::Shuffle, true), 5);
    assert_ne!(new, old);
    for sp in &old.species {
        let mut a = gen1_stats(&old, sp.number);
        let mut b = gen1_stats(&new, sp.number);
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "{}", sp.name);
        assert_eq!(new.species(sp.number).expect("there").stats.sp_defense, 0);
    }
}

#[test]
fn gen1_random_spreads_five_stats() {
    let s = BaseStatSettings {
        random_evolution_stats: true,
        ..settings(BaseStatMode::Random, true)
    };
    for seed in 0..10 {
        let old = gen1_game();
        let new = run_on(old.clone(), s.clone(), seed);
        for sp in &old.species {
            let after = new.species(sp.number).expect("there");
            assert_eq!(after.stats.total(), sp.stats.total(), "{}", sp.name);
            assert_eq!(after.stats.sp_defense, 0);
            let st = gen1_stats(&new, sp.number);
            assert!(st[0] >= 20 && st[1..].iter().all(|&v| v >= 10), "{st:?}");
        }
    }
}

/// [`game`] with Mega Evolutions: 900 of species 3, and 901 and 902 of species 6, each with 100 more in total.
fn mega_game() -> GameData {
    let mut g = game();
    for (n, base, number) in [(900, 3, 1), (901, 6, 1), (902, 6, 2)] {
        let mut sp = g.species(base).expect("there").clone();
        sp.number = n;
        let [hp, atk, def, spa, spd, spe] = stat_order(sp.stats);
        sp.stats = from_stat_order([hp + 12, atk + 31, def + 7, spa + 23, spd + 17, spe + 10]);
        sp.forme = ppr_model::Forme {
            base: Some(base),
            number,
            mega: true,
            ..ppr_model::Forme::default()
        };
        g.species.push(sp);
    }
    g
}

fn following_megas(mode: BaseStatMode) -> BaseStatSettings {
    BaseStatSettings {
        follow_megas: true,
        ..settings(mode, true)
    }
}

/// `n`'s stats as loaded, spread like `like`'s in `g`.
fn spread_like(g: &GameData, n: DexNumber, like: DexNumber) -> [u8; 6] {
    let mut copy = mega_game().species(n).expect("there").clone();
    assert!(set_ratios(
        &mut copy,
        &current_ratios(g.species(like).expect("there"), false),
        false
    ));
    stat_order(copy.stats)
}

#[test]
fn random_stats_leave_mega_evolutions_unless_they_follow() {
    let old = mega_game();
    let new = run_on(old.clone(), settings(BaseStatMode::Random, true), 5);
    for n in [900, 901, 902] {
        assert_eq!(
            stats(&new, n),
            stats(&old, n),
            "as alternate formes, as in the original"
        );
    }
    // Following Mega Evolutions without following evolutions leaves them as alternate formes too.
    let loose = BaseStatSettings {
        follow_megas: true,
        ..settings(BaseStatMode::Random, false)
    };
    let new = run_on(old.clone(), loose, 5);
    assert_eq!(stats(&new, 900), stats(&old, 900));
}

#[test]
fn random_stats_spread_mega_evolutions_like_their_base_species() {
    for seed in 0..10 {
        let old = mega_game();
        let new = run_on(old.clone(), following_megas(BaseStatMode::Random), seed);
        assert_eq!(stats(&new, 900), spread_like(&new, 900, 3), "{seed}");
        // Two Mega Evolutions of one species split: their extra 100 is spread at random.
        for n in [901, 902] {
            let after = new.species(n).expect("there");
            assert_eq!(after.stats.total(), old.species(n).expect("there").stats.total());
            let (base, mega) = (stats(&new, 6), stats(&new, n));
            assert!(
                base.iter().zip(mega).all(|(&b, m)| m + 1 >= b),
                "{seed}: {base:?} {mega:?}"
            );
        }
    }
}

#[test]
fn random_evolution_stats_reach_mega_evolutions() {
    let s = BaseStatSettings {
        random_evolution_stats: true,
        ..following_megas(BaseStatMode::Random)
    };
    let mut differs = false;
    for seed in 0..10 {
        let new = run_on(mega_game(), s.clone(), seed);
        let (base, mega) = (stats(&new, 3), stats(&new, 900));
        assert!(
            base.iter().zip(mega).all(|(&b, m)| m + 1 >= b),
            "{seed}: {base:?} {mega:?}"
        );
        differs |= mega != spread_like(&new, 900, 3);
    }
    assert!(differs, "the extra total is not spread like the base species");
}

#[test]
fn shuffled_split_mega_evolutions_get_their_own_order() {
    let mut own = false;
    for seed in 0..10 {
        let old = mega_game();
        let new = run_on(old.clone(), following_megas(BaseStatMode::Shuffle), seed);
        let base = order(stats(&old, 3), stats(&new, 3));
        assert_eq!(order(stats(&old, 900), stats(&new, 900)), base, "{seed}");
        let six = order(stats(&old, 6), stats(&new, 6));
        own |= [901, 902].iter().any(|&n| order(stats(&old, n), stats(&new, n)) != six);
        // Without following Mega Evolutions they copy as alternate formes.
        let formes = run_on(old.clone(), settings(BaseStatMode::Shuffle, true), seed);
        let six = order(stats(&old, 6), stats(&formes, 6));
        for n in [901, 902] {
            assert_eq!(order(stats(&old, n), stats(&formes, n)), six, "{seed}");
        }
    }
    assert!(own, "split Mega Evolutions are basic when shuffling");
}
