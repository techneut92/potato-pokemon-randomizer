//! Tests of the Gen 1 handler against real ROMs (`PPR_TEST_ROMS`), and against reference dumps of them
//! (`PPR_REFERENCE_DUMPS`: a folder with the harness's `GbDump` output for each ROM, named after the ROM file
//! with `.txt`).

use std::fmt::Write as _;
use std::io::Cursor;
use std::path::PathBuf;

use ppr_model::{Effectiveness, ExpCurve, GameData, LevelUpMove, Trainer, TrainerPokemon, Type};

use super::constants as c;
use super::*;
use crate::type_order::TYPE_ORDER;
use crate::{Detection, detect};

/// A real ROM from `PPR_TEST_ROMS` whose file name contains `name`.
fn real_rom(name: &str) -> Option<(PathBuf, Vec<u8>)> {
    let dir = std::env::var_os("PPR_TEST_ROMS")?;
    let path = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains(name) && !p.to_string_lossy().contains("randomized"))?;
    let bytes = std::fs::read(&path).expect("readable");
    Some((path, bytes))
}

fn load(bytes: Vec<u8>) -> Gen1Handler {
    let identity = match detect(&mut Cursor::new(&bytes)).expect("readable") {
        Detection::Supported(identity) => identity,
        other @ Detection::Unsupported(_) => panic!("not detected: {other:?}"),
    };
    Gen1Handler::load(bytes, identity).expect("loads")
}

fn list<T: std::fmt::Display>(items: impl IntoIterator<Item = T>) -> String {
    let items: Vec<String> = items.into_iter().map(|i| i.to_string()).collect();
    format!("[{}]", items.join(", "))
}

fn curve(c: ExpCurve) -> &'static str {
    match c {
        ExpCurve::MediumFast => "Medium fast",
        ExpCurve::Erratic => "Erratic",
        ExpCurve::Fluctuating => "Fluctuating",
        ExpCurve::MediumSlow => "Medium slow",
        ExpCurve::Fast => "Fast",
        ExpCurve::Slow => "Slow",
    }
}

/// What `GbDump` prints, from the game data.
#[expect(clippy::too_many_lines, reason = "one block per section of GbDump")]
fn dump(d: &GameData) -> String {
    let mut s = String::new();
    s.push_str("# species\n");
    for sp in &d.species {
        let b = sp.stats;
        let _ = writeln!(
            s,
            "{}|{}|{:?}|{}|hp={}, attack={}, defense={}, speed={}, special={}|{}|{}",
            sp.number,
            sp.name,
            sp.primary_type,
            sp.secondary_type.map_or_else(|| "-".to_owned(), |t| format!("{t:?}")),
            b.hp,
            b.attack,
            b.defense,
            b.speed,
            b.sp_attack,
            sp.catch_rate,
            curve(sp.growth_curve)
        );
    }
    s.push_str("# moves\n");
    for m in &d.moves {
        let changes = m.stat_changes.iter().fold(String::new(), |mut out, c| {
            let _ = write!(out, "{:?}:{}:{},", c.stat, c.stages, c.percent_chance);
            out
        });
        let _ = writeln!(
            s,
            "{}|{}|{}|{}|{}|{}|{}|{:?}|{}|{:?}|{}|{:?}|{}|{}|{:?}|{}|{}|{}|{}|{}",
            m.number,
            m.name,
            m.effect,
            m.power,
            m.pp,
            m.accuracy,
            m.move_type.map_or_else(|| "null".to_owned(), |t| format!("{t:?}")),
            m.category,
            m.hit_count_hundredths,
            m.stat_change_move_type,
            changes,
            m.status_move_type,
            m.status.map_or_else(|| "None".to_owned(), |t| format!("{t:?}")),
            m.status_percent_chance,
            m.critical_chance,
            m.flinch_percent_chance,
            m.recoil_percent,
            m.absorb_percent,
            m.is_charge,
            m.is_recharge
        );
    }
    s.push_str("# learnsets\n");
    for (n, moves) in &d.learnsets {
        let _ = write!(s, "{n}:");
        for m in moves {
            let _ = write!(s, " {}/{}", m.level, m.move_id);
        }
        s.push('\n');
    }
    s.push_str("# eggmoves\n# evolutions\n");
    for sp in &d.species {
        for e in d.evolutions.iter().filter(|e| e.from == sp.number) {
            let _ = writeln!(s, "{}>{} {:?} {}", e.from, e.to, e.kind, e.parameter);
        }
    }
    s.push_str("# starters\n");
    for n in &d.starters {
        let _ = writeln!(s, "{n}");
    }
    s.push_str("# items\n");
    for i in &d.items {
        let _ = writeln!(s, "{}|{}|{}|{}|{}", i.id, i.name, i.allowed, i.bad, i.tm);
    }
    let _ = writeln!(s, "# tms\n{}\n{}", list(&d.tm_moves), list(&d.hm_moves));
    s.push_str("# compat\n");
    for (n, flags) in &d.tm_hm_compat {
        let bits: String = flags.iter().map(|&f| if f { '1' } else { '0' }).collect();
        let _ = writeln!(s, "{n}:{bits}");
    }
    s.push_str("# tutors\n[]\n# typechart\n");
    let types: Vec<Type> = TYPE_ORDER.into_iter().filter(|t| d.types.contains(t)).collect();
    for &x in &types {
        for &y in &types {
            if let Some(e) = d.type_chart.get(x, y).filter(|&e| e != Effectiveness::Neutral) {
                let _ = writeln!(s, "{x:?}>{y:?} {}", format!("{e:?}").to_uppercase());
            }
        }
    }
    s.push_str("# encounters\n");
    for set in &d.encounters {
        let _ = write!(
            s,
            "{}|{}|{:?}|{}|{}|",
            set.name,
            set.rate,
            set.kind,
            set.map.map_or_else(|| "-".to_owned(), |m| m.to_string()),
            set.location.as_deref().unwrap_or("null")
        );
        for e in &set.encounters {
            let max = if e.max_level == e.min_level { 0 } else { e.max_level };
            let _ = write!(s, "{}/{}/{max},", e.species, e.min_level);
        }
        let mut banned = set.banned.clone();
        banned.sort_unstable();
        let _ = writeln!(s, "|{}", list(banned));
    }
    s.push_str("# statics\n");
    for st in &d.statics {
        let _ = writeln!(s, "{}:{}", st.species, st.level);
    }
    s.push_str("# trades\n");
    for t in &d.trades {
        let _ = writeln!(
            s,
            "{}|{}|{}|null|0|[]|0",
            t.requested.unwrap_or_default(),
            t.given,
            t.nickname
        );
    }
    s.push_str("# trainers\n");
    for c in &d.trainer_class_names {
        let _ = writeln!(s, "class {c}");
    }
    for t in &d.trainers {
        let _ = write!(
            s,
            "{} null {} {} {}",
            t.index,
            t.class,
            t.tag.as_deref().unwrap_or("null"),
            t.force_starter_position.map_or(-1, i16::from)
        );
        for p in &t.pokemon {
            let _ = write!(s, " | {} {} 0 0 0 0 0", p.species, p.level);
        }
        s.push('\n');
    }
    let themes: Vec<String> = d
        .gym_and_elite_themes
        .iter()
        .map(|(k, t)| format!("{k}={}", format!("{t:?}").to_uppercase()))
        .collect();
    let _ = writeln!(
        s,
        "elite {}\nthemes {{{}}}",
        list(&d.elite_four_trainers),
        themes.join(", ")
    );
    s.push_str("# shops\n");
    for shop in &d.shops {
        let items = shop.items.iter().fold(String::new(), |mut out, i| {
            let _ = write!(out, "{i},");
            out
        });
        let _ = writeln!(s, "{}|{}|{}|{items}", shop.name, shop.main_game, shop.special);
    }
    for (id, price) in d.item_prices.iter().filter(|&(_, &p)| p != 0) {
        let _ = writeln!(s, "{id}={price}");
    }
    s.push_str("# field\n");
    let field = d.field_items.iter().fold(String::new(), |mut out, i| {
        let _ = write!(out, "{i},");
        out
    });
    let mut required = d.required_field_tms.clone();
    required.sort_unstable();
    let _ = writeln!(s, "{field}\nrequired {}", list(required));
    s.push_str("# misc\n");
    let mut evo = d.evolution_items.clone();
    evo.sort_unstable();
    let mut op = d.op_shop_items.clone();
    op.sort_unstable();
    let _ = writeln!(
        s,
        "breaking {}\nfield moves {}\nearly hms {}\nbanned levelup {}\nevo items {}\nop items {}\nmax move name {}",
        list(&d.game_breaking_moves),
        list(&d.field_moves),
        list(&d.early_required_hm_moves),
        list(&d.banned_level_up_moves),
        list(evo),
        list(op),
        d.max_move_name_length
    );
    s
}

fn tm_price_line(line: &str) -> bool {
    line.split_once('=')
        .and_then(|(id, _)| id.parse::<u16>().ok())
        .is_some_and(|id| (c::TM01..c::TM01 + 50).contains(&id))
}

/// A dump with the deliberate differences taken out: the chance of damaging stat drops (the reference's 85/256
/// "percent" prints as 0), TM prices (the reference reads them from the wrong bytes), negative map indices (no map
/// here), and misc lines the game data doesn't hold.
fn normalize(dump: &str, reference: bool) -> String {
    let mut out = String::new();
    let mut section = "";
    for line in dump.lines() {
        if let Some(name) = line.strip_prefix("# ") {
            section = name;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let line = match section {
            "moves" if reference && line.contains("|DamageTarget|") => line.replace(":-1:0,", ":-1:33,"),
            "encounters" if reference => {
                let mut parts: Vec<String> = line.split('|').map(str::to_owned).collect();
                if parts.len() > 3 && parts[3].starts_with('-') {
                    "-".clone_into(&mut parts[3]);
                }
                parts.join("|")
            }
            "shops" if tm_price_line(line) => continue,
            "misc"
                if ["tweaks", "trainer name", "trade nick"]
                    .iter()
                    .any(|p| line.starts_with(p)) =>
            {
                continue;
            }
            _ => line.to_owned(),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` for the comparison): Yellow loads as the reference reads it, and
/// an unchanged game saves byte for byte.
#[test]
#[ignore = "needs a real ROM"]
fn real_yellow_matches_reference() {
    let Some((path, bytes)) = real_rom("Yellow") else {
        return;
    };
    let h = load(bytes.clone());
    let d = h.game_data();
    assert_eq!(d.species.len(), 151);
    assert_eq!(d.moves.len(), 165);
    assert_eq!(d.starters, [25, 133]);
    assert_eq!(d.trainers.len(), 396);
    let trades = h.edit_rules().trades;
    assert!(trades.texts);
    assert_eq!((trades.max_nickname_length, trades.max_ot_name_length), (10, 0));
    let personal: Vec<&str> = d
        .personal_trainer_classes
        .iter()
        .map(|&c| d.trainer_class_names[usize::from(c)].as_str())
        .collect();
    assert_eq!(
        personal,
        [
            "GIOVANNI", "BRUNO", "BROCK", "MISTY", "LT.SURGE", "ERIKA", "KOGA", "BLAINE", "SABRINA", "LORELEI",
            "AGATHA", "LANCE"
        ]
    );
    assert_eq!(
        (
            d.text_length("BROCK"),
            d.text_length("JR.TRAINER♂"),
            d.text_length("POKéMANIAC")
        ),
        (5, 11, 10)
    );
    assert!(d.is_yellow() && !d.fixed_trainer_text);
    // TM prices, from pokered's tm_prices.asm: Mega Punch 3000, Razor Wind 2000, Swords Dance 2000, Whirlwind 1000.
    let price = |id| d.item_prices.get(&id).copied();
    assert_eq!(
        (price(328), price(329), price(330), price(331)),
        (Some(3000), Some(2000), Some(2000), Some(1000))
    );

    let mut unchanged = load(bytes.clone());
    let same = unchanged.game_data().clone();
    unchanged.apply(&same).expect("applies");
    let mut out = Vec::new();
    unchanged.save(&mut out).expect("saves");
    assert!(out == bytes, "an unchanged game saves byte for byte");

    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a GbDump of the ROM");
        let ours = normalize(&dump(d), false);
        let theirs = normalize(&reference, true);
        for (i, (a, b)) in ours.lines().zip(theirs.lines()).enumerate() {
            assert_eq!(a, b, "line {}", i + 1);
        }
        assert_eq!(ours.lines().count(), theirs.lines().count());
    }
}

/// With `PPR_TEST_ROMS`: what is written to Yellow reads back, data that grows included.
#[test]
#[ignore = "needs a real ROM"]
fn real_yellow_writes_read_back() {
    let Some((_, bytes)) = real_rom("Yellow") else {
        return;
    };
    let mut h = load(bytes);
    let mut d = h.game_data().clone();
    d.starters = vec![4, 7];
    d.species[0].name = "SEEDLING".into();
    d.species[0].stats.attack = 99;
    d.species[0].secondary_type = None;
    "BIG HIT".clone_into(&mut d.moves[0].name);
    d.moves[0].power = 55;
    for set in &mut d.encounters {
        for e in &mut set.encounters {
            e.species = 150;
            e.min_level = 50;
            e.max_level = 50;
        }
    }
    // A trainer of each class gets two more Pokémon, so every class grows.
    let mut seen = Vec::new();
    for t in &mut d.trainers {
        if !seen.contains(&t.class) {
            seen.push(t.class);
            let level = t.pokemon[0].level;
            t.pokemon.push(TrainerPokemon::new(1, level));
            t.pokemon.push(TrainerPokemon::new(4, level + 1));
        }
    }
    // Every shop grows by one item, while the trainers above grow too.
    for shop in &mut d.shops {
        shop.items.fill(1);
        shop.items.push(2);
    }
    d.item_prices.insert(1, 1234);
    d.item_prices.insert(328, 9000);
    // Too high, and a TM price between thousands: written as they fit.
    d.item_prices.insert(2, 1_234_567);
    d.item_prices.insert(329, 12_500);
    d.learnsets
        .get_mut(&1)
        .expect("Bulbasaur")
        .push(LevelUpMove::new(60, 76));
    for e in d.evolutions.iter_mut().filter(|e| e.from == 1) {
        e.parameter = 10;
    }
    d.tm_moves[0] = 94;
    d.statics[0].level = 42;
    "SPUD".clone_into(&mut d.trades[0].nickname);
    d.field_items[0] = 1;
    d.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Half);
    d.palettes.get_mut(&1).expect("Bulbasaur").id = Some(20);
    h.apply(&d).expect("applies");
    let mut out = Vec::new();
    h.save(&mut out).expect("saves");

    // Yellow's second starter is the rival's first Pokémon.
    let rival = d
        .trainers
        .iter()
        .position(|t| t.tag.as_deref() == Some("RIVAL1-0"))
        .expect("first rival battle");
    d.trainers[rival].pokemon[0].species = 7;
    let back = load(out);
    let b = back.game_data();
    assert_eq!(b.starters, d.starters);
    assert_eq!(b.species, d.species);
    assert_eq!(b.moves[0].name, d.moves[0].name);
    assert_eq!(b.moves[0].power, 55);
    assert_eq!(b.encounters, d.encounters);
    assert_eq!(teams(&b.trainers), teams(&d.trainers));
    assert_eq!(b.shops, d.shops);
    assert_eq!(b.item_prices.get(&1), Some(&1234));
    assert_eq!(b.item_prices.get(&328), Some(&9000));
    assert_eq!(b.item_prices.get(&2), Some(&999_999));
    assert_eq!(b.item_prices.get(&329), Some(&9000));
    assert_eq!(b.learnsets, d.learnsets);
    assert_eq!(evolutions(b), evolutions(&d));
    assert_eq!(b.tm_moves, d.tm_moves);
    assert_eq!(b.statics, d.statics);
    assert_eq!(b.trades, d.trades);
    assert_eq!(b.field_items, d.field_items);
    assert_eq!(b.type_chart, d.type_chart);
    assert_eq!(b.palettes, d.palettes);
}

/// The diff of a pack's graphics over the player's, against the loaded ROM.
fn player_graphics_diff(bytes: &[u8], pack: &str) -> String {
    let mut h = load(bytes.to_vec());
    let mut d = h.game_data().clone();
    d.player_graphics = Some(ppr_model::PlayerGraphicsChoice {
        pack: pack.into(),
        replace: ppr_model::PlayerCharacter::Pc1,
    });
    h.apply(&d).expect("applies");
    crate::gb::testing::diff(bytes, h.rom.bytes())
}

/// With `PPR_TEST_ROMS`: Yellow's palettes read as the reference reads them, and graphics packs change the ROM as
/// the reference changes it; with `PPR_REFERENCE_DUMPS`, every pack the reference was run with (`cpg/y-…`).
#[test]
#[ignore = "needs a real ROM"]
fn real_yellow_palettes_and_player_graphics_match_reference() {
    use crate::gb::testing::{fnv1a, reference_player_graphics};
    let Some((path, bytes)) = real_rom("Yellow") else {
        return;
    };
    let h = load(bytes.clone());
    let palettes = h.game_data().palettes.iter().fold(String::new(), |mut out, (n, p)| {
        let _ = writeln!(out, "{n} {}", p.id.expect("a palette number"));
        out
    });
    assert_eq!(
        (fnv1a(&palettes), h.game_data().palettes.len()),
        (16_524_233_469_872_163_093, 151),
        "palettes"
    );
    assert_eq!(h.game_data().player_graphics_packs.len(), 24, "packs");
    for (pack, hash) in [
        ("red", 287_609_203_487_994_120),
        ("chris/2", 3_831_286_440_612_344_269),
        ("prof_oak", 8_819_047_523_382_005_287),
    ] {
        let diff = player_graphics_diff(&bytes, pack);
        assert_eq!(fnv1a(&diff), hash, "{pack}");
    }
    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.pal.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a Pal dump of the ROM");
        assert_eq!(palettes.trim_end(), reference.trim_end());
    }
    let mut failed = Vec::new();
    for (pack, _, reference) in reference_player_graphics("y") {
        if player_graphics_diff(&bytes, &pack) != reference {
            failed.push(pack);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

fn teams(trainers: &[Trainer]) -> Vec<Vec<(u16, u8)>> {
    trainers
        .iter()
        .map(|t| t.pokemon.iter().map(|p| (p.species, p.level)).collect())
        .collect()
}

fn evolutions(d: &GameData) -> Vec<(u16, u16, ppr_model::EvolutionType, u16)> {
    d.evolutions
        .iter()
        .map(|e| (e.from, e.to, e.kind, e.parameter))
        .collect()
}

/// With `PPR_TEST_ROMS`: every Yellow shop takes one more item, as "Add Rare Candy" gives them.
#[test]
#[ignore = "needs a real ROM"]
fn real_yellow_shops_take_a_rare_candy() {
    let Some((_, bytes)) = real_rom("Yellow") else {
        return;
    };
    let mut h = load(bytes);
    let mut d = h.game_data().clone();
    for shop in &mut d.shops {
        shop.items.push(ppr_model::RARE_CANDY);
    }
    h.apply(&d).expect("applies");
    let mut out = Vec::new();
    h.save(&mut out).expect("saves");
    assert_eq!(load(out).game_data().shops, d.shops);
}

/// The shops as "index offset bytes" lines: where each one is and what it holds.
fn shop_placement(rom: &[u8]) -> String {
    let entry = load(rom.to_vec()).entry;
    offsets(&entry, "ShopPointerOffsets")
        .iter()
        .enumerate()
        .fold(String::new(), |mut s, (i, &pointer)| {
            let at = crate::gb::calculate_offset(
                u16::from_le_bytes([rom[pointer], rom[pointer + 1]]),
                crate::gb::bank_of(pointer),
            );
            let len = usize::from(rom[at + 1]) + 3;
            let bytes = rom[at..at + len].iter().fold(String::new(), |mut h, b| {
                let _ = write!(h, "{b:02x}");
                h
            });
            let _ = writeln!(s, "{i} {at:x} {bytes}");
            s
        })
}

fn fnv1a(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// Yellow with shops grown: `extra` items in every shop, or only in shop `only`, each repeating the shop's first item.
fn grow_shops(bytes: &[u8], extra: usize, only: Option<usize>) -> (Vec<ppr_model::Shop>, Vec<u8>) {
    let mut h = load(bytes.to_vec());
    let mut d = h.game_data().clone();
    for (i, shop) in d.shops.iter_mut().enumerate() {
        if only.is_none_or(|o| o == i) {
            let first = shop.items[0];
            shop.items.extend(std::iter::repeat_n(first, extra));
        }
    }
    h.apply(&d).expect("applies");
    let mut out = Vec::new();
    h.save(&mut out).expect("saves");
    (d.shops, out)
}

/// With `PPR_TEST_ROMS`: shops that grow land where the reference puts them. The hashes are of [`shop_placement`] in
/// the reference's output (the harness's `GbShopGrow`): every shop one item longer, and the Viridian shop (in bank 7,
/// with the most free space) grown to 255 items, the most its count byte holds, which moves the other shops too.
#[test]
#[ignore = "needs a real ROM"]
fn real_yellow_shop_growth_matches_reference() {
    let Some((_, bytes)) = real_rom("Yellow") else {
        return;
    };
    for (extra, only, reference) in [
        (1, None, 0x0fd4_a26e_b767_89b0_u64),
        (250, Some(0), 0x61f5_64ef_843a_e3fa),
    ] {
        let (shops, out) = grow_shops(&bytes, extra, only);
        assert_eq!(load(out.clone()).game_data().shops, shops, "{extra} {only:?}");
        assert_eq!(fnv1a(&shop_placement(&out)), reference, "{extra} {only:?}");
    }
    let mut h = load(bytes);
    let mut d = h.game_data().clone();
    d.shops[0].items.resize(256, 4);
    assert!(h.apply(&d).is_err(), "a count byte holds at most 255 items");
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_PRICES {
        assert!(!(c::TM01..c::HM01).contains(&id), "item {id} is a TM");
        assert_eq!(ppr_model::PriceFormat::DECIMAL.check(price), Ok(()), "item {id}");
    }
}
