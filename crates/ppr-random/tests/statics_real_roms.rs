//! Static Pokémon on the real games in `PPR_TEST_ROMS`: the hidden hollows of Black 2, which may only hold some
//! species, and the Ultra Beasts of Ultra Sun, which stay among themselves in "legendary ↔ legendary" mode.

use std::path::PathBuf;

use ppr_model::{DexNumber, GameData, is_legendary};
use ppr_random::{CancelToken, Progress, RunData, Seed, standard_pipeline};
use ppr_settings::{Settings, StaticMode};

/// The ROM in `PPR_TEST_ROMS` whose file name contains `name`; `None` when it is not there.
fn rom(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a directory of ROM dumps");
    std::fs::read_dir(dir)
        .expect("PPR_TEST_ROMS is readable")
        .map(|e| e.expect("lists").path())
        .filter(|p| !p.to_string_lossy().contains("randomized"))
        .find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().contains(name)))
}

fn run(game: &GameData, mode: StaticMode, randomize_600_bst: bool, seed: u64) -> GameData {
    let mut settings = Settings::default();
    settings.statics.mode = mode;
    settings.statics.randomize_600_bst = randomize_600_bst;
    let mut data = RunData {
        game: game.clone(),
        settings,
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

const MODES: [(StaticMode, bool); 4] = [
    (StaticMode::RandomMatching, false),
    (StaticMode::Random, false),
    (StaticMode::SimilarStrength, false),
    (StaticMode::SimilarStrength, true),
];

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn real_hidden_hollows_hold_only_allowed_species() {
    let Some(path) = rom("Black Version 2") else {
        eprintln!("no Black 2 in PPR_TEST_ROMS; skipped");
        return;
    };
    let mut rom = ppr_romio::open(&path).expect("opens");
    let game = rom.game_data().clone();
    let hollows: Vec<usize> = (0..game.statics.len())
        .filter(|&i| game.statics[i].allowed_species.is_some())
        .collect();
    assert!(hollows.len() >= 50, "{} hidden hollow encounters", hollows.len());
    let allowed = |n: DexNumber| n <= 493 || (505..=632).contains(&n);
    // The other statics may become anything.
    assert!(game.statics.iter().any(|s| s.allowed_species.is_none()));
    for (mode, strong) in MODES {
        for seed in 0..3 {
            let new = run(&game, mode, strong, seed);
            for &i in &hollows {
                let st = &new.statics[i];
                let list = st.allowed_species.as_deref().expect("still limited");
                assert!(list.contains(&st.species), "{mode:?} seed {seed}: {st:?}");
                assert!(allowed(st.species), "{mode:?} seed {seed}: {}", st.species);
            }
            // Completely random, the others still get other Unova species.
            if mode == StaticMode::Random {
                assert!(
                    new.statics
                        .iter()
                        .any(|s| s.allowed_species.is_none() && !allowed(s.species)),
                    "seed {seed}"
                );
            }
        }
    }

    // What is written reads back.
    let new = run(&game, StaticMode::Random, false, 1);
    rom.apply(&new).expect("applies");
    let mut bytes = Vec::new();
    rom.save(&mut bytes).expect("saves");
    let back = ppr_romio::open_bytes(bytes).expect("reopens");
    let read: Vec<DexNumber> = hollows.iter().map(|&i| back.game_data().statics[i].species).collect();
    let written: Vec<DexNumber> = hollows.iter().map(|&i| new.statics[i].species).collect();
    assert_eq!(read, written);
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn real_ultra_beasts_stay_ultra_beasts() {
    let Some(path) = rom("Ultra Sun") else {
        eprintln!("no Ultra Sun in PPR_TEST_ROMS; skipped");
        return;
    };
    let rom = ppr_romio::open(&path).expect("opens");
    let game = rom.game_data().clone();
    let ultra_beasts = game.ultra_beasts();
    assert_eq!(ultra_beasts.len(), 11);
    let count = game
        .statics
        .iter()
        .filter(|s| ultra_beasts.contains(&s.species))
        .count();
    assert!(count >= 1, "Ultra Sun has Ultra Beast statics");
    for seed in 0..5 {
        let new = run(&game, StaticMode::RandomMatching, false, seed);
        for (old, st) in game.statics.iter().zip(&new.statics) {
            assert_eq!(
                ultra_beasts.contains(&old.species),
                ultra_beasts.contains(&st.species),
                "seed {seed}: {old:?} -> {st:?}"
            );
            assert_eq!(
                is_legendary(game.base_forme_of(old.species)),
                is_legendary(game.base_forme_of(st.species)),
                "seed {seed}: {old:?} -> {st:?}"
            );
        }
    }
    // Outside that mode they are species like any other.
    let other = (0..5)
        .map(|seed| run(&game, StaticMode::Random, false, seed))
        .flat_map(|new| new.statics)
        .filter(|s| ultra_beasts.contains(&s.species))
        .count();
    assert!(other < count * 5, "{other} Ultra Beasts in random mode");
}
