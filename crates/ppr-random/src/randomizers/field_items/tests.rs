use std::collections::BTreeMap;

use ppr_model::{GameData, Item};
use ppr_settings::{FieldItemMode, FieldItemSettings, Settings};

use super::*;
use crate::randomizers::testing::emerald;
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Items 1–20 (5 bad, 20 not allowed) and TMs 328–377; field items alternate between items and TMs.
fn game() -> GameData {
    let mut g = emerald(Vec::new());
    g.items = (1..=20)
        .map(|id| Item {
            id,
            name: format!("I{id}"),
            allowed: id != 20,
            bad: id == 5,
            tm: false,
        })
        .chain((328..378).map(|id| Item {
            id,
            name: format!("TM{id}"),
            allowed: true,
            bad: false,
            tm: true,
        }))
        .collect();
    g.field_items = vec![1, 328, 2, 2, 329, 3, 4, 330, 5, 6];
    g.required_field_tms = vec![340];
    g
}

fn run(mode: FieldItemMode, ban_bad: bool, seed: u64) -> GameData {
    let mut data = RunData {
        game: game(),
        settings: Settings {
            field_items: FieldItemSettings { mode, ban_bad },
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn kinds(g: &GameData) -> Vec<bool> {
    g.field_items.iter().map(|&i| is_tm(g, i)).collect()
}

#[test]
fn shuffling_keeps_the_items_and_where_tms_are() {
    let g = run(FieldItemMode::Shuffle, false, 1);
    assert_eq!(kinds(&g), kinds(&game()));
    let count = |items: &[ItemId]| {
        items.iter().fold(BTreeMap::new(), |mut m, &i| {
            *m.entry(i).or_insert(0) += 1;
            m
        })
    };
    assert_eq!(count(&g.field_items), count(&game().field_items));
}

#[test]
fn random_tms_are_distinct_and_include_the_required_ones() {
    for seed in 0..20 {
        let g = run(FieldItemMode::Random, true, seed);
        assert_eq!(kinds(&g), kinds(&game()), "seed {seed}");
        let tms: Vec<ItemId> = g.field_items.iter().copied().filter(|&i| is_tm(&g, i)).collect();
        assert!(tms.contains(&340));
        assert_eq!(tms.iter().collect::<BTreeSet<_>>().len(), tms.len());
        assert!(!g.field_items.contains(&5), "no bad items");
        assert!(!g.field_items.contains(&20), "only allowed items");
    }
}

#[test]
fn even_items_appear_once_before_any_twice() {
    let mut g = game();
    g.field_items = (0..19).map(|i| 1 + i % 3).collect();
    g.required_field_tms.clear();
    let mut data = RunData {
        game: g,
        settings: Settings {
            field_items: FieldItemSettings {
                mode: FieldItemMode::RandomEven,
                ban_bad: false,
            },
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(2), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    let distinct: BTreeSet<ItemId> = data.game.field_items.iter().copied().collect();
    assert_eq!(distinct.len(), 19, "19 allowed items, each once");
}

/// Field items over 40 item slots, with items 1–4 Mega Stones, for `mode` with or without balanced shop prices.
fn run_with_mega_stones(mode: FieldItemMode, balanced: bool, seed: u64) -> Vec<ItemId> {
    let mut g = game();
    g.field_items = vec![6; 40];
    g.required_field_tms.clear();
    g.mega_stones = vec![1, 2, 3, 4];
    let mut data = RunData {
        game: g,
        settings: Settings {
            field_items: FieldItemSettings { mode, ban_bad: false },
            shops: ppr_settings::ShopItemSettings {
                balance_prices: balanced,
                ..ppr_settings::ShopItemSettings::default()
            },
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game.field_items
}

fn most_of_a_mega_stone(items: &[ItemId]) -> usize {
    (1..=4)
        .map(|m| items.iter().filter(|&&i| i == m).count())
        .max()
        .unwrap_or(0)
}

#[test]
fn each_mega_stone_is_placed_once_unless_prices_are_balanced() {
    for mode in [FieldItemMode::Random, FieldItemMode::RandomEven] {
        let mut repeated = false;
        for seed in 0..20 {
            let items = run_with_mega_stones(mode, false, seed);
            assert_eq!(items.len(), 40);
            assert!(most_of_a_mega_stone(&items) <= 1, "{mode:?}, seed {seed}");
            repeated |= most_of_a_mega_stone(&run_with_mega_stones(mode, true, seed)) > 1;
        }
        assert!(
            repeated,
            "{mode:?}: with balanced prices a Mega Stone may be placed again"
        );
    }
}

#[test]
fn evenly_spread_items_go_round_again_without_the_mega_stones_placed() {
    // 19 allowed items: the first round places every one, the second round all but the Mega Stones.
    // The items are put back from the last chosen.
    let mut items = run_with_mega_stones(FieldItemMode::RandomEven, false, 3);
    items.reverse();
    let distinct: BTreeSet<ItemId> = items[..19].iter().copied().collect();
    assert_eq!(distinct.len(), 19);
    let second: BTreeSet<ItemId> = items[19..34].iter().copied().collect();
    assert_eq!(second.len(), 15);
    assert!(second.iter().all(|i| !(1..=4).contains(i)));
}
