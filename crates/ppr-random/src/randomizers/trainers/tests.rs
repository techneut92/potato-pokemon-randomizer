use ppr_model::{Evolution, EvolutionType, Trainer, TrainerPokemon, Type};
use ppr_settings::{Settings, TrainerMode, TrainerSettings};

use crate::randomizers::testing::{emerald, species};
use crate::{CancelToken, Progress, RunData, Seed, standard_pipeline};

/// Bulbasaur (#1) → Ivysaur at 16 → Venusaur at 32; a rival's first battle and a regular trainer.
fn run(trainers: TrainerSettings) -> Vec<Trainer> {
    let mut g = emerald((1..=3).map(|n| species(n, Type::Grass, None, 50)).collect());
    g.evolutions = vec![
        Evolution::new(1, 2, EvolutionType::Level, 16),
        Evolution::new(2, 3, EvolutionType::Level, 32),
    ];
    g.trainers = vec![
        Trainer::new(1, "RIVAL", 0, vec![TrainerPokemon::new(1, 20)]),
        Trainer::new(2, "T", 0, vec![TrainerPokemon::new(1, 20), TrainerPokemon::new(1, 40)]),
    ];
    g.trainers[0].tag = Some("RIVAL1-0".into());
    let mut data = RunData {
        game: g,
        settings: Settings {
            trainers,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game.trainers
}

fn species_levels(trainers: &[Trainer]) -> Vec<Vec<(u16, u8)>> {
    trainers
        .iter()
        .map(|t| t.pokemon.iter().map(|p| (p.species, p.level)).collect())
        .collect()
}

#[test]
fn levels_change_but_the_first_rival_battle_only_gets_easier() {
    let up = run(TrainerSettings {
        level_modifier: Some(50),
        ..TrainerSettings::default()
    });
    assert_eq!(species_levels(&up), [vec![(1, 20)], vec![(1, 30), (1, 60)]]);
    let down = run(TrainerSettings {
        level_modifier: Some(-50),
        ..TrainerSettings::default()
    });
    assert_eq!(species_levels(&down), [vec![(1, 10)], vec![(1, 10), (1, 20)]]);
}

#[test]
fn pokemon_evolve_as_far_as_their_level_allows() {
    let plain = run(TrainerSettings {
        evolve: Some(0),
        ..TrainerSettings::default()
    });
    assert_eq!(species_levels(&plain), [vec![(2, 20)], vec![(2, 20), (3, 40)]]);
    // Evolution levels 30% higher: 20.8 and 41.6.
    let later = run(TrainerSettings {
        evolve: Some(30),
        ..TrainerSettings::default()
    });
    assert_eq!(species_levels(&later), [vec![(1, 20)], vec![(1, 20), (2, 40)]]);
}

/// 60 species in turn Grass, Fire, Water, Bug/Flying and Psychic; #1 → #2 → #3 at 16 and 32; #60 is Mewtwo's
/// number stand-in: legendary #150 is added too. Trainers: a gym (`GYM1`, two), an Elite Four member (index 5), a
/// rival battle per variant and a regular trainer.
fn league() -> ppr_model::GameData {
    let types = [Type::Grass, Type::Fire, Type::Water, Type::Bug, Type::Psychic];
    let mut species: Vec<_> = (1..=60)
        .map(|n| {
            let t = types[usize::from(n) % 5];
            let second = (t == Type::Bug).then_some(Type::Flying);
            species(n, t, second, u8::try_from(n % 50 + 20).expect("fits"))
        })
        .collect();
    species.push(species_of(150));
    let mut g = emerald(species);
    g.evolutions = vec![
        Evolution::new(1, 2, EvolutionType::Level, 16),
        Evolution::new(2, 3, EvolutionType::Level, 32),
    ];
    g.starters = vec![1, 4, 7];
    let team = |levels: &[u8]| levels.iter().map(|&l| TrainerPokemon::new(10, l)).collect::<Vec<_>>();
    let tagged = |index: u16, tag: &str, levels: &[u8]| {
        let mut t = Trainer::new(index, "T", 0, team(levels));
        t.tag = Some(tag.into());
        t
    };
    g.trainers = vec![
        tagged(1, "GYM1", &[10, 12]),
        tagged(2, "GYM1-LEADER", &[14, 15, 16]),
        tagged(3, "RIVAL1-0", &[5]),
        tagged(4, "RIVAL2-1", &[20, 22, 25]),
        tagged(5, "ELITE1", &[50, 52, 55, 60]),
        Trainer::new(6, "T", 0, team(&[30, 30, 30, 30, 30, 30])),
    ];
    g.elite_four_trainers = vec![5];
    g.gym_and_elite_themes = [("GYM1".to_owned(), Type::Fire)].into();
    g
}

fn species_of(n: u16) -> ppr_model::Species {
    species(n, Type::Psychic, None, 100)
}

fn randomized(mut g: ppr_model::GameData, trainers: TrainerSettings, seed: u64) -> Vec<Trainer> {
    g.species.sort_by_key(|s| s.number);
    let mut data = RunData {
        game: g,
        settings: Settings {
            trainers,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game.trainers
}

fn all_types(game: &ppr_model::GameData, t: &Trainer) -> Vec<Vec<Type>> {
    t.pokemon
        .iter()
        .map(|p| {
            let sp = game.species(p.species).expect("species");
            std::iter::once(sp.primary_type).chain(sp.secondary_type).collect()
        })
        .collect()
}

#[test]
fn random_trainers_get_new_species_with_reset_moves() {
    for seed in 0..5 {
        let trainers = randomized(
            league(),
            TrainerSettings {
                mode: TrainerMode::Random,
                no_legendaries: true,
                ..TrainerSettings::default()
            },
            seed,
        );
        let all: Vec<_> = trainers.iter().flat_map(|t| &t.pokemon).collect();
        assert!(all.iter().all(|p| p.reset_moves && p.species != 150));
        assert!(all.iter().filter(|p| p.species != 10).count() > 10);
    }
}

#[test]
fn type_themes_are_shared_by_groups_and_kept_from_the_game() {
    let g = league();
    let themed = randomized(
        g.clone(),
        TrainerSettings {
            mode: TrainerMode::TypeThemed,
            ..TrainerSettings::default()
        },
        3,
    );
    let shared = |t: &Trainer| {
        let types = all_types(&g, t);
        types[0]
            .iter()
            .copied()
            .find(|ty| types.iter().all(|ts| ts.contains(ty)))
    };
    let gym = shared(&themed[0]).expect("the gym trainer has a theme");
    assert_eq!(shared(&themed[1]), Some(gym), "the leader shares the gym's type");
    for t in &themed {
        assert!(shared(t).is_some(), "trainer {} has a theme", t.index);
    }
    // Keep: the gym is Fire as in the game; the regular trainer's Grass team (all #10) stays Grass.
    let kept = randomized(
        g.clone(),
        TrainerSettings {
            mode: TrainerMode::KeepThemed,
            ..TrainerSettings::default()
        },
        4,
    );
    for (t, theme) in [(&kept[0], Type::Fire), (&kept[1], Type::Fire), (&kept[5], Type::Grass)] {
        assert!(
            all_types(&g, t).iter().all(|ts| ts.contains(&theme)),
            "trainer {}",
            t.index
        );
    }
}

#[test]
fn duplicates_are_avoided_and_types_diverse() {
    let g = league();
    let trainers = randomized(
        g.clone(),
        TrainerSettings {
            mode: TrainerMode::Random,
            avoid_duplicates: true,
            diverse_types: [false, false, true],
            ..TrainerSettings::default()
        },
        5,
    );
    let regular = &trainers[5];
    let species: std::collections::BTreeSet<_> = regular.pokemon.iter().map(|p| p.species).collect();
    assert_eq!(species.len(), 6);
    // Five types plus Flying: the sixth Pokémon may repeat one only when none is left.
    let types = all_types(&g, regular);
    let firsts: std::collections::BTreeSet<_> = types.iter().take(5).flatten().collect();
    assert_eq!(firsts.len(), types.iter().take(5).map(Vec::len).sum::<usize>());
}

#[test]
fn rivals_carry_the_starter_strong_against_the_players() {
    let trainers = randomized(
        league(),
        TrainerSettings {
            mode: TrainerMode::Random,
            rival_carries_starter: true,
            ..TrainerSettings::default()
        },
        6,
    );
    // Variant 0 gets starter 1 (#4) at level 5; variant 1 starter 2 (#7) as its ace, the last one.
    assert_eq!(trainers[2].pokemon[0].species, 4);
    assert_eq!(trainers[3].pokemon[2].species, 7);
}

#[test]
fn added_pokemon_sit_before_the_ace_at_lower_levels() {
    let trainers = randomized(
        league(),
        TrainerSettings {
            additional_pokemon: [2, 0, 3],
            ..TrainerSettings::default()
        },
        7,
    );
    let leader = &trainers[1];
    assert_eq!(leader.pokemon.len(), 5);
    assert_eq!(leader.pokemon.last().map(|p| p.level), Some(16));
    assert!(leader.pokemon[..4].iter().all(|p| (14..=15).contains(&p.level)));
    // Unchanged keeps the original species and the theme: all #10's type, Fire.
    let gym = &trainers[0];
    assert_eq!(gym.pokemon.len(), 5);
    assert!(gym.pokemon.iter().filter(|p| !p.added).all(|p| p.species == 10));
    assert_eq!(gym.pokemon.iter().filter(|p| p.added).count(), 3);
    // A full team gets nothing.
    assert_eq!(trainers[5].pokemon.len(), 6);
}

#[test]
fn the_elite_fours_strongest_pokemon_are_unique() {
    for seed in 0..5 {
        let trainers = randomized(
            league(),
            TrainerSettings {
                mode: TrainerMode::Random,
                elite_four_unique: Some(2),
                ..TrainerSettings::default()
            },
            seed,
        );
        let elite = &trainers[4];
        let unique = [elite.pokemon[3].species, elite.pokemon[2].species];
        for t in trainers.iter().filter(|t| t.index != 5) {
            assert!(t.pokemon.iter().all(|p| !unique.contains(&p.species)), "seed {seed}");
        }
    }
}

#[test]
fn no_premature_evolutions_for_trainers() {
    let mut g = emerald((1..=3).map(|n| species(n, Type::Grass, None, 50)).collect());
    g.evolutions = vec![
        Evolution::new(1, 2, EvolutionType::Level, 16),
        Evolution::new(2, 3, EvolutionType::Level, 32),
    ];
    g.trainers = (1..=20)
        .map(|i| Trainer::new(i, "T", 0, vec![TrainerPokemon::new(1, 12), TrainerPokemon::new(1, 20)]))
        .collect();
    let mut settings = Settings {
        trainers: TrainerSettings {
            mode: TrainerMode::Random,
            ..TrainerSettings::default()
        },
        ..Settings::default()
    };
    settings.general.no_premature_evolutions = true;
    let mut data = RunData { game: g, settings };
    standard_pipeline()
        .run(&mut data, Seed(3), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    for t in &data.game.trainers {
        assert_eq!(t.pokemon[0].species, 1, "only Bulbasaur at level 12");
        assert_ne!(t.pokemon[1].species, 3, "no Venusaur at level 20");
    }
    assert!(data.game.trainers.iter().any(|t| t.pokemon[1].species == 2));
}

#[test]
fn a_fixed_starter_position_is_kept() {
    let mut g = league();
    // The rival's ace would be the last one; the game fixes the starter first.
    g.trainers[3].force_starter_position = Some(0);
    let trainers = randomized(
        g,
        TrainerSettings {
            mode: TrainerMode::Random,
            rival_carries_starter: true,
            ..TrainerSettings::default()
        },
        6,
    );
    // Variant 1 gets starter 2 (#7), evolved for level 20 (no evolutions of #7 here).
    assert_eq!(trainers[3].pokemon[0].species, 7);
}

#[test]
fn a_fixed_last_starter_stays_last_when_pokemon_are_added() {
    let mut g = league();
    g.trainers[3].force_starter_position = Some(2);
    g.trainers[2].force_starter_position = Some(0);
    let trainers = randomized(
        g,
        TrainerSettings {
            additional_pokemon: [0, 2, 2],
            ..TrainerSettings::default()
        },
        7,
    );
    assert_eq!(trainers[3].pokemon.len(), 5);
    assert_eq!(trainers[3].force_starter_position, Some(4));
    assert_eq!(trainers[2].force_starter_position, Some(0), "only a last place moves");
}

/// Yellow: Pikachu and Eevee (#133), which branches into #134–#136 at 30; rival battles 1 and 3 before the
/// branch, battle 5 in its three variants after it.
fn yellow() -> ppr_model::GameData {
    let mut all: Vec<_> = (1..=30).map(|n| species(n, Type::Normal, None, 50)).collect();
    all.extend((133..=136).map(|n| species(n, Type::Normal, None, 60)));
    let mut g = crate::randomizers::testing::gen1(all, true);
    g.evolutions = (134..=136)
        .map(|to| Evolution::new(133, to, EvolutionType::Level, 30))
        .collect();
    let rival = |index: u16, tag: &str, levels: &[u8]| {
        let mut t = Trainer::new(
            index,
            "RIVAL",
            0,
            levels.iter().map(|&l| TrainerPokemon::new(1, l)).collect(),
        );
        t.tag = Some(tag.into());
        t
    };
    g.trainers = vec![
        rival(1, "RIVAL1-0", &[5]),
        rival(2, "RIVAL3-0", &[18, 20]),
        rival(3, "RIVAL5-0", &[38, 40]),
        rival(4, "RIVAL5-1", &[38, 40]),
        rival(5, "RIVAL5-2", &[38, 40]),
    ];
    g
}

#[test]
fn yellows_rival_carries_eevee_branching_per_variant() {
    let mut branches = std::collections::BTreeSet::new();
    for seed in 0..10 {
        let trainers = randomized(
            yellow(),
            TrainerSettings {
                mode: TrainerMode::Random,
                rival_carries_starter: true,
                ..TrainerSettings::default()
            },
            seed,
        );
        assert_eq!(trainers[0].pokemon[0].species, 133);
        assert_eq!(trainers[1].pokemon[1].species, 133, "the ace, before the branch");
        let late: Vec<_> = trainers[2..].iter().map(|t| t.pokemon[1].species).collect();
        assert!(late.iter().all(|s| (134..=136).contains(s)), "{late:?}");
        branches.insert(late);
    }
    assert!(
        branches.iter().any(|l| l[0] != l[1] || l[1] != l[2]),
        "variants branch on their own"
    );
}

#[test]
fn trainers_the_game_has_no_room_for_get_no_pokemon() {
    let mut g = league();
    g.trainer_pokemon_additions = [true, true, false];
    let trainers = randomized(
        g,
        TrainerSettings {
            additional_pokemon: [2, 0, 3],
            ..TrainerSettings::default()
        },
        7,
    );
    assert_eq!(trainers[1].pokemon.len(), 5, "the leader, a boss");
    assert_eq!(trainers[0].pokemon.len(), 2, "the regular gym trainer");
}

/// 400 trainers of six Pokémon each, chosen at random with `trainers`, and run with `seed`.
fn run_many(trainers: TrainerSettings, seed: u64) -> Vec<Trainer> {
    let mut g = emerald((1..=20).map(|n| species(n, Type::Grass, None, 50)).collect());
    g.trainers = (1..=400)
        .map(|i| Trainer::new(i, "T", 0, (0..6).map(|_| TrainerPokemon::new(1, 30)).collect()))
        .collect();
    let mut data = RunData {
        game: g,
        settings: Settings {
            trainers,
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game.trainers
}

fn shiny_flags(trainers: &[Trainer]) -> Vec<bool> {
    trainers
        .iter()
        .flat_map(|t| t.pokemon.iter().map(|p| p.shiny))
        .collect()
}

#[test]
fn random_shiny_trainer_pokemon_are_one_in_256_and_follow_the_seed() {
    let shiny = TrainerSettings {
        mode: TrainerMode::Random,
        random_shiny: true,
        ..TrainerSettings::default()
    };
    let first = shiny_flags(&run_many(shiny.clone(), 7));
    assert_eq!(first, shiny_flags(&run_many(shiny.clone(), 7)));
    assert_ne!(first, shiny_flags(&run_many(shiny.clone(), 8)));
    // 2400 Pokémon at 1 in 256: about 9.4 per run, 94 over ten seeds.
    let total: usize = (0..10)
        .map(|seed| {
            shiny_flags(&run_many(shiny.clone(), seed))
                .iter()
                .filter(|&&s| s)
                .count()
        })
        .sum();
    assert!((60..=130).contains(&total), "{total} shiny of 24000");
}

#[test]
fn trainer_pokemon_stay_plain_without_the_option_or_new_species() {
    let off = run_many(
        TrainerSettings {
            mode: TrainerMode::Random,
            ..TrainerSettings::default()
        },
        7,
    );
    assert!(!shiny_flags(&off).contains(&true));
    // Like the original, the chance comes with choosing species.
    let unchanged = run_many(
        TrainerSettings {
            random_shiny: true,
            evolve: Some(0),
            ..TrainerSettings::default()
        },
        7,
    );
    assert!(!shiny_flags(&unchanged).contains(&true));
}

/// 60 species in turn Grass, Fire, Water, Bug and Psychic, with Mega Evolutions for #50 (Grass, stone 700), #41
/// (Fire, stones 701 and 702) and #30 (Grass, no stone needed); one regular trainer whose #50 holds its stone, #10
/// holds #50's stone and #20 holds nothing.
fn mega_game() -> ppr_model::GameData {
    let types = [Type::Grass, Type::Fire, Type::Water, Type::Bug, Type::Psychic];
    let mut all: Vec<_> = (1..=60)
        .map(|n| species(n, types[usize::from(n) % 5], None, 60))
        .collect();
    for (number, base, forme, stone) in [
        (201, 50, 1, Some(700)),
        (202, 41, 1, Some(701)),
        (203, 41, 2, Some(702)),
        (204, 30, 1, None),
    ] {
        let mut sp = species(number, types[usize::from(base) % 5], None, 80);
        sp.forme.base = Some(base);
        sp.forme.number = forme;
        sp.forme.mega = true;
        sp.forme.mega_item = stone;
        sp.forme.irregular = true;
        all.push(sp);
    }
    let mut g = emerald(all);
    let holding = |n, item| TrainerPokemon {
        held_item: item,
        ..TrainerPokemon::new(n, 30)
    };
    g.trainers = vec![Trainer::new(
        1,
        "T",
        0,
        vec![holding(50, Some(700)), holding(10, Some(700)), holding(20, None)],
    )];
    g
}

/// The Mega Stones of `n` in [`mega_game`].
fn stones_of(n: u16) -> Vec<u16> {
    match n {
        50 => vec![700],
        41 => vec![701, 702],
        _ => Vec::new(),
    }
}

#[test]
fn pokemon_holding_their_mega_stone_swap_to_species_that_can_mega_evolve() {
    let swap = TrainerSettings {
        mode: TrainerMode::Random,
        swap_mega_evos: true,
        ..TrainerSettings::default()
    };
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..30 {
        let t = &randomized(mega_game(), swap.clone(), seed)[0];
        let mega = &t.pokemon[0];
        let stone = mega.held_item.expect("a Mega Stone");
        assert!(stones_of(mega.species).contains(&stone), "{mega:?}");
        seen.insert(stone);
        // A Mega Stone of another species is no reason to swap.
        assert_eq!(t.pokemon[1].held_item, Some(700));
        assert_eq!(t.pokemon[2].held_item, None);
    }
    assert_eq!(
        seen,
        [700, 701, 702].into(),
        "every stone of every species that can Mega Evolve"
    );
    // Without the option the stone stays with whatever replaces the species.
    for seed in 0..10 {
        let plain = TrainerSettings {
            swap_mega_evos: false,
            ..swap.clone()
        };
        let t = &randomized(mega_game(), plain, seed)[0];
        assert_eq!(t.pokemon[0].held_item, Some(700));
    }
}

#[test]
fn swapped_mega_evolvables_keep_their_theme_when_they_can() {
    let themed = TrainerSettings {
        mode: TrainerMode::TypeThemed,
        swap_mega_evos: true,
        ..TrainerSettings::default()
    };
    let g = mega_game();
    for seed in 0..40 {
        let t = &randomized(mega_game(), themed.clone(), seed)[0];
        let theme = all_types(&g, t)[2][0];
        let mega = &t.pokemon[0];
        let sp = g.species(mega.species).expect("species");
        assert!(sp.has_type(theme), "{mega:?} has the theme {theme:?}");
        match theme {
            Type::Grass => assert_eq!((mega.species, mega.held_item), (50, Some(700))),
            Type::Fire => assert!(mega.species == 41 && stones_of(41).contains(&mega.held_item.expect("stone"))),
            // No species of the theme can Mega Evolve, so the replacement holds nothing.
            _ => assert_eq!(mega.held_item, None, "{mega:?}"),
        }
    }
}

#[test]
fn mega_stone_holders_keep_their_stone_and_added_copies_get_no_item() {
    let mut g = mega_game();
    g.trainers[0].pokemon.truncate(1);
    g.held_item_pools.all = vec![1, 2, 3];
    let settings = TrainerSettings {
        mode: TrainerMode::Random,
        swap_mega_evos: true,
        additional_pokemon: [0, 0, 2],
        held_items: [true; 3],
        ..TrainerSettings::default()
    };
    for seed in 0..10 {
        let t = &randomized(g.clone(), settings.clone(), seed)[0];
        let items: Vec<_> = t.pokemon.iter().map(|p| p.held_item).collect();
        assert_eq!(items[..2], [None, None], "added copies of the stone holder");
        let ace = &t.pokemon[2];
        assert!(
            stones_of(ace.species).contains(&ace.held_item.expect("stone")),
            "{ace:?}"
        );
        // Without swapping, everyone gets a random item.
        let plain = TrainerSettings {
            swap_mega_evos: false,
            ..settings.clone()
        };
        let t = &randomized(g.clone(), plain, seed)[0];
        assert!(
            t.pokemon
                .iter()
                .all(|p| p.held_item.is_some_and(|i| (1..=3).contains(&i))),
            "{t:?}"
        );
    }
}
