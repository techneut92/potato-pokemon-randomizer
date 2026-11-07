//! Runs the `ppr` binary on synthetic ROMs.

use std::path::Path;
use std::process::{Command, Output};

fn ppr(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ppr"))
        .args(args)
        .output()
        .expect("ppr runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8")
}

fn emerald(dir: &Path) -> String {
    let path = dir.join("emerald.gba");
    std::fs::write(&path, ppr_romio::testing::emerald()).expect("write");
    path.to_str().expect("UTF-8 path").to_owned()
}

#[test]
fn identify_reports_the_game() {
    let dir = tempfile::tempdir().expect("temp dir");
    let output = ppr(&["identify", &emerald(dir.path())]);
    assert!(output.status.success());
    assert!(stdout(&output).contains("Emerald (U)"), "{}", stdout(&output));
}

#[test]
fn randomize_writes_custom_starters_and_reports_json() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("out.gba");
    let output = ppr(&[
        "--json",
        "randomize",
        &emerald(dir.path()),
        "-o",
        out.to_str().expect("UTF-8 path"),
        "--seed",
        "5",
        "--custom",
        "1,4,7",
    ]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    assert_eq!(report["seed"], "5");
    let after: Vec<_> = report["starters"]
        .as_array()
        .expect("array")
        .iter()
        .map(|s| s["after"].as_str().expect("name").to_owned())
        .collect();
    assert_eq!(after, ["MON1", "MON4", "MON7"]);
    assert!(out.exists());
}

#[test]
fn randomize_reports_changed_statics_and_trades() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("out.gba");
    let rom = emerald(dir.path());
    let args = |extra: &[&'static str]| {
        let mut args = vec![
            "randomize",
            rom.as_str(),
            "-o",
            out.to_str().expect("UTF-8 path"),
            "--seed",
            "2",
        ];
        args.extend_from_slice(extra);
        args
    };
    let output = ppr(&args(&[
        "--statics",
        "similar-strength",
        "--statics-fix-music",
        "--static-levels",
        "-20",
    ]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        stdout(&output).contains("Static Pokémon: 26 of 26 changed"),
        "{}",
        stdout(&output)
    );

    let output = ppr(&args(&["--trades", "both", "--trade-nicknames", "--trade-ivs"]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        stdout(&output).contains("In-game trades: 4 of 4 changed"),
        "{}",
        stdout(&output)
    );

    let output = ppr(&args(&["--statics", "random", "--statics-randomize-600-bst"]));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--statics similar-strength"));

    let output = ppr(&args(&["--base-stats", "random", "--base-stats-follow-evolutions"]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(
        stdout(&output).contains("Base stats: ") && stdout(&output).contains(" of 386 species changed"),
        "{}",
        stdout(&output)
    );

    let output = ppr(&args(&[
        "--base-stats",
        "shuffle",
        "--base-stats-follow-evolutions",
        "--base-stats-random-evolution-stats",
    ]));
    assert!(!output.status.success(), "needs random base stats");

    let output = ppr(&args(&["--update-base-stats", "7"]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout(&output).contains("Base stats: "), "{}", stdout(&output));
    assert!(!ppr(&args(&["--update-base-stats", "5"])).status.success());

    let output = ppr(&args(&[
        "--bst",
        "buff-nerf",
        "--bst-buff-nerf",
        "30",
        "--bst-follow-evolutions",
    ]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout(&output).contains("Base stats: "), "{}", stdout(&output));
    assert!(
        !ppr(&args(&["--bst", "buff-nerf", "--bst-buff-nerf", "51"]))
            .status
            .success()
    );

    let output = ppr(&args(&[
        "--evolutions",
        "random",
        "--evolutions-force-change",
        "--change-impossible-evolutions",
        "--easier-evolutions",
        "40",
    ]));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(stdout(&output).contains("Evolutions: "), "{}", stdout(&output));
    let output = ppr(&args(&[
        "--evolutions",
        "random-every-level",
        "--evolutions-similar-strength",
    ]));
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--evolutions random"));
}

#[test]
fn a_type_without_single_type_is_rejected() {
    let dir = tempfile::tempdir().expect("temp dir");
    let out = dir.path().join("out.gba");
    let output = ppr(&[
        "randomize",
        &emerald(dir.path()),
        "-o",
        out.to_str().expect("UTF-8 path"),
        "--starters",
        "random",
        "--starter-type",
        "fire",
    ]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--starter-types single-type"));
    assert!(!out.exists());
}

#[test]
fn unsupported_files_fail_without_output() {
    let dir = tempfile::tempdir().expect("temp dir");
    let input = dir.path().join("notes.txt");
    std::fs::write(&input, "not a ROM").expect("write");
    let out = dir.path().join("out.gba");
    let output = ppr(&[
        "randomize",
        input.to_str().expect("UTF-8 path"),
        "-o",
        out.to_str().expect("UTF-8 path"),
    ]);
    assert!(!output.status.success());
    assert!(!out.exists());
}

#[test]
fn randomize_takes_settings_strings_and_files() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = emerald(dir.path());
    let mut shared = ppr_service::SharedSettings::default();
    shared.settings.starters.mode = ppr_service::StarterMode::Custom;
    shared.settings.starters.custom = vec![Some(1), Some(4), Some(7)];
    let text = ppr_service::settings_string(&shared);
    let file = ppr_service::save_settings_file(&dir.path().join("mine"), &shared).expect("saves");
    for (flag, value) in [
        ("--settings-string", text.as_str()),
        ("--settings-file", file.to_str().expect("UTF-8")),
    ] {
        let out = dir.path().join("out.gba");
        let output = ppr(&[
            "--json",
            "randomize",
            &rom,
            "-o",
            out.to_str().expect("UTF-8"),
            flag,
            value,
        ]);
        assert!(
            output.status.success(),
            "{flag}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
        let after: Vec<_> = report["starters"]
            .as_array()
            .expect("array")
            .iter()
            .map(|s| s["after"].as_str().expect("name").to_owned())
            .collect();
        assert_eq!(after, ["MON1", "MON4", "MON7"], "{flag}");
    }
    let output = ppr(&["randomize", &rom, "-o", "x.gba", "--settings-string", "nonsense"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid"));
}

#[test]
fn randomize_takes_premade_seeds() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = emerald(dir.path());
    let mut shared = ppr_service::SharedSettings::default();
    shared.settings.starters.mode = ppr_service::StarterMode::Random;
    let premade = ppr_service::PremadeSeed {
        seed: "-7".parse().expect("a seed"),
        shared,
    };
    let file = ppr_service::save_premade_seed(&dir.path().join("race"), &premade).expect("saves");
    let run = |name: &str| {
        let out = dir.path().join(name);
        let output = ppr(&[
            "--json",
            "randomize",
            &rom,
            "-o",
            out.to_str().expect("UTF-8"),
            "--premade-seed",
            file.to_str().expect("UTF-8"),
        ]);
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        std::fs::read(out).expect("written")
    };
    assert_eq!(run("a.gba"), run("b.gba"), "the same premade seed makes the same game");
    let output = ppr(&[
        "randomize",
        &rom,
        "-o",
        "x.gba",
        "--premade-seed",
        "x.rndp",
        "--seed",
        "1",
    ]);
    assert!(!output.status.success());
}

#[test]
fn batch_writes_numbered_roms() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = emerald(dir.path());
    let text = ppr_service::settings_string(&ppr_service::SharedSettings::default());
    let out = dir.path().to_str().expect("UTF-8");
    let output = ppr(&[
        "--json",
        "batch",
        &rom,
        "-d",
        out,
        "-n",
        "2",
        "--start",
        "7",
        "--logs",
        "--settings-string",
        &text,
    ]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    assert_eq!(report["nextStart"], 9);
    assert!(dir.path().join("random7.gba").exists());
    assert!(dir.path().join("random8.log").exists());
    let output = ppr(&["batch", &rom, "-d", out, "-n", "2"]);
    assert!(!output.status.success(), "settings are required");
}

#[test]
fn layered_fs_is_only_for_3ds_games() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = emerald(dir.path());
    let out = dir.path().join("out");
    let output = ppr(&[
        "randomize",
        &rom,
        "-d",
        "-o",
        out.to_str().expect("UTF-8"),
        "--seed",
        "1",
    ]);
    assert!(!output.status.success());
    let err = String::from_utf8_lossy(&output.stderr);
    assert!(
        err.contains("only 3DS games can be saved as a LayeredFS folder"),
        "{err}"
    );
    assert!(!out.exists(), "nothing written");

    let text = ppr_service::settings_string(&ppr_service::SharedSettings::default());
    let batch_dir = dir.path().join("batch");
    std::fs::create_dir(&batch_dir).expect("out dir");
    let output = ppr(&[
        "batch",
        &rom,
        "-d",
        batch_dir.to_str().expect("UTF-8"),
        "-n",
        "1",
        "--layered-fs",
        "--settings-string",
        &text,
    ]);
    assert!(!output.status.success());
    assert_eq!(
        std::fs::read_dir(&batch_dir).expect("readable").count(),
        0,
        "nothing written"
    );
}

#[test]
fn randomize_uses_custom_names() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = emerald(dir.path());
    let names = dir.path().join("names");
    std::fs::create_dir(&names).expect("names dir");
    std::fs::write(
        names.join("PokemonNicknames.txt"),
        "Pebble\nComet\nMochi\nNova\nZiggy\n",
    )
    .expect("writes");
    let out = dir.path().join("out.gba");
    let output = ppr(&[
        "randomize",
        &rom,
        "-o",
        out.to_str().expect("UTF-8"),
        "--trades",
        "given",
        "--trade-nicknames",
        "--log",
        "--custom-names",
        names.to_str().expect("UTF-8"),
    ]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let log = std::fs::read_to_string(dir.path().join("out.log")).expect("a log");
    assert!(
        ["Pebble", "Comet", "Mochi", "Nova", "Ziggy"]
            .iter()
            .any(|n| log.contains(n)),
        "the trades use the custom nicknames"
    );
}

#[test]
fn randomize_applies_manual_edits_from_a_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let rom = emerald(dir.path());
    let view = ppr(&["edit-view", &rom]);
    assert!(view.status.success(), "{}", String::from_utf8_lossy(&view.stderr));
    let view: serde_json::Value = serde_json::from_str(&stdout(&view)).expect("JSON");
    assert_eq!(view["starters"][0], serde_json::json!({ "slot": 0, "species": 252 }));
    let edits = serde_json::json!({
        "version": 1,
        "game": view["game"],
        "edits": [{ "kind": "starter", "slot": 0, "species": 9 }],
    });
    let file = dir.path().join("edits.json");
    std::fs::write(&file, edits.to_string()).expect("write");
    let out = dir.path().join("out.gba");
    let args = |edits: &Path| {
        ppr(&[
            "--json",
            "randomize",
            &rom,
            "-o",
            out.to_str().expect("UTF-8 path"),
            "--edits",
            edits.to_str().expect("UTF-8 path"),
        ])
    };
    let output = args(&file);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let report: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON");
    assert_eq!(report["manualEdits"], 1);
    assert_eq!(report["starters"][0]["after"], "MON9");

    std::fs::remove_file(&out).expect("remove");
    let bad = serde_json::json!({
        "version": 1,
        "game": view["game"],
        "edits": [{ "kind": "starter", "slot": 0, "species": 9999 }],
    });
    std::fs::write(&file, bad.to_string()).expect("write");
    let output = args(&file);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("species #9999"));
    assert!(!out.exists(), "nothing written");
}
