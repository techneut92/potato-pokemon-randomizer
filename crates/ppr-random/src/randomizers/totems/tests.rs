use ppr_model::{TotemAlly, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::common::{SpeciesSet, bst};
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const CONSUMABLES: [ItemId; 3] = [150, 151, 158];

fn ally(species: DexNumber, level: u8) -> TotemAlly {
    TotemAlly {
        species,
        forme: 0,
        level,
        held_item: None,
    }
}

fn totem(species: DexNumber, level: u8, held_item: Option<ItemId>, aura: u8, allies: Vec<TotemAlly>) -> TotemPokemon {
    TotemPokemon {
        species,
        forme: 0,
        level,
        held_item,
        aura: Aura::from_byte(aura),
        allies,
    }
}

/// Species 1–200, each stat `n % 100 + 20`; four totems: one with no aura and no item, one whose aura raises every
/// stat.
fn game() -> GameData {
    let species = (1..=200)
        .map(|number: DexNumber| {
            let each = u8::try_from(number % 100 + 20).expect("small");
            testing::species(number, Type::Normal, None, each)
        })
        .collect();
    let mut game = emerald(species);
    game.totems = vec![
        totem(20, 12, Some(151), 4, vec![ally(19, 10)]),
        totem(46, 20, Some(158), 5, vec![ally(94, 18), ally(46, 18)]),
        totem(80, 50, None, 0, Vec::new()),
        totem(143, 60, Some(150), 20, vec![ally(79, 58)]),
    ];
    game.held_item_pools.consumable = CONSUMABLES.to_vec();
    game
}

fn run_on(game: GameData, settings: TotemSettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            totems: settings,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn species(game: &GameData) -> (Vec<DexNumber>, Vec<DexNumber>) {
    let totems = game.totems.iter().map(|t| t.species).collect();
    let allies = game.totems.iter().flat_map(|t| &t.allies).map(|a| a.species).collect();
    (totems, allies)
}

#[test]
fn random_totems_change_and_never_repeat_while_the_pool_lasts() {
    let settings = TotemSettings {
        mode: TotemMode::Random,
        ..TotemSettings::default()
    };
    let before = game();
    for seed in 0..20 {
        let after = run_on(game(), settings.clone(), seed);
        let (totems, allies) = species(&after);
        assert_eq!(allies, species(&before).1, "allies stay: seed {seed}");
        let mut unique = totems.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), totems.len(), "seed {seed}");
        // Nothing else changes.
        for (b, a) in before.totems.iter().zip(&after.totems) {
            assert_eq!((b.level, b.held_item, b.aura), (a.level, a.held_item, a.aura));
        }
    }
    assert_ne!(species(&run_on(game(), settings, 1)).0, species(&before).0);
}

#[test]
fn similar_strength_stays_close_and_changes_the_species() {
    let settings = TotemSettings {
        mode: TotemMode::SimilarStrength,
        ally_mode: TotemMode::SimilarStrength,
        ..TotemSettings::default()
    };
    let before = game();
    let (old_totems, old_allies) = species(&before);
    for seed in 0..30 {
        let after = run_on(game(), settings.clone(), seed);
        let (totems, allies) = species(&after);
        for (&old, &new) in old_totems.iter().chain(&old_allies).zip(totems.iter().chain(&allies)) {
            assert_ne!(old, new, "seed {seed}");
            let (a, b) = (bst(&before, old), bst(&before, new));
            assert!(a.abs_diff(b) * 10 <= a * 3, "seed {seed}: {a} -> {b}");
        }
    }
}

#[test]
fn allies_change_without_the_totems() {
    let settings = TotemSettings {
        ally_mode: TotemMode::Random,
        ..TotemSettings::default()
    };
    let before = game();
    let after = run_on(game(), settings, 3);
    let ((old_totems, old_allies), (totems, allies)) = (species(&before), species(&after));
    assert_eq!(totems, old_totems);
    assert_eq!(allies.len(), old_allies.len());
    assert_ne!(allies, old_allies);
    let mut unique = allies.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), allies.len());
}

#[test]
fn random_auras_raise_a_stat_or_all_by_one_to_three_stages() {
    let settings = TotemSettings {
        aura_mode: AuraMode::Random,
        ..TotemSettings::default()
    };
    let mut seen = BTreeSet::new();
    for seed in 0..40 {
        for t in run_on(game(), settings.clone(), seed).totems {
            assert_ne!(t.aura.stat, AuraStat::None);
            assert!((1..=3).contains(&t.aura.stages));
            seen.insert(t.aura.to_byte());
        }
    }
    assert_eq!(seen, (1..=18).collect(), "every random aura comes up");
}

#[test]
fn same_strength_auras_keep_their_stages() {
    let settings = TotemSettings {
        aura_mode: AuraMode::SameStrength,
        ..TotemSettings::default()
    };
    let before = game();
    let mut changed = false;
    for seed in 0..20 {
        let after = run_on(game(), settings.clone(), seed);
        for (b, a) in before.totems.iter().zip(&after.totems) {
            assert_eq!(a.aura.stages, b.aura.stages, "seed {seed}");
            match b.aura.stat {
                AuraStat::None | AuraStat::All => assert_eq!(a.aura, b.aura),
                _ => assert!(AuraStat::SINGLE.contains(&a.aura.stat)),
            }
            changed |= a.aura != b.aura;
        }
    }
    assert!(changed);
}

#[test]
fn only_totems_with_an_item_get_a_consumable_one() {
    let settings = TotemSettings {
        randomize_held_items: true,
        ..TotemSettings::default()
    };
    let mut seen = BTreeSet::new();
    for seed in 0..20 {
        let after = run_on(game(), settings.clone(), seed);
        assert_eq!(after.totems[2].held_item, None);
        for t in [&after.totems[0], &after.totems[1], &after.totems[3]] {
            let item = t.held_item.expect("still holds one");
            assert!(CONSUMABLES.contains(&item));
            seen.insert(item);
        }
        // Allies keep theirs.
        assert!(
            after
                .totems
                .iter()
                .flat_map(|t| &t.allies)
                .all(|a| a.held_item.is_none())
        );
    }
    assert_eq!(seen.len(), CONSUMABLES.len());
}

#[test]
fn held_items_need_consumables() {
    let mut no_items = game();
    no_items.held_item_pools.consumable.clear();
    let settings = TotemSettings {
        randomize_held_items: true,
        ..TotemSettings::default()
    };
    let mut rng = Seed(1).rng_for(step_ids::TOTEMS);
    assert!(randomize(&no_items, &settings, &SpeciesSet::new(), &mut rng).is_err());
}

#[test]
fn levels_change_for_totems_and_allies() {
    let settings = TotemSettings {
        level_modifier: 50,
        ..TotemSettings::default()
    };
    let after = run_on(game(), settings, 1);
    assert_eq!(species(&after), species(&game()), "levels only");
    let levels: Vec<u8> = after.totems.iter().map(|t| t.level).collect();
    assert_eq!(levels, [18, 30, 75, 90]);
    let allies: Vec<u8> = after.totems.iter().flat_map(|t| &t.allies).map(|a| a.level).collect();
    assert_eq!(allies, [15, 27, 27, 87]);
    let down = run_on(
        game(),
        TotemSettings {
            level_modifier: -100,
            ..TotemSettings::default()
        },
        1,
    );
    assert!(down.totems.iter().all(|t| t.level == 1));
}

#[test]
fn games_without_totems_or_changes_are_left_alone() {
    let mut empty = game();
    empty.totems.clear();
    let random = TotemSettings {
        mode: TotemMode::Random,
        ..TotemSettings::default()
    };
    let data = |game: GameData, totems: TotemSettings| RunData {
        game,
        settings: Settings {
            totems,
            ..Settings::default()
        },
    };
    assert!(!TotemRandomizer.applies(&data(empty, random.clone())));
    assert!(!TotemRandomizer.applies(&data(game(), TotemSettings::default())));
    assert!(TotemRandomizer.applies(&data(game(), random)));
}

#[test]
fn limited_species_are_respected() {
    let mut limited = game();
    limited.restricted_species = Some((1..=20).collect());
    let settings = TotemSettings {
        mode: TotemMode::Random,
        ally_mode: TotemMode::SimilarStrength,
        ..TotemSettings::default()
    };
    for seed in 0..10 {
        let after = run_on(limited.clone(), settings.clone(), seed);
        let (totems, allies) = species(&after);
        assert!(totems.iter().chain(&allies).all(|&n| n <= 20), "seed {seed}");
    }
}

#[test]
fn the_same_seed_gives_the_same_totems() {
    let settings = TotemSettings {
        mode: TotemMode::SimilarStrength,
        ally_mode: TotemMode::Random,
        aura_mode: AuraMode::Random,
        randomize_held_items: true,
        allow_alt_formes: true,
        level_modifier: 10,
    };
    let a = run_on(game(), settings.clone(), 42);
    let b = run_on(game(), settings.clone(), 42);
    assert_eq!(a.totems, b.totems);
    assert_ne!(a.totems, run_on(game(), settings, 43).totems);
}

/// [`game`] with alternate formes: 301 of species 20 (forme 1), the cosmetic 302 of species 30 (forme 1) and the
/// irregular 303 of species 40 (forme 2); species 25 has the cosmetic formes 1 and 2 without data.
fn game_with_formes() -> GameData {
    let mut game = game();
    game.species.push(testing::alt_forme(301, 20, 1, 60, false, false));
    game.species.push(testing::alt_forme(302, 30, 1, 60, true, false));
    game.species.push(testing::alt_forme(303, 40, 2, 60, false, true));
    if let Some(sp) = game.species.iter_mut().find(|sp| sp.number == 25) {
        sp.forme.cosmetic_formes = vec![1, 2];
    }
    game
}

fn run_with(game: GameData, totems: TotemSettings, no_irregular: bool, seed: u64) -> GameData {
    let mut settings = Settings {
        totems,
        ..Settings::default()
    };
    settings.general.no_irregular_alt_formes = no_irregular;
    let mut data = RunData { game, settings };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

/// Every species and forme the totems and allies got over `seeds`.
fn chosen(game: &GameData, totems: &TotemSettings, no_irregular: bool, seeds: u64) -> Vec<(DexNumber, u8)> {
    (0..seeds)
        .flat_map(|seed| {
            let after = run_with(game.clone(), totems.clone(), no_irregular, seed);
            after
                .totems
                .iter()
                .flat_map(|t| {
                    std::iter::once((t.species, t.forme)).chain(t.allies.iter().map(|a| (a.species, a.forme)))
                })
                .collect::<Vec<_>>()
        })
        .collect()
}

#[test]
fn alternate_formes_join_the_pool_only_when_allowed() {
    let random = TotemSettings {
        mode: TotemMode::Random,
        ally_mode: TotemMode::Random,
        ..TotemSettings::default()
    };
    let without = chosen(&game_with_formes(), &random, false, 60);
    assert!(without.iter().all(|&(n, _)| n <= 200), "base species only");

    let allowed = TotemSettings {
        allow_alt_formes: true,
        ..random
    };
    let with = chosen(&game_with_formes(), &allowed, false, 60);
    assert!(with.contains(&(301, 1)), "an alternate forme, with its forme number");
    assert!(with.contains(&(303, 2)), "irregular formes are allowed");
    assert!(
        !with.iter().any(|&(n, _)| n == 302),
        "cosmetic formes are never chosen as themselves"
    );
    let banned = chosen(&game_with_formes(), &allowed, true, 60);
    assert!(!banned.iter().any(|&(n, _)| n == 303), "No Irregular Alt Formes");
}

#[test]
fn a_new_base_species_gets_a_random_cosmetic_forme() {
    let random = TotemSettings {
        mode: TotemMode::Random,
        ally_mode: TotemMode::Random,
        ..TotemSettings::default()
    };
    let formes: BTreeSet<u8> = chosen(&game_with_formes(), &random, false, 200)
        .into_iter()
        .filter(|&(n, _)| n == 25)
        .map(|(_, f)| f)
        .collect();
    assert_eq!(formes, BTreeSet::from([0, 1, 2]));
    let others = chosen(&game_with_formes(), &random, false, 20);
    assert!(others.iter().filter(|&&(n, _)| n != 25).all(|&(_, f)| f == 0));
}

#[test]
fn unchanged_totems_keep_their_formes() {
    let mut game = game_with_formes();
    game.totems[0].species = 301;
    game.totems[0].forme = 1;
    let allies = TotemSettings {
        ally_mode: TotemMode::Random,
        ..TotemSettings::default()
    };
    let after = run_with(game, allies, false, 1);
    assert_eq!((after.totems[0].species, after.totems[0].forme), (301, 1));
}
