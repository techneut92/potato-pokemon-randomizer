use ppr_model::{Evolution, EvolutionType as E, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::testing::{emerald, species};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Gen 3's evolutions that the improvements change, with Bulbasaur's line for levels.
fn evolutions() -> Vec<Evolution> {
    vec![
        Evolution::new(1, 2, E::Level, 16),
        Evolution::new(2, 3, E::Level, 32),
        Evolution::new(64, 65, E::Trade, 0),
        Evolution::new(61, 186, E::TradeItem, 221),
        Evolution::new(79, 199, E::TradeItem, 221),
        Evolution::new(117, 230, E::TradeItem, 235),
        Evolution::new(95, 208, E::TradeItem, 233),
        Evolution::new(366, 367, E::TradeItem, 226),
        Evolution::new(366, 368, E::TradeItem, DEEP_SEA_SCALE),
        Evolution::new(133, 196, E::HappinessDay, 0),
        Evolution::new(133, 197, E::HappinessNight, 0),
        Evolution::new(349, 350, E::HighBeauty, 170),
        Evolution::new(25, 26, E::Stone, 83),
    ]
}

fn game() -> GameData {
    let mut numbers: Vec<DexNumber> = evolutions().iter().flat_map(|e| [e.from, e.to]).collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mut g = emerald(
        numbers
            .into_iter()
            .map(|n| species(n, Type::Normal, None, 50))
            .collect(),
    );
    g.evolutions = evolutions();
    g.estimate_evolution_levels();
    g
}

fn fire_red() -> GameData {
    let mut g = game();
    g.identity.code = "BPRE".into();
    g
}

fn run_on(game: GameData, evolutions: EvolutionSettings) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            evolutions,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn method(g: &GameData, from: DexNumber, to: DexNumber) -> (E, u16) {
    let e = g
        .evolutions
        .iter()
        .find(|e| e.from == from && e.to == to)
        .expect("there");
    (e.kind, e.parameter)
}

fn impossible(estimated: bool) -> EvolutionSettings {
    EvolutionSettings {
        change_impossible: true,
        use_estimated_levels: estimated,
        ..EvolutionSettings::default()
    }
}

#[test]
fn trade_evolutions_happen_by_level_or_stone() {
    let g = run_on(game(), impossible(false));
    assert_eq!(method(&g, 64, 65), (E::Level, 37));
    assert_eq!(method(&g, 61, 186), (E::Level, 37));
    assert_eq!(method(&g, 79, 199), (E::Stone, WATER_STONE));
    assert_eq!(method(&g, 117, 230), (E::Level, 40));
    assert_eq!(method(&g, 95, 208), (E::Level, 30));
    assert_eq!(method(&g, 366, 367), (E::Level, 30));
    assert_eq!(method(&g, 366, 368), (E::Stone, WATER_STONE));
    // Emerald has a clock and contests.
    assert_eq!(method(&g, 133, 196), (E::HappinessDay, 0));
    assert_eq!(method(&g, 349, 350), (E::HighBeauty, 170));
    assert_eq!(method(&g, 25, 26), (E::Stone, 83));
    assert!(!g.easier_happiness_evolutions);
}

#[test]
fn fire_red_has_no_clock_or_contests() {
    let g = run_on(fire_red(), impossible(false));
    assert_eq!(method(&g, 133, 196), (E::Stone, SUN_STONE));
    assert_eq!(method(&g, 133, 197), (E::Stone, MOON_STONE));
    assert_eq!(method(&g, 349, 350), (E::Level, 35));
}

#[test]
fn estimated_levels_replace_the_fixed_ones() {
    let before = game();
    let g = run_on(game(), impossible(true));
    let estimate = |from, to| {
        before
            .evolutions
            .iter()
            .find(|e| e.from == from && e.to == to)
            .expect("there")
            .estimated_level
    };
    assert_eq!(method(&g, 64, 65), (E::Level, estimate(64, 65)));
    assert_eq!(method(&g, 79, 199), (E::Stone, WATER_STONE), "stones stay stones");
}

#[test]
fn easier_evolutions_condense_levels_and_happiness() {
    let s = EvolutionSettings {
        easier_max_level: Some(30),
        ..EvolutionSettings::default()
    };
    let g = run_on(game(), s.clone());
    // 2 evolves again, so 1 into it at most at 23 (three quarters of 30, up); 16 stays.
    assert_eq!(method(&g, 1, 2), (E::Level, 16));
    assert_eq!(method(&g, 2, 3), (E::Level, 30));
    assert!(g.evolutions.iter().all(|e| e.estimated_level <= 30));
    assert!(g.easier_happiness_evolutions);

    let mut deep = game();
    deep.evolutions[0].update_method(E::Level, 28);
    let g = run_on(deep, s);
    assert_eq!(method(&g, 1, 2), (E::Level, 23));

    let g = run_on(
        game(),
        EvolutionSettings {
            easier_max_level: Some(55),
            ..EvolutionSettings::default()
        },
    );
    assert_eq!(g.evolutions, game().evolutions, "nothing above the maximum");
}

#[test]
fn time_based_evolutions_happen_at_any_time() {
    let mut g = game();
    g.evolutions.push(Evolution::new(361, 362, E::LevelDusk, 30));
    g.evolutions.push(Evolution::new(298, 183, E::HappinessNight, 0));
    let g = run_on(
        g,
        EvolutionSettings {
            remove_time_based: true,
            ..EvolutionSettings::default()
        },
    );
    assert_eq!(method(&g, 133, 196), (E::Stone, SUN_STONE));
    assert_eq!(method(&g, 133, 197), (E::Stone, MOON_STONE));
    assert_eq!(method(&g, 361, 362), (E::Stone, DUSK_STONE));
    assert_eq!(method(&g, 298, 183), (E::Happiness, 0));
}

#[test]
fn nothing_changes_by_default() {
    assert_eq!(run_on(game(), EvolutionSettings::default()), game());
}

#[test]
fn gen1_trade_evolutions_happen_at_37_and_nothing_gets_easier_happiness() {
    let mut g = crate::randomizers::testing::gen1(
        [1, 2, 3, 64, 65].map(|n| species(n, Type::Normal, None, 50)).to_vec(),
        false,
    );
    g.evolutions = vec![
        Evolution::new(1, 2, E::Level, 16),
        Evolution::new(2, 3, E::Level, 32),
        Evolution::new(64, 65, E::Trade, 0),
    ];
    g.estimate_evolution_levels();
    let s = EvolutionSettings {
        change_impossible: true,
        easier_max_level: Some(30),
        ..EvolutionSettings::default()
    };
    let g = run_on(g, s);
    assert_eq!(method(&g, 64, 65), (E::Level, 30), "37, condensed to 30");
    assert_eq!(method(&g, 2, 3), (E::Level, 30));
    assert!(!g.easier_happiness_evolutions);
}

/// A Gen 4 game with the evolutions Gen 4 changes: by trade, beauty, place, move and party member.
fn gen4(family: &str) -> GameData {
    let evolutions = vec![
        Evolution::new(64, 65, E::Trade, 0),
        Evolution::new(79, 199, E::TradeItem, 221),
        Evolution::new(95, 208, E::TradeItem, 233),
        Evolution::new(349, 350, E::HighBeauty, 170),
        Evolution::new(82, 462, E::MagneticField, 0),
        Evolution::new(133, 470, E::MossRock, 0),
        Evolution::new(133, 471, E::IceRock, 0),
        Evolution::new(190, 424, E::WithMove, 458),
        Evolution::new(221, 473, E::WithMove, 246),
        Evolution::new(223, 224, E::WithOther, 222),
    ];
    let mut numbers: Vec<DexNumber> = evolutions.iter().flat_map(|e| [e.from, e.to]).collect();
    numbers.sort_unstable();
    numbers.dedup();
    let mut g = emerald(
        numbers
            .into_iter()
            .map(|n| species(n, Type::Normal, None, 50))
            .collect(),
    );
    g.identity.generation = ppr_model::Generation::new(4).expect("Gen 4");
    g.identity.family = family.into();
    g.evolutions = evolutions;
    g.learnsets.insert(190, vec![ppr_model::LevelUpMove::new(32, 458)]);
    g.estimate_evolution_levels();
    g
}

#[test]
fn gen4_trade_evolutions_happen_by_level_or_held_item() {
    let g = run_on(gen4("Plat"), impossible(false));
    assert_eq!(method(&g, 64, 65), (E::Level, 37));
    assert_eq!(method(&g, 79, 199), (E::Stone, WATER_STONE));
    assert_eq!(method(&g, 95, 208), (E::Item, 233));
    // Platinum has contests and the places; movesets are unchanged.
    assert_eq!(method(&g, 349, 350), (E::HighBeauty, 170));
    assert_eq!(method(&g, 82, 462), (E::MagneticField, 0));
    assert_eq!(method(&g, 190, 424), (E::WithMove, 458));
}

#[test]
fn heart_gold_lacks_beauty_and_the_places() {
    let g = run_on(gen4("HGSS"), impossible(false));
    assert_eq!(method(&g, 349, 350), (E::Level, 35));
    assert_eq!(method(&g, 82, 462), (E::Level, 40));
    assert_eq!(method(&g, 133, 470), (E::Stone, LEAF_STONE));
    assert_eq!(method(&g, 133, 471), (E::Stone, DAWN_STONE));
}

#[test]
fn gen4_move_and_party_evolutions_follow_the_other_randomizers() {
    let mut s = impossible(false);
    let mut g = gen4("Plat");
    let others = Others {
        move_evolutions: true,
        party_evolutions: false,
    };
    improve(&mut g, &s, others);
    // Aipom learns Double Hit at 32; Piloswine learns Ancient Power from no level.
    assert_eq!(method(&g, 190, 424), (E::Level, 32));
    assert_eq!(method(&g, 221, 473), (E::Level, 45));
    assert_eq!(method(&g, 223, 224), (E::WithOther, 222));

    s.easier_max_level = Some(60);
    let mut g = gen4("Plat");
    let others = Others {
        move_evolutions: false,
        party_evolutions: true,
    };
    improve(&mut g, &s, others);
    assert_eq!(method(&g, 190, 424), (E::WithMove, 458));
    assert_eq!(method(&g, 223, 224), (E::Level, 35));
}

#[test]
fn gen_5_trade_evolutions_hold_their_items_and_pair_up() {
    let mut g = game();
    g.identity.generation = ppr_model::Generation::new(5).expect("generation 5");
    g.evolutions.push(Evolution::new(KARRABLAST, 589, E::TradeSpecial, 0));
    g.evolutions.push(Evolution::new(SHELMET, 617, E::TradeSpecial, 0));
    g.evolutions.push(Evolution::new(221, 473, E::WithMove, 246));
    g.learnsets
        .insert(221, vec![ppr_model::LevelUpMove { level: 1, move_id: 246 }]);
    g.estimate_evolution_levels();
    let r = run_on(g.clone(), impossible(false));
    assert_eq!(method(&r, 64, 65), (E::Level, 37));
    assert_eq!(method(&r, 61, 186), (E::Item, 221));
    assert_eq!(method(&r, 79, 199), (E::Stone, WATER_STONE));
    assert_eq!(method(&r, KARRABLAST, 589), (E::WithOther, SHELMET));
    assert_eq!(method(&r, SHELMET, 617), (E::WithOther, KARRABLAST));
    assert_eq!(method(&r, 221, 473), (E::WithMove, 246), "level-up moves don't change");

    let s = EvolutionSettings {
        easier_max_level: Some(60),
        ..impossible(false)
    };
    let others = Others {
        move_evolutions: true,
        party_evolutions: true,
    };
    improve(&mut g, &s, others);
    assert_eq!(method(&g, 221, 473), (E::Level, 45), "learnt at level 1");
    assert_eq!(method(&g, KARRABLAST, 589), (E::Level, 35));
}

fn gen7(name: &str, family: &str) -> GameData {
    let mut g = game();
    g.identity.generation = ppr_model::Generation::new(7).expect("generation 7");
    g.identity.name = name.into();
    g.identity.family = family.into();
    // Cosmoem, Rockruff, Inkay, Magneton, Crabrawler.
    g.evolutions.extend([
        Evolution::new(790, 791, E::LevelGameThis, 53),
        Evolution::new(790, 792, E::LevelGameOther, 53),
        Evolution::new(744, 745, E::LevelGameThisDay, 25),
        Evolution::new(744, 745, E::LevelGameOtherNight, 25),
        Evolution::new(686, 687, E::LevelUpsideDown, 30),
        Evolution::new(82, 462, E::MagneticField, 0),
        Evolution::new(739, 740, E::Snowy, 0),
    ]);
    g.estimate_evolution_levels();
    g
}

#[test]
fn gen7_game_specific_evolutions_follow_the_time_of_day() {
    let r = run_on(gen7("Ultra Sun", "USUM"), impossible(false));
    assert_eq!(method(&r, 64, 65), (E::Level, 37));
    assert_eq!(method(&r, 61, 186), (E::Item, 221));
    assert_eq!(method(&r, 790, 791), (E::LevelDay, 53));
    assert_eq!(method(&r, 790, 792), (E::LevelNight, 53));
    let rockruff: Vec<(E, u16)> = r
        .evolutions
        .iter()
        .filter(|e| e.from == 744)
        .map(|e| (e.kind, e.parameter))
        .collect();
    assert_eq!(rockruff, [(E::LevelDay, 25), (E::LevelNight, 25)]);

    let r = run_on(gen7("Moon", "SM"), impossible(false));
    assert_eq!(
        method(&r, 790, 791),
        (E::LevelNight, 53),
        "Moon's own evolution by night"
    );
    assert_eq!(method(&r, 790, 792), (E::LevelDay, 53));
}

#[test]
fn gen7_easier_evolutions_need_no_place_in_sun_and_moon() {
    let easier = EvolutionSettings {
        easier_max_level: Some(60),
        ..EvolutionSettings::default()
    };
    let r = run_on(gen7("Sun", "SM"), easier.clone());
    assert_eq!(method(&r, 686, 687), (E::Level, 30));
    let magneton: Vec<(E, u16)> = r
        .evolutions
        .iter()
        .filter(|e| e.from == 82)
        .map(|e| (e.kind, e.parameter))
        .collect();
    assert_eq!(magneton, [(E::MagneticField, 0), (E::Level, 35)]);
    assert!(r.evolutions.iter().any(|e| e.from == 739 && e.kind == E::Level));
    assert!(r.easier_happiness_evolutions);

    // Only Sun and Moon get them.
    let r = run_on(gen7("Ultra Moon", "USUM"), easier);
    assert_eq!(method(&r, 686, 687), (E::Level, 30));
    assert_eq!(r.evolutions.iter().filter(|e| e.from == 82).count(), 1);
}

#[test]
fn gen_6_changes_what_gen_5_does_and_turns_inkay_around() {
    const INKAY: DexNumber = 686;
    let mut g = game();
    g.identity.generation = ppr_model::Generation::new(6).expect("generation 6");
    g.evolutions.push(Evolution::new(KARRABLAST, 589, E::TradeSpecial, 0));
    g.evolutions.push(Evolution::new(SHELMET, 617, E::TradeSpecial, 0));
    g.evolutions.push(Evolution::new(INKAY, 687, E::LevelUpsideDown, 30));
    g.estimate_evolution_levels();
    let r = run_on(g.clone(), impossible(false));
    assert_eq!(method(&r, 64, 65), (E::Level, 37));
    assert_eq!(method(&r, 61, 186), (E::Item, 221), "trade items are held instead");
    assert_eq!(method(&r, 79, 199), (E::Stone, WATER_STONE));
    assert_eq!(method(&r, KARRABLAST, 589), (E::WithOther, SHELMET));
    assert_eq!(
        method(&r, INKAY, 687),
        (E::LevelUpsideDown, 30),
        "only easier evolutions turn Inkay"
    );

    let s = EvolutionSettings {
        easier_max_level: Some(60),
        ..impossible(false)
    };
    let others = Others {
        move_evolutions: false,
        party_evolutions: true,
    };
    improve(&mut g, &s, others);
    assert_eq!(method(&g, INKAY, 687), (E::Level, 30));
    assert_eq!(method(&g, KARRABLAST, 589), (E::Level, 35));
}
