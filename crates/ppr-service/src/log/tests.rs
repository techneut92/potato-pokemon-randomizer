use ppr_model::{Aura, Evolution, InGameTrade, LevelUpMove, TotemAlly, TotemPokemon};

use super::*;

/// The synthetic Emerald, as loaded.
fn emerald() -> GameData {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("emerald.gba");
    std::fs::write(&path, ppr_romio::testing::emerald()).expect("write");
    ppr_romio::open(&path).expect("loads").game_data().clone()
}

fn log(before: &GameData, after: &GameData, settings: &Settings) -> String {
    write(&LogInput {
        before,
        after,
        settings,
        seed: Seed(42),
        file_name: "emerald.gba",
        crc32: 0x1F1C_08FB,
        update: None,
        elapsed: Duration::from_millis(7),
    })
}

#[test]
fn diagnostics_name_the_game_update() {
    let game = emerald();
    let settings = Settings::default();
    let text = write(&LogInput {
        before: &game,
        after: &game,
        settings: &settings,
        seed: Seed(42),
        file_name: "x.3ds",
        crc32: 1,
        update: Some(UpdateInfo {
            file_name: "x-update.cia".into(),
            version: "Ver. 1.5".into(),
        }),
        elapsed: Duration::from_millis(7),
    });
    assert!(text.contains("File name: x.3ds\nOriginal ROM CRC32: 00000001\nGame update: x-update.cia (Ver. 1.5)\n"));
    assert!(!log(&game, &game, &settings).contains("Game update"));
}

fn section(log: &str, (title, shortcut): Section) -> &str {
    let start = log
        .find(&format!(" ( {title} {{{shortcut}}} )"))
        .unwrap_or_else(|| panic!("no section {title}"));
    let rest = &log[start..];
    &rest[..rest.find(SEPARATOR).unwrap_or(rest.len())]
}

#[test]
fn an_unchanged_game_has_only_the_fixed_sections() {
    let g = emerald();
    let text = log(&g, &g, &Settings::default());
    assert!(text.contains("Base game: Emerald (U)\n"));
    assert!(text.contains("Random seed: 42\n"));
    assert!(text.contains(" Table of Contents--------------------------------{TABL}\n"));
    assert!(text.contains(" Randomization/ROM Diagnostics--------------------{DIAG}\n"));
    assert!(text.contains("Pokemon Base Statistics: Unchanged\n"));
    assert!(!text.contains("Randomized/Changed"));
    assert!(text.contains("No Misc. Tweaks were applied.\n"));
    assert!(text.contains("Time elapsed: 7ms\n"));
    assert!(text.ends_with("File name: emerald.gba\nOriginal ROM CRC32: 1F1C08FB\n"));
    // Head, table of contents, overview and statistics end with a separator; the diagnostics don't.
    assert_eq!(text.matches(SEPARATOR).count(), 4);
}

#[test]
fn changed_parts_get_their_sections() {
    let before = emerald();
    let mut after = before.clone();
    let mut settings = Settings::default();
    settings.starters.mode = StarterMode::Random;
    after.starters[0] = 25;
    after.starter_held_items = vec![Some(after.items[1].id)];
    after.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Double);
    after.applied_tweaks.insert(MiscTweak::FastestText);

    let text = log(&before, &after, &settings);
    assert!(text.contains(" Starter Pokemon----------------------------------{SRPK}\n"));
    assert!(text.contains(" Type Effectiveness-------------------------------{TPEF}\n"));
    assert!(!text.contains("{PKST}"));
    assert!(text.contains("Starter Pokemon: Randomized/Changed\n"));
    assert!(text.contains("The following Misc. Tweaks were applied:\nFastest Text\n"));
    let starters = section(&text, STARTERS);
    assert!(starters.contains("Mode: Random (completely)\n"));
    let item = &after.items[1].name;
    let pikachu = species_name(&after, 25);
    assert!(starters.contains(&format!("Set starter 1 to {pikachu}, holding {item}\n")));
    let chart = section(&text, TYPE_CHART);
    assert!(chart.contains("\nNORMAL  |"));
    assert!(chart.contains("Attacker (left), Defender (top)\n--- = Neutral\nSE! = Super Effective\n"));
}

#[test]
fn trainers_list_teams_or_movesets() {
    let mut before = emerald();
    before.trainers = vec![
        Trainer::new(1, "ROXANNE", 0, vec![TrainerPokemon::new(75, 12)]),
        Trainer::new(2, "BRAWLY", 0, vec![TrainerPokemon::new(67, 16)]),
    ];
    before.trainers[0].tag = Some("GYM1-LEADER".into());
    before.trainers[1].tag = Some("GYM2-LEADER".into());
    let mut after = before.clone();
    after.trainers[0].pokemon[0].species = 74;
    after.trainers[1].pokemon[0].species = 66;
    after.trainers[1].pokemon[0].moves = [1, 0, 0, 0];
    after.trainers[1].pokemon[0].held_item = Some(after.items[1].id);
    let mut settings = Settings::default();
    settings.battle_style.mode = BattleStyleMode::Random;

    let text = log(&before, &after, &settings);
    let roxanne = trainer_name(&after, &after.trainers[0]);
    assert!(roxanne.ends_with("ROXANNE"));
    assert!(section(&text, LEVEL_CAPS).contains(&format!("{roxanne} - Level Cap: 12\n")));
    let trainers = section(&text, TRAINERS);
    let geodude = species_name(&after, 74);
    assert!(trainers.contains(&format!(
        "#1 ({roxanne}) - {geodude} Lv12 (Battle Style: Single)\n\n#2 ("
    )));
    let machop = species_name(&after, 66);
    let item = &after.items[1].name;
    assert!(trainers.contains(&format!(")\n{machop}@{item} Lv16, Ability: ")));
    let mv = move_name(&after, 1);
    assert!(trainers.contains(&format!(" - {mv}\n (Battle Style: Single)\n")));

    // Renamed trainers show both names.
    after.trainers[0].name = "SIDNEY".into();
    let text = log(&before, &after, &settings);
    let sidney = trainer_name(&after, &after.trainers[0]);
    assert!(section(&text, TRAINERS).contains(&format!("#1 ({roxanne} => {sidney}) - ")));
}

#[test]
fn statics_count_repeats_and_trades_make_a_table() {
    let mut before = emerald();
    before.statics = vec![StaticEncounter::new(100, 25), StaticEncounter::new(100, 25)];
    before.trades = vec![InGameTrade {
        requested: Some(280),
        given: 273,
        nickname: "DOTS".into(),
        ot_name: "KOBE".into(),
        ot_id: 1,
        ivs: vec![5; 6],
        held_item: None,
    }];
    let mut after = before.clone();
    after.statics[1].species = 101;
    after.statics[1].linked_levels = vec![30];
    after.trades[0].requested = None;
    after.trades[0].nickname = "NEW".into();
    let mut settings = Settings::default();
    settings.statics.level_modifier = 10;

    let text = log(&before, &after, &settings);
    let (voltorb, electrode) = (species_name(&after, 100), species_name(&after, 101));
    assert!(section(&text, STATICS).contains(&format!(
        "{voltorb} Lv25 => {voltorb} Lv25\n{voltorb} Lv25(2) => {electrode} Lv25 / Lv30\n"
    )));
    let trades = section(&text, TRADES);
    assert!(trades.contains("OLD REQUESTED|OLD GIVEN|OLD NICKNAME|NEW REQUESTED|NEW GIVEN|NEW NICKNAME\n"));
    let (ralts, seedot) = (species_name(&after, 280), species_name(&after, 273));
    let row = trades.lines().nth(3).expect("the trade's row");
    let cells: Vec<&str> = row.split('|').map(str::trim_end).collect();
    assert_eq!(
        cells,
        [
            ralts.as_str(),
            seedot.as_str(),
            "DOTS",
            "Any Pokemon",
            seedot.as_str(),
            "NEW"
        ]
    );
}

#[test]
fn totems_show_their_aura_and_allies() {
    let mut before = emerald();
    before.totems = vec![TotemPokemon {
        species: 20,
        forme: 0,
        level: 12,
        held_item: None,
        aura: Aura::from_byte(4),
        allies: vec![TotemAlly {
            species: 19,
            forme: 0,
            level: 10,
            held_item: None,
        }],
    }];
    let text = log(&before, &before, &Settings::default());
    assert!(text.contains("Totem Pokemon: Unchanged\n"));
    assert!(!text.contains("{TOPK}"));

    let mut after = before.clone();
    after.totems[0].species = 25;
    after.totems[0].held_item = Some(after.items[1].id);
    after.totems[0].aura = Aura::from_byte(17);
    after.totems[0].allies[0].species = 26;
    let text = log(&before, &after, &Settings::default());
    assert!(text.contains("Totem Pokemon: Randomized/Changed\n"));
    assert!(text.contains(" Totem Pokemon------------------------------------{TOPK}\n"));
    let (raticate, pikachu, raichu) = (
        species_name(&after, 20),
        species_name(&after, 25),
        species_name(&after, 26),
    );
    let item = &after.items[1].name;
    assert!(section(&text, TOTEMS).contains(&format!(
        "{raticate} =>\n{pikachu}@{item} Lv12\n\tAura: All +2\n\tAlly 1: {raichu} Lv10\n"
    )));
    // Games without Totem Pokémon don't mention them.
    assert!(!log(&emerald(), &emerald(), &Settings::default()).contains("Totem Pokemon"));
}

#[test]
fn evolution_improvements_compare_old_and_new_methods() {
    let mut before = emerald();
    before.evolutions = vec![Evolution::new(133, 134, EvolutionType::Trade, 0)];
    let mut after = before.clone();
    after.evolutions = vec![Evolution::new(133, 134, EvolutionType::Level, 37)];
    let mut settings = Settings::default();
    settings.evolutions.change_impossible = true;

    let text = log(&before, &after, &settings);
    assert!(!text.contains("{PKEV}"), "the improvements replace the evolution list");
    let improvements = section(&text, EVOLUTION_IMPROVEMENTS);
    assert!(
        improvements
            .contains("Some evolutions have been changed through the option(s):\n- Change Impossible Evolutions\n\n\n")
    );
    let row = improvements
        .lines()
        .find(|l| l.contains("Level-up, lvl. 37+"))
        .expect("Eevee's row");
    let cells: Vec<&str> = row.split('|').map(str::trim_end).collect();
    let (eevee, vaporeon) = (species_name(&after, 133), species_name(&after, 134));
    assert_eq!(
        cells,
        [
            eevee.as_str(),
            vaporeon.as_str(),
            "Trade (estimated evo lvl. 0)",
            "Level-up, lvl. 37+"
        ]
    );
}

#[test]
fn movesets_show_stats_moves_and_stab() {
    let before = emerald();
    let mut after = before.clone();
    let n = after.species[0].number;
    let attack = after
        .moves
        .iter()
        .find(|m| m.power > 0 && m.move_type.is_some())
        .expect("a damaging move")
        .clone();
    after.species[0].primary_type = attack.move_type.expect("typed");
    after.learnsets.insert(n, vec![LevelUpMove::new(1, attack.number)]);
    let text = log(&before, &after, &Settings::default());
    let movesets = section(&text, MOVESETS);
    let s = &after.species[0];
    assert!(movesets.contains(&format!("{:03} {} -> ", s.number, s.name)));
    assert!(movesets.contains("  HP| ATK| DEF|SATK|SDEF| SPD \n"));
    let line = movesets
        .lines()
        .find(|l| l.starts_with("Level 1 : "))
        .expect("the level 1 move");
    let name = format!("Level 1 : {:<w$}| ", attack.name, w = after.max_move_name_length);
    assert!(line.starts_with(&name), "{line}");
    assert!(line.ends_with(" (STAB)"), "{line}");

    let mut metronome = Settings::default();
    metronome.movesets.mode = MovesetMode::MetronomeOnly;
    let text = log(&before, &before, &metronome);
    assert!(section(&text, MOVESETS).contains("Metronome only mode - every Pokémon learns only Metronome.\n"));
    assert!(section(&text, TMS).contains("Metronome only mode - every TM contains Metronome.\n"));
}

#[test]
fn pickup_chances_add_up_per_level_range() {
    for (family, count) in [("Ruby", 10), ("Sapp", 10), ("FRLG", 16), ("Em", 29)] {
        let chances = pickup_chances(family, count);
        for range in 0..10 {
            let total: u32 = chances.iter().map(|c| u32::from(c[range])).sum();
            assert_eq!(total, 100, "{family} levels {}-{}", range * 10 + 1, range * 10 + 10);
        }
    }
}
