use ppr_model::{InGameTrade, Item, Type};
use ppr_settings::Settings;

use super::*;
use crate::names::{POKEMON_NICKNAMES, TRAINER_NAMES};
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

fn trade(requested: DexNumber, given: DexNumber, nickname: &str) -> InGameTrade {
    InGameTrade {
        requested: Some(requested),
        given,
        nickname: nickname.into(),
        ot_name: "KOBE".into(),
        ot_id: 1,
        ivs: vec![5; 6],
        held_item: None,
    }
}

/// Species 1–30 called `MON<n>`, three trades (the second named after its species, the third a species for
/// itself) and items 1–3, of which 3 is not allowed.
fn game() -> GameData {
    let species = (1..=30)
        .map(|number: DexNumber| {
            let mut sp = testing::species(number, Type::Normal, None, 50);
            sp.name = format!("MON{number}");
            sp
        })
        .collect();
    GameData {
        items: (1..=3)
            .map(|id| Item {
                id,
                name: format!("Item{id}"),
                allowed: id != 3,
                bad: false,
                tm: false,
            })
            .collect(),
        trades: vec![trade(1, 2, "DOTS"), trade(3, 4, "mon4"), trade(5, 5, "SAME")],
        ..emerald(species)
    }
}

fn run_on(game: GameData, settings: TradeSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            trades: settings,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn with_mode(mode: TradeMode) -> TradeSettings {
    TradeSettings {
        mode,
        ..TradeSettings::default()
    }
}

#[test]
fn given_only_keeps_the_requests_and_the_rest() {
    let before = game();
    for seed in 0..30 {
        let after = run_on(game(), with_mode(TradeMode::Given), seed);
        let given: Vec<DexNumber> = after.trades.iter().map(|t| t.given).collect();
        let mut unique = given.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 3, "seed {seed}: every trade gives another species");
        assert_eq!(after.trades[0].requested, Some(1));
        assert_eq!(after.trades[1].requested, Some(3));
        // A species for itself stays that way.
        assert_eq!(after.trades[2].requested, Some(given[2]));
        // Named after its species, so renamed with it; other names, OTs, IVs and items stay.
        assert_eq!(after.trades[1].nickname, format!("MON{}", given[1]));
        assert_eq!(after.trades[0].nickname, "DOTS");
        for (b, a) in before.trades.iter().zip(&after.trades) {
            assert_eq!(
                (&a.ot_name, a.ot_id, &a.ivs, a.held_item),
                (&b.ot_name, b.ot_id, &b.ivs, b.held_item)
            );
        }
    }
}

#[test]
fn both_sides_ask_for_another_species_than_they_give() {
    for seed in 0..30 {
        let after = run_on(game(), with_mode(TradeMode::GivenAndRequested), seed);
        assert_ne!(after.trades[0].requested, Some(after.trades[0].given), "seed {seed}");
        assert_ne!(after.trades[0].requested, after.trades[1].requested, "seed {seed}");
        assert_eq!(after.trades[2].requested, Some(after.trades[2].given), "seed {seed}");
    }
}

#[test]
fn names_ivs_and_items_are_randomized_when_asked() {
    let settings = TradeSettings {
        random_nicknames: true,
        random_ots: true,
        random_ivs: true,
        random_items: true,
        ..with_mode(TradeMode::Given)
    };
    let mut ivs_changed = false;
    for seed in 0..30 {
        let after = run_on(game(), settings.clone(), seed);
        let nicknames: Vec<&str> = after.trades.iter().map(|t| t.nickname.as_str()).collect();
        assert!(nicknames.iter().all(|n| POKEMON_NICKNAMES.contains(n)), "{nicknames:?}");
        let mut unique = nicknames.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 3, "no nickname twice");
        for t in &after.trades {
            assert!(
                TRAINER_NAMES.contains(&t.ot_name.as_str()) && t.ot_name.chars().count() <= 7,
                "{}",
                t.ot_name
            );
            assert!(t.ivs.iter().all(|&iv| iv < 32));
            assert!(matches!(t.held_item, Some(1 | 2)), "allowed items only");
            ivs_changed |= t.ivs != [5; 6];
        }
    }
    assert!(ivs_changed);
}

#[test]
fn unchanged_or_tradeless_games_are_left_alone() {
    let data = |game: GameData, mode: TradeMode| RunData {
        game,
        settings: Settings {
            trades: with_mode(mode),
            ..Settings::default()
        },
    };
    assert!(!TradeRandomizer.applies(&data(game(), TradeMode::Unchanged)));
    let mut none = game();
    none.trades.clear();
    assert!(!TradeRandomizer.applies(&data(none, TradeMode::Given)));
}

#[test]
fn only_names_that_fit_are_used() {
    let mut g = game();
    assert!(fitting(&g, &TRAINER_NAMES, 7).iter().all(|n| n.len() <= 7));
    assert!(fitting(&g, &TRAINER_NAMES, 7).contains(&"Esteban"));
    assert!(!fitting(&g, &TRAINER_NAMES, 7).contains(&"Jonathan"));
    assert_eq!(fitting(&g, &["A", "A", "B"], 1), ["A", "B"]);
    // Measured in the game's bytes: "[PK]" takes one.
    g.text_tokens = ["A", "B", "[PK]"].map(String::from).to_vec();
    g.text_tokens.sort();
    assert_eq!(fitting(&g, &["[PK]A", "ABA"], 2), ["[PK]A"]);
}

#[test]
fn gen1_trades_keep_what_they_dont_have() {
    let mut g = game();
    let species = g.species.clone();
    let gen1 = testing::gen1(species, false);
    g.identity = gen1.identity;
    for t in &mut g.trades {
        t.ot_name = String::new();
        t.ot_id = 0;
        t.ivs = Vec::new();
    }
    let before = g.clone();
    let s = TradeSettings {
        random_ots: true,
        random_ivs: true,
        random_items: true,
        ..with_mode(TradeMode::Given)
    };
    let after = run_on(g, s, 1);
    for (a, b) in after.trades.iter().zip(&before.trades) {
        assert_eq!(
            (&a.ot_name, a.ot_id, &a.ivs, a.held_item),
            (&b.ot_name, b.ot_id, &b.ivs, b.held_item)
        );
    }
}
