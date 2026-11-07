//! Tests of the Gen 4 handler against real ROMs (`PPR_TEST_ROMS`), and against reference dumps of them
//! (`PPR_REFERENCE_DUMPS`: a folder with the harness's `DsDump` output for each ROM, named after the ROM file
//! with `.txt`).

use std::fmt::Write as _;
use std::io::Cursor;
use std::path::PathBuf;

use ppr_model::{Effectiveness, ExpCurve, GameData, Type};

use super::*;
use crate::type_order::TYPE_ORDER;
use crate::{Detection, detect};

/// A real ROM from `PPR_TEST_ROMS` whose file name contains `name`.
pub(super) fn real_rom(name: &str) -> Option<(PathBuf, Vec<u8>)> {
    let dir = std::env::var_os("PPR_TEST_ROMS")?;
    let path = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains(name) && !p.to_string_lossy().contains("randomized"))?;
    let bytes = std::fs::read(&path).expect("readable");
    Some((path, bytes))
}

pub(super) fn load(bytes: Vec<u8>) -> Gen4Handler {
    let identity = match detect(&mut Cursor::new(&bytes)).expect("readable") {
        Detection::Supported(identity) => identity,
        other @ Detection::Unsupported(_) => panic!("not detected: {other:?}"),
    };
    Gen4Handler::load(bytes, identity).expect("loads")
}

pub(super) fn save(h: &Gen4Handler) -> Vec<u8> {
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
    // The reference lists the base species here; the formes' data follows in the learnsets and compatibility.
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

/// A dump with the deliberate differences taken out: misc lines the game data doesn't hold.
fn normalize(dump: &str) -> String {
    let mut out = String::new();
    let mut section = "";
    for line in dump.lines() {
        if let Some(name) = line.strip_prefix("# ") {
            section = name;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let skip = match section {
            "misc" => ["tweaks", "trainer name", "trade nick"]
                .iter()
                .any(|p| line.starts_with(p)),
            _ => false,
        };
        if !skip {
            // The reference marks Shedinja's fixed HP.
            let mut line = line.replace("hp=1(Shedinja)", "hp=1");
            // A highest level equal to the lowest is printed as 0, which the reference doesn't always do, and
            // areas without a map as `-`, where the reference has negative numbers.
            if section == "encounters" {
                let mut fields: Vec<&str> = line.split('|').collect();
                if fields.get(3).is_some_and(|m| m.starts_with('-')) {
                    fields[3] = "-";
                }
                line = fields
                    .join("|")
                    .split(',')
                    .map(|e| match e.rsplit_once('/') {
                        Some((rest, max)) if rest.rsplit('/').next() == Some(max) => format!("{rest}/0"),
                        _ => e.to_owned(),
                    })
                    .collect::<Vec<_>>()
                    .join(",");
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    out
}

/// The lines of each section of a dump.
fn sections(dump: &str) -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in dump.lines() {
        if let Some(name) = line.strip_prefix("# ") {
            out.push((name.to_owned(), Vec::new()));
        } else if let Some((_, lines)) = out.last_mut() {
            lines.push(line.to_owned());
        }
    }
    out
}

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` for the comparison): Platinum loads as the reference reads it,
/// and an unchanged game saves byte for byte.
#[test]
#[ignore = "needs a real ROM"]
fn real_platinum_matches_reference() {
    let Some((path, bytes)) = real_rom("Platinum") else {
        return;
    };
    let h = load(bytes.clone());
    let d = h.game_data();
    assert_eq!(d.species.len(), 505, "493 species and 12 alternate formes");
    assert_eq!(d.base_species().count(), 493);
    let origin = d.species(499).expect("Giratina's Origin Forme");
    assert_eq!(
        (origin.forme.base, origin.forme.number, origin.full_name().as_str()),
        (Some(487), 1, "GIRATINA-Origin")
    );
    assert_eq!(d.species(201).expect("Unown").forme.cosmetic_formes.len(), 27);
    assert_eq!(d.formes.banned_for_trainers, [499]);
    assert!(h.capabilities().functional_formes);
    assert_eq!(d.moves.len(), 467);
    assert_eq!(d.starters, [387, 390, 393]);

    let mut unchanged = load(bytes.clone());
    let same = unchanged.game_data().clone();
    unchanged.apply(&same).expect("applies");
    assert!(save(&unchanged) == bytes, "an unchanged game saves byte for byte");

    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a dump of the ROM");
        let ours = sections(&normalize(&dump(d)));
        let theirs = sections(&normalize(&reference));
        let mut failures = Vec::new();
        for ((name, a), (_, b)) in ours.iter().zip(&theirs) {
            if let Some(i) = (0..a.len().max(b.len())).find(|&i| a.get(i) != b.get(i)) {
                failures.push(format!(
                    "{name}, line {}: ours {:?}, reference {:?}",
                    i + 1,
                    a.get(i),
                    b.get(i)
                ));
            }
        }
        assert_eq!(ours.len(), theirs.len());
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}

/// With `PPR_TEST_ROMS`: what is written to Platinum reads back, data that grows included, and the tweaks apply.
#[test]
#[ignore = "needs a real ROM"]
#[expect(clippy::too_many_lines, reason = "one change per part of the game data")]
fn real_platinum_writes_read_back() {
    use ppr_model::{Evolution, EvolutionType, LevelUpMove, MiscTweak};

    let Some((_, bytes)) = real_rom("Platinum") else {
        return;
    };
    let mut h = load(bytes);
    let mut d = h.game_data().clone();

    let bulbasaur = &mut d.species[0];
    bulbasaur.name = "BULBY".into();
    bulbasaur.stats.attack = 99;
    bulbasaur.primary_type = Type::Fire;
    bulbasaur.secondary_type = None;
    bulbasaur.abilities = vec![22, 0];
    bulbasaur.catch_rate = 3;
    d.wild_held_items.insert(
        1,
        WildHeldItems {
            guaranteed: Some(234),
            ..WildHeldItems::default()
        },
    );
    d.wild_held_items.insert(
        2,
        WildHeldItems {
            common: Some(155),
            rare: Some(156),
            ..WildHeldItems::default()
        },
    );

    let tackle = d.moves.iter_mut().find(|m| m.number == 33).expect("Tackle");
    tackle.power = 70;
    tackle.move_type = Some(Type::Dragon);
    tackle.accuracy = 90;
    tackle.pp = 15;
    tackle.name = "Body Slam X".into();

    d.evolutions.retain(|e| e.from != 1);
    d.evolutions.push(Evolution::new(1, 3, EvolutionType::Item, 84));
    d.evolutions.push(Evolution::new(1, 2, EvolutionType::Level, 10));
    d.learnsets
        .get_mut(&1)
        .expect("Bulbasaur")
        .extend((0..20).map(|i| LevelUpMove::new(60 + i, 300 + u16::from(i))));
    let eggs = d.egg_moves.get_mut(&1).expect("Bulbasaur's egg moves");
    eggs.pop();
    d.tm_moves[0] = 89;
    d.tutor_moves[0] = 7;
    d.tm_hm_compat.get_mut(&1).expect("Bulbasaur")[0] = !d.tm_hm_compat[&1][0];
    let tutor = d.tutor_compat.get_mut(&1).expect("Bulbasaur");
    let last = tutor.len() - 1;
    tutor[0] = !tutor[0];
    tutor[last] = !tutor[last];
    d.starters = vec![1, 4, 493];
    d.starter_held_items = vec![Some(1)];
    d.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Double);
    d.shops[0].items.extend([1, 50]);
    d.shops[1].items.push(50);
    d.item_prices.insert(1, 1234);
    d.item_prices.insert(2, 70_000);
    d.field_items[0] = 50;
    // Not the first items of the tables, which find them.
    d.pickup_items[5] = 1;
    let available = d.available_tweaks.clone();
    d.applied_tweaks.extend(available.iter().copied().filter(|t| {
        matches!(
            t,
            MiscTweak::FastestText
                | MiscTweak::NationalDexAtStart
                | MiscTweak::RunWithoutRunningShoes
                | MiscTweak::FasterHpAndExpBars
                | MiscTweak::FastDistortionWorld
                | MiscTweak::ReusableTms
                | MiscTweak::ForgettableHms
                | MiscTweak::FastEggHatching
                | MiscTweak::NoEvYields
        )
    }));
    d.guaranteed_catching = true;
    d.easier_happiness_evolutions = true;
    // The alternate formes: Giratina's Origin Forme's own data, a trainer's Pokémon that becomes Heat Rotom and a
    // Shellos of the East Sea, and Giratina's static encounter in its Origin Forme becomes Shaymin's Sky Forme.
    let origin = d.species.iter_mut().find(|s| s.number == 499).expect("Origin Forme");
    origin.stats.speed = 1;
    origin.primary_type = Type::Fire;
    origin.abilities = vec![22, 0];
    origin.growth_curve = ExpCurve::Fast;
    d.learnsets
        .get_mut(&499)
        .expect("Origin Forme")
        .push(LevelUpMove::new(99, 33));
    d.tm_hm_compat.get_mut(&505).expect("Mow Rotom")[0] = !d.tm_hm_compat[&505][0];
    d.tutor_compat.get_mut(&505).expect("Mow Rotom")[1] = !d.tutor_compat[&505][1];
    let team = &mut d.trainers[1].pokemon;
    team[0].species = 501;
    team[0].forme = 1;
    if let Some(p) = team.get_mut(1) {
        p.species = 422;
        p.forme = 1;
    }
    // Platinum's static encounters store no forme: an alternate forme is written as its base species.
    let giratina = d.statics.iter().position(|s| s.species == 487).expect("Giratina");
    d.statics[giratina].species = 500;
    d.statics[giratina].forme = 1;

    h.apply(&d).expect("applies");
    let again = load(save(&h));
    let r = again.game_data();
    assert_eq!(r.species, d.species);
    assert_eq!(r.wild_held_items, d.wild_held_items);
    assert_eq!(r.moves, d.moves);
    let mut expected = d.evolutions.clone();
    let night = expected
        .iter()
        .position(|e| e.kind == EvolutionType::Item)
        .expect("item evolution");
    expected[night].kind = EvolutionType::ItemDay;
    let mut night_evo = expected[night];
    night_evo.kind = EvolutionType::ItemNight;
    let own_end = expected.iter().rposition(|e| e.from == 1).expect("own") + 1;
    expected.insert(own_end, night_evo);
    let mut ours: Vec<_> = r
        .evolutions
        .iter()
        .map(|e| (e.from, e.to, e.kind, e.parameter))
        .collect();
    let mut theirs: Vec<_> = expected.iter().map(|e| (e.from, e.to, e.kind, e.parameter)).collect();
    ours.sort_unstable();
    theirs.sort_unstable();
    assert_eq!(ours, theirs);
    assert_eq!(r.learnsets, d.learnsets);
    assert_eq!(r.egg_moves, d.egg_moves);
    assert_eq!(r.tm_moves, d.tm_moves);
    assert_eq!(r.tutor_moves, d.tutor_moves);
    assert_eq!(r.tm_hm_compat, d.tm_hm_compat);
    assert_eq!(r.tutor_compat, d.tutor_compat);
    assert_eq!(r.starters, d.starters);
    assert_eq!(r.starter_held_items, d.starter_held_items);
    assert_eq!(r.type_chart, d.type_chart);
    assert_eq!(r.shops, d.shops);
    // Too high: written as the highest price.
    d.item_prices.insert(2, 65_535);
    assert_eq!(r.item_prices, d.item_prices);
    assert_eq!(r.field_items, d.field_items);
    assert_eq!(r.pickup_items, d.pickup_items);
    assert_eq!(r.items, d.items);
    assert_eq!(r.trainers[1].pokemon, d.trainers[1].pokemon);
    assert_eq!(
        (r.statics[giratina].species, r.statics[giratina].forme),
        (492, 0),
        "Shaymin, as the Sky Forme can't be stored"
    );

    // A second save of what was read back changes nothing.
    let mut twice = load(save(&h));
    let same = twice.game_data().clone();
    twice.apply(&same).expect("applies");
    let (again, once) = (save(&twice), save(&h));
    assert_eq!(again.len(), once.len());
    assert_eq!(
        again.iter().zip(&once).position(|(x, y)| x != y),
        None,
        "first differing offset"
    );
}

/// "number normal shiny" lines of the palettes, each palette as the hex of its color words, as the harness's `Pal 4`
/// prints them.
fn palettes_dump(d: &GameData) -> String {
    let hex = |colors: &[ppr_model::Color]| {
        colors
            .iter()
            .flat_map(|c| c.to_word().to_le_bytes())
            .fold(String::new(), |mut s, b| {
                write!(s, "{b:02X}").expect("writes to a string");
                s
            })
    };
    d.palettes.iter().fold(String::new(), |mut out, (n, p)| {
        writeln!(out, "{n} {} {}", hex(&p.normal), hex(&p.shiny)).expect("writes to a string");
        out
    })
}

/// With `PPR_TEST_ROMS`: Platinum's palettes are what the reference reads, the formes' from the other graphics
/// archive included, and changed ones read back while the rest of the archives stays as it was.
#[test]
#[ignore = "needs a real ROM"]
fn real_platinum_palettes_match_reference_and_read_back() {
    // FNV-1a of palettes_dump, from the reference harness (`Pal 4`) on Platinum (Europe).
    const REFERENCE_HASH: u64 = 13_690_910_316_141_181_476;
    let Some((_, bytes)) = real_rom("Platinum") else {
        return;
    };
    let mut h = load(bytes);
    let d = h.game_data().clone();
    assert_eq!(d.palette_files.as_deref(), Some("DP"));
    assert_eq!(d.palettes.len(), 493);
    assert!(d.palettes.values().all(|p| p.normal.len() == 16 && p.shiny.len() == 16));
    let hash = palettes_dump(&d).bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    assert_eq!(hash, REFERENCE_HASH);

    let mut changed = d.clone();
    // Bulbasaur in the graphics archive, Castform and Shaymin in the other one.
    for n in [1, 351, 492] {
        let p = changed.palettes.get_mut(&n).expect("has palettes");
        p.normal = vec![ppr_model::Color::rgb(255, 0, 0); 16];
        p.shiny = vec![ppr_model::Color::rgb(0, 0, 255); 16];
    }
    h.apply(&changed).expect("applies");
    let again = load(save(&h));
    assert_eq!(again.game_data().palettes, changed.palettes);
    // The formes keep theirs: sunny Castform and Sky Shaymin.
    let mut ours = load(save(&h));
    let mut theirs = load(real_rom("Platinum").expect("still there").1);
    let other = theirs.rom.entry.file("OtherPokemonGraphics").expect("archive");
    for file in [159, 230] {
        assert_eq!(
            ours.rom.files.narc_file(&other, file).expect("file").clone(),
            theirs.rom.files.narc_file(&other, file).expect("file").clone()
        );
    }
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_ITEM_PRICES {
        assert_eq!(ppr_model::PriceFormat::WORD.check(price), Ok(()), "item {id}");
    }
}
