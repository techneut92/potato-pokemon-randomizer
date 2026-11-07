use ppr_model::{Item, StaticEncounter, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::testing::{emerald, species};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

fn item(id: u16, tm: bool) -> Item {
    Item {
        id,
        name: format!("ITEM {id}"),
        allowed: true,
        bad: false,
        tm,
    }
}

fn game() -> GameData {
    let mut g = emerald([1, 4, 7, 122].map(|n| species(n, Type::Normal, None, 50)).to_vec());
    g.species[3].name = "MR. MIME".into();
    g.items = vec![item(13, false), item(LUCKY_EGG, false), item(328, true)];
    g.available_tweaks = vec![
        MiscTweak::FastestText,
        MiscTweak::RandomizePcPotion,
        MiscTweak::LowerCaseNames,
        MiscTweak::RandomizeCatchingTutorial,
        MiscTweak::BanLuckyEgg,
        MiscTweak::ReusableTms,
    ];
    g.catching_tutorial_opponents = vec![1, 4];
    g.catching_tutorial_players = vec![7];
    g.statics = vec![StaticEncounter::new(1, 5), StaticEncounter::new(4, 5)];
    g.fossils = vec![1];
    g
}

fn run(game: GameData, tweaks: &[MiscTweak]) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            misc_tweaks: tweaks.to_vec(),
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(3), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn chosen_tweaks_apply_where_available() {
    let all = [
        MiscTweak::FastestText,
        MiscTweak::RandomizePcPotion,
        MiscTweak::LowerCaseNames,
        MiscTweak::RandomizeCatchingTutorial,
        MiscTweak::BanLuckyEgg,
        MiscTweak::ReusableTms,
        // Not available in this game.
        MiscTweak::NationalDexAtStart,
        MiscTweak::BalanceStaticLevels,
    ];
    let g = run(game(), &all);
    assert_eq!(
        g.applied_tweaks.iter().copied().collect::<Vec<_>>(),
        [MiscTweak::FastestText, MiscTweak::ReusableTms]
    );
    assert!(!g.items[1].allowed, "Lucky Egg banned");
    // Neither the banned Lucky Egg nor the TM.
    assert_eq!(g.pc_potion, Some(13));
    assert_eq!(g.species[3].name, "Mr. Mime");
    let t = g.catching_tutorial.expect("set");
    assert!([1, 4].contains(&t.opponent) && t.player == 7);
    assert!(!g.pickup_tms);
    assert_eq!(g.statics[1].level, 5, "fossils only with the tweak available");
}

#[test]
fn nothing_changes_without_tweaks() {
    let g = run(game(), &[]);
    assert_eq!(g, game());
}

#[test]
fn fossils_are_balanced_to_level_30() {
    let mut g = game();
    g.available_tweaks.push(MiscTweak::BalanceStaticLevels);
    let g = run(g, &[MiscTweak::BalanceStaticLevels]);
    assert_eq!((g.statics[0].level, g.statics[1].level), (5, 30));
    assert!(
        g.applied_tweaks.contains(&MiscTweak::BalanceStaticLevels),
        "for games that write fossil levels themselves"
    );
}

#[test]
fn big_maniac_items_are_banned() {
    let mut g = game();
    g.items
        .extend([item(580, false), item(590, false), item(206, false), item(584, false)]);
    g.available_tweaks.push(MiscTweak::BanBigManiacItems);
    let g = run(g, &[MiscTweak::BanBigManiacItems]);
    let allowed: Vec<u16> = g.items.iter().filter(|i| i.allowed).map(|i| i.id).collect();
    assert_eq!(allowed, [13, LUCKY_EGG, 328, 584], "Relic Copper stays");
}

#[test]
fn names_are_title_cased() {
    assert_eq!(camel_case("FARFETCH'D"), "Farfetch'd");
    assert_eq!(camel_case("HO-OH"), "Ho-Oh");
    assert_eq!(camel_case("NIDORAN♀"), "Nidoran♀");
    assert_eq!(camel_case("PORYGON2"), "Porygon2");
}
