//! Tests of the Gen 6 handler against a real X image (`PPR_TEST_ROMS`) and a reference dump of it
//! (`PPR_REFERENCE_DUMPS`: a folder with the harness's dump of each ROM, named after the ROM file with `.txt`).
//!
//! Saved images are not written to disk: they go into an [`Overlay`] that keeps only the 64 KiB chunks that differ
//! from the source, so a 2 GiB image needs no temporary file. Run in release mode.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use ppr_model::{Effectiveness, ExpCurve, GameData, Type};

use super::*;
use crate::{Detection, detect};

/// A real image from `PPR_TEST_ROMS` whose file name contains `name`.
fn real_rom(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("PPR_TEST_ROMS")?;
    std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains(name) && !p.to_string_lossy().contains("randomized"))
}

fn identity_of(src: &mut (impl Read + Seek)) -> RomIdentity {
    src.seek(SeekFrom::Start(0)).expect("seek");
    let identity = match detect(src).expect("readable") {
        Detection::Supported(identity) => identity,
        other @ Detection::Unsupported(_) => panic!("not detected: {other:?}"),
    };
    src.seek(SeekFrom::Start(0)).expect("seek");
    identity
}

fn open(path: &PathBuf) -> Gen6Handler {
    let mut file = BufReader::with_capacity(1 << 16, File::open(path).expect("readable"));
    let identity = identity_of(&mut file);
    Gen6Handler::open(Box::new(file), identity).expect("loads")
}

/// Saves into an overlay of the source.
fn save(h: &Gen6Handler, path: &PathBuf) -> Overlay {
    let base = lock(&h.rom).as_ref().expect("loaded").image().identity().ncch_offset;
    let mut out = Overlay::new(path, base);
    h.save(&mut out).expect("saves");
    out
}

fn reopen(mut overlay: Overlay) -> Gen6Handler {
    overlay.pos = 0;
    let identity = identity_of(&mut overlay);
    Gen6Handler::open(Box::new(overlay), identity).expect("loads again")
}

/// A saved image: the source's game partition with the chunks that differ laid over it.
struct Overlay {
    src: File,
    /// Where the game partition starts in the source.
    base: u64,
    /// Length of the game partition.
    src_len: u64,
    chunks: BTreeMap<u64, Vec<u8>>,
    len: u64,
    pos: u64,
}

const CHUNK: u64 = 1 << 16;

impl Overlay {
    fn new(path: &PathBuf, base: u64) -> Self {
        let mut src = File::open(path).expect("readable");
        let mut size = [0u8; 4];
        src.seek(SeekFrom::Start(base + 0x104)).expect("seek");
        src.read_exact(&mut size).expect("header");
        Self {
            src,
            base,
            src_len: u64::from(u32::from_le_bytes(size)) * 0x200,
            chunks: BTreeMap::new(),
            len: 0,
            pos: 0,
        }
    }

    /// Reads source bytes at `at` of the partition; past its end they are zeros.
    fn read_source(&mut self, at: u64, buf: &mut [u8]) -> io::Result<()> {
        buf.fill(0);
        if at < self.src_len {
            let n = usize::try_from((self.src_len - at).min(buf.len() as u64)).expect("small");
            self.src.seek(SeekFrom::Start(self.base + at))?;
            self.src.read_exact(&mut buf[..n])?;
        }
        Ok(())
    }
}

impl Write for Overlay {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut done = 0;
        let mut source = vec![0u8; usize::try_from(CHUNK).expect("small")];
        while done < buf.len() {
            let idx = self.pos / CHUNK;
            let off = usize::try_from(self.pos % CHUNK).expect("small");
            let n = (source.len() - off).min(buf.len() - done);
            let piece = &buf[done..done + n];
            if let Some(chunk) = self.chunks.get_mut(&idx) {
                chunk[off..off + n].copy_from_slice(piece);
            } else {
                self.read_source(idx * CHUNK, &mut source)?;
                if &source[off..off + n] != piece {
                    let mut chunk = source.clone();
                    chunk[off..off + n].copy_from_slice(piece);
                    self.chunks.insert(idx, chunk);
                }
            }
            self.pos += n as u64;
            self.len = self.len.max(self.pos);
            done += n;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for Overlay {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        let idx = self.pos / CHUNK;
        let off = usize::try_from(self.pos % CHUNK).expect("small");
        let n = (usize::try_from(CHUNK).expect("small") - off)
            .min(buf.len())
            .min(usize::try_from(self.len - self.pos).unwrap_or(usize::MAX));
        if let Some(chunk) = self.chunks.get(&idx) {
            buf[..n].copy_from_slice(&chunk[off..off + n]);
        } else {
            let at = self.pos;
            self.read_source(at, &mut buf[..n])?;
        }
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Overlay {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.pos = match to {
            SeekFrom::Start(p) => p,
            SeekFrom::End(d) => self.len.saturating_add_signed(d),
            SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
        };
        Ok(self.pos)
    }
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
    // The reference's type order, with Fairy last.
    let types: Vec<Type> = crate::type_order::TYPE_ORDER
        .into_iter()
        .chain([Type::Fairy])
        .filter(|t| d.types.contains(t))
        .collect();
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

/// A dump with the deliberate differences taken out: negative map indices are no map, and lines the game data
/// doesn't hold go.
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
        if section.is_empty() || line.starts_with("error ") {
            continue;
        }
        let line = match section {
            "species" if reference => line.replace("(Shedinja)", ""),
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

/// X's formes: the files are numbered from 722 without gaps, Mega Evolutions with their stones, and the flags.
fn assert_formes(h: &Gen6Handler, d: &GameData) {
    let files: Vec<usize> = h.formes.formes.keys().copied().collect();
    assert_eq!(files, (722..=798).collect::<Vec<_>>());
    assert_eq!(
        h.formes.formes.get(&usize::from(c::MEOWSTIC_F)),
        Some(&species::Forme {
            base: c::MEOWSTIC,
            number: 1
        })
    );
    let meowstic_f = d.species(c::MEOWSTIC_F).expect("the female Meowstic");
    assert_eq!(
        (meowstic_f.full_name(), meowstic_f.base_number()),
        ("Meowstic-F".to_owned(), c::MEOWSTIC)
    );
    assert!(
        d.evolutions
            .iter()
            .any(|e| e.from == c::ESPURR && e.to == c::MEOWSTIC_F)
    );
    let megas: Vec<&ppr_model::Species> = d.mega_evolutions().collect();
    assert_eq!(megas.len(), 30);
    assert!(
        megas
            .iter()
            .all(|sp| sp.forme.mega_item.is_some() && sp.forme.irregular)
    );
    let charizard_x = d.species(761).expect("Mega Charizard X");
    assert_eq!(
        (charizard_x.full_name().as_str(), charizard_x.generation()),
        ("Charizard-Mega-X", 6)
    );
    assert_eq!(d.irregular_formes().len(), 38);
    assert_eq!(d.alt_formes().filter(|sp| sp.forme.cosmetic).count(), 21);
    assert_eq!(
        d.species(743).map(ppr_model::Species::full_name).as_deref(),
        Some("Keldeo-Resolute")
    );
    assert_eq!(d.species(666).map(|sp| sp.forme.cosmetic_formes.len()), Some(19));
    assert_eq!(d.species(493).map(|sp| sp.forme.cosmetic_formes.len()), Some(0));
    assert!(d.formes.trainers && d.formes.wild && d.formes.statics);
    assert!(h.capabilities().functional_formes && h.capabilities().mega_evolutions);
    assert!(d.statics.iter().any(|st| st.species == 729));
    // Lucario comes with its Mega Stone, so it is always replaced by a species that can Mega Evolve.
    assert!(
        d.statics
            .iter()
            .any(|st| st.species == 448 && st.held_item == Some(673))
    );
    assert!(d.formes.force_swap_static_megas);
}

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` for the comparison): X loads as the reference reads it, and an
/// unchanged game saves byte for byte.
#[test]
#[ignore = "needs a real ROM"]
fn real_x_matches_reference() {
    // A hash of "number ability1 ability2 ability3" over all species, from a reference dump.
    const ABILITIES_HASH: u64 = 11_042_066_492_218_069_078;
    let Some(path) = real_rom("Pokemon X") else {
        return;
    };
    let mut h = open(&path);
    let d = h.game_data().clone();
    assert_eq!(d.species.len(), 721 + 77);
    assert_eq!(d.base_species().count(), 721);
    assert_eq!(d.moves.len(), 617);
    assert_eq!(d.starters, [650, 653, 656, 1, 4, 7]);
    assert_eq!(d.starter_held_items.len(), 6);
    assert_eq!(d.tm_moves.len(), 100);
    assert_eq!(d.hm_moves, [15, 19, 57, 70, 127]);
    assert_eq!(d.tutor_moves, [] as [u16; 0]);
    assert_eq!(d.encounters.len(), 321);
    let caps = h.capabilities();
    assert!(caps.intro_pokemon && caps.starter_held_items && caps.english && !caps.move_tutors);

    let hash = d.base_species().fold(0u64, |h, sp| {
        std::iter::once(u64::from(sp.number))
            .chain(sp.abilities.iter().map(|&a| u64::from(a)))
            .fold(h, |h, v| h.wrapping_mul(31).wrapping_add(v))
    });
    assert_eq!(hash, ABILITIES_HASH);
    assert_eq!(d.ability_names.len(), 189);
    assert_eq!(
        (d.ability_names[25].as_str(), d.ability_names[188].as_str()),
        ("Wonder Guard", "Aura Break")
    );

    h.apply(&d).expect("applies");
    let saved = save(&h, &path);
    assert!(saved.chunks.is_empty(), "an unchanged game saves byte for byte");
    assert_eq!(saved.len, saved.src_len);

    assert_formes(&h, &d);
    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a dump of the ROM");
        let ours = normalize(&dump(&d), false);
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

/// With `PPR_TEST_ROMS`: what is written to X reads back, the tweaks apply, and a written game saves unchanged
/// again.
#[test]
#[ignore = "needs a real ROM"]
#[expect(clippy::too_many_lines, reason = "one change per part of the game data")]
fn real_x_writes_read_back() {
    let Some(path) = real_rom("Pokemon X") else {
        return;
    };
    let mut h = open(&path);
    let mut d = h.game_data().clone();
    d.starters = vec![25, 133, 147, 152, 155, 158];
    d.starter_held_items[0] = Some(213);
    d.starter_held_items[1] = None;
    d.intro_pokemon = Some(25);
    d.species[0].stats.attack = 99;
    d.species[0].secondary_type = None;
    d.species[0].abilities = vec![1, 2, 3];
    d.species[0].catch_rate = 3;
    d.wild_held_items.insert(
        1,
        ppr_model::WildHeldItems {
            common: Some(213),
            rare: Some(1),
            ..ppr_model::WildHeldItems::default()
        },
    );
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
    if let Some(moves) = d.egg_moves.get_mut(&1) {
        moves[0] = 33;
    }
    for e in d.evolutions.iter_mut().filter(|e| e.from == 1) {
        e.parameter = 10;
    }
    d.tm_moves[0] = 1;
    d.tm_moves[99] = 2;
    if let Some(f) = d.tm_hm_compat.get_mut(&1) {
        f[0] = !f[0];
        f[104] = !f[104];
    }
    for set in d.encounters.iter_mut().take(5) {
        for e in &mut set.encounters {
            e.species = 150;
            e.forme = 0;
            e.min_level = 50;
            e.max_level = 50;
        }
    }
    // Landorus's Therian Forme, which has data of its own.
    d.encounters[0].encounters[0].species = 746;
    d.encounters[0].encounters[0].forme = 1;
    // A Mega Evolution's data, level-up moves and compatibility.
    let mega = d.species.iter().position(|sp| sp.number == 760).expect("Mega Venusaur");
    d.species[mega].stats.speed = 101;
    d.species[mega].abilities = vec![65, 0, 34];
    d.learnsets
        .insert(760, vec![ppr_model::LevelUpMove { level: 1, move_id: 22 }]);
    if let Some(f) = d.tm_hm_compat.get_mut(&760) {
        f[3] = !f[3];
    }
    let falling = d
        .encounters
        .iter()
        .position(|s| s.kind == ppr_model::EncounterKind::Ambush)
        .expect("falling");
    d.encounters[falling].encounters[0].species = 151;
    d.statics[0].species = 151;
    d.statics[0].forme = 0;
    d.statics[0].level = 77;
    d.statics[0].held_item = Some(1);
    let last_static = d.statics.len() - 1;
    // Rotom's Wash Forme.
    d.statics[last_static].species = 730;
    d.statics[last_static].forme = 2;
    d.trades[0].given = 25;
    d.trades[0].requested = Some(26);
    "Sparky".clone_into(&mut d.trades[0].nickname);
    "Potato".clone_into(&mut d.trades[0].ot_name);
    d.trades[0].held_item = Some(1);
    d.trades[1].given = 4;
    // Furfrou's Heart Trim, a cosmetic forme with data of its own.
    d.trainers[0].pokemon[0].species = 750;
    d.trainers[0].pokemon[0].forme = 2;
    d.trainers[0].pokemon[0].level = 60;
    d.trainers[0].pokemon[0].held_item = Some(213);
    d.trainers[0].battle_style = ppr_model::BattleStyle::Double;
    d.trainers[0].forced_battle_style = true;
    "Potato".clone_into(&mut d.trainers[1].name);
    let last = d.trainers.len() - 1;
    d.trainers[last].pokemon[0].species = 25;
    d.trainers[last].pokemon[0].forme = 0;
    "Farmer".clone_into(&mut d.trainer_class_names[2]);
    // Not the first ones, by which the table is found.
    d.shops[2].items[5] = 50;
    d.item_prices.insert(1, 1230);
    // Above 65,535, too high, and between tens.
    d.item_prices.insert(328, 100_000);
    d.item_prices.insert(329, 700_000);
    d.item_prices.insert(330, 4_567);
    d.field_items[0] = 50;
    let hidden = d.field_items.len() - 1;
    d.field_items[hidden] = 51;
    d.pickup_items[5] = 1;
    d.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Double);
    d.guaranteed_catching = true;
    d.easier_happiness_evolutions = true;
    d.applied_tweaks = d.available_tweaks.iter().copied().collect();

    h.apply(&d).expect("applies");
    let saved = save(&h, &path);
    assert!(!saved.chunks.is_empty());
    let back = reopen(saved);
    let r = back.game_data();
    assert_eq!(r.starters, d.starters);
    assert_eq!(r.starter_held_items, d.starter_held_items);
    assert_eq!(r.species, d.species);
    assert_eq!(r.wild_held_items, d.wild_held_items);
    assert_eq!(r.moves, d.moves);
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
    assert_eq!(r.tm_hm_compat, d.tm_hm_compat);
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
    {
        let mut slot = lock(&back.rom);
        let rom = slot.as_mut().expect("loaded");
        let smdh = rom.image_mut().smdh_mut().expect("icon");
        assert_eq!(smdh.publisher(1).as_deref(), Some(PUBLISHER));
        let stats = back.paths.stats.clone();
        let espurr_line = rom.garc_file(&stats, false, usize::from(c::MEOWSTIC_F)).expect("forme");
        assert_eq!(espurr_line[c::BS_GENDER], species::MEOWSTIC_F_GENDER);
        let bulbasaur = rom.garc_file(&stats, false, 1).expect("Bulbasaur");
        assert_eq!(bulbasaur[c::BS_EGG_CYCLES], 1);
        assert_eq!(bulbasaur.u16_at(c::BS_EV_YIELD).expect("fits"), 0);
    }

    let mut again = back;
    let same = again.game_data().clone();
    again.apply(&same).expect("applies");
    let before = lock(&again.rom).as_ref().expect("loaded").image().is_modified();
    assert!(!before, "an unchanged reopened game has no changes");
}

/// With `PPR_TEST_ROMS`: an alternate forme in the intro of X loads its base species' model in that forme, with the
/// forme's own cries.
#[test]
#[ignore = "needs a real ROM"]
fn real_x_intro_shows_a_forme() {
    use super::rom::find_after;

    let Some(path) = real_rom("Pokemon X") else {
        return;
    };
    let mut h = open(&path);
    let mut d = h.game_data().clone();
    assert!(d.intro_candidates.len() > c::SPECIES_COUNT, "formes may be chosen");
    let mega = d
        .species
        .iter()
        .find(|sp| sp.is_mega() && sp.base_number() == 6)
        .expect("Mega Charizard");
    assert!(d.intro_candidates.contains(&mega.number));
    let forme = mega.forme.number;
    assert_ne!(forme, 0);
    d.intro_pokemon = Some(mega.number);
    h.apply(&d).expect("applies");

    let mut slot = lock(&h.rom);
    let rom = slot.as_mut().expect("loaded");
    let cries = find_after(rom.code(), c::CRIES_TABLE_PREFIX_XY).expect("cry table");
    let entry = |i: usize| rom.code().u32_at(cries + i * 0x14 + 4).expect("cry");
    let formes = usize::try_from(rom.code().u32_at(cries + 6 * 0x14).expect("formes")).expect("fits");
    let forme_cry = entry(formes + usize::from(forme) - 1);
    let base_cry = entry(6);
    assert_ne!(forme_cry, base_cry, "the forme has a cry of its own");
    let repeated_cry = rom
        .code()
        .u32_at(cries + (formes + usize::from(forme) - 1) * 0x14 + 0x10)
        .expect("cry");

    let intro = rom.file(&h.paths.intro).expect("intro");
    let model = find_after(intro, c::INTRO_MODEL_PREFIX_XY).expect("model");
    assert_eq!(intro.u16_at(model).expect("species"), 6);
    assert_eq!(intro[model + 2], forme);
    let cry3 = find_after(intro, c::INTRO_CRY_PREFIXES_XY[2]).expect("cry");
    assert_eq!(intro.u32_at(cry3 + 0x18).expect("cry"), forme_cry);
    let repeated = find_after(intro, c::INTRO_REPEATED_CRY_PREFIX_XY).expect("repeated cry");
    assert_eq!(intro.u32_at(repeated).expect("cry"), repeated_cry);
}

/// With `PPR_TEST_ROMS`: a shop with another number of items than X has room for fails, instead of losing items.
#[test]
#[ignore = "needs a real ROM"]
fn real_x_shops_keep_their_size() {
    let Some(path) = real_rom("Pokemon X") else {
        return;
    };
    let mut h = open(&path);
    let loaded = h.game_data().clone();
    let mut longer = loaded.clone();
    longer.shops[0].items.push(50);
    let err = h.apply(&longer).expect_err("a shop can't grow");
    assert!(err.to_string().contains("but room for"), "{err}");
    let mut shorter = loaded;
    shorter.shops[0].items.pop();
    assert!(h.apply(&shorter).is_err(), "a shop can't shrink");
}

/// Every file below `root`, as paths relative to it with `/`, sorted.
fn files_below(root: &std::path::Path) -> Vec<String> {
    let mut all = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(dir).expect("readable") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                stack.push(p);
            } else {
                all.push(
                    p.strip_prefix(root)
                        .expect("inside")
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    all.sort();
    all
}

/// With `PPR_TEST_ROMS`: a LayeredFS folder holds only the changed files at their RomFS paths, and a synthetic
/// update laid over X (new stats for Bulbasaur and a version on the title screen) is read, refuses a full image,
/// and has its changes saved as LayeredFS, which laid over the game and update gives the randomized game.
#[test]
#[ignore = "needs a real ROM"]
#[expect(clippy::too_many_lines, reason = "the update is built from the game itself")]
fn real_x_with_an_update_saves_layered_fs() {
    use ppr_formats::ctr::testing::{Cxi, cia, cxi};
    use ppr_formats::ctr::{Decompression, Garc};

    let Some(path) = real_rom("Pokemon X") else {
        return;
    };
    let dir = tempfile::tempdir().expect("temp dir");

    // Without an update: only the changed files.
    let mut h = open(&path);
    let mut d = h.game_data().clone();
    d.trainers[0].pokemon[0].level = 61;
    h.apply(&d).expect("applies");
    h.set_seed(42);
    let root = h.save_layered_fs(dir.path()).expect("saves");
    assert_eq!(root, dir.path().join("0004000000055D00"));
    assert_eq!(files_below(&root), [format!("romfs/{}", h.paths.trainer_pokemon)]);

    // An update with Bulbasaur's HP at 77 and a version on the title screen.
    let mut h = open(&path);
    let (stats_path, text_path) = (h.paths.stats.clone(), h.paths.text.clone());
    let title_screen = usize::try_from(h.entry.int("TitleScreenTextOffset").expect("entry")).expect("index");
    let version_line = usize::try_from(h.entry.int("UpdateStringOffset").expect("entry")).expect("index");
    let (code, stats, text) = {
        let mut slot = lock(&h.rom);
        let rom = slot.as_mut().expect("loaded");
        let code = rom.code().to_vec();
        let mut stats =
            Garc::parse(rom.file(&stats_path).expect("stats").to_vec(), &Decompression::None).expect("parses");
        let mut bulbasaur = stats.file(1).expect("Bulbasaur");
        bulbasaur[0] = 77;
        stats.set_file(1, bulbasaur).expect("exists");
        let mut text = Garc::parse(rom.file(&text_path).expect("text").to_vec(), &Decompression::None).expect("parses");
        let old = text.file(title_screen).expect("title screen");
        let mut lines = crate::ctr_text::read_texts(&old, true).expect("texts");
        assert!(lines.len() <= version_line, "the base game shows no version");
        lines.resize(version_line, String::new());
        lines.push("Ver. 1.5".into());
        let new = crate::ctr_text::write_texts(Some(&old), &lines, true).expect("writes");
        text.set_file(title_screen, new).expect("exists");
        (
            code,
            stats.to_bytes().expect("builds"),
            text.to_bytes().expect("builds"),
        )
    };
    let update = cia(
        &cxi(&Cxi {
            product_code: "CTR-P-EKJA",
            title_id: 0x0004_000E_0005_5D00,
            code: &code,
            compress_code: true,
            files: &[(stats_path.as_str(), &stats), (text_path.as_str(), &text)],
            decrypted: true,
        }),
        5232,
    );
    let other = cia(
        &cxi(&Cxi {
            product_code: "CTR-P-EK2A",
            title_id: 0x0004_000E_0005_5E00,
            code: &[0; 16],
            compress_code: false,
            files: &[("DllBattle.cro", b"other")],
            decrypted: true,
        }),
        5216,
    );
    assert!(matches!(
        h.load_update(Box::new(io::Cursor::new(other))),
        Err(RomError::Unsupported(_))
    ));
    assert_eq!(h.game_update_version(), None, "the game stays as it was");
    h.load_update(Box::new(io::Cursor::new(update.clone())))
        .expect("loads the update");
    assert_eq!(h.game_data().species[0].stats.hp, 77);
    assert_eq!(h.game_update_version().as_deref(), Some("Ver. 1.5"));

    // Unchanged, there is nothing to lay over the game.
    let d = h.game_data().clone();
    h.apply(&d).expect("applies");
    let root = h.save_layered_fs(dir.path()).expect("saves");
    assert_eq!(files_below(&root), [] as [&str; 0]);

    // Changed: only the changed files, from the update where it has them; no full image.
    let mut changed = d.clone();
    changed.species[0].stats.attack = 99;
    changed.trainers[0].pokemon[0].level = 61;
    h.apply(&changed).expect("applies");
    h.set_seed(42);
    assert!(matches!(h.save(&mut io::sink()), Err(RomError::Unsupported(_))));
    let root = h.save_layered_fs(dir.path()).expect("saves");
    let written = files_below(&root);
    assert!(written.contains(&format!("romfs/{stats_path}")), "{written:?}");
    assert!(
        written.contains(&format!("romfs/{}", h.paths.trainer_pokemon)),
        "{written:?}"
    );
    assert!(!written.contains(&format!("romfs/{text_path}")), "{written:?}");
    assert!(
        !written.iter().any(|f| f == "code.bin"),
        "the executable did not change"
    );

    // The console lays the folder over the game and its update.
    let file = BufReader::with_capacity(1 << 16, File::open(&path).expect("readable"));
    let mut base = crate::ctr::open(BoxedSource(Box::new(file))).expect("opens");
    let update = ppr_formats::ctr::Ncch::open(io::Cursor::new(update)).expect("opens");
    let mut update = update;
    for name in update.file_names().map(str::to_owned).collect::<Vec<_>>() {
        let data = update.read_file(&name).expect("readable");
        base.replace_file(&name, data).expect("replaces");
    }
    for name in &written {
        let data = std::fs::read(root.join(name)).expect("readable");
        base.replace_file(name.strip_prefix("romfs/").expect("a RomFS file"), data)
            .expect("replaces");
    }
    let identity = h.loaded.identity.clone();
    let rom = Rom::new(ppr_formats::ctr::Game::new(base)).expect("reads");
    let laid = Gen6Handler::load(rom, identity).expect("loads");
    assert_eq!(laid.game_data().species, changed.species);
    assert_eq!(laid.game_data().trainers[0].pokemon, changed.trainers[0].pokemon);
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_PRICES {
        assert_eq!(ppr_model::PriceFormat::TENS.check(price), Ok(()), "item {id}");
    }
}
