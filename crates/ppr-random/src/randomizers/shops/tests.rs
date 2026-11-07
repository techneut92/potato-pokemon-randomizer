use ppr_model::{GameData, Item};
use ppr_settings::{Settings, ShopItemMode, ShopItemSettings};

use super::*;
use crate::randomizers::testing::emerald;
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Items 1–100 (TM 99, item 90 bad) and four shops: two special main-game ones, one special after the game and
/// one regular Poké Mart.
fn game() -> GameData {
    let mut g = emerald(Vec::new());
    g.items = (1..=100)
        .map(|id| Item {
            id,
            name: format!("I{id}"),
            allowed: true,
            bad: id == 90,
            tm: id == 99,
        })
        .collect();
    let shop = |items: Vec<ItemId>, main_game, special| Shop {
        name: "S".into(),
        items,
        main_game,
        special,
    };
    g.shops = vec![
        shop(vec![45, 46, 47, 48, 49, 52], true, true),
        shop(vec![80, 81, 82, 83, 84], true, true),
        shop(vec![30, 31, 32], false, true),
        shop(vec![4, 17], false, false),
    ];
    g.item_prices = (1..=100).map(|id| (id, 100)).collect();
    g.balanced_item_prices = [(1, 3000), (50, 24_900)].into_iter().collect();
    g.op_shop_items = vec![50, 86, 87];
    g.evolution_items = vec![80, 81, 82, 83, 84, 85];
    g
}

fn run(s: ShopItemSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game: game(),
        settings: Settings {
            shops: s,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn shuffling_moves_items_between_special_shops_only() {
    let g = run(
        ShopItemSettings {
            mode: ShopItemMode::Shuffle,
            ..ShopItemSettings::default()
        },
        1,
    );
    let sorted = |g: &GameData| {
        let mut v: Vec<ItemId> = g
            .shops
            .iter()
            .filter(|s| s.special)
            .flat_map(|s| s.items.clone())
            .collect();
        v.sort_unstable();
        v
    };
    assert_eq!(sorted(&g), sorted(&game()));
    assert_eq!(g.shops[3], game().shops[3]);
    assert!(
        g.shops
            .iter()
            .zip(&game().shops)
            .all(|(a, b)| a.items.len() == b.items.len())
    );
}

#[test]
fn random_shops_follow_the_bans_and_guarantees() {
    for seed in 0..20 {
        let g = run(
            ShopItemSettings {
                mode: ShopItemMode::Random,
                ban_overpowered: true,
                ban_bad: true,
                ban_regular: true,
                guarantee_evolution_items: true,
                guarantee_x_items: false,
                ..ShopItemSettings::default()
            },
            seed,
        );
        let main: Vec<ItemId> = g.shops[..2].iter().flat_map(|s| s.items.clone()).collect();
        for evo in 80..=85 {
            assert!(
                main.contains(&evo),
                "seed {seed}: evolution item {evo} in a main-game shop"
            );
            assert!(!g.shops[2].items.contains(&evo));
        }
        let special: Vec<ItemId> = g.shops[..3].iter().flat_map(|s| s.items.clone()).collect();
        for banned in [99, 90, 50, 86, 4, 17, 28] {
            assert!(!special.contains(&banned), "seed {seed}: {banned}");
        }
        assert_eq!(g.shops[3], game().shops[3], "regular marts stay");
    }
}

#[test]
fn prices_balance_and_rare_candies_are_added_everywhere() {
    let g = run(
        ShopItemSettings {
            balance_prices: true,
            add_cheap_rare_candies: true,
            ..ShopItemSettings::default()
        },
        1,
    );
    assert_eq!(g.item_prices[&1], 3000);
    assert_eq!(g.item_prices[&50], 10, "candies are cheap after balancing");
    assert!(g.shops.iter().all(|s| s.items.last() == Some(&RARE_CANDY)));
}

#[test]
fn gen_6_and_7_shops_keep_their_size_with_cheap_rare_candies() {
    let mut data = RunData {
        game: game(),
        settings: Settings {
            shops: ShopItemSettings {
                add_cheap_rare_candies: true,
                ..ShopItemSettings::default()
            },
            ..Settings::default()
        },
    };
    data.game.identity.generation = ppr_model::Generation::new(7).expect("valid");
    standard_pipeline()
        .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    assert_eq!(data.game.shops, game().shops);
    assert_eq!(data.game.item_prices[&RARE_CANDY], 10);
}

#[test]
fn too_many_guaranteed_items_stop_the_run() {
    let mut data = RunData {
        game: game(),
        settings: Settings {
            shops: ShopItemSettings {
                mode: ShopItemMode::Random,
                guarantee_evolution_items: true,
                guarantee_x_items: true,
                ..ShopItemSettings::default()
            },
            ..Settings::default()
        },
    };
    let result = standard_pipeline().run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {});
    assert!(result.is_err(), "14 guaranteed items, 11 main-game slots");
}
