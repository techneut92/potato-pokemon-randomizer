//! Tests of the Gen 2 handler against real ROMs (`PPR_TEST_ROMS`), and against reference dumps of them
//! (`PPR_REFERENCE_DUMPS`: a folder with the harness's `GbDump` output for each ROM, named after the ROM file
//! with `.txt`).

use std::fmt::Write as _;
use std::io::Cursor;
use std::path::PathBuf;

use ppr_model::{
    Effectiveness, EncounterSet, EvolutionType, ExpCurve, GameData, LevelUpMove, MiscTweak, Trainer, TrainerPokemon,
    Type,
};

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

fn load(bytes: Vec<u8>) -> Gen2Handler {
    let identity = match detect(&mut Cursor::new(&bytes)).expect("readable") {
        Detection::Supported(identity) => identity,
        other @ Detection::Unsupported(_) => panic!("not detected: {other:?}"),
    };
    Gen2Handler::load(bytes, identity).expect("loads")
}

fn save(h: &Gen2Handler) -> Vec<u8> {
    let mut out = Vec::new();
    h.save(&mut out).expect("saves");
    out
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

fn flags(f: &[bool]) -> String {
    f.iter().map(|&f| if f { '1' } else { '0' }).collect()
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
            "{}|{}|{:?}|{}|hp={}, attack={}, defense={}, spatk={}, spdef={}, speed={}|{}|{}",
            sp.number,
            sp.name,
            sp.primary_type,
            sp.secondary_type.map_or_else(|| "-".to_owned(), |t| format!("{t:?}")),
            b.hp,
            b.attack,
            b.defense,
            b.sp_attack,
            b.sp_defense,
            b.speed,
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
    s.push_str("# eggmoves\n");
    for (n, moves) in &d.egg_moves {
        let _ = writeln!(s, "{n}: {}", list(moves));
    }
    s.push_str("# evolutions\n");
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
    for (n, f) in &d.tm_hm_compat {
        let _ = writeln!(s, "{n}:{}", flags(f));
    }
    let _ = writeln!(s, "# tutors\n{}", list(&d.tutor_moves));
    for (n, f) in &d.tutor_compat {
        let _ = writeln!(s, "{n}:{}", flags(f));
    }
    s.push_str("# typechart\n");
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
            "{}|{}|{}|{}|{}|{}|{}",
            t.requested.unwrap_or_default(),
            t.given,
            t.nickname,
            t.ot_name,
            t.ot_id,
            list(&t.ivs),
            t.held_item.unwrap_or_default()
        );
    }
    s.push_str("# trainers\n");
    for c in &d.trainer_class_names {
        let _ = writeln!(s, "class {c}");
    }
    for t in &d.trainers {
        let _ = write!(
            s,
            "{} {} {} {} {}",
            t.index,
            t.name,
            t.class,
            t.tag.as_deref().unwrap_or("null"),
            t.force_starter_position.map_or(-1, i16::from)
        );
        for p in &t.pokemon {
            let _ = write!(s, " | {} {} {}", p.species, p.level, p.held_item.unwrap_or_default());
            for m in p.moves {
                let _ = write!(s, " {m}");
            }
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

/// A dump with the deliberate differences taken out: the Odd Egg Pokémon's level (0 in the reference, 1 here as
/// for every egg), negative map indices (no map here), and misc lines the game data doesn't hold.
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
            "statics" if reference && line.ends_with(":0") => line.replace(":0", ":1"),
            "encounters" if reference => {
                let mut parts: Vec<String> = line.split('|').map(str::to_owned).collect();
                if parts.len() > 3 && parts[3].starts_with('-') {
                    "-".clone_into(&mut parts[3]);
                }
                parts.join("|")
            }
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

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` for the comparison): Crystal loads as the reference reads it,
/// and an unchanged game saves byte for byte.
#[test]
#[ignore = "needs a real ROM"]
fn real_crystal_matches_reference() {
    let Some((path, bytes)) = real_rom("Crystal") else {
        return;
    };
    let h = load(bytes.clone());
    let d = h.game_data();
    assert_eq!(d.species.len(), 251);
    assert_eq!(d.moves.len(), 251);
    assert_eq!(d.starters, [155, 158, 152]);
    assert_eq!(d.starter_held_items.len(), 3);
    assert_eq!(d.tutor_moves, [53, 85, 58]);
    assert!(!d.fixed_trainer_text && d.trainer_name_shares_class_line);
    assert_eq!(d.max_trainer_names_total, Some(4124));
    assert_eq!(d.encounters.iter().filter(|e| e.post_game_from.is_some()).count(), 12);
    let quick_attack = d.moves.iter().find(|m| m.number == 98).expect("Quick Attack");
    assert_eq!(quick_attack.priority, 1);
    let caps = h.capabilities();
    assert!(caps.move_tutors && caps.starter_held_items && caps.english && caps.shop_sizes);
    let trades = h.edit_rules().trades;
    assert!(trades.texts);
    assert_eq!((trades.max_nickname_length, trades.max_ot_name_length), (10, 7));

    let mut unchanged = load(bytes.clone());
    let same = unchanged.game_data().clone();
    unchanged.apply(&same).expect("applies");
    assert!(save(&unchanged) == bytes, "an unchanged game saves byte for byte");

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

/// Species, level, item and moves of a trainer Pokémon.
type Member = (u16, u8, Option<u16>, [u16; 4]);

fn teams(trainers: &[Trainer]) -> Vec<Vec<Member>> {
    trainers
        .iter()
        .map(|t| {
            t.pokemon
                .iter()
                .map(|p| (p.species, p.level, p.held_item, p.moves))
                .collect()
        })
        .collect()
}

fn evolutions(d: &GameData) -> Vec<(u16, u16, EvolutionType, u16)> {
    d.evolutions
        .iter()
        .map(|e| (e.from, e.to, e.kind, e.parameter))
        .collect()
}

/// With `PPR_TEST_ROMS`: what is written to Crystal reads back, data that grows included.
#[test]
#[ignore = "needs a real ROM"]
#[expect(clippy::too_many_lines, reason = "one change per part of the game data")]
fn real_crystal_writes_read_back() {
    let Some((_, bytes)) = real_rom("Crystal") else {
        return;
    };
    let mut h = load(bytes);
    let mut d = h.game_data().clone();
    d.starters = vec![1, 4, 7];
    d.starter_held_items = vec![Some(1), None, Some(213)];
    d.species[0].name = "SEEDLING".into();
    d.species[0].stats.attack = 99;
    d.species[0].stats.sp_defense = 12;
    d.species[0].secondary_type = None;
    d.wild_held_items.insert(
        1,
        ppr_model::WildHeldItems {
            common: Some(213),
            rare: Some(1),
            ..ppr_model::WildHeldItems::default()
        },
    );
    "MEGA POTATO".clone_into(&mut d.moves[0].name);
    d.moves[0].power = 55;
    d.moves[0].accuracy = 90;
    for set in &mut d.encounters {
        for e in &mut set.encounters {
            e.species = 150;
            e.min_level = 50;
            e.max_level = if set.name == "Bug Catching Contest" { 52 } else { 50 };
        }
    }
    // The bosses get two more Pokémon with items and moves, so their classes grow; the bank has no room for that
    // much more of every trainer.
    for t in d.trainers.iter_mut().filter(|t| t.is_boss()) {
        let level = t.pokemon[0].level;
        let mut extra = TrainerPokemon::new(1, level);
        extra.held_item = Some(213);
        extra.moves = [33, 45, 0, 0];
        t.pokemon.push(extra);
        let mut reset = TrainerPokemon::new(4, level + 1);
        reset.reset_moves = true;
        t.pokemon.push(reset);
    }
    for shop in &mut d.shops {
        shop.items.push(50);
    }
    d.item_prices.insert(1, 1234);
    // Too high: written as the highest price.
    d.item_prices.insert(2, 70_000);
    d.learnsets
        .get_mut(&1)
        .expect("Bulbasaur")
        .extend([LevelUpMove::new(60, 76), LevelUpMove::new(61, 38)]);
    d.egg_moves.insert(1, vec![13, 14, 15, 16, 17, 18, 19, 20]);
    // A species without egg moves shares its empty list with others.
    d.egg_moves.insert(150, vec![94]);
    for e in d.evolutions.iter_mut().filter(|e| e.from == 1) {
        e.parameter = 10;
    }
    let pichu = d.evolutions.iter().position(|e| e.from == 172).expect("Pichu");
    d.evolutions[pichu].kind = EvolutionType::HappinessNight;
    let poliwhirl = d
        .evolutions
        .iter()
        .position(|e| e.from == 61 && e.to == 186)
        .expect("Politoed");
    d.evolutions[poliwhirl].kind = EvolutionType::Trade;
    d.evolutions[poliwhirl].parameter = 0;
    d.tm_moves[0] = 94;
    d.tm_hm_compat.get_mut(&1).expect("Bulbasaur")[56] = true;
    d.tutor_moves = vec![94, 89, 57];
    d.tutor_compat.insert(1, vec![true, false, true]);
    d.statics[0].level = 42;
    let last = d.statics.len() - 1;
    d.statics[last].species = 25;
    let game_corner = d.statics.len() - 15;
    d.statics[game_corner].species = 143;
    "SPUD".clone_into(&mut d.trades[0].nickname);
    "POTATO".clone_into(&mut d.trades[0].ot_name);
    d.trades[0].ivs = vec![15, 14, 13, 12];
    d.trades[0].held_item = Some(1);
    d.trades[0].ot_id = 4321;
    d.field_items[0] = 1;
    d.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Half);
    // A new immunity of Ghost, in place of a resistance, as the chart has no room for more.
    d.type_chart.set(Type::Fire, Type::Ghost, Effectiveness::Zero);
    d.type_chart.set(Type::Normal, Type::Rock, Effectiveness::Neutral);
    d.applied_tweaks.insert(MiscTweak::FastEggHatching);
    d.palettes.get_mut(&1).expect("Bulbasaur").normal =
        vec![ppr_model::Color::from_word(0x7C1F), ppr_model::Color::from_word(0x0421)];
    h.apply(&d).expect("applies");
    let out = save(&h);

    let back = load(out);
    let b = back.game_data();
    assert_eq!(b.starters, d.starters);
    assert_eq!(b.starter_held_items, d.starter_held_items);
    assert_eq!(b.species, d.species);
    assert_eq!(b.wild_held_items, d.wild_held_items);
    assert_eq!(b.moves[0].name, d.moves[0].name);
    assert_eq!((b.moves[0].power, b.moves[0].accuracy), (55, 90));
    assert_eq!(b.encounters, d.encounters);
    // A Pokémon to know its level-up moves is written with them.
    let mut expected = d.trainers.clone();
    for p in expected.iter_mut().flat_map(|t| &mut t.pokemon) {
        if p.reset_moves {
            p.moves = d.moves_at_level(p.species, p.level);
        }
    }
    assert_eq!(teams(&b.trainers), teams(&expected));
    assert_eq!(b.shops, d.shops);
    assert_eq!(b.item_prices.get(&1), Some(&1234));
    assert_eq!(b.item_prices.get(&2), Some(&65_535));
    assert_eq!(b.learnsets, d.learnsets);
    assert_eq!(b.egg_moves, d.egg_moves);
    assert_eq!(evolutions(b), evolutions(&d));
    assert_eq!(b.tm_moves, d.tm_moves);
    assert_eq!(b.tm_hm_compat, d.tm_hm_compat);
    assert_eq!(b.tutor_moves, d.tutor_moves);
    assert_eq!(b.tutor_compat, d.tutor_compat);
    assert_eq!(b.statics, d.statics);
    assert_eq!(b.trades, d.trades);
    assert_eq!(b.field_items, d.field_items);
    assert_eq!(b.type_chart, d.type_chart);
    assert_eq!(b.palettes, d.palettes);
    let egg_cycles = back
        .rom
        .byte(back.stats_offset(1).expect("Bulbasaur") + c::BS_EGG_CYCLES);
    assert_eq!(egg_cycles.ok(), Some(1));
}

/// With `PPR_TEST_ROMS`: the view by time of day reads every grass set three times and the time-specific fishing
/// sets, and what is written to it reads back.
#[test]
#[ignore = "needs a real ROM"]
fn real_crystal_time_of_day_encounters_read_back() {
    let Some((_, bytes)) = real_rom("Crystal") else {
        return;
    };
    let mut h = load(bytes);
    let maps = maps::load(&h.rom, &h.entry, &h.text).expect("maps");
    let timed = encounters::read(&h.rom, &h.entry, h.family, &maps, true).expect("timed view");
    let land = |sets: &[EncounterSet]| sets.iter().filter(|s| s.name.contains("Grass/Cave")).count();
    assert_eq!(land(&timed.sets), 3 * land(&h.loaded.encounters));
    assert_eq!(
        timed
            .sets
            .iter()
            .filter(|s| s.name.starts_with("Time-Specific Fishing"))
            .count(),
        11
    );
    assert!(timed.sets.iter().any(|s| s.name.ends_with("(Night)")));
    let mut sets = timed.sets.clone();
    for set in &mut sets {
        for e in &mut set.encounters {
            e.species = 201;
        }
    }
    encounters::write(&mut h.rom, &timed, &sets, &species_byte).expect("writes");
    let back = encounters::read(&h.rom, &h.entry, h.family, &maps, true).expect("timed view");
    assert_eq!(back.sets, sets);
    // The view without time of day sees the same.
    let untimed = encounters::read(&h.rom, &h.entry, h.family, &maps, false).expect("view");
    assert!(
        untimed
            .sets
            .iter()
            .flat_map(|s| &s.encounters)
            .all(|e| e.species == 201)
    );
}

/// The shops as "index offset bytes" lines: where each one is and what it holds.
fn shop_placement(rom: &[u8]) -> String {
    let table = need(&load(rom.to_vec()).entry, "ShopItemOffset").expect("shop table");
    let shops = load(rom.to_vec()).game_data().shops.len();
    (0..shops).fold(String::new(), |mut s, i| {
        let pointer = table + 2 * i;
        let at = crate::gb::calculate_offset(
            u16::from_le_bytes([rom[pointer], rom[pointer + 1]]),
            crate::gb::bank_of(pointer),
        );
        let len = usize::from(rom[at]) + 2;
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

/// With `PPR_TEST_ROMS`: shops that grow land where the reference puts them. The hashes are of [`shop_placement`] in
/// the reference's output (the harness's `GbShopGrow`): every shop one item longer, and the fourth shop grown to 255
/// items, the most its count byte holds, which moves the other shops too.
#[test]
#[ignore = "needs a real ROM"]
fn real_crystal_shop_growth_matches_reference() {
    let Some((_, bytes)) = real_rom("Crystal") else {
        return;
    };
    for (extra, only, reference) in [
        (1, None, 0x84d9_cbbd_35d9_3813_u64),
        (245, Some(3), 0x2693_b3ed_d694_7806),
    ] {
        let mut h = load(bytes.clone());
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
        assert_eq!(load(out.clone()).game_data().shops, d.shops, "{extra} {only:?}");
        assert_eq!(fnv1a(&shop_placement(&out)), reference, "{extra} {only:?}");
    }
}

/// The diff of a pack's graphics over a player character's, against the loaded ROM.
fn player_graphics_diff(bytes: &[u8], pack: &str, replace: ppr_model::PlayerCharacter) -> String {
    let mut h = load(bytes.to_vec());
    let mut d = h.game_data().clone();
    d.player_graphics = Some(ppr_model::PlayerGraphicsChoice {
        pack: pack.into(),
        replace,
    });
    h.apply(&d).expect("applies");
    crate::gb::testing::diff(bytes, h.rom.bytes())
}

/// With `PPR_TEST_ROMS`: Crystal's palettes read as the reference reads them, and graphics packs change the ROM as
/// the reference changes it, for Chris and for Kris; with `PPR_REFERENCE_DUMPS`, every pack the reference was run
/// with (`cpg/c-…`).
#[test]
#[ignore = "needs a real ROM"]
fn real_crystal_palettes_and_player_graphics_match_reference() {
    use ppr_model::PlayerCharacter::{Pc1, Pc2};

    use crate::gb::testing::{fnv1a, reference_player_graphics};
    let Some((path, bytes)) = real_rom("Crystal") else {
        return;
    };
    let h = load(bytes.clone());
    let hex = |colors: &[ppr_model::Color]| -> String {
        colors
            .iter()
            .flat_map(|c| c.to_word().to_le_bytes())
            .fold(String::new(), |mut out, b| {
                let _ = write!(out, "{b:02X}");
                out
            })
    };
    let palettes = h.game_data().palettes.iter().fold(String::new(), |mut out, (n, p)| {
        let _ = writeln!(out, "{n} {} {}", hex(&p.normal), hex(&p.shiny));
        out
    });
    assert_eq!(
        (fnv1a(&palettes), h.game_data().palettes.len()),
        (4_107_031_292_596_853_843, 251),
        "palettes"
    );
    assert_eq!(h.game_data().player_graphics_packs.len(), 47, "packs");
    for (pack, pc, hash) in [
        ("red", Pc1, 3_874_975_200_234_389_283),
        ("red", Pc2, 3_691_122_811_195_152_076),
        ("chris/2", Pc2, 10_483_927_463_598_339_610),
    ] {
        let diff = player_graphics_diff(&bytes, pack, pc);
        assert_eq!(fnv1a(&diff), hash, "{pack} {pc:?}");
    }
    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.pal.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a Pal dump of the ROM");
        assert_eq!(palettes.trim_end(), reference.trim_end());
    }
    let mut failed = Vec::new();
    for (pack, pc, reference) in reference_player_graphics("c") {
        if player_graphics_diff(&bytes, &pack, pc) != reference {
            failed.push(format!("{pack} {pc:?}"));
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_PRICES {
        assert_eq!(ppr_model::PriceFormat::WORD.check(price), Ok(()), "item {id}");
    }
}
