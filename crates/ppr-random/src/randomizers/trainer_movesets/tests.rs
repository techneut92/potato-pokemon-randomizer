use ppr_model::{LevelUpMove, Move, MoveCategory, Trainer, TrainerPokemon, Type};
use ppr_settings::{Settings, TrainerSettings};

use super::pool::{distinct, obsolete_moves};
use super::*;
use crate::randomizers::testing::{emerald, species};
use crate::{CancelToken, Progress, Seed, standard_pipeline};

const TACKLE: MoveId = 33;
const BODY_SLAM: MoveId = 34;
const EMBER: MoveId = 52;
const FLAMETHROWER: MoveId = 53;

/// 354 moves: Normal and Fire attacks of rising power at odd numbers, harmless status moves at even ones.
fn game() -> GameData {
    let mut g = emerald(vec![
        species(1, Type::Fire, None, 60),
        species(2, Type::Fire, None, 80),
        species(3, Type::Normal, None, 50),
    ]);
    g.moves = (1..=354)
        .map(|n| {
            if n % 2 == 1 {
                let (t, c) = if n % 4 == 1 {
                    (Type::Normal, MoveCategory::Physical)
                } else {
                    (Type::Fire, MoveCategory::Special)
                };
                Move::basic(n, "D", 30 + n % 90, t, c)
            } else {
                Move::basic(n, "S", 0, Type::Normal, MoveCategory::Status)
            }
        })
        .collect();
    g.moves[usize::from(TACKLE) - 1] = Move::basic(TACKLE, "TACKLE", 35, Type::Normal, MoveCategory::Physical);
    g.moves[usize::from(BODY_SLAM) - 1] = Move::basic(BODY_SLAM, "BODY SLAM", 85, Type::Normal, MoveCategory::Physical);
    g.moves[usize::from(EMBER) - 1] = Move::basic(EMBER, "EMBER", 40, Type::Fire, MoveCategory::Special);
    g.moves[usize::from(FLAMETHROWER) - 1] =
        Move::basic(FLAMETHROWER, "FLAMETHROWER", 95, Type::Fire, MoveCategory::Special);
    for s in &mut g.species {
        s.abilities = vec![66, 0];
    }
    let many: Vec<LevelUpMove> = (0..30).map(|i| LevelUpMove::new(1 + i, 101 + u16::from(i))).collect();
    g.learnsets.insert(1, many.clone());
    g.learnsets.insert(2, many);
    g.learnsets
        .insert(3, vec![LevelUpMove::new(1, TACKLE), LevelUpMove::new(7, 2)]);
    g.evolutions.push(ppr_model::Evolution {
        from: 1,
        to: 2,
        kind: ppr_model::EvolutionType::Level,
        parameter: 16,
        estimated_level: 16,
    });
    let team = || vec![TrainerPokemon::new(2, 40), TrainerPokemon::new(3, 40)];
    g.trainers = vec![
        Trainer::new(1, "Leader", 0, team()),
        Trainer::new(2, "Youngster", 0, team()),
        Trainer::new(3, "First rival", 0, team()),
    ];
    g.trainers[0].tag = Some("GYM1-LEADER".into());
    g.trainers[2].tag = Some("RIVAL1-0".into());
    for t in &mut g.trainers {
        for tp in &mut t.pokemon {
            tp.reset_moves = true;
        }
    }
    g
}

fn run(game: GameData, better: [bool; 3], seed: u64) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            trainers: TrainerSettings {
                better_movesets: better,
                ..TrainerSettings::default()
            },
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn chosen_tiers_get_four_different_moves_they_could_know() {
    for seed in 0..20 {
        let g = run(game(), [false, false, true], seed);
        let original = game();
        assert_eq!(g.trainers[0], original.trainers[0], "bosses not chosen");
        assert_eq!(
            g.trainers[2], original.trainers[2],
            "the first rival battle gets no buffs"
        );
        let tp = &g.trainers[1].pokemon[0];
        assert!(!tp.reset_moves);
        let known: Vec<MoveId> = (101..131).collect();
        assert!(tp.moves.iter().all(|m| known.contains(m)), "{:?}", tp.moves);
        let unique: BTreeSet<_> = tp.moves.iter().collect();
        assert_eq!(unique.len(), 4, "{:?}", tp.moves);
        // The first pick is a good damaging move.
        let first = &g.moves[usize::from(tp.moves[0]) - 1];
        assert!(is_good_damaging(first, perfect_accuracy(&g)), "{first:?}");
    }
}

#[test]
fn few_moves_are_kept_as_they_are() {
    let g = run(game(), [true; 3], 1);
    // Species 3 knows Tackle and one status move by level 40.
    assert_eq!(g.trainers[0].pokemon[1].moves, [TACKLE, 2, 0, 0]);
    assert!(!g.trainers[0].pokemon[1].reset_moves);
}

#[test]
fn stronger_moves_outdo_weaker_ones_of_their_kind() {
    let g = game();
    let mv = |n: MoveId| &g.moves[usize::from(n) - 1];
    let pool = [mv(TACKLE), mv(BODY_SLAM), mv(EMBER), mv(FLAMETHROWER), mv(2)];
    let obsolete: Vec<MoveId> = obsolete_moves(&pool).iter().map(|m| m.number).collect();
    assert_eq!(obsolete, [TACKLE, EMBER]);
    assert_eq!(distinct(&[mv(2), mv(4), mv(2)]).len(), 2);
}

#[test]
fn pre_evolution_and_egg_moves_join_the_pool() {
    let mut g = game();
    g.learnsets.insert(2, vec![LevelUpMove::new(1, TACKLE)]);
    g.egg_moves.insert(1, (0..40).map(|i| 201 + 2 * i).collect());
    g.trainers.truncate(2);
    let mut seen = BTreeSet::new();
    for seed in 0..30 {
        let after = run(g.clone(), [false, false, true], seed);
        seen.extend(after.trainers[1].pokemon[0].moves);
    }
    assert!(
        seen.iter().any(|m| (101..160).contains(m)),
        "pre-evolution moves: {seen:?}"
    );
    assert!(seen.iter().any(|m| (201..281).contains(m)), "egg moves: {seen:?}");
}

#[test]
fn games_without_trainer_movesets_are_left_alone() {
    let mut g = game();
    g.trainer_custom_movesets = [false; 3];
    assert_eq!(run(g.clone(), [true; 3], 1), g);
}

/// The test game with an alternate forme (#4) of species 2, which evolves from species 1, without evolutions of
/// its own, knowing only Tackle; species 1 has egg moves.
fn game_with_forme() -> GameData {
    let mut g = game();
    let mut forme = species(4, Type::Fire, None, 80);
    forme.forme.base = Some(2);
    forme.forme.number = 1;
    forme.abilities = vec![66, 0];
    g.species.push(forme);
    g.learnsets.insert(4, vec![LevelUpMove::new(1, TACKLE)]);
    g.egg_moves.insert(1, (0..40).map(|i| 201 + 2 * i).collect());
    g
}

/// The moves a level 40 Pokémon of the alternate forme might get, over many draws.
fn forme_pool(g: &GameData) -> BTreeSet<MoveId> {
    use rand_core::SeedableRng;
    let tp = TrainerPokemon::new(4, 40);
    let mut seen = BTreeSet::new();
    for seed in 0..30 {
        let mut rng = StepRng::seed_from_u64(seed);
        seen.extend(pool::selection_pool(g, &tp, false, &mut rng).iter().map(|m| m.number));
    }
    seen
}

#[test]
fn alternate_formes_take_their_base_species_pre_evolution_and_egg_moves_before_gen_7() {
    let g = game_with_forme();
    let seen = forme_pool(&g);
    assert!(seen.contains(&TACKLE));
    assert!(
        seen.iter().any(|m| (101..131).contains(m)),
        "pre-evolution moves: {seen:?}"
    );
    assert!(seen.iter().any(|m| (201..281).contains(m)), "egg moves: {seen:?}");

    // From Gen 7 on, alternate formes evolve on their own: one without evolutions has no pre-evolutions.
    let mut gen7 = g;
    gen7.identity.generation = ppr_model::Generation::new(7).expect("valid");
    let seen = forme_pool(&gen7);
    assert_eq!(seen, BTreeSet::from([TACKLE]));
}

/// Basculin's Blue-Striped Form in Black 2 has no evolutions or egg moves of its own: it gets Basculin's egg moves,
/// as in the original.
#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn basculins_blue_striped_form_gets_basculins_egg_moves_in_black_2() {
    const BASCULIN: u16 = 550;
    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a directory of ROM dumps");
    let mut tested = 0;
    for entry in std::fs::read_dir(dir).expect("PPR_TEST_ROMS is readable") {
        let path = entry.expect("lists").path();
        if path.to_string_lossy().contains("randomized") {
            continue;
        }
        let Ok(rom) = ppr_romio::open(&path) else { continue };
        let g = rom.game_data();
        if g.identity.generation.number() != 5 {
            continue;
        }
        let blue = g.formes_of(BASCULIN).next().expect("Basculin has a forme").number;
        assert!(!g.egg_moves.contains_key(&blue), "{}", path.display());
        let eggs: BTreeSet<MoveId> = g.egg_moves[&BASCULIN].iter().copied().collect();
        let tp = TrainerPokemon::new(blue, 5);
        let mut seen = BTreeSet::new();
        for seed in 0..50 {
            use rand_core::SeedableRng;
            let mut rng = StepRng::seed_from_u64(seed);
            seen.extend(pool::selection_pool(g, &tp, false, &mut rng).iter().map(|m| m.number));
        }
        assert!(seen.iter().any(|m| eggs.contains(m)), "{}: {seen:?}", path.display());
        tested += 1;
    }
    assert!(tested > 0, "no Gen 5 ROMs");
}
