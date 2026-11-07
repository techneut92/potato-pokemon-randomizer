//! Tests on real ROMs from `PPR_TEST_ROMS`, a directory of the developer's own dumps. The repository never
//! contains ROMs (ADR 0007), so these are ignored by default:
//!
//! ```sh
//! PPR_TEST_ROMS=~/roms cargo test -p ppr-service --test local_roms -- --ignored
//! ```

use std::fmt::Write as _;
use std::path::PathBuf;

use ppr_service::{
    CancelToken, Progress, RandomizeRequest, RomReport, Seed, Settings, StarterMode, WildMode, WildZones,
    identify_file, load_rom, randomize_file,
};

/// The supported ROMs in `PPR_TEST_ROMS`.
fn roms() -> Vec<PathBuf> {
    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a directory of ROM dumps");
    let mut roms: Vec<_> = std::fs::read_dir(dir)
        .expect("PPR_TEST_ROMS is readable")
        .map(|e| e.expect("readable entry").path())
        // The app's own output, like `… -randomized.gba`, is no dump.
        .filter(|p| !p.to_string_lossy().contains("randomized"))
        .filter(|p| p.is_file() && matches!(identify_file(p), Ok(RomReport::Supported { .. })))
        // Games of a generation that isn't ported yet open, but can't be randomized.
        .filter(|p| load_rom(p).is_ok_and(|r| r.randomizable))
        .collect();
    roms.sort();
    roms
}

/// A folder for the randomized games, on disk below the target folder: a randomized 3DS game takes gigabytes,
/// too much for a `/tmp` in memory.
fn out_dir() -> tempfile::TempDir {
    tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).expect("temp dir")
}

/// Most of `total`: the share of species, moves or the like a randomizer is to change at least.
fn most_of(total: usize) -> usize {
    total * 3 / 4
}

/// One of the tournament trainers of Black 2 and White 2, the last 17: the game sets their levels and battle
/// styles.
fn in_tournament(game: &ppr_model::GameData, t: &ppr_model::Trainer) -> bool {
    let tournament = if ["Black 2", "White 2"].iter().any(|g| game.identity.name.starts_with(g)) {
        17
    } else {
        0
    };
    usize::from(t.index) + tournament > game.trainers.len()
}

/// Says why a test that needs a particular game checked nothing.
fn skipped(why: &str) {
    eprintln!("skipped: {why}");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn starters_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let mut settings = Settings::default();
        settings.starters.mode = StarterMode::Random;
        settings.starters.types = ppr_service::StarterTypes::TypeTriangle;
        settings.starters.random_held_items = true;
        settings.starters.ban_bad_items = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(1)),
            log: false,
            edits: None,
        };
        match randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()) {
            Ok(report) => {
                let original = ppr_romio::open(&rom).expect("input loads");
                // Yellow has two.
                let count = original.game_data().starters.len();
                assert_eq!(report.starters.len(), count, "{}", rom.display());
                assert!(
                    report.starters.iter().any(|s| s.before != s.after),
                    "{}: starters unchanged",
                    rom.display()
                );
                let saved = ppr_romio::open(&request.output).expect("output loads");
                let game = saved.game_data();
                let names: Vec<_> = game
                    .starters
                    .iter()
                    .map(|&n| game.species(n).expect("starter is a species").name.clone())
                    .collect();
                let after: Vec<_> = report.starters.iter().map(|s| s.after.clone()).collect();
                assert_eq!(names, after, "{}: saved starters differ from the report", rom.display());
                if game.starter_held_items.is_empty() {
                    // Gen 1 and Gen 5 starters hold nothing.
                    eprintln!("{}: {after:?}", rom.display());
                    tested += 1;
                    continue;
                }
                // One for each starter in Gen 2, Gen 6 and Gen 7, one for all in Gen 3.
                let expected = match game.identity.generation.number() {
                    2 | 7 => 3,
                    6 => game.starters.len(),
                    _ => 1,
                };
                assert_eq!(game.starter_held_items.len(), expected, "{}", rom.display());
                let mut held_names = Vec::new();
                for (i, held) in game.starter_held_items.iter().enumerate() {
                    let held = held.unwrap_or_else(|| panic!("{}: {:?}", rom.display(), game.starter_held_items));
                    let item = game.items.iter().find(|i| i.id == held).expect("an item");
                    assert!(item.allowed && !item.bad, "{}: {item:?}", rom.display());
                    assert_eq!(report.starter_held_items[i].after, item.name);
                    held_names.push(item.name.clone());
                }
                eprintln!("{}: {:?}, holding {:?}", rom.display(), after, held_names);
                tested += 1;
            }
            Err(e) if e.code() == "generation_not_supported" => {}
            Err(e) => panic!("{}: {e}", rom.display()),
        }
    }
    assert!(tested > 0, "no supported ROM in PPR_TEST_ROMS");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn wild_pokemon_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let Ok(original) = ppr_romio::open(&rom) else {
            continue;
        };
        let before = original.game_data();
        // Gen 1 has one set per map and kind of encounter, Gen 3 more.
        let least = if before.identity.generation.number() == 1 {
            50
        } else {
            170
        };
        assert!(before.encounters.len() > least, "{}: few encounter sets", rom.display());
        assert!(
            before
                .encounters
                .iter()
                .all(|s| s.location.is_some() && !s.name.contains('\\')),
            "{}: every set is tagged and has a clean name",
            rom.display()
        );
        let mut settings = Settings::default();
        settings.wild.mode = WildMode::Random;
        settings.wild.zones = WildZones::Location;
        settings.wild.similar_strength = true;
        settings.wild.min_catch_rate = 4;
        settings.wild.level_modifier = 10;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(1)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let after = saved.game_data();
        assert_eq!(after.encounters.len(), before.encounters.len(), "{}", rom.display());
        let changed = before
            .encounters
            .iter()
            .zip(&after.encounters)
            .filter(|(b, a)| {
                b.encounters
                    .iter()
                    .zip(&a.encounters)
                    .any(|(x, y)| x.species != y.species)
            })
            .count();
        assert!(
            changed > before.encounters.len() / 2,
            "{}: {changed} sets changed",
            rom.display()
        );
        assert!(after.species.iter().all(|s| s.catch_rate == 255), "{}", rom.display());
        tested += 1;
    }
    assert!(tested > 0, "no supported ROM in PPR_TEST_ROMS");
}

/// Ultra Sun's wild Pokémon by time of day: with the option, the day and night sets are randomized on their own
/// and read back; without it, the night halves get the day's Pokémon, so the night tables only keep Pokémon of
/// their own with time of day.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn gen7_time_of_day_encounters_randomize_and_reload() {
    let Some(rom) = roms().into_iter().find(|p| p.to_string_lossy().contains("Ultra Sun")) else {
        return skipped("no Ultra Sun in PPR_TEST_ROMS");
    };
    let mut original = ppr_romio::open(&rom).expect("loads");
    original.use_time_of_day(true).expect("reads");
    let before = original.game_data().encounters.clone();
    let species = |s: &ppr_model::EncounterSet| s.encounters.iter().map(|e| e.species).collect::<Vec<_>>();
    let own_nights =
        |sets: &[ppr_model::EncounterSet]| sets.chunks(2).filter(|p| species(&p[0]) != species(&p[1])).count();
    let tables = before.len() / 2;
    assert!(own_nights(&before) > 100, "Ultra Sun has night Pokémon of its own");
    for time_of_day in [false, true] {
        let out_dir = out_dir();
        let mut settings = Settings::default();
        settings.wild.mode = WildMode::Random;
        settings.wild.zones = WildZones::Slot;
        settings.wild.time_of_day = time_of_day;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(154)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()).expect("randomizes");
        let mut saved = ppr_romio::open(&request.output).expect("output loads");
        assert_eq!(saved.game_data().encounters.len(), tables);
        saved.use_time_of_day(true).expect("reads");
        let after = &saved.game_data().encounters;
        assert_eq!(after.len(), before.len());
        let changed = |half: usize| {
            before
                .iter()
                .zip(after)
                .skip(half)
                .step_by(2)
                .filter(|(b, a)| species(b) != species(a))
                .count()
        };
        let (days, nights) = (changed(0), changed(1));
        assert!(
            days > tables * 3 / 4,
            "time of day {time_of_day}: {days} day sets changed"
        );
        assert!(
            nights > tables * 3 / 4,
            "time of day {time_of_day}: {nights} night sets changed"
        );
        if time_of_day {
            assert!(
                own_nights(after) > tables * 3 / 4,
                "the night sets are randomized on their own"
            );
        } else {
            // The tables written (the day set changed) have the day's Pokémon by night; the others are as they were.
            for (b, a) in before.chunks(2).zip(after.chunks(2)) {
                if a[0] == b[0] {
                    assert_eq!(a[1], b[1], "{}", a[1].name);
                } else {
                    assert_eq!(a[0].encounters, a[1].encounters, "{}", a[1].name);
                }
            }
        }
    }
}

/// A manual edit of an Ultra Sun wild set, made in the editor's view without time of day, gives the edited slots
/// by day and by night in a run with time of day, while the other tables keep night Pokémon of their own.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn gen7_wild_set_edits_apply_by_day_and_by_night() {
    use ppr_model::{Edit, EditList, WildSetEdit, WildSlotEdit};

    let Some(rom) = roms().into_iter().find(|p| p.to_string_lossy().contains("Ultra Sun")) else {
        return skipped("no Ultra Sun in PPR_TEST_ROMS");
    };
    let (_, session) = ppr_service::RomSession::open(&rom, None).expect("loads");
    let session = session.expect("ported");
    let editable = session.editable();
    // Route 1's first table, as the editor shows it.
    let table = 0;
    assert!(!editable.wild_sets[table].name.contains("(Day)"));
    let slots: Vec<WildSlotEdit> = editable.wild_sets[table]
        .value
        .slots
        .iter()
        .map(|_| WildSlotEdit {
            species: 25,
            min_level: 7,
            max_level: 9,
        })
        .collect();
    let mut edits = EditList::new(&session.game().identity);
    edits.set(Edit::WildSet(WildSetEdit { index: table, slots }));
    assert_eq!(session.validate(&edits).expect("checks"), []);

    let out_dir = out_dir();
    let mut settings = Settings::default();
    settings.wild.mode = WildMode::Random;
    settings.wild.zones = WildZones::Slot;
    settings.wild.time_of_day = true;
    let request = RandomizeRequest {
        update: None,
        layered_fs: false,
        input: rom.clone(),
        output: out_dir.path().join("out"),
        settings,
        seed: Some(Seed(154)),
        log: false,
        edits: Some(edits),
    };
    let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()).expect("randomizes");
    assert_eq!(report.manual_edits, 1);
    let mut saved = ppr_romio::open(&request.output).expect("output loads");
    saved.use_time_of_day(true).expect("reads");
    let sets = &saved.game_data().encounters;
    for set in &sets[2 * table..2 * table + 2] {
        assert!(
            set.encounters
                .iter()
                .all(|e| (e.species, e.min_level, e.max_level) == (25, 7, 9)),
            "{}",
            set.name
        );
    }
    let own_nights = sets
        .chunks(2)
        .filter(|p| {
            p[0].encounters
                .iter()
                .map(|e| e.species)
                .ne(p[1].encounters.iter().map(|e| e.species))
        })
        .count();
    assert!(
        own_nights > sets.len() / 4,
        "the other night sets are randomized on their own"
    );
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn statics_and_trades_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let mut settings = Settings::default();
        settings.statics.mode = ppr_service::StaticMode::SimilarStrength;
        settings.statics.limit_main_game_legendaries = true;
        settings.statics.fix_music = true;
        settings.trades.mode = ppr_service::TradeMode::GivenAndRequested;
        settings.trades.random_nicknames = true;
        settings.trades.random_ots = true;
        settings.trades.random_ivs = true;
        settings.trades.random_items = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(3)),
            log: false,
            edits: None,
        };
        match randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()) {
            Ok(report) => {
                assert!(report.statics_changed > 0, "{}: statics unchanged", rom.display());
                let original = ppr_romio::open(&rom).expect("input loads");
                let saved = ppr_romio::open(&request.output).expect("output loads");
                let (before, after) = (&original.game_data().statics, &saved.game_data().statics);
                assert_eq!(before.len(), after.len(), "{}", rom.display());
                let changed = before.iter().zip(after).filter(|(b, a)| b.species != a.species).count();
                assert_eq!(changed, report.statics_changed as usize, "{}", rom.display());
                eprintln!("{}: {changed} of {} statics changed", rom.display(), after.len());
                let trades = &saved.game_data().trades;
                assert_eq!(report.trades_changed as usize, trades.len(), "{}", rom.display());
                assert!(
                    trades.iter().all(|t| t.ot_name.chars().count() <= 7),
                    "{}",
                    rom.display()
                );
                tested += 1;
            }
            Err(e) if e.code() == "generation_not_supported" => {}
            Err(e) => panic!("{}: {e}", rom.display()),
        }
    }
    assert!(tested > 0, "no supported ROM in PPR_TEST_ROMS");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn totems_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("input loads");
        let before = original.game_data();
        if before.totems.is_empty() {
            continue;
        }
        let mut settings = Settings::default();
        settings.totems.mode = ppr_service::TotemMode::SimilarStrength;
        settings.totems.ally_mode = ppr_service::TotemMode::Random;
        settings.totems.aura_mode = ppr_service::AuraMode::SameStrength;
        settings.totems.randomize_held_items = true;
        settings.totems.level_modifier = 20;
        let request = RandomizeRequest {
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(40)),
            log: true,
            update: None,
            layered_fs: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let after = &saved.game_data().totems;
        assert_eq!(report.totems as usize, before.totems.len(), "{}", rom.display());
        assert_eq!(
            report.totems_changed as usize,
            after.len(),
            "every level changed: {}",
            rom.display()
        );
        assert_eq!(after.len(), before.totems.len());
        let consumables = &before.held_item_pools.consumable;
        // 20% more, rounded, at most 100.
        let raised = |level: u8| u8::try_from(((u16::from(level) * 12 + 5) / 10).min(100)).expect("small");
        for (b, a) in before.totems.iter().zip(after) {
            assert_ne!(a.species, b.species, "{}", rom.display());
            assert_eq!(a.level, raised(b.level));
            assert_eq!(a.aura.stages, b.aura.stages);
            assert_eq!(a.held_item.is_some(), b.held_item.is_some());
            assert!(a.held_item.is_none_or(|i| consumables.contains(&i)), "{a:?}");
            assert_eq!(a.allies.len(), b.allies.len());
            for (ba, aa) in b.allies.iter().zip(&a.allies) {
                assert_eq!(aa.level, raised(ba.level));
                assert_eq!(aa.held_item, ba.held_item, "allies keep their items");
            }
        }
        let log = report.log.as_deref().expect("a log");
        assert!(log.contains("Totem Pokemon: Randomized/Changed\n"));
        assert!(log.contains("{TOPK}"));
        // The rest of the static encounters stays as it was.
        assert_eq!(saved.game_data().statics, before.statics, "{}", rom.display());
        eprintln!("{}: {} Totem Pokémon changed", rom.display(), after.len());
        tested += 1;
    }
    if tested == 0 {
        skipped("no game with Totem Pokémon in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn base_stats_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        for mode in [ppr_service::BaseStatMode::Shuffle, ppr_service::BaseStatMode::Random] {
            let mut settings = Settings::default();
            settings.base_stats.mode = mode;
            settings.base_stats.follow_evolutions = true;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join(format!("out-{mode:?}")),
                settings,
                seed: Some(Seed(5)),
                log: false,
                edits: None,
            };
            match randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()) {
                Ok(report) => {
                    let original = ppr_romio::open(&rom).expect("input loads");
                    let saved = ppr_romio::open(&request.output).expect("output loads");
                    let (before, after) = (&original.game_data().species, &saved.game_data().species);
                    let changed = before.iter().zip(after).filter(|(b, a)| b.stats != a.stats).count();
                    assert_eq!(changed, report.base_stats_changed as usize, "{}", rom.display());
                    assert!(changed > most_of(before.len()), "{}: {changed} changed", rom.display());
                    for (b, a) in before.iter().zip(after) {
                        assert_eq!(b.stats.total(), a.stats.total(), "{} {}", rom.display(), b.name);
                    }
                    if let Some(shedinja) = after.iter().find(|s| s.number == 292) {
                        assert_eq!(shedinja.stats.hp, 1, "{}", rom.display());
                    }
                    eprintln!("{} {mode:?}: {changed} of {} changed", rom.display(), after.len());
                    tested += 1;
                }
                Err(e) if e.code() == "generation_not_supported" => {}
                Err(e) => panic!("{}: {e}", rom.display()),
            }
        }
    }
    assert!(tested > 0, "no supported ROM in PPR_TEST_ROMS");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn bsts_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        for mode in [
            ppr_service::BstMode::BuffNerf,
            ppr_service::BstMode::Shuffle,
            ppr_service::BstMode::Random,
        ] {
            let mut settings = Settings::default();
            settings.bsts.mode = mode;
            settings.bsts.max_buff_nerf_percent = 50;
            // Following evolutions, branches take the donor's first evolution's total (lines stay even), so
            // only shuffling without it keeps exactly the same totals.
            settings.bsts.follow_evolutions = mode == ppr_service::BstMode::BuffNerf;
            settings.bsts.separate_legendaries = mode == ppr_service::BstMode::Shuffle;
            settings.base_stats.update_to_generation = (mode != ppr_service::BstMode::Shuffle).then_some(7);
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join(format!("out-{mode:?}")),
                settings,
                seed: Some(Seed(6)),
                log: false,
                edits: None,
            };
            match randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()) {
                Ok(report) => {
                    let original = ppr_romio::open(&rom).expect("input loads");
                    let saved = ppr_romio::open(&request.output).expect("output loads");
                    let (before, after) = (&original.game_data().species, &saved.game_data().species);
                    let changed = before.iter().zip(after).filter(|(b, a)| b.stats != a.stats).count();
                    assert_eq!(changed, report.base_stats_changed as usize, "{}", rom.display());
                    assert!(changed > most_of(before.len()), "{}: {changed} changed", rom.display());
                    if let Some(shedinja) = after.iter().find(|s| s.number == 292) {
                        assert_eq!(shedinja.stats.hp, 1, "{}", rom.display());
                    }
                    if mode == ppr_service::BstMode::Shuffle {
                        // The same totals, but for Shedinja's, which its five stats round. Alternate formes follow
                        // their base species instead.
                        let mut left: Vec<u16> = before
                            .iter()
                            .filter(|s| s.is_base_forme())
                            .map(ppr_model::Species::bst)
                            .collect();
                        for s in after.iter().filter(|s| s.is_base_forme()) {
                            if let Some(i) = left.iter().position(|&t| t == s.bst()) {
                                left.swap_remove(i);
                            }
                        }
                        assert!(left.len() <= 1, "{}: {left:?} not given out", rom.display());
                    }
                    eprintln!("{} {mode:?}: {changed} of {} changed", rom.display(), after.len());
                    tested += 1;
                }
                Err(e) if e.code() == "generation_not_supported" => {}
                Err(e) => panic!("{}: {e}", rom.display()),
            }
        }
    }
    assert!(tested > 0, "no supported ROM in PPR_TEST_ROMS");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn types_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let mut settings = Settings::default();
        settings.types.mode = ppr_service::TypeMode::FollowEvolutions;
        settings.types.force_dual = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(7)),
            log: false,
            edits: None,
        };
        match randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()) {
            Ok(report) => {
                let saved = ppr_romio::open(&request.output).expect("output loads");
                let game = saved.game_data();
                assert!(
                    report.types_changed as usize > most_of(game.species.len()),
                    "{}: {}",
                    rom.display(),
                    report.types_changed
                );
                assert!(
                    game.species.iter().all(|s| s.secondary_type.is_some()),
                    "{}",
                    rom.display()
                );
                // Bulbasaur's line shares its types.
                let types = |n| game.species(n).map(|s| (s.primary_type, s.secondary_type));
                assert_eq!(types(1), types(2), "{}", rom.display());
                assert_eq!(types(2), types(3), "{}", rom.display());
                tested += 1;
            }
            Err(e) if e.code() == "generation_not_supported" => {}
            Err(e) => panic!("{}: {e}", rom.display()),
        }
    }
    assert!(tested > 0, "no supported ROM in PPR_TEST_ROMS");
}

/// [`evolutions_hash`] of every unmodified Gen 3 game.
const VANILLA_EVOLUTIONS: u64 = 10_913_402_203_943_396_373;

/// FNV-1a of `from to kind parameter` lines of the game's evolutions.
fn evolutions_hash(game: &ppr_model::GameData) -> u64 {
    let mut dump = String::new();
    for e in &game.evolutions {
        writeln!(dump, "{} {} {:?} {}", e.from, e.to, e.kind, e.parameter).expect("writes");
    }
    dump.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn evolution_improvements_match_reference() {
    // The evolutions after all four improvements at level 35, without and with estimated levels, as reference hashes;
    // read back from the saved ROM.
    let expected = |game: &str| match game {
        "Emerald (U)" => Some([11_096_222_699_700_441_956, 2_676_542_206_846_913_004]),
        "Fire Red (U) 1.0" => Some([7_215_336_415_395_138_090, 6_899_844_970_999_168_457]),
        _ => None,
    };
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads");
        let Some(hashes) = expected(&original.game_data().identity.name) else {
            continue;
        };
        if evolutions_hash(original.game_data()) != VANILLA_EVOLUTIONS {
            continue;
        }
        for (estimated, hash) in [false, true].into_iter().zip(hashes) {
            let mut settings = Settings::default();
            settings.evolutions.change_impossible = true;
            settings.evolutions.easier_max_level = Some(35);
            settings.evolutions.use_estimated_levels = estimated;
            settings.evolutions.remove_time_based = true;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(1)),
                log: false,
                edits: None,
            };
            let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()).expect("runs");
            assert!(report.evolutions_changed > 0, "{}", rom.display());
            let saved = ppr_romio::open(&request.output).expect("output loads");
            assert_eq!(
                evolutions_hash(saved.game_data()),
                hash,
                "{} {estimated}",
                rom.display()
            );
        }
        tested += 1;
    }
    if tested == 0 {
        skipped("no unmodified Emerald or FireRed in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn adjusted_evolution_levels_match_reference() {
    // Adjust Evolution Levels on the totals as loaded gives the same evolutions in every unmodified Gen 3 game; read
    // back from the saved ROM.
    const REFERENCE_HASH: u64 = 13_320_718_997_754_838_727;
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads");
        if original.game_data().identity.generation.number() != 3
            || evolutions_hash(original.game_data()) != VANILLA_EVOLUTIONS
        {
            continue;
        }
        let mut settings = Settings::default();
        settings.evolutions.adjust_levels = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(1)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()).expect("runs");
        let saved = ppr_romio::open(&request.output).expect("output loads");
        assert_eq!(evolutions_hash(saved.game_data()), REFERENCE_HASH, "{}", rom.display());
        tested += 1;
    }
    if tested == 0 {
        skipped("no unmodified Gen 3 game in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn random_evolutions_randomize_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads");
        // Games randomized before may have more evolutions than a loop-free set without convergence can hold.
        let vanilla = evolutions_hash(original.game_data()) == VANILLA_EVOLUTIONS
            || original.game_data().identity.generation.number() == 1;
        for mode in [
            ppr_service::EvolutionMode::Random,
            ppr_service::EvolutionMode::RandomEveryLevel,
        ] {
            if !vanilla && mode == ppr_service::EvolutionMode::Random {
                continue;
            }
            let mut settings = Settings::default();
            settings.evolutions.mode = mode;
            settings.evolutions.same_typing = true;
            settings.evolutions.no_convergence = mode == ppr_service::EvolutionMode::Random;
            settings.exp_curves.standardize = Some(ppr_model::ExpCurve::MediumFast);
            settings.exp_curves.apply_to = ppr_service::ExpCurveScope::All;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(1)),
                log: false,
                edits: None,
            };
            let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
            assert!(report.evolutions_changed > 0, "{}", rom.display());
            let saved = ppr_romio::open(&request.output).expect("output loads");
            let game = saved.game_data();
            for e in &game.evolutions {
                assert_ne!(e.from, e.to, "{}", rom.display());
            }
            if mode == ppr_service::EvolutionMode::RandomEveryLevel {
                // One for each species, and in Gen 7 for each alternate forme that is not only cosmetic (cosmetic
                // formes keep theirs); the evolutions without a method (Burmy into Wormadam's other Cloaks) are the
                // game's own.
                let in_pool = |n: ppr_model::DexNumber| {
                    game.species(n)
                        .is_some_and(|sp| sp.is_base_forme() || (game.formes.evolutions && !sp.forme.cosmetic))
                };
                let evolutions = game
                    .evolutions
                    .iter()
                    .filter(|e| e.kind != ppr_model::EvolutionType::None && in_pool(e.from))
                    .count();
                let pool = game.species.iter().filter(|sp| in_pool(sp.number)).count();
                assert_eq!(evolutions, pool, "{}", rom.display());
            }
        }
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn move_data_randomizes_updates_and_reloads() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads");
        let before = original.game_data().moves.clone();
        let run = |settings: Settings| {
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(1)),
                log: false,
                edits: None,
            };
            let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
            assert!(report.moves_changed > 0, "{}", rom.display());
            ppr_romio::open(&request.output)
                .expect("output loads")
                .game_data()
                .moves
                .clone()
        };

        let mut settings = Settings::default();
        settings.move_data.update_to_generation = Some(9);
        let updated = run(settings);
        let of = |moves: &[ppr_model::Move], n: u16| moves[usize::from(n) - 1].clone();
        assert_eq!(
            (of(&updated, 57).power, of(&updated, 33).power),
            (90, 40),
            "Surf, Tackle"
        );
        assert_eq!(of(&updated, 105).pp, 5, "Recover");

        let mut settings = Settings::default();
        settings.move_data.power = true;
        settings.move_data.accuracy = true;
        settings.move_data.pp = true;
        settings.move_data.types = true;
        settings.move_data.names = true;
        let random = run(settings);
        assert_eq!(random.len(), before.len(), "{}", rom.display());
        let max_len = original.game_data().max_move_name_length;
        // Names follow the game's case: capitals before Gen 4.
        let upper_case = before.iter().all(|m| m.name == m.name.to_uppercase());
        let mut names: Vec<&str> = random.iter().map(|m| m.name.as_str()).collect();
        names.sort_unstable();
        names.dedup();
        assert!(names.len() > most_of(random.len()), "{}: names repeat", rom.display());
        for (b, a) in before.iter().zip(&random) {
            if b.number == ppr_model::STRUGGLE {
                assert_eq!(b, a, "{}", rom.display());
            } else {
                assert!(
                    a.pp.is_multiple_of(5) && (5..=40).contains(&a.pp),
                    "{}: {}",
                    rom.display(),
                    a.number
                );
                assert!(
                    a.name.chars().count() <= max_len && (!upper_case || a.name == a.name.to_uppercase()),
                    "{}: {}",
                    rom.display(),
                    a.name
                );
            }
        }
    }
}

/// The moves no random moveset may have: the HMs', the game-breaking ones, those banned from level-up movesets
/// (Transform in Gen 1) and Struggle.
fn banned_moves(game: &ppr_model::GameData) -> Vec<u16> {
    let mut banned = game.hm_moves.clone();
    banned.extend(&game.game_breaking_moves);
    banned.extend(&game.banned_level_up_moves);
    banned.push(ppr_model::STRUGGLE);
    banned
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn movesets_randomize_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.movesets.mode = ppr_service::MovesetMode::RandomPreferSameType;
        settings.movesets.guaranteed_level_one_moves = Some(4);
        settings.movesets.reorder_damaging = true;
        settings.movesets.no_game_breaking = true;
        settings.movesets.good_damaging_percent = Some(30);
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(1)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        assert_eq!(report.movesets_changed, report.species, "{}", rom.display());
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let original = ppr_romio::open(&rom).expect("loads");
        let banned = banned_moves(original.game_data());
        for (n, moves) in &game.learnsets {
            assert!(
                moves.iter().filter(|m| m.level == 1).count() >= 4,
                "{}: #{n}",
                rom.display()
            );
            let mut ids: Vec<u16> = moves.iter().map(|m| m.move_id).collect();
            assert!(
                ids.iter()
                    .all(|id| (1..=game.moves.len()).contains(&usize::from(*id)) && !banned.contains(id)),
                "{}: #{n}: {ids:?}",
                rom.display()
            );
            ids.sort_unstable();
            ids.dedup();
            assert_eq!(ids.len(), moves.len(), "{}: #{n} repeats a move", rom.display());
        }
        let lengths = |g: &ppr_model::GameData| g.egg_moves.values().map(Vec::len).collect::<Vec<_>>();
        assert_eq!(lengths(game), lengths(original.game_data()), "{}", rom.display());
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn tms_randomize_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.tms.random = true;
        settings.tms.keep_field_moves = true;
        settings.tms.good_damaging_percent = Some(50);
        settings.move_data.names = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(1)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        assert!(report.tms_changed > 30, "{}", rom.display());
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let tms = &game.tm_moves;
        let mut distinct = tms.clone();
        distinct.sort_unstable();
        distinct.dedup();
        assert_eq!(distinct.len(), tms.len(), "{}", rom.display());
        assert!(
            tms.iter()
                .all(|m| !game.hm_moves.contains(m) && !game.banned_level_up_moves.contains(m)),
            "{}: {tms:?}",
            rom.display()
        );
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn tutors_randomize_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.tms.random = true;
        settings.tutors.random = true;
        settings.tutors.keep_field_moves = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(2)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        if report.tutors == 0 {
            assert!(game.tutor_moves.is_empty(), "{}", rom.display());
            continue;
        }
        assert!(report.tutors_changed > 0, "{}", rom.display());
        assert_eq!(game.tutor_moves.len(), usize::try_from(report.tutors).expect("fits"));
        assert!(
            game.tutor_moves.iter().all(|m| !game.tm_moves.contains(m)),
            "{}",
            rom.display()
        );
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn tm_hm_compat_randomizes_and_reloads() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.tms.random = true;
        settings.tm_hm_compat.mode = ppr_service::CompatMode::RandomPreferSameType;
        settings.tm_hm_compat.level_up_sanity = true;
        settings.tm_hm_compat.follow_evolutions = true;
        settings.tm_hm_compat.full_hm = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(3)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        assert!(
            report.tm_hm_compat_changed as usize > most_of(game.species.len()),
            "{}",
            rom.display()
        );
        for (n, flags) in &game.tm_hm_compat {
            let tms = game.tm_moves.len();
            assert!(
                flags[tms..].iter().all(|&f| f),
                "{}: {n} learns every HM",
                rom.display()
            );
            for learnt in &game.learnsets[n] {
                if let Some(i) = game.tm_moves.iter().position(|&m| m == learnt.move_id) {
                    assert!(flags[i], "{}: {n} learns TM {} by level", rom.display(), i + 1);
                }
            }
        }
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn tutor_compat_randomizes_and_reloads() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.tutors.random = true;
        settings.tutor_compat.mode = ppr_service::CompatMode::RandomCompletely;
        settings.tutor_compat.level_up_sanity = true;
        settings.tutor_compat.follow_evolutions = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(4)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        if game.tutor_moves.is_empty() {
            assert_eq!(report.tutor_compat_changed, 0, "{}", rom.display());
            continue;
        }
        // Most species change with Gen 3's many tutors; with Crystal's three, one in eight keeps its flags.
        assert!(
            usize::try_from(report.tutor_compat_changed).expect("small") > game.tutor_compat.len() / 2,
            "{}: {}",
            rom.display(),
            report.tutor_compat_changed
        );
        for (n, flags) in &game.tutor_compat {
            for learnt in &game.learnsets[n] {
                if let Some(i) = game.tutor_moves.iter().position(|&m| m == learnt.move_id) {
                    assert!(flags[i], "{}: {n} learns tutor move {i} by level", rom.display());
                }
            }
        }
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn field_items_randomize_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let mut settings = Settings::default();
        settings.field_items.mode = ppr_service::FieldItemMode::Random;
        settings.field_items.ban_bad = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(5)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        assert!(report.field_items_changed > 150, "{}", rom.display());
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let is_tm = |id| game.items.iter().any(|i| i.id == id && i.tm);
        assert_eq!(game.field_items.len(), original.field_items.len(), "{}", rom.display());
        for (&old, &new) in original.field_items.iter().zip(&game.field_items) {
            assert_eq!(is_tm(old), is_tm(new), "{}", rom.display());
        }
        for tm in &game.required_field_tms {
            assert!(game.field_items.contains(tm), "{}: TM item {tm}", rom.display());
        }
        // Without balanced shop prices, each Mega Stone is placed once at most.
        for stone in &original.mega_stones {
            let placed = game.field_items.iter().filter(|&i| i == stone).count();
            assert!(
                placed <= 1,
                "{}: Mega Stone {stone} placed {placed} times",
                rom.display()
            );
        }
        if game.identity.generation.number() >= 6 {
            assert!(!original.mega_stones.is_empty(), "{}", rom.display());
        }
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn shops_randomize_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.shops.mode = ppr_service::ShopItemMode::Random;
        settings.shops.ban_overpowered = true;
        settings.shops.guarantee_evolution_items = true;
        settings.shops.balance_prices = true;
        settings.shops.add_cheap_rare_candies = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(6)),
            log: false,
            edits: None,
        };
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        if original.identity.generation.number() >= 6 {
            // The shops keep their size, so the Rare Candies only get cheap.
            let sizes = |g: &ppr_model::GameData| g.shops.iter().map(|s| s.items.len()).collect::<Vec<_>>();
            assert_eq!(
                sizes(game),
                sizes(&original),
                "{}: shops keep their size",
                rom.display()
            );
        } else {
            assert_eq!(
                report.shops_changed,
                report.shops,
                "{}: every shop sells Rare Candies",
                rom.display()
            );
        }
        assert_eq!(game.item_prices[&ppr_model::RARE_CANDY], 10, "{}", rom.display());
        let main: Vec<u16> = game
            .shops
            .iter()
            .filter(|s| s.special && s.main_game)
            .flat_map(|s| s.items.clone())
            .collect();
        for evo in &game.evolution_items {
            assert!(main.contains(evo), "{}: evolution item {evo}", rom.display());
        }
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn pickup_items_randomize() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.pickup.random = true;
        settings.pickup.ban_bad = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(7)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        if ppr_romio::open(&rom)
            .expect("loads")
            .game_data()
            .pickup_items
            .is_empty()
        {
            // Gen 1 has no Pickup.
            assert_eq!(report.pickup_items_changed, 0, "{}", rom.display());
            continue;
        }
        // The table is found by its first items, so it is not found again in the output.
        assert!(report.pickup_items >= 10, "{}", rom.display());
        assert!(report.pickup_items_changed > 5, "{}", rom.display());
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn type_effectiveness_randomizes_and_reloads() {
    let out_dir = out_dir();
    for rom in roms() {
        // The randomizers keep the number of non-neutral pairs of the updated chart.
        let mut settings = Settings::default();
        settings.type_effectiveness.update = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("updated"),
            settings,
            seed: Some(Seed(8)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let pairs = ppr_romio::open(&request.output)
            .expect("output loads")
            .game_data()
            .type_chart
            .non_neutral_count();
        for (mode, seed) in [
            (ppr_service::TypeEffectivenessMode::RandomBalanced, 8),
            (ppr_service::TypeEffectivenessMode::KeepIdentities, 9),
        ] {
            let mut settings = Settings::default();
            settings.type_effectiveness.mode = mode;
            settings.type_effectiveness.update = true;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(seed)),
                log: false,
                edits: None,
            };
            let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
            assert!(report.type_pairs_changed > 20, "{}: {mode:?}", rom.display());
            let saved = ppr_romio::open(&request.output).expect("output loads");
            assert_eq!(
                saved.game_data().type_chart.non_neutral_count(),
                pairs,
                "{}",
                rom.display()
            );
        }
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn palettes_randomize_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let generation = original.identity.generation.number();
        assert!(
            !(3..=5).contains(&generation) || !original.palettes.is_empty(),
            "{}: Gen 3, 4 and 5 games have palettes",
            rom.display()
        );
        if original.palettes.is_empty() {
            continue;
        }
        let mut settings = Settings::default();
        settings.palettes.random = true;
        // Gen 1's ten palettes fit the types so well that many species would keep theirs.
        settings.palettes.follow_types = original.identity.generation.number() > 1;
        settings.palettes.follow_evolutions = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(7)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let after = &saved.game_data().palettes;
        let changed = original
            .palettes
            .iter()
            .filter(|(n, p)| after.get(n) != Some(p))
            .count();
        assert!(
            changed > most_of(original.palettes.len()),
            "{}: {changed} of {} palettes changed",
            rom.display(),
            original.palettes.len()
        );
        tested += 1;
    }
    if tested == 0 {
        skipped("no game with palettes in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn player_graphics_are_written_and_reload() {
    let out_dir = tempfile::tempdir().expect("temp dir");
    let mut tested = 0;
    for rom in roms() {
        let loaded = load_rom(&rom).expect("loads");
        let Some(pack) = loaded.player_graphics_packs.first() else {
            continue;
        };
        let mut settings = Settings::default();
        settings.player_graphics.pack = Some(pack.pack.id.clone());
        if loaded.multiple_player_characters {
            settings.player_graphics.replace = ppr_model::PlayerCharacter::Pc2;
        }
        let request = RandomizeRequest {
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(7)),
            log: false,
            update: None,
            layered_fs: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let original = ppr_romio::open(&rom).expect("loads");
        assert_eq!(
            saved.game_data().species,
            original.game_data().species,
            "{}",
            rom.display()
        );
        assert_ne!(
            std::fs::read(&request.output).expect("output"),
            std::fs::read(&rom).expect("input"),
            "{}: the graphics are written",
            rom.display()
        );
        tested += 1;
    }
    if tested == 0 {
        skipped("no game with player graphics packs in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn double_battles_are_written_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.battle_style.mode = ppr_service::BattleStyleMode::SingleStyle;
        settings.battle_style.style = ppr_service::BattleStyle::Double;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(10)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        if saved.game_data().identity.generation.number() < 3 {
            // Only single battles before Gen 3.
            assert_eq!(report.trainers_changed, 0, "{}", rom.display());
            continue;
        }
        assert!(report.trainers_changed > 100, "{}", rom.display());
        let game = saved.game_data();
        let changeable = game.trainers.iter().filter(|t| {
            t.multi_battle != ppr_model::MultiBattleStatus::Always
                && !t.should_not_get_buffs()
                && !in_tournament(game, t)
        });
        // Emerald's Mossdeep Steven is no trainer to the game, so he has no style. The five partners of the player
        // in Diamond, Pearl and Platinum keep theirs, or their wild double battles would break.
        let name = &game.identity.name;
        let skip = if name.contains("Emerald") {
            1
        } else if ["Diamond", "Pearl", "Platinum"].iter().any(|g| name.contains(g)) {
            5
        } else {
            0
        };
        let singles: Vec<u16> = changeable
            .filter(|t| t.battle_style != ppr_service::BattleStyle::Double || t.pokemon.len() < 2)
            .map(|t| t.index)
            .collect();
        assert!(singles.len() <= skip, "{}: {singles:?}", rom.display());
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn trainer_levels_and_evolutions_are_written_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let mut settings = Settings::default();
        settings.trainers.level_modifier = Some(50);
        settings.trainers.evolve = Some(0);
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(11)),
            log: false,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        assert!(report.trainers_changed > 100, "{}", rom.display());
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let mut evolved = 0;
        for (before, after) in original.trainers.iter().zip(&saved.game_data().trainers) {
            assert_eq!(before.pokemon.len(), after.pokemon.len(), "{}", rom.display());
            for (b, a) in before.pokemon.iter().zip(&after.pokemon) {
                if !before.is_first_rival_or_friend() && !in_tournament(&original, before) {
                    let expected = (f64::from(b.level) * 1.5 + 0.5).floor().min(100.0);
                    assert_eq!(
                        f64::from(a.level),
                        expected,
                        "{}: trainer {}",
                        rom.display(),
                        before.index
                    );
                }
                evolved += usize::from(a.species != b.species);
            }
        }
        assert!(evolved > 100, "{}: {evolved}", rom.display());
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn trainer_pokemon_are_written_and_reload() {
    use ppr_service::TrainerMode;
    let out_dir = out_dir();
    let modes = [
        TrainerMode::Random,
        TrainerMode::RandomEvenDistribution,
        TrainerMode::TypeThemed,
        TrainerMode::TypeThemedEliteFourGyms,
        TrainerMode::KeepThemed,
        TrainerMode::KeepThemesOrPrimary,
    ];
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        for (k, mode) in modes.into_iter().enumerate() {
            let mut settings = Settings::default();
            let s = &mut settings.trainers;
            s.mode = mode;
            s.rival_carries_starter = true;
            s.similar_strength = k % 2 == 0;
            s.avoid_duplicates = true;
            s.no_early_wonder_guard = true;
            s.local_pokemon = k == 1;
            s.elite_four_unique = Some(2);
            s.additional_pokemon = [2, 1, 1];
            s.diverse_types = [false, false, k == 0];
            s.evolve = (k == 3).then_some(0);
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(20 + k as u64)),
                log: false,
                edits: None,
            };
            let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {mode:?}: {e}", rom.display()));
            assert!(report.trainers_changed > 300, "{}: {mode:?}", rom.display());
            let saved = ppr_romio::open(&request.output).expect("output loads");
            let game = saved.game_data();
            let mut changed = 0;
            for (before, after) in original.trainers.iter().zip(&game.trainers) {
                assert!(
                    after.pokemon.len() >= before.pokemon.len(),
                    "{}: {mode:?}",
                    rom.display()
                );
                let custom = after.pokemon.iter().any(|p| p.moves.iter().any(|&m| m != 0));
                for p in &after.pokemon {
                    if !before.pokemon.iter().any(|b| b.species == p.species) {
                        changed += 1;
                        if custom && after.index as usize <= original.trainers.len() {
                            assert_eq!(
                                p.moves,
                                game.moves_at_level(p.species, p.level),
                                "{}: {mode:?}",
                                rom.display()
                            );
                        }
                    }
                }
            }
            assert!(changed > 500, "{}: {mode:?}: {changed}", rom.display());
        }
    }
}

/// Random shiny trainer Pokémon in Gen 7: the flags the randomizer sets are the ones the saved game has, and the
/// game's own shiny Pokémon stay shiny.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn shiny_trainer_pokemon_are_written_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        if original.identity.generation.number() != 7 {
            continue;
        }
        let mut settings = Settings::default();
        settings.trainers.mode = ppr_service::TrainerMode::Random;
        settings.trainers.random_shiny = true;
        let seed = Seed(256);
        let mut expected = ppr_random::RunData {
            game: original.clone(),
            settings: settings.clone(),
        };
        ppr_random::standard_pipeline()
            .run(&mut expected, seed, &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(seed),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let flags = |g: &ppr_model::GameData| -> Vec<bool> {
            g.trainers
                .iter()
                .flat_map(|t| t.pokemon.iter().map(|p| p.shiny))
                .collect()
        };
        let (before, wanted, after) = (flags(&original), flags(&expected.game), flags(saved.game_data()));
        assert_eq!(after, wanted, "{}", rom.display());
        assert!(
            before.iter().zip(&after).all(|(&b, &a)| !b || a),
            "{}: the game's shiny Pokémon stay shiny",
            rom.display()
        );
        // One in 256 of a few thousand Pokémon.
        let new = after.iter().filter(|&&s| s).count() - before.iter().filter(|&&s| s).count();
        eprintln!("{}: {new} new shiny of {}", rom.display(), after.len());
        assert!(
            (3..=40).contains(&new),
            "{}: {new} new shiny of {}",
            rom.display(),
            after.len()
        );
        tested += 1;
    }
    if tested == 0 {
        skipped("no Gen 7 game in PPR_TEST_ROMS");
    }
}

/// Whether `tp` holds the Mega Stone of one of its species' Mega Evolutions.
fn can_mega_evolve(game: &ppr_model::GameData, tp: &ppr_model::TrainerPokemon) -> bool {
    tp.held_item.is_some()
        && game
            .mega_evolutions()
            .any(|m| m.forme.base == Some(tp.species) && m.forme.mega_item == tp.held_item)
}

/// "Swap Mega Evolvables" on X and Ultra Sun: every trainer Pokémon that could Mega Evolve still can, with another
/// species for most, the saved game has what the randomizer chose, and random held items leave their stones alone.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn swapped_trainer_mega_evolvables_are_written_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let name = rom.to_string_lossy();
        if !name.contains("Pokemon X") && !name.contains("Ultra Sun") {
            continue;
        }
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let megas: Vec<(usize, usize)> = original
            .trainers
            .iter()
            .enumerate()
            .flat_map(|(i, t)| t.pokemon.iter().enumerate().map(move |(p, tp)| (i, p, tp)))
            .filter(|&(_, _, tp)| can_mega_evolve(&original, tp))
            .map(|(i, p, _)| (i, p))
            .collect();
        assert!(megas.len() >= 5, "{}: {} Mega Evolvables", rom.display(), megas.len());
        let mut settings = Settings::default();
        settings.trainers.mode = ppr_service::TrainerMode::Random;
        settings.trainers.swap_mega_evos = true;
        settings.trainers.held_items = [true; 3];
        let seed = Seed(167);
        let mut expected = ppr_random::RunData {
            game: original.clone(),
            settings: settings.clone(),
        };
        ppr_random::standard_pipeline()
            .run(&mut expected, seed, &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        let request = RandomizeRequest {
            update: None,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(seed),
            log: false,
            edits: None,
            layered_fs: false,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let held = |g: &ppr_model::GameData| -> Vec<(u16, Option<u16>)> {
            g.trainers
                .iter()
                .flat_map(|t| t.pokemon.iter().map(|p| (p.species, p.held_item)))
                .collect()
        };
        assert_eq!(held(game), held(&expected.game), "{}", rom.display());
        let mut swapped = 0;
        for &(i, p) in &megas {
            let (before, after) = (&original.trainers[i].pokemon[p], &game.trainers[i].pokemon[p]);
            assert!(
                can_mega_evolve(game, after),
                "{}: trainer {} holds {:?} with {}",
                rom.display(),
                original.trainers[i].index,
                after.held_item,
                after.species
            );
            swapped += usize::from(before.species != after.species);
        }
        eprintln!(
            "{}: {swapped} of {} Mega Evolvables swapped",
            rom.display(),
            megas.len()
        );
        assert!(
            swapped * 2 > megas.len(),
            "{}: {swapped} of {}",
            rom.display(),
            megas.len()
        );
        tested += 1;
    }
    if tested == 0 {
        skipped("neither X nor Ultra Sun in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn trainer_held_items_are_written_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let pools = &original.held_item_pools;
        if pools.all.is_empty() {
            // Gen 1 Pokémon hold no items.
            continue;
        }
        let name = |id: u16| original.items.iter().find(|i| i.id == id).map(|i| i.name.as_str());
        // The standard item numbers are these items.
        let expected_names: &[(u16, &str)] = if original.identity.generation.number() == 2 {
            &[
                (213, "BRIGHTPOWDER"),
                (234, "LEFTOVERS"),
                (249, "CHARCOAL"),
                (43, "BERRY JUICE"),
            ]
        } else {
            &[
                (220, "CHOICE BAND"),
                (234, "LEFTOVERS"),
                (201, "LIECHI BERRY"),
                (43, "BERRY JUICE"),
            ]
        };
        // Names are in capitals before Gen 4.
        for &(id, expected) in expected_names {
            assert_eq!(
                name(id).map(str::to_uppercase).as_deref(),
                Some(expected),
                "{}",
                rom.display()
            );
        }
        assert!(pools.all.iter().all(|&i| name(i).is_some()), "{}", rom.display());
        for (k, (consumable, sensible, highest)) in [(false, false, false), (true, true, false), (false, true, true)]
            .into_iter()
            .enumerate()
        {
            let mut settings = Settings::default();
            let s = &mut settings.trainers;
            s.held_items = [true, true, k != 2];
            s.consumable_items_only = consumable;
            s.sensible_items_only = sensible;
            s.highest_level_gets_item = highest;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(40 + k as u64)),
                log: false,
                edits: None,
            };
            randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {k}: {e}", rom.display()));
            let saved = ppr_romio::open(&request.output).expect("output loads");
            let game = saved.game_data();
            let mut allowed = if consumable {
                pools.consumable.clone()
            } else {
                pools.all.clone()
            };
            if sensible && !consumable {
                // Sensible items include those for one species, like Gen 2's Light Ball for Pikachu.
                allowed.extend(pools.species_boosting.values().flatten());
            }
            let mut with_items = 0;
            for (before, after) in original.trainers.iter().zip(&game.trainers) {
                assert_eq!(before.pokemon.len(), after.pokemon.len());
                for (b, a) in before.pokemon.iter().zip(&after.pokemon) {
                    assert_eq!((b.species, b.level, b.moves), (a.species, a.level, a.moves));
                    match a.held_item {
                        Some(i) if a.held_item != b.held_item => {
                            assert!(allowed.contains(&i), "{}: {k}: {i}", rom.display());
                            with_items += 1;
                        }
                        _ => {}
                    }
                }
                let given = after
                    .pokemon
                    .iter()
                    .zip(&before.pokemon)
                    .filter(|(a, b)| a.held_item != b.held_item);
                if highest && !after.should_not_get_buffs() {
                    assert!(given.count() <= 1, "{}: {k}: {}", rom.display(), after.index);
                }
            }
            // Crystal has no room for items of regular trainers.
            let regular = original.trainer_held_item_additions[2] && k != 2;
            let least = if regular { 200 } else { 20 };
            assert!(with_items > least, "{}: {k}: {with_items}", rom.display());
        }
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn trainer_names_are_written_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        let mut settings = Settings::default();
        settings.trainers.random_names = true;
        settings.trainers.random_class_names = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(50)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let changed = |a: &[String], b: &[String]| a.iter().zip(b).filter(|(a, b)| a != b).count();
        let classes = changed(&original.trainer_class_names, &game.trainer_class_names);
        let least = original.trainer_class_names.len() - original.personal_trainer_classes.len();
        // Gen 1 class names keep their length, so some find no other name.
        assert!(classes > least / 2, "{}: {classes} class names changed", rom.display());
        assert!(
            game.trainer_class_names
                .iter()
                .all(|c| c.chars().count() <= original.max_trainer_class_name_length),
            "{}",
            rom.display()
        );
        // Gen 1 trainers have no names; the classes named after a person, like the gym leaders, count as theirs.
        let names = |g: &ppr_model::GameData| {
            g.trainers
                .iter()
                .map(|t| t.name.clone())
                .chain(
                    g.personal_trainer_classes
                        .iter()
                        .map(|&c| g.trainer_class_names[usize::from(c)].clone()),
                )
                .collect::<Vec<_>>()
        };
        let total = names(&original).iter().filter(|n| !n.is_empty()).count();
        let renamed = changed(&names(&original), &names(game));
        assert!(
            renamed > most_of(total),
            "{}: {renamed} of {total} trainers renamed",
            rom.display()
        );
        // New names follow the game's case: capitals before Gen 4, mixed case after.
        let capitals = original.trainers.iter().all(|t| t.name == t.name.to_uppercase());
        for (before, after) in original.trainers.iter().zip(&game.trainers) {
            assert_eq!(before.pokemon, after.pokemon, "{}", rom.display());
            if capitals {
                assert_eq!(after.name, after.name.to_uppercase(), "{}", rom.display());
            }
        }
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn better_trainer_movesets_are_written_and_reload() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        for (k, mode) in [ppr_service::TrainerMode::Unchanged, ppr_service::TrainerMode::Random]
            .into_iter()
            .enumerate()
        {
            let mut settings = Settings::default();
            settings.trainers.mode = mode;
            settings.trainers.better_movesets = [true, true, true];
            settings.trainers.held_items = [true, true, true];
            settings.trainers.sensible_items_only = true;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(60 + k as u64)),
                log: false,
                edits: None,
            };
            randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {k}: {e}", rom.display()));
            let saved = ppr_romio::open(&request.output).expect("output loads");
            let game = saved.game_data();
            if !original.trainer_custom_movesets.contains(&true) {
                // Gen 1 trainer Pokémon have no moves of their own.
                for (before, after) in original.trainers.iter().zip(&game.trainers) {
                    let moves = |t: &ppr_model::Trainer| t.pokemon.iter().map(|p| p.moves).collect::<Vec<_>>();
                    assert_eq!(moves(before), moves(after), "{}", rom.display());
                }
                continue;
            }
            let mut custom = 0;
            for (before, after) in original.trainers.iter().zip(&game.trainers) {
                assert_eq!(before.pokemon.len(), after.pokemon.len(), "{}", rom.display());
                // Gen 2 regular trainers keep their moves: their banks have no room for moves of their own.
                let tier = if after.is_boss() {
                    0
                } else if after.is_important() {
                    1
                } else {
                    2
                };
                if after.should_not_get_buffs() || !original.trainer_custom_movesets[tier] {
                    continue;
                }
                for p in &after.pokemon {
                    let moves: Vec<_> = p.moves.iter().filter(|&&m| m != 0).collect();
                    assert!(!moves.is_empty(), "{}: {} has no moves", rom.display(), after.index);
                    let unique: std::collections::BTreeSet<_> = moves.iter().collect();
                    assert_eq!(unique.len(), moves.len(), "{}: {:?}", rom.display(), p.moves);
                    if moves.len() == 4 {
                        custom += 1;
                    }
                }
            }
            let least = if original.trainer_custom_movesets[2] { 500 } else { 100 };
            assert!(custom > least, "{}: {k}: {custom}", rom.display());
        }
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn metronome_only_mode_is_written_and_reloads() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.movesets.mode = ppr_service::MovesetMode::MetronomeOnly;
        settings.trainers.mode = ppr_service::TrainerMode::Random;
        settings.trainers.better_movesets = [true, true, true];
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(70)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        for (species, learnset) in &game.learnsets {
            assert!(
                learnset.is_empty() || learnset.iter().all(|l| l.move_id == 118 && l.level == 1),
                "{}: {species}: {learnset:?}",
                rom.display()
            );
        }
        assert!(game.tm_moves.iter().all(|&m| m == 118), "{}", rom.display());
        assert!(game.tutor_moves.iter().all(|&m| m == 118), "{}", rom.display());
        let metronome = &game.moves[117];
        assert_eq!((metronome.number, metronome.pp), (118, 40), "{}", rom.display());
        for &hm in &game.hm_moves {
            assert_eq!(game.moves[usize::from(hm) - 1].pp, 0, "{}", rom.display());
        }
        assert!(
            game.trainers
                .iter()
                .flat_map(|t| &t.pokemon)
                .all(|p| p.moves == [0; 4] || p.moves == game.moves_at_level(p.species, p.level)),
            "{}",
            rom.display()
        );
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn no_premature_evolutions_on_real_games() {
    let out_dir = out_dir();
    for rom in roms() {
        let mut settings = Settings::default();
        settings.general.no_premature_evolutions = true;
        settings.wild.mode = WildMode::Random;
        // Per slot: zones map a species once for every level it appears at.
        settings.wild.zones = WildZones::Slot;
        settings.trainers.mode = ppr_service::TrainerMode::TypeThemed;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(80)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let legal = |n: u16, level: u8| {
            let to: Vec<_> = game.evolutions.iter().filter(|e| e.to == n).collect();
            to.is_empty() || to.iter().any(|e| u16::from(level) >= e.estimated_level)
        };
        for set in &game.encounters {
            for e in &set.encounters {
                assert!(
                    legal(e.species, e.min_level),
                    "{}: wild #{} at {}",
                    rom.display(),
                    e.species,
                    e.min_level
                );
            }
        }
        for p in game.trainers.iter().flat_map(|t| &t.pokemon) {
            assert!(
                legal(p.species, p.level),
                "{}: trainer #{} at {}",
                rom.display(),
                p.species,
                p.level
            );
        }
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn limit_pokemon_on_real_games() {
    let out_dir = out_dir();
    for rom in roms() {
        for relatives in [false, true] {
            let mut settings = Settings::default();
            settings.general.limit_pokemon = Some(ppr_service::GenerationLimit {
                generations: [true, false, false, false, false, false, false],
                allow_evolutionary_relatives: relatives,
            });
            settings.general.random_intro_pokemon = true;
            settings.starters.mode = StarterMode::Random;
            settings.wild.mode = WildMode::Random;
            settings.trainers.mode = ppr_service::TrainerMode::Random;
            settings.statics.mode = ppr_service::StaticMode::Random;
            settings.trades.mode = ppr_service::TradeMode::GivenAndRequested;
            let request = RandomizeRequest {
                update: None,
                layered_fs: false,
                input: rom.clone(),
                output: out_dir.path().join("out"),
                settings,
                seed: Some(Seed(81)),
                log: false,
                edits: None,
            };
            randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
                .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
            let original = ppr_romio::open(&rom).expect("input loads");
            let original = original.game_data();
            let saved = ppr_romio::open(&request.output).expect("output loads");
            let game = saved.game_data();
            // Gen 1, and with relatives Pichu, Cleffa, Igglybuff, Smoochum, Elekid, Magby, Azurill and the
            // Gen 2 evolutions of Gen 1 Pokémon.
            let allowed = |n: u16| n <= 151 || (relatives && original.family(n).iter().any(|&m| m <= 151));
            let mut seen: Vec<(&str, u16)> = Vec::new();
            seen.extend(game.starters.iter().map(|&n| ("starter", n)));
            // Crystal's unused fishing sets are left alone.
            seen.extend(
                game.encounters
                    .iter()
                    .filter(|s| s.kind != ppr_model::EncounterKind::Unused)
                    .flat_map(|s| &s.encounters)
                    .map(|e| ("wild", e.species)),
            );
            seen.extend(
                game.trainers
                    .iter()
                    .flat_map(|t| &t.pokemon)
                    .map(|p| ("trainer", p.species)),
            );
            seen.extend(game.statics.iter().map(|s| ("static", s.species)));
            seen.extend(game.trades.iter().map(|t| ("trade", t.given)));
            seen.extend(game.trades.iter().filter_map(|t| t.requested).map(|n| ("request", n)));
            seen.extend(game.intro_pokemon.map(|n| ("intro", n)));
            for (what, n) in seen {
                assert!(allowed(n), "{} (relatives {relatives}): {what} #{n}", rom.display());
            }
            // The evolutions without a method (Burmy into Wormadam's other Cloaks) are the game's own.
            for e in game
                .evolutions
                .iter()
                .filter(|e| e.kind != ppr_model::EvolutionType::None)
            {
                assert!(allowed(e.from) && allowed(e.to), "{}: evolution {e:?}", rom.display());
            }
            if !relatives {
                // Pikachu no longer evolves from Pichu.
                assert!(!game.evolutions.iter().any(|e| e.to == 25), "{}", rom.display());
            }
        }
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn check_values_match_reference() {
    // The check values of the unrandomized dumps, by game code and revision.
    let known = [
        ("BPRE", 0, 0x285C_CECB_u32),
        ("BPEE", 0, 0xA70E_6AF7),
        ("AXVE", 1, 0xCA55_D440),
    ];
    let mut tested = 0;
    for rom in roms() {
        let handler = ppr_romio::open(&rom).expect("loads");
        let game = handler.game_data();
        let id = &game.identity;
        eprintln!(
            "{} {} {:?} {:08X}",
            rom.display(),
            id.code,
            id.version,
            ppr_service::check_value(game)
        );
        if let Some(&(_, _, expected)) = known
            .iter()
            .find(|(code, version, _)| id.code == *code && id.version.unwrap_or(0) == *version)
        {
            assert_eq!(ppr_service::check_value(game), expected, "{}", rom.display());
            tested += 1;
        }
    }
    if tested == 0 {
        skipped("no dump with a known check value in PPR_TEST_ROMS");
    }
}

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn misc_tweaks_on_real_games() {
    let out_dir = out_dir();
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("input loads");
        let original = original.game_data();
        assert!(original.available_tweaks.contains(&ppr_service::MiscTweak::FastestText));
        let mut settings = Settings::default();
        settings.misc_tweaks.clone_from(&original.available_tweaks);
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(Seed(82)),
            log: false,
            edits: None,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        let bulbasaur = game.species(1).expect("Bulbasaur");
        assert_eq!(bulbasaur.name, "Bulbasaur", "{}", rom.display());
        // Not every generation can balance the static levels.
        if original
            .available_tweaks
            .contains(&ppr_service::MiscTweak::BalanceStaticLevels)
        {
            let fossils: Vec<u8> = game.fossils.iter().map(|&i| game.statics[i].level).collect();
            assert!(fossils.iter().all(|&l| l == 30), "{}: {fossils:?}", rom.display());
        }
        // The rest is checked byte for byte elsewhere; here it must at least load again.
        assert_eq!(game.species.len(), original.species.len());
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn logs_name_everything_that_changed() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let mut settings = Settings::default();
        settings.starters.mode = StarterMode::Random;
        settings.evolutions.mode = ppr_service::EvolutionMode::Random;
        settings.movesets.mode = ppr_service::MovesetMode::RandomCompletely;
        settings.type_effectiveness.mode = ppr_service::TypeEffectivenessMode::Random;
        settings.tms.random = true;
        settings.tm_hm_compat.mode = ppr_service::CompatMode::RandomCompletely;
        settings.tutors.random = true;
        settings.wild.mode = WildMode::Random;
        settings.trainers.mode = ppr_service::TrainerMode::Random;
        settings.trades.mode = ppr_service::TradeMode::GivenAndRequested;
        settings.statics.mode = ppr_service::StaticMode::Random;
        settings.shops.mode = ppr_service::ShopItemMode::Random;
        settings.pickup.random = true;
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input: rom.clone(),
            output: out_dir.path().join("out.gba"),
            settings,
            seed: Some(Seed(11)),
            log: true,
            edits: None,
        };
        let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let log = report.log.expect("a log");
        for shortcut in [
            "PKEV", "SRPK", "STPK", "IGTR", "PKMV", "LVLC", "TRPK", "WDPK", "TMMV", "TMCB", "MTMV", "SHMS", "PUMS",
            "TPEF",
        ] {
            // Once in the table of contents and once as the section's title; Ruby has no move tutors.
            // Gen 1 has no Pickup either.
            let missing =
                (shortcut == "MTMV" && report.tutors == 0) || (shortcut == "PUMS" && report.pickup_items == 0);
            let expected = if missing { 0 } else { 2 };
            let found = log.matches(&format!("{{{shortcut}}}")).count();
            assert_eq!(found, expected, "{}: {shortcut}", rom.display());
        }
        // Every species, item and move resolves to its name; Gen 6 and Gen 7 evolution methods speak of a
        // "Fairy-type move".
        let log = log.replace("-type move", "-type");
        for unresolved in ["|#", " item ", " move "] {
            let line = log.lines().find(|l| l.contains(unresolved));
            assert!(line.is_none(), "{}: {unresolved:?} in the log: {line:?}", rom.display());
        }
        tested += 1;
    }
    assert!(tested > 0, "no supported ROMs in PPR_TEST_ROMS");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn capabilities_follow_the_game() {
    for rom in roms() {
        let loaded = load_rom(&rom).expect("loads");
        let name = &loaded.identity.name;
        // Diamond and Pearl are the only games from Platinum on without tutors.
        let tutors = [
            "Emerald",
            "Fire",
            "Leaf",
            "Crystal",
            "Platinum",
            "HeartGold",
            "SoulSilver",
            "Black 2",
            "White 2",
            "Omega Ruby",
            "Alpha Sapphire",
            "Ultra Sun",
            "Ultra Moon",
        ]
        .iter()
        .any(|g| name.starts_with(g));
        assert_eq!(loaded.capabilities.move_tutors, tutors, "{name}");
        // Gen 1 codes are titles like `POKEMON YELLOW`; English games are named `(U)`.
        let english = if loaded.identity.generation.number() == 1 {
            name.contains("(U)")
        } else if loaded.identity.generation.number() == 5 {
            // One English release for America and Europe.
            loaded.identity.code.ends_with('O')
        } else if matches!(loaded.identity.generation.number(), 6 | 7) {
            // Every 3DS release has English text.
            true
        } else {
            loaded.identity.code.ends_with('E')
        };
        assert_eq!(loaded.capabilities.english, english, "{name}");
        assert_eq!(
            loaded.capabilities.totem_pokemon,
            loaded.identity.generation.number() == 7,
            "{name}"
        );
    }
}

/// With a game update, X is saved as a LayeredFS folder named after its title ID, with only changed files at
/// RomFS paths of the game; the log names the update. The update is made from X itself, as no real one is at hand.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn x_with_an_update_saves_a_layered_fs_folder() {
    use ppr_formats::ctr::testing::{Cxi, cia, cxi};

    let Some(rom) = roms().into_iter().find(|p| p.to_string_lossy().contains("Pokemon X")) else {
        return skipped("no X in PPR_TEST_ROMS");
    };
    let out_dir = out_dir();
    let file = std::io::BufReader::new(std::fs::File::open(&rom).expect("readable"));
    let mut base = ppr_romio::ctr::open(file).expect("opens");
    let code = base.code().expect("code").to_vec();
    let battle = base.read_file("DllBattle.cro").expect("battle module");
    let names: Vec<String> = base.file_names().map(str::to_owned).collect();
    let update_path = out_dir.path().join("x-update.cia");
    let update = cia(
        &cxi(&Cxi {
            product_code: "CTR-P-EKJA",
            title_id: 0x0004_000E_0005_5D00,
            code: &code,
            compress_code: true,
            files: &[("DllBattle.cro", &battle)],
            decrypted: true,
        }),
        5232,
    );
    std::fs::write(&update_path, update).expect("writes");

    let loaded = ppr_service::load_rom_with_update(&rom, Some(&update_path)).expect("loads");
    assert_eq!(
        loaded.update_version.as_deref(),
        Some("Unpatched"),
        "the title screen shows no version"
    );
    let mut settings = Settings::default();
    settings.starters.mode = StarterMode::Random;
    let output = out_dir.path().join("out");
    let request = RandomizeRequest {
        input: rom.clone(),
        update: Some(update_path),
        layered_fs: false,
        output: output.clone(),
        settings,
        seed: Some(Seed(1)),
        log: true,
        edits: None,
    };
    let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()).expect("randomizes");
    let root = output.join("0004000000055D00");
    assert_eq!(report.output, root);
    let top: Vec<_> = std::fs::read_dir(&output)
        .expect("written")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(top, ["0004000000055D00"], "no temporary folder left");
    let mut files = Vec::new();
    let mut stack = vec![root.join("romfs")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(dir).expect("readable") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(root.join("romfs")).expect("inside");
                files.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    assert!(!files.is_empty(), "the starters changed");
    assert!(files.iter().all(|f| names.contains(f)), "{files:?}");
    let log = report.log.expect("a log");
    assert!(
        log.contains("Game update: x-update.cia (Unpatched)"),
        "the log names the update"
    );
}

/// Without a game update, Ultra Sun can be saved as a LayeredFS folder too: named after its title ID, it holds only
/// changed files at RomFS paths of the game, each the same as in the full ROM saved with the same seed and settings,
/// and `code.bin` when the code changed.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn ultra_sun_saves_a_layered_fs_folder_without_an_update() {
    let Some(rom) = roms().into_iter().find(|p| p.to_string_lossy().contains("Ultra Sun")) else {
        return skipped("no Ultra Sun in PPR_TEST_ROMS");
    };
    let out_dir = out_dir();
    let mut settings = Settings::default();
    settings.starters.mode = StarterMode::Random;
    settings.wild.mode = WildMode::Random;
    let output = out_dir.path().join("out");
    let request = RandomizeRequest {
        input: rom.clone(),
        update: None,
        layered_fs: true,
        output: output.clone(),
        settings,
        seed: Some(Seed(153)),
        log: false,
        edits: None,
    };
    let report = randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default()).expect("randomizes");
    let title_id = {
        let file = std::io::BufReader::new(std::fs::File::open(&rom).expect("readable"));
        ppr_romio::ctr::open(file).expect("opens").identity().title_id.clone()
    };
    let root = output.join(&title_id);
    assert_eq!(report.output, root);
    let top: Vec<_> = std::fs::read_dir(&output)
        .expect("written")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(top, [title_id.as_str()], "no temporary folder left");

    // The same run saved as a full ROM, to compare the files with.
    let full = RandomizeRequest {
        layered_fs: false,
        output: out_dir.path().join("full.3ds"),
        ..request.clone()
    };
    randomize_file(&full, &mut |_: Progress| {}, &CancelToken::default()).expect("randomizes");
    let mut saved = ppr_romio::ctr::open(std::io::BufReader::new(
        std::fs::File::open(&full.output).expect("readable"),
    ))
    .expect("opens");
    let mut base =
        ppr_romio::ctr::open(std::io::BufReader::new(std::fs::File::open(&rom).expect("readable"))).expect("opens");

    let mut files = Vec::new();
    let mut stack = vec![root.join("romfs")];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(dir).expect("readable") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                let rel = p.strip_prefix(root.join("romfs")).expect("inside");
                files.push((rel.to_string_lossy().replace('\\', "/"), p));
            }
        }
    }
    assert!(!files.is_empty(), "the starters and wild Pokémon changed");
    let mut changed = 0;
    for (name, path) in &files {
        assert!(base.has_file(name), "{name} is a file of the game");
        let written = std::fs::read(path).expect("readable");
        assert_eq!(written, saved.read_file(name).expect("in the full ROM"), "{name}");
        if written != base.read_file(name).expect("in the game") {
            changed += 1;
        }
    }
    assert!(changed > 0, "the folder holds changes");
    let code = root.join("code.bin");
    if code.exists() {
        assert_eq!(std::fs::read(&code).expect("readable"), saved.code().expect("code"));
    } else {
        assert_eq!(
            saved.code().expect("code"),
            base.code().expect("code"),
            "unchanged code is left out"
        );
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn editable_views_are_valid_edits_that_change_nothing() {
    use ppr_model::Edit;
    let mut failures = Vec::new();
    for rom in roms() {
        let (_, session) = ppr_service::RomSession::open(&rom, None).expect("opens");
        let session = session.expect("ported");
        let view = session.editable();
        let mut list = ppr_service::EditList::new(&session.game().identity);
        list.edits.extend(view.starters.iter().cloned().map(Edit::Starter));
        list.edits
            .extend(view.starter_held_items.iter().cloned().map(Edit::StarterHeldItem));
        list.edits
            .extend(view.species.iter().map(|s| Edit::Species(s.value.clone())));
        list.edits
            .extend(view.trainers.iter().map(|t| Edit::Trainer(t.value.clone())));
        list.edits
            .extend(view.wild_sets.iter().map(|w| Edit::WildSet(w.value.clone())));
        list.edits
            .extend(view.statics.iter().map(|s| Edit::Static(s.value.clone())));
        list.edits.extend(view.trades.iter().cloned().map(Edit::Trade));
        list.edits
            .extend(view.shops.iter().map(|s| Edit::Shop(s.value.clone())));
        list.edits.extend(view.item_prices.iter().cloned().map(Edit::ItemPrice));
        list.edits.extend(view.field_items.iter().cloned().map(Edit::FieldItem));
        let issues = session.validate(&list).expect("this game");
        if !issues.is_empty() {
            let mut kinds: Vec<_> = issues.iter().map(|i| i.message.clone()).collect();
            kinds.dedup();
            failures.push(format!("{}: {:?}", rom.display(), &kinds[..kinds.len().min(8)]));
            continue;
        }
        let mut game = session.game().clone();
        list.apply(&mut game).expect("applies");
        if ppr_service::editable_game(&game, session.rules()) != view {
            failures.push(format!("{}: changed", rom.display()));
        }
        if view.trainers.iter().any(|t| t.class_name.is_empty()) {
            failures.push(format!("{}: a trainer class without a name", rom.display()));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn emerald_runs_with_manual_edits() {
    use ppr_model::{Edit, ItemPriceEdit, ShopEdit, StarterEdit, TradeEdit};
    let Some(rom) = roms()
        .into_iter()
        .find(|p| load_rom(p).is_ok_and(|r| r.identity.name == "Emerald (U)"))
    else {
        return skipped("no Emerald in PPR_TEST_ROMS");
    };
    let out_dir = out_dir();
    let (_, session) = ppr_service::RomSession::open(&rom, None).expect("opens");
    let mut session = session.expect("ported");
    let view = session.editable();
    let mut list = ppr_service::EditList::new(&session.game().identity);
    list.set(Edit::Starter(StarterEdit { slot: 1, species: 25 }));
    let leader = view
        .trainers
        .iter()
        .find(|t| t.tag.as_deref() == Some("GYM1-LEADER"))
        .expect("Roxanne");
    let mut team = leader.value.clone();
    team.pokemon[0].species = 150;
    team.pokemon[0].level = 70;
    list.set(Edit::Trainer(team.clone()));
    let mut wild = view.wild_sets[0].value.clone();
    for slot in &mut wild.slots {
        slot.species = 133;
    }
    list.set(Edit::WildSet(wild.clone()));
    list.set(Edit::Shop(ShopEdit {
        index: 2,
        items: vec![1, 2, 3],
    }));
    list.set(Edit::ItemPrice(ItemPriceEdit { item: 4, price: 777 }));
    let trade: TradeEdit = TradeEdit {
        nickname: "SPUD".into(),
        ..view.trades[0].clone()
    };
    list.set(Edit::Trade(trade));
    assert_eq!(session.set_edits(list.clone()).expect("this game"), []);

    let mut settings = Settings::default();
    settings.starters.mode = StarterMode::Random;
    settings.wild.mode = WildMode::Random;
    let request = RandomizeRequest {
        input: rom.clone(),
        update: None,
        layered_fs: false,
        output: out_dir.path().join("out.gba"),
        settings,
        seed: Some(Seed(5)),
        log: false,
        edits: Some(session.edits().clone()),
    };
    let mut steps = Vec::new();
    let report = randomize_file(
        &request,
        &mut |p: Progress| steps.push(p.step.into_owned()),
        &CancelToken::default(),
    )
    .expect("randomizes");
    assert_eq!(report.manual_edits, 6);
    assert!(steps.contains(&"Applying 6 manual edits".to_owned()), "{steps:?}");

    let saved = ppr_romio::open(&request.output).expect("output loads");
    let game = saved.game_data();
    assert_eq!(game.starters[1], 25);
    assert_ne!(game.starters, session.game().starters, "the other starters are random");
    let roxanne = game.trainers.iter().find(|t| t.index == team.index).expect("Roxanne");
    assert_eq!((roxanne.pokemon[0].species, roxanne.pokemon[0].level), (150, 70));
    assert!(game.encounters[0].encounters.iter().all(|e| e.species == 133));
    assert!(
        game.encounters[1..]
            .iter()
            .zip(&session.game().encounters[1..])
            .any(|(a, b)| a.encounters != b.encounters),
        "the other wild sets are random"
    );
    assert_eq!(game.shops[2].items, [1, 2, 3]);
    assert_eq!(game.item_prices.get(&4), Some(&777));
    assert_eq!(game.trades[0].nickname, "SPUD");
}

/// With "Allow Alternate Formes" for starters, static and Totem Pokémon, the saved game has the species and formes
/// the randomizers chose, alternate formes among them.
#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn alternate_forme_starters_statics_and_totems_are_written_and_reload() {
    let out_dir = out_dir();
    let mut tested = 0;
    for rom in roms() {
        let original = ppr_romio::open(&rom).expect("loads").game_data().clone();
        if !original.formes.starters {
            continue;
        }
        let mut settings = Settings::default();
        settings.starters.mode = ppr_service::StarterMode::Random;
        settings.starters.allow_alt_formes = true;
        settings.statics.mode = ppr_service::StaticMode::Random;
        settings.statics.allow_alt_formes = true;
        settings.totems.mode = ppr_service::TotemMode::Random;
        settings.totems.ally_mode = ppr_service::TotemMode::Random;
        settings.totems.allow_alt_formes = true;
        let seed = Seed(167);
        let mut expected = ppr_random::RunData {
            game: original.clone(),
            settings: settings.clone(),
        };
        ppr_random::standard_pipeline()
            .run(&mut expected, seed, &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        let request = RandomizeRequest {
            update: None,
            input: rom.clone(),
            output: out_dir.path().join("out"),
            settings,
            seed: Some(seed),
            log: false,
            edits: None,
            layered_fs: false,
        };
        randomize_file(&request, &mut |_: Progress| {}, &CancelToken::default())
            .unwrap_or_else(|e| panic!("{}: {e}", rom.display()));
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let (wanted, game) = (&expected.game, saved.game_data());
        assert_eq!(game.starters, wanted.starters, "{}", rom.display());
        assert_eq!(game.statics, wanted.statics, "{}", rom.display());
        assert_eq!(game.totems, wanted.totems, "{}", rom.display());
        let is_alt = |n: ppr_model::DexNumber| game.species(n).is_some_and(|sp| !sp.is_base_forme());
        let alt_statics = game.statics.iter().filter(|s| is_alt(s.species)).count();
        let alt_totems = game
            .totems
            .iter()
            .flat_map(|t| std::iter::once(t.species).chain(t.allies.iter().map(|a| a.species)))
            .filter(|&n| is_alt(n))
            .count();
        eprintln!(
            "{}: {alt_statics} alternate forme statics, {alt_totems} in totems",
            rom.display()
        );
        assert!(alt_statics > 0, "{}", rom.display());
        assert!(game.totems.is_empty() || alt_totems > 0, "{}", rom.display());
        tested += 1;
    }
    if tested == 0 {
        skipped("no Gen 6 or 7 game in PPR_TEST_ROMS");
    }
}
