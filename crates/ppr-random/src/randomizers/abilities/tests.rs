use ppr_model::{Evolution, EvolutionType, Type};
use ppr_settings::Settings;

use super::*;
use crate::randomizers::testing::{self, emerald};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

/// Species 1–60 with abilities 1 and 2; 1 → 2 → 3, and Shedinja (292) with Wonder Guard. Abilities 0–77 as in
/// Gen 3.
fn game() -> GameData {
    let mut numbers: Vec<DexNumber> = (1..=60).collect();
    numbers.push(292);
    let species = numbers
        .into_iter()
        .map(|n| {
            let mut sp = testing::species(n, Type::Normal, None, 50);
            sp.abilities = if n == 292 { vec![WONDER_GUARD, 0] } else { vec![1, 2] };
            sp
        })
        .collect();
    let mut g = emerald(species);
    g.evolutions = [(1, 2), (2, 3)]
        .into_iter()
        .map(|(from, to)| Evolution::new(from, to, EvolutionType::Level, 16))
        .collect();
    g.ability_names = (0..=77).map(|i| format!("AB{i}")).collect();
    g.ability_variations = [(15, vec![15, 72]), (13, vec![13, 77])].into_iter().collect();
    g.useless_abilities = vec![59, 76];
    g
}

fn run_on(game: GameData, abilities: AbilitySettings, seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            abilities,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn random() -> AbilitySettings {
    AbilitySettings {
        mode: AbilityMode::Random,
        ..AbilitySettings::default()
    }
}

fn of(g: &GameData, n: DexNumber) -> Vec<u16> {
    g.species(n).expect("there").abilities.clone()
}

#[test]
fn unchanged_or_without_abilities_changes_nothing() {
    assert_eq!(run_on(game(), AbilitySettings::default(), 1), game());
    let mut none = game();
    none.ability_names.clear();
    assert_eq!(run_on(none.clone(), random(), 1), none);
}

#[test]
fn random_abilities_respect_the_bans() {
    let s = AbilitySettings {
        ban_trapping: true,
        ban_negative: true,
        ban_bad: true,
        ..random()
    };
    for seed in 0..10 {
        let g = run_on(game(), s.clone(), seed);
        assert_eq!(of(&g, 292), [WONDER_GUARD, 0], "Wonder Guard species keep theirs");
        for sp in g.species.iter().filter(|sp| sp.number != 292) {
            let [first, second] = <[u16; 2]>::try_from(sp.abilities.clone()).expect("two slots");
            assert!((1..=77).contains(&first));
            assert_ne!(first, second);
            for a in [first, second].into_iter().filter(|&a| a != 0) {
                assert!(
                    ![WONDER_GUARD, 59, 76, 23, 42, 71, 54, 57, 58].contains(&a),
                    "{seed}: {} got {a}",
                    sp.name
                );
            }
        }
        let singles = g.species.iter().filter(|sp| sp.abilities[1] == 0).count();
        assert!((10..50).contains(&singles), "about half have one ability: {singles}");
    }
}

#[test]
fn options_shape_the_result() {
    let s = AbilitySettings {
        follow_evolutions: true,
        ensure_two: true,
        allow_wonder_guard: true,
        combine_duplicates: true,
        ..random()
    };
    let mut wonder_guard = false;
    for seed in 0..30 {
        let g = run_on(game(), s.clone(), seed);
        assert_eq!(of(&g, 1), of(&g, 2));
        assert_eq!(of(&g, 2), of(&g, 3));
        for sp in g.species.iter().filter(|sp| sp.number != 292) {
            assert!(sp.abilities[1] != 0, "{} has two", sp.name);
            // Twins are only reached through their base ability; Air Lock counts as Cloud Nine's.
            assert!(![73, 74, 75, 76].contains(&sp.abilities[0]), "{}", sp.name);
        }
        wonder_guard |= g
            .species
            .iter()
            .any(|sp| sp.number != 292 && sp.abilities.contains(&WONDER_GUARD));
    }
    assert!(wonder_guard, "allowed Wonder Guard is given out");
}

#[test]
fn combined_twins_are_picked_through_their_base() {
    let g = game();
    let s = AbilitySettings {
        combine_duplicates: true,
        ..random()
    };
    let allowed = allowed(&g, &s);
    assert!(!allowed.contains(&72) && !allowed.contains(&77) && allowed.contains(&15));
    let mut rng = Seed(1).rng_for(0);
    let mut seen = BTreeSet::new();
    for _ in 0..2000 {
        seen.insert(pick(&mut rng, &g, &allowed, true, &[]).expect("some"));
    }
    assert!(seen.contains(&72) && seen.contains(&77), "twins come up");
    assert_eq!(pick(&mut rng, &g, &[15], true, &[15]).ok(), None);
    assert_eq!(pick(&mut rng, &g, &[15], true, &[72]).ok(), Some(15));
}

/// [`game`] with Mega Evolutions: 900 of species 3, and 901 and 902 of species 40.
fn mega_game() -> GameData {
    let mut g = game();
    for (n, base, number) in [(900, 3, 1), (901, 40, 1), (902, 40, 2)] {
        let mut sp = testing::species(n, Type::Normal, None, 50);
        sp.abilities = vec![1, 2];
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

#[test]
fn mega_evolutions_keep_their_base_species_abilities_when_they_follow() {
    let follow = AbilitySettings {
        follow_evolutions: true,
        follow_megas: true,
        ..random()
    };
    let (mut own, mut formes_own) = (false, 0);
    for seed in 0..20 {
        let g = run_on(mega_game(), follow.clone(), seed);
        assert_eq!(of(&g, 900), of(&g, 3), "{seed}");
        // Two Mega Evolutions of one species split, so they get abilities of their own.
        own |= of(&g, 901) != of(&g, 40) || of(&g, 902) != of(&g, 40);
        // Without following Mega Evolutions, or without following evolutions, they get their own as alternate formes.
        for s in [
            AbilitySettings {
                follow_megas: false,
                ..follow.clone()
            },
            AbilitySettings {
                follow_evolutions: false,
                ..follow.clone()
            },
        ] {
            let g = run_on(mega_game(), s, seed);
            formes_own += usize::from(of(&g, 900) != of(&g, 3));
        }
    }
    assert!(own);
    assert!(formes_own > 30, "{formes_own}");
}
