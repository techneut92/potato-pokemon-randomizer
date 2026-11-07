use ppr_model::{StatChange, StatChangeType};
use ppr_settings::{MoveDataSettings, Settings};

use super::*;
use crate::randomizers::testing::emerald;
use crate::{CancelToken, Progress, Seed, standard_pipeline};

fn game() -> GameData {
    let mut g = emerald(Vec::new());
    g.max_move_name_length = 12;
    g.moves = (1..=354)
        .map(|n| Move::basic(n, &format!("MOVE{n}"), 40, Type::Fire, MoveCategory::Physical))
        .collect();
    g.moves[0].move_type = None;
    g
}

fn names(g: &GameData) -> Vec<&str> {
    g.moves.iter().map(|m| m.name.as_str()).collect()
}

fn run_on(game: GameData) -> GameData {
    let mut data = RunData {
        game,
        settings: Settings {
            move_data: MoveDataSettings {
                names: true,
                ..MoveDataSettings::default()
            },
            ..Settings::default()
        },
    };
    standard_pipeline()
        .run(&mut data, Seed(3), &CancelToken::default(), &mut |_: Progress| {})
        .expect("runs");
    data.game
}

#[test]
fn the_word_lists_parse_into_keyed_lists() {
    assert_eq!(TYPE_WORDS.len(), 18);
    for t in Type::ALL {
        assert!(TYPE_WORDS.contains_key(type_key(t)), "{t:?}");
    }
    assert_eq!(
        CATEGORY_WORDS.keys().copied().collect::<Vec<_>>(),
        ["PHYSICAL", "SPECIAL", "STATUS"]
    );
    assert_eq!(CATEGORY_WORDS["SPECIAL"][..3], ["Blast", "Pulse", "Wave"]);
    assert_eq!(EXTRA_WORDS.len(), 13);
    assert_eq!(EXTRA_WORDS["SOUND"].last(), Some(&"Blare"));
}

#[test]
fn names_fit_are_unique_and_upper_case_like_the_game() {
    let before = game();
    let g = run_on(game());
    let mut seen = BTreeSet::new();
    for (old, new) in before.moves.iter().zip(&g.moves) {
        if old.number == STRUGGLE {
            assert_eq!(new.name, old.name);
            continue;
        }
        assert!(new.name.len() <= 12, "{}", new.name);
        assert_eq!(new.name, new.name.to_uppercase());
        assert!(seen.insert(new.name.clone()), "{} twice", new.name);
    }
    assert!(names(&g)[0].starts_with("ATTACK"), "typeless moves: {}", names(&g)[0]);
}

#[test]
fn mixed_case_games_keep_mixed_case() {
    let mut g = game();
    for m in &mut g.moves {
        m.name = "Pound".into();
    }
    let g = run_on(g);
    assert!(g.moves.iter().any(|m| m.name.chars().any(char::is_lowercase)));
}

#[test]
fn action_words_follow_what_the_move_does() {
    let words = |mv: &Move| action_words(mv).to_vec();
    let base = Move::basic(1, "X", 40, Type::Normal, MoveCategory::Physical);
    assert_eq!(words(&base), CATEGORY_WORDS["PHYSICAL"]);
    assert_eq!(
        words(&Move {
            is_punch: true,
            is_sound: true,
            ..base.clone()
        }),
        EXTRA_WORDS["PUNCH"]
    );
    assert_eq!(
        words(&Move {
            absorb_percent: 50,
            ..base.clone()
        }),
        EXTRA_WORDS["DRAIN"]
    );
    assert_eq!(
        words(&Move {
            is_trap: true,
            ..base.clone()
        }),
        EXTRA_WORDS["STATUS_TRAP"]
    );
    let status = Move {
        category: MoveCategory::Status,
        power: 0,
        ..base.clone()
    };
    assert_eq!(words(&status), CATEGORY_WORDS["STATUS"], "Splash");
    assert_eq!(
        words(&Move {
            number: 105,
            ..status.clone()
        }),
        EXTRA_WORDS["STATUS_HEAL"],
        "Recover"
    );
    let toxic = Move {
        status: Some(StatusType::ToxicPoison),
        ..status.clone()
    };
    assert_eq!(words(&toxic), EXTRA_WORDS["INFLICT_POISON"]);
    let stat = |kind, stages| Move {
        stat_change_move_type: kind,
        stat_changes: vec![StatChange {
            stat: StatChangeType::Attack,
            stages,
            percent_chance: 0,
        }],
        ..status.clone()
    };
    assert_eq!(
        words(&stat(StatChangeMoveType::NoDamageUser, 2)),
        EXTRA_WORDS["STATUS_BUFF"],
        "Swords Dance"
    );
    assert_eq!(
        words(&stat(StatChangeMoveType::NoDamageUser, -1)),
        EXTRA_WORDS["STATUS_DEBUFF"]
    );
    assert_eq!(
        words(&stat(StatChangeMoveType::NoDamageTarget, -1)),
        EXTRA_WORDS["STATUS_DEBUFF"],
        "Growl"
    );
    assert_eq!(
        words(&stat(StatChangeMoveType::NoDamageAlly, 1)),
        EXTRA_WORDS["STATUS_BUFF"]
    );
}

#[test]
fn the_last_resort_name_fits() {
    let mut used: BTreeSet<String> = BTreeSet::new();
    let mv = Move::basic(1, "X", 40, Type::Normal, MoveCategory::Physical);
    let mut rng = Seed(1).rng_for(step_ids::MOVE_NAMES);
    // Nothing fits in three letters.
    assert_eq!(random_name(&mv, 3, &mut used, &mut rng), "Atk");
}

#[test]
fn upper_case_names_are_detected() {
    assert!(upper_case_names(["POUND", "KARATE CHOP"].into_iter()));
    assert!(upper_case_names(["123"].into_iter()));
    assert!(!upper_case_names(["Pound"].into_iter()));
    assert!(upper_case_names(["POUNDPOUNDPOUNDx"].into_iter()));
}

/// FNV-1a hash of the known-good action words of every move of a real Gen 3 game, as "number|first|last|count"
/// lines.
const ACTION_WORDS_HASH: u64 = 699_091_785_066_994_153;

#[test]
#[ignore = "needs PPR_TEST_ROMS"]
fn real_action_words_match_the_known_good_hash() {
    use std::fmt::Write as _;

    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a directory of ROM dumps");
    let mut tested = 0;
    for entry in std::fs::read_dir(dir).expect("PPR_TEST_ROMS is readable") {
        let path = entry.expect("lists").path();
        // The app's own output is no dump.
        if path.to_string_lossy().contains("randomized") {
            continue;
        }
        let Ok(rom) = ppr_romio::open(&path) else { continue };
        let game = rom.game_data();
        if game.identity.generation.number() != 3 {
            continue;
        }
        let mut dump = String::new();
        for mv in &game.moves {
            let w = action_words(mv);
            writeln!(dump, "{}|{}|{}|{}", mv.number, w[0], w[w.len() - 1], w.len()).expect("writes to a string");
        }
        let hash = dump.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        assert_eq!(hash, ACTION_WORDS_HASH, "{}", path.display());
        tested += 1;
    }
    assert!(tested > 0, "no Gen 3 ROMs");
}
