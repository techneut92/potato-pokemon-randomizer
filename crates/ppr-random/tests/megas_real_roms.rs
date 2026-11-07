//! Mega Evolutions and alternate formes in the species randomizers, on the real Gen 6 and Gen 7 games in
//! `PPR_TEST_ROMS`: following Mega Evolutions in base stats, types and abilities, and the base stat updates of
//! alternate formes.

use std::path::PathBuf;

use ppr_model::{DexNumber, GameData, Species};
use ppr_random::{CancelToken, Progress, RunData, Seed, standard_pipeline};
use ppr_settings::{AbilityMode, BaseStatMode, Settings, TypeMode};

/// The Gen 6 and Gen 7 games in `PPR_TEST_ROMS`, loaded.
fn games() -> Vec<(PathBuf, GameData)> {
    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a directory of ROM dumps");
    let mut games = Vec::new();
    for entry in std::fs::read_dir(dir).expect("PPR_TEST_ROMS is readable") {
        let path = entry.expect("lists").path();
        // The app's own output is no dump.
        if path.to_string_lossy().contains("randomized") {
            continue;
        }
        let Ok(rom) = ppr_romio::open(&path) else { continue };
        let game = rom.game_data().clone();
        if game.identity.generation.number() >= 6 {
            games.push((path, game));
        }
    }
    assert!(!games.is_empty(), "no Gen 6 or Gen 7 ROMs");
    games
}

fn run(game: &GameData, settings: Settings, seed: u64) -> GameData {
    let mut data = RunData {
        game: game.clone(),
        settings,
    };
    standard_pipeline()
        .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

fn species(game: &GameData, n: DexNumber) -> &Species {
    game.species(n).expect("there")
}

/// Random base stats, types and abilities that follow evolutions, and Mega Evolutions when `megas`.
fn following(megas: bool) -> Settings {
    let mut s = Settings::default();
    s.base_stats.mode = BaseStatMode::Random;
    s.base_stats.follow_evolutions = true;
    s.base_stats.follow_megas = megas;
    s.types.mode = TypeMode::FollowEvolutions;
    s.types.follow_megas = megas;
    s.abilities.mode = AbilityMode::Random;
    s.abilities.follow_evolutions = true;
    s.abilities.follow_megas = megas;
    s
}

/// The Mega Evolutions of `game` with their base species, and whether they split (Charizard's and Mewtwo's).
fn megas(game: &GameData) -> Vec<(DexNumber, DexNumber, bool)> {
    let megas: Vec<&Species> = game.mega_evolutions().collect();
    megas
        .iter()
        .map(|m| {
            let base = m.conceptual_base().expect("a forme");
            let split = megas.iter().filter(|o| o.conceptual_base() == Some(base)).count() > 1;
            (m.number, base, split)
        })
        .collect()
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn real_mega_evolutions_follow_their_base_species() {
    for (path, game) in games() {
        let name = path.display();
        let all = megas(&game);
        assert!(all.len() >= 28, "{name}: {} Mega Evolutions", all.len());
        let new = run(&game, following(true), 1);
        for &(n, base, split) in &all {
            let (b, m) = (species(&new, base), species(&new, n));
            assert_eq!(m.stats.total(), species(&game, n).stats.total(), "{name}: {}", m.name);
            if split {
                continue;
            }
            assert_eq!(m.abilities, b.abilities, "{name}: {}", m.name);
            assert_eq!(m.primary_type, b.primary_type, "{name}: {}", m.name);
            if b.secondary_type.is_some() {
                assert_eq!(m.secondary_type, b.secondary_type, "{name}: {}", m.name);
            }
            // Spread like the base species: each stat in proportion to the totals, give or take rounding, unless a
            // stat hits the ceiling and the rest moves.
            let (bs, ms) = (stats(b), stats(m));
            if ms.contains(&255) {
                continue;
            }
            let scale = f64::from(m.stats.total()) / f64::from(b.stats.total());
            for (x, y) in bs.iter().zip(ms) {
                let expected = f64::from(*x) * scale;
                assert!(
                    (f64::from(y) - expected).abs() <= 2.0,
                    "{name}: {} {bs:?} {ms:?}",
                    m.name
                );
            }
        }

        // Without following Mega Evolutions they keep their stats and get abilities of their own.
        let new = run(&game, following(false), 1);
        let mut own = 0;
        for &(n, base, _) in &all {
            assert_eq!(species(&new, n).stats, species(&game, n).stats, "{name}");
            own += usize::from(species(&new, n).abilities != species(&new, base).abilities);
        }
        assert!(own > all.len() / 2, "{name}: {own} of {}", all.len());
    }
}

fn stats(sp: &Species) -> [u8; 6] {
    let s = sp.stats;
    [s.hp, s.attack, s.defense, s.sp_attack, s.sp_defense, s.speed]
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn real_base_stat_updates_reach_alternate_formes() {
    for (path, game) in games() {
        let name = path.display();
        let generation = game.identity.generation.number();
        let mega_alakazam = game
            .formes_of(65)
            .find(|sp| sp.is_mega())
            .expect("Mega Alakazam")
            .number;
        let blade = game.forme_species(681, 1);
        assert_ne!(blade, 681, "{name}: Aegislash's Blade Forme");
        // As loaded: Gen 7 already has Mega Alakazam's Sp. Def of Gen 7.
        let old_spd = if generation == 6 { 95 } else { 105 };
        assert_eq!(species(&game, mega_alakazam).stats.sp_defense, old_spd, "{name}");
        assert_eq!(
            (
                species(&game, blade).stats.attack,
                species(&game, blade).stats.sp_attack
            ),
            (150, 150),
            "{name}"
        );

        let mut settings = Settings::default();
        settings.base_stats.update_to_generation = Some(8);
        let new = run(&game, settings, 1);
        assert_eq!(species(&new, mega_alakazam).stats.sp_defense, 105, "{name}");
        let blade_stats = species(&new, blade).stats;
        assert_eq!(
            (
                blade_stats.attack,
                blade_stats.defense,
                blade_stats.sp_attack,
                blade_stats.sp_defense
            ),
            (140, 50, 140, 50),
            "{name}"
        );
        let shield = species(&new, 681).stats;
        assert_eq!(
            (shield.attack, shield.defense, shield.sp_attack, shield.sp_defense),
            (50, 140, 50, 140),
            "{name}"
        );
        // Nothing else of theirs changes.
        let changed = |n| {
            let (before, after) = (stats(species(&game, n)), stats(species(&new, n)));
            before.iter().zip(after).filter(|&(x, y)| *x != y).count()
        };
        assert_eq!(changed(mega_alakazam), usize::from(generation == 6), "{name}");
        assert_eq!(changed(blade), 2, "{name}");
    }
}
