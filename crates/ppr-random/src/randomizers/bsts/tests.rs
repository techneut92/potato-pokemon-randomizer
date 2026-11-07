use ppr_model::{Evolution, EvolutionType, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::stat_points::{SHEDINJA, from_stat_order};
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Lines 1 → 2 → 3 and 4 → 5 → 6, 7 → 8, 9 → 10, singles 11–14, legendaries 150 and 151, Nincada (290) →
/// Ninjask (291) and Shedinja (292); totals differ.
fn game() -> GameData {
    let mut numbers: Vec<DexNumber> = (1..=14).collect();
    numbers.extend([150, 151, 290, 291, 292]);
    let species = numbers
        .into_iter()
        .map(|n| {
            let mut sp = testing::species(n, Type::Normal, None, 0);
            let b = u8::try_from(n % 40).expect("small");
            sp.stats = from_stat_order([40 + b, 50 + b, 45 + b, 60 + b, 55 + b, 35 + 2 * b]);
            sp
        })
        .collect();
    let mut g = emerald(species);
    let i = g.species.iter().position(|s| s.number == SHEDINJA).expect("there");
    g.species[i].stats = from_stat_order([1, 90, 45, 30, 30, 40]);
    g.evolutions = [(1, 2), (2, 3), (4, 5), (5, 6), (7, 8), (9, 10), (290, 291), (290, 292)]
        .into_iter()
        .map(|(from, to)| Evolution::new(from, to, EvolutionType::Level, 20))
        .collect();
    g
}

fn run_on(game: GameData, settings: BstSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            bsts: settings,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn settings(mode: BstMode) -> BstSettings {
    BstSettings {
        mode,
        max_buff_nerf_percent: 20,
        follow_evolutions: false,
        separate_legendaries: false,
    }
}

fn bsts(g: &GameData) -> BTreeMap<DexNumber, u32> {
    g.species.iter().map(|s| (s.number, u32::from(s.bst()))).collect()
}

#[test]
fn buff_nerf_stays_within_the_percentage() {
    let old = game();
    for seed in 0..5 {
        let new = run_on(old.clone(), settings(BstMode::BuffNerf), seed);
        assert_ne!(bsts(&new), bsts(&old));
        for (n, before) in bsts(&old) {
            let after = bsts(&new)[&n];
            let (low, high) = (f64::from(before) * 0.79, f64::from(before) * 1.21);
            assert!((low..=high).contains(&f64::from(after)), "{n}: {before} -> {after}");
        }
        assert_eq!(new.species(SHEDINJA).expect("there").stats.hp, 1);
    }
}

#[test]
fn buff_nerf_following_evolutions_shares_the_factor() {
    let s = BstSettings {
        follow_evolutions: true,
        max_buff_nerf_percent: 50,
        ..settings(BstMode::BuffNerf)
    };
    let old = bsts(&game());
    let new = bsts(&run_on(game(), s, 3));
    let factor = |n: DexNumber| f64::from(new[&n]) / f64::from(old[&n]);
    for (a, b) in [(1, 2), (2, 3), (4, 6), (7, 8)] {
        assert!(
            (factor(a) - factor(b)).abs() < 0.02,
            "{a} {b}: {} {}",
            factor(a),
            factor(b)
        );
    }
}

#[test]
fn zero_percent_changes_nothing() {
    let s = BstSettings {
        max_buff_nerf_percent: 0,
        ..settings(BstMode::BuffNerf)
    };
    assert_eq!(run_on(game(), s, 1), game());
}

#[test]
fn shuffle_swaps_totals() {
    let old = bsts(&game());
    let new = bsts(&run_on(game(), settings(BstMode::Shuffle), 4));
    assert_ne!(new, old);
    let (mut a, mut b): (Vec<u32>, Vec<u32>) = (old.values().copied().collect(), new.values().copied().collect());
    a.sort_unstable();
    b.sort_unstable();
    // Shedinja's total for power levels is rounded through its five stats.
    let close = a.iter().zip(&b).filter(|(x, y)| x.abs_diff(**y) > 1).count();
    assert_eq!(close, 0, "{a:?}\n{b:?}");
}

#[test]
fn shuffle_following_evolutions_swaps_whole_lines() {
    let s = BstSettings {
        follow_evolutions: true,
        separate_legendaries: true,
        ..settings(BstMode::Shuffle)
    };
    let old = bsts(&game());
    for seed in 0..10 {
        let new = bsts(&run_on(game(), s.clone(), seed));
        // The three-stage lines swap as a whole, or keep theirs.
        let line = |m: &BTreeMap<DexNumber, u32>, first: DexNumber| [m[&first], m[&(first + 1)], m[&(first + 2)]];
        let (l1, l4) = (line(&new, 1), line(&new, 4));
        assert!(
            (l1 == line(&old, 1) && l4 == line(&old, 4)) || (l1 == line(&old, 4) && l4 == line(&old, 1)),
            "{seed}: {l1:?} {l4:?}"
        );
        // Legendaries swap among themselves.
        let mut legendary = [new[&150], new[&151]];
        legendary.sort_unstable();
        let mut expected = [old[&150], old[&151]];
        expected.sort_unstable();
        assert_eq!(legendary, expected);
    }
}

#[test]
fn random_totals_are_in_range() {
    let new = run_on(game(), settings(BstMode::Random), 5);
    for sp in &new.species {
        let total = u32::from(sp.bst());
        assert!(
            (SUNKERN_BST - 1..ARCEUS_BST + 1).contains(&total),
            "{}: {total}",
            sp.name
        );
    }
}
