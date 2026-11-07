//! Tests of the Gen 5 handler: its parts on their own, and Black 2 against a real ROM (`PPR_TEST_ROMS`) and a
//! reference dump of it (`PPR_REFERENCE_DUMPS`: a folder with the harness's dump of each ROM, named after the ROM
//! file with `.txt`).

use std::fmt::Write as _;
use std::io::Cursor;
use std::path::PathBuf;

use ppr_model::{Effectiveness, ExpCurve, GameData, Type};

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

fn load(bytes: Vec<u8>) -> Gen5Handler {
    let identity = match detect(&mut Cursor::new(&bytes)).expect("readable") {
        Detection::Supported(identity) => identity,
        other @ Detection::Unsupported(_) => panic!("not detected: {other:?}"),
    };
    Gen5Handler::load(bytes, identity).expect("loads")
}

fn save(h: &Gen5Handler) -> Vec<u8> {
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
    for sp in d.base_species() {
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
            let _ = write!(s, "{}/{}/{},", e.species, e.min_level, e.max_level);
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

/// A dump with the deliberate differences taken out: empty egg move lists go, and misc lines the game data doesn't
/// hold go.
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
            "species" if reference => line.replace("(Shedinja)", ""),
            "eggmoves" if reference && line.ends_with(": []") => continue,
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

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` for the comparison): Black 2 loads as the reference reads it,
/// and an unchanged game saves byte for byte.
#[test]
#[ignore = "needs a real ROM"]
fn real_black2_matches_reference() {
    let Some((path, bytes)) = real_rom("Black Version 2") else {
        return;
    };
    let h = load(bytes.clone());
    let d = h.game_data();
    assert_eq!((d.species.len(), d.base_species().count()), (673, 649));
    for &(base, forme, number) in &c::FORME_NUMBERS {
        let s = &d.species[usize::from(number) - 1];
        assert_eq!((s.number, s.forme.base, s.forme.number), (number, Some(base), forme));
    }
    let kyurem_white = &d.species[667];
    assert_eq!(
        (kyurem_white.full_name(), kyurem_white.forme.irregular),
        ("Kyurem-White".to_owned(), true)
    );
    assert!(d.species[669].forme.cosmetic, "Keldeo's Resolute Forme");
    assert_eq!(d.irregular_formes().len(), 7);
    assert_eq!(d.species[200].forme.cosmetic_formes.len(), 27, "Unown");
    assert!(d.species[585].forme.cosmetic_formes.is_empty(), "Deerling");
    assert!(d.formes.trainers && d.formes.wild && d.formes.statics);
    assert_eq!(d.moves.len(), 559);
    assert_eq!(d.starters, [495, 498, 501]);
    let caps = h.capabilities();
    assert!(caps.move_tutors && caps.english && caps.shop_sizes && caps.time_based_encounters);

    let mut unchanged = load(bytes.clone());
    let same = unchanged.game_data().clone();
    unchanged.apply(&same).expect("applies");
    assert!(save(&unchanged) == bytes, "an unchanged game saves byte for byte");

    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a dump of the ROM");
        let ours = normalize(&dump(d), false);
        let theirs = normalize(&reference, true);
        if let Some(out) = std::env::var_os("PPR_DUMP_OUT") {
            std::fs::write(PathBuf::from(&out).join("ours.txt"), &ours).expect("writes");
            std::fs::write(PathBuf::from(&out).join("theirs.txt"), &theirs).expect("writes");
        }
        for (i, (a, b)) in ours.lines().zip(theirs.lines()).enumerate() {
            assert_eq!(a, b, "line {}", i + 1);
        }
        assert_eq!(ours.lines().count(), theirs.lines().count());
    }
}

/// With `PPR_TEST_ROMS`: what is written to Black 2 reads back, and the tweaks apply.
#[test]
#[ignore = "needs a real ROM"]
#[expect(clippy::too_many_lines, reason = "one change per part of the game data")]
fn real_black2_writes_read_back() {
    let Some((_, bytes)) = real_rom("Black Version 2") else {
        return;
    };
    let mut h = load(bytes);
    let mut d = h.game_data().clone();
    d.starters = vec![1, 4, 7];
    d.intro_pokemon = Some(25);
    d.species[0].name = "Seedling".into();
    d.species[0].stats.attack = 99;
    d.species[0].secondary_type = None;
    d.species[0].abilities = vec![1, 2, 3];
    d.wild_held_items.insert(
        1,
        ppr_model::WildHeldItems {
            common: Some(213),
            rare: Some(1),
            dark_grass: Some(50),
            ..ppr_model::WildHeldItems::default()
        },
    );
    if let Some(p) = d.palettes.get_mut(&1) {
        p.normal[1] = ppr_model::Color::from_word(0x7FFF);
    }
    "Mega Potato".clone_into(&mut d.moves[0].name);
    d.moves[0].power = 55;
    d.moves[0].accuracy = 90;
    d.moves[0].move_type = Some(Type::Fire);
    d.learnsets.insert(
        1,
        vec![
            ppr_model::LevelUpMove { level: 1, move_id: 1 },
            ppr_model::LevelUpMove { level: 5, move_id: 2 },
        ],
    );
    d.egg_moves.insert(4, vec![1, 2, 3, 4, 5]);
    d.egg_moves.insert(1, vec![33]);
    for e in d.evolutions.iter_mut().filter(|e| e.from == 1) {
        e.parameter = 10;
    }
    d.tm_moves[0] = 1;
    d.tutor_moves[0] = 2;
    if let Some(f) = d.tm_hm_compat.get_mut(&1) {
        f[0] = !f[0];
    }
    if let Some(f) = d.tutor_compat.get_mut(&1) {
        f[0] = !f[0];
    }
    for set in d.encounters.iter_mut().take(5) {
        for e in &mut set.encounters {
            e.species = 150;
            e.forme = 0;
            e.min_level = 50;
            e.max_level = 50;
        }
    }
    // The alternate formes: Kyurem-White's own data, a wild Zen Mode Darmanitan, a trainer's Heat Rotom, and a
    // static Therian Landorus.
    let white = d.species.iter_mut().find(|s| s.number == 668).expect("Kyurem-White");
    white.stats.speed = 1;
    white.primary_type = Type::Fire;
    white.abilities = vec![22, 0, 0];
    white.growth_curve = ExpCurve::Fast;
    d.learnsets
        .get_mut(&673)
        .expect("Landorus-Therian")
        .push(ppr_model::LevelUpMove::new(99, 33));
    d.tm_hm_compat.get_mut(&650).expect("Deoxys-Attack")[0] = !d.tm_hm_compat[&650][0];
    d.tutor_compat.get_mut(&650).expect("Deoxys-Attack")[1] = !d.tutor_compat[&650][1];
    if let Some(e) = d.encounters[5].encounters.first_mut() {
        e.species = 666;
        e.forme = 1;
    }
    d.statics[0].species = 150;
    d.statics[0].level = 77;
    // The Kyurem formes, which store a forme, and the last hidden hollow.
    d.statics[6].species = 673;
    d.statics[6].forme = 1;
    d.statics[7].species = 250;
    d.statics[7].forme = 0;
    let hollow = d.statics.len() - 1;
    d.statics[hollow].species = 252;
    d.statics[hollow].forme = 0;
    d.statics[hollow].level = 40;
    d.correct_static_music = true;
    d.trades[0].given = 25;
    "Sparky".clone_into(&mut d.trades[0].nickname);
    d.trades[0].held_item = Some(1);
    d.trainers[0].pokemon[0].species = 657;
    d.trainers[0].pokemon[0].forme = 1;
    d.trainers[0].pokemon[0].held_item = Some(213);
    d.trainers[0].battle_style = ppr_model::BattleStyle::Double;
    d.trainers[0].forced_battle_style = true;
    "Potato".clone_into(&mut d.trainers[1].name);
    let last = d.trainers.len() - 1;
    d.trainers[last].pokemon[0].species = 25;
    "Farmer".clone_into(&mut d.trainer_class_names[2]);
    d.shops[0].items.push(50);
    d.item_prices.insert(1, 1230);
    // Above 65,535, too high, and between tens.
    d.item_prices.insert(328, 100_000);
    d.item_prices.insert(329, 700_000);
    d.item_prices.insert(330, 4_567);
    d.field_items[0] = 50;
    // Not the first ones, by which the table is found.
    d.pickup_items[5] = 1;
    d.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Double);
    d.guaranteed_catching = true;
    d.easier_happiness_evolutions = true;
    d.applied_tweaks = d.available_tweaks.iter().copied().collect();

    h.apply(&d).expect("applies");
    let back = load(save(&h));
    let r = back.game_data();
    assert_eq!(r.starters, d.starters);
    assert_eq!(r.species, d.species);
    assert_eq!(r.wild_held_items, d.wild_held_items);
    assert_eq!(r.palettes, d.palettes);
    assert_eq!(r.moves[0].name, "Mega Potato");
    assert_eq!(
        (r.moves[0].power, r.moves[0].accuracy, r.moves[0].move_type),
        (55, 90, Some(Type::Fire))
    );
    assert_eq!(r.learnsets, d.learnsets);
    assert_eq!(r.egg_moves, d.egg_moves);
    let evolutions = |d: &GameData| -> Vec<_> {
        d.evolutions
            .iter()
            .map(|e| (e.from, e.to, e.kind, e.parameter))
            .collect()
    };
    assert_eq!(evolutions(r), evolutions(&d));
    assert_eq!(r.tm_moves, d.tm_moves);
    assert_eq!(r.tutor_moves, d.tutor_moves);
    assert_eq!(r.tm_hm_compat, d.tm_hm_compat);
    assert_eq!(r.tutor_compat, d.tutor_compat);
    assert_eq!(r.encounters, d.encounters);
    assert_eq!(r.statics, d.statics);
    assert_eq!(r.trades, d.trades);
    assert_eq!(r.trainers[0].pokemon, d.trainers[0].pokemon);
    assert_eq!(r.trainers[0].battle_style, ppr_model::BattleStyle::Double);
    assert_eq!(r.trainers[1].name, "Potato");
    assert_eq!(r.trainers[last].pokemon[0].species, 25);
    assert_eq!(r.trainer_class_names, d.trainer_class_names);
    assert_eq!(r.shops, d.shops);
    d.item_prices.insert(329, 655_350);
    d.item_prices.insert(330, 4_560);
    assert_eq!(r.item_prices, d.item_prices);
    assert_eq!(r.field_items, d.field_items);
    assert_eq!(r.pickup_items, d.pickup_items);
    assert_eq!(r.type_chart, d.type_chart);
    let mut unchanged = load(save(&h));
    let same = unchanged.game_data().clone();
    let before = save(&unchanged);
    unchanged.apply(&same).expect("applies");
    assert!(save(&unchanged) == before, "a written game saves unchanged again");
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_ITEM_PRICES {
        assert_eq!(ppr_model::PriceFormat::TENS.check(price), Ok(()), "item {id}");
    }
}
