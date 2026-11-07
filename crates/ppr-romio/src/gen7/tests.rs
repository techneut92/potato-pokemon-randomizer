//! Tests of the Gen 7 handler against a real game (`PPR_TEST_ROMS`), and against a reference dump of it
//! (`PPR_REFERENCE_DUMPS`: a folder with the harness's dump of each game, named after the game file with `.txt`).

use std::fmt::Write as _;
use std::path::PathBuf;

use ppr_formats::ctr::{Decompression, Garc};
use ppr_model::{DexNumber, Effectiveness, ExpCurve, GameData, Type};

use super::*;

/// A real game from `PPR_TEST_ROMS` whose file name contains `name`.
fn real_rom(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("PPR_TEST_ROMS")?;
    std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains(name) && !p.to_string_lossy().contains("randomized"))
}

fn identity_of(path: &PathBuf) -> RomIdentity {
    let mut f = std::io::BufReader::new(std::fs::File::open(path).expect("readable"));
    match crate::detect(&mut f).expect("readable") {
        crate::Detection::Supported(identity) => identity,
        other @ crate::Detection::Unsupported(_) => panic!("not detected: {other:?}"),
    }
}

fn load(path: &PathBuf) -> Gen7Handler {
    let file = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(path).expect("readable"));
    Gen7Handler::open(Box::new(file), identity_of(path)).expect("loads")
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

/// The type chart order of the dump.
const CHART_ORDER: [Type; 18] = [
    Type::Normal,
    Type::Fighting,
    Type::Flying,
    Type::Grass,
    Type::Water,
    Type::Fire,
    Type::Rock,
    Type::Ground,
    Type::Psychic,
    Type::Bug,
    Type::Dragon,
    Type::Electric,
    Type::Ghost,
    Type::Poison,
    Type::Ice,
    Type::Steel,
    Type::Dark,
    Type::Fairy,
];

/// What the harness prints, from the game data.
#[expect(clippy::too_many_lines, reason = "one block per section of the dump")]
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
    // The harness lists the evolutions of the species, not those of the alternate formes.
    s.push_str("# evolutions\n");
    for sp in d.base_species() {
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
    for &x in &CHART_ORDER {
        for &y in &CHART_ORDER {
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
        let ivs: Vec<i16> = t.ivs.iter().map(|&v| i16::from(i8::from_le_bytes([v]))).collect();
        let _ = writeln!(
            s,
            "{}|{}|{}|{}|{}|{}|{}",
            t.requested.unwrap_or_default(),
            t.given,
            t.nickname,
            t.ot_name,
            t.ot_id,
            list(ivs),
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
    let _ = writeln!(
        s,
        "trainer name max {} class max {}\ntrade nick max {} ot max {}",
        d.max_trainer_name_length,
        d.max_trainer_class_name_length,
        d.max_trade_nickname_length,
        d.max_trade_ot_name_length
    );
    s
}

/// A species number as its base species; anything else as it is.
fn map_species(text: &str, base: &dyn Fn(usize) -> usize) -> String {
    text.parse::<usize>()
        .map_or_else(|_| text.to_owned(), |n| base(n).to_string())
}

/// The reference dump with what the game data deliberately leaves out taken out: the harness's lines before the
/// first section, full trade trainer IDs and the misc tweak flags go.
fn normalize_reference(dump: &str) -> String {
    let mut out = String::new();
    let mut section = "";
    for line in dump.lines() {
        if let Some(name) = line.strip_prefix("# ") {
            section = name;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if section.is_empty() {
            continue;
        }
        let line = match section {
            "species" => line.replace("(Shedinja)", ""),
            "encounters" => {
                let mut parts: Vec<String> = line.split('|').map(str::to_owned).collect();
                if parts.len() > 5 && parts[3].starts_with('-') {
                    "-".clone_into(&mut parts[3]);
                }
                parts.join("|")
            }
            "misc" if line.starts_with("tweaks") => continue,
            _ => line.to_owned(),
        };
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// Ultra Sun's formes: their entries follow the species without gaps, each forme's species number is its entry,
/// Mega Evolutions come with their stones, and the flags.
fn assert_formes(h: &Gen7Handler, d: &GameData) {
    let personal = &h.layout.personal;
    let entries: Vec<usize> = personal.formes.keys().copied().collect();
    assert_eq!(entries, (808..=975).collect::<Vec<_>>());
    for sp in d.alt_formes() {
        let base = sp.forme.base.expect("a base");
        assert_eq!(
            personal.species_of(base, sp.forme.number),
            sp.number,
            "{}",
            sp.full_name()
        );
        assert_eq!(personal.base(sp.number), base);
    }
    let alolan_rattata = d.species(personal.species_of(19, 1)).expect("Alolan Rattata");
    assert_eq!(
        (alolan_rattata.full_name().as_str(), alolan_rattata.generation()),
        ("Rattata-Alolan", 7)
    );
    let megas: Vec<&ppr_model::Species> = d.mega_evolutions().collect();
    assert_eq!(megas.len(), 48);
    assert_eq!(d.irregular_formes().len(), 65);
    assert_eq!(d.alt_formes().filter(|sp| sp.forme.cosmetic).count(), 60);
    assert!(megas.iter().all(|sp| sp.forme.irregular && sp.generation() == 6));
    assert!(
        megas
            .iter()
            .filter(|sp| sp.forme.mega_item.is_none())
            .all(|sp| sp.forme.base == Some(384))
    );
    let charizard_x = d.species(personal.species_of(6, 1)).expect("Mega Charizard X");
    assert_eq!(charizard_x.full_name(), "Charizard-Mega-X");
    let meowstic_f = personal.species_of(678, 1);
    assert_eq!(
        d.species(meowstic_f).map(ppr_model::Species::full_name).as_deref(),
        Some("Meowstic-F")
    );
    assert!(
        d.evolutions
            .iter()
            .any(|e| e.from == c::ESPURR && e.to == meowstic_f && e.kind != ppr_model::EvolutionType::None)
    );
    assert!(d.formes.trainers && d.formes.wild && d.formes.statics && d.formes.starters && d.formes.evolutions);
    let caps = h.capabilities();
    assert!(caps.functional_formes && caps.mega_evolutions && caps.wild_alt_formes);
    assert!(caps.static_alt_formes && caps.starter_alt_formes);
    // Static encounters, gifts and totems of a forme with an entry of its own are that forme's species.
    let zygarde_10 = personal.species_of(718, 1);
    assert!(
        d.statics.iter().any(|s| s.species == zygarde_10 && s.forme == 1),
        "Zygarde 10%"
    );
    assert!(
        d.statics.iter().any(|s| s.held_item.is_some()),
        "static encounters hold items"
    );
    // Totems of a forme with an entry of its own (the Totem formes, Necrozma's) are that forme's species.
    let totem_formes = d
        .totems
        .iter()
        .filter_map(|t| d.species(t.species))
        .filter(|sp| sp.forme.suffix.ends_with("-Totem"))
        .count();
    assert_eq!(totem_formes, 12, "totems in their Totem formes");
}

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` for the comparison): Ultra Sun loads as the reference reads it.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_matches_reference() {
    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let h = load(&path);
    let d = h.game_data();
    assert_eq!(d.species.len(), 807 + 168);
    assert_eq!(d.base_species().count(), 807);
    assert_eq!(d.moves.len(), 728);
    assert_eq!(d.starters, [722, 725, 728]);
    assert_eq!(d.tm_moves.len(), 100);
    assert_eq!(d.tutor_moves.len(), 67);
    let caps = h.capabilities();
    assert!(caps.move_tutors && caps.starter_held_items && caps.physical_special_split);
    assert_formes(&h, d);

    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.txt", path.file_name().expect("file").to_string_lossy());
        let reference = std::fs::read_to_string(PathBuf::from(dir).join(name)).expect("a dump of the game");
        let ours = dump(d);
        let theirs = normalize_reference(&reference);
        if let Some(out) = std::env::var_os("PPR_DUMP_OUT") {
            let out = PathBuf::from(out);
            std::fs::write(out.join("ours.txt"), &ours).expect("writes");
            std::fs::write(out.join("theirs.txt"), &theirs).expect("writes");
        }
        let differences: Vec<String> = ours
            .lines()
            .zip(theirs.lines())
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, (a, b))| format!("line {}:\n  ours   {a}\n  theirs {b}", i + 1))
            .collect();
        assert!(
            differences.is_empty(),
            "{} differences, the first:\n{}",
            differences.len(),
            differences.iter().take(25).cloned().collect::<Vec<_>>().join("\n")
        );
        assert_eq!(ours.lines().count(), theirs.lines().count());
    }
}

/// The totems as the reference harness's `Totems` prints them, by base species.
fn totems_dump(d: &GameData) -> String {
    let encounter = |species: DexNumber, level: u8, item: Option<u16>| {
        format!("{} L{level} item {}", d.base_forme_of(species), item.unwrap_or(0))
    };
    let mut out = String::new();
    for t in &d.totems {
        out.push_str(&encounter(t.species, t.level, t.held_item));
        write!(out, " aura {}", t.aura.to_byte()).expect("writes");
        for a in &t.allies {
            write!(out, " | {}", encounter(a.species, a.level, a.held_item)).expect("writes");
        }
        out.push('\n');
    }
    out
}

/// Ultra Sun's Totem Pokémon read as the reference reads them: the FNV-1a hash of the harness's `Totems` output.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_totems_match_reference() {
    const REFERENCE_HASH: u64 = 5_807_172_152_517_855_543;
    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let h = load(&path);
    let d = h.game_data();
    assert!(h.capabilities().totem_pokemon);
    assert_eq!(d.totems.len(), 18);
    assert_eq!(d.totems.iter().map(|t| t.allies.len()).sum::<usize>(), 22);
    let dump = totems_dump(d);
    let hash = dump.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    assert_eq!(hash, REFERENCE_HASH, "{dump}");
}

/// A saved game partition: the source's game partition with the 64 KiB chunks that differ laid over it, so that
/// saving a 4 GiB game needs no temporary file.
#[derive(Debug)]
struct Overlay {
    src: std::fs::File,
    /// Where the game partition starts in the source.
    base: u64,
    /// Length of the game partition.
    src_len: u64,
    chunks: std::collections::BTreeMap<u64, Vec<u8>>,
    len: u64,
    pos: u64,
}

const CHUNK: u64 = 1 << 16;

impl Overlay {
    fn new(path: &PathBuf) -> Self {
        use std::io::{Read as _, Seek as _, SeekFrom};
        let mut src = std::fs::File::open(path).expect("readable");
        let base = crate::ctr::open(std::io::BufReader::new(std::fs::File::open(path).expect("readable")))
            .expect("opens")
            .identity()
            .ncch_offset;
        let mut size = [0u8; 4];
        src.seek(SeekFrom::Start(base + 0x104)).expect("seek");
        src.read_exact(&mut size).expect("header");
        Self {
            src,
            base,
            src_len: u64::from(u32::from_le_bytes(size)) * 0x200,
            chunks: std::collections::BTreeMap::new(),
            len: 0,
            pos: 0,
        }
    }

    /// Source bytes at `at` of the partition; past its end they are zeros.
    fn read_source(&mut self, at: u64, buf: &mut [u8]) -> std::io::Result<()> {
        use std::io::{Read as _, Seek as _, SeekFrom};
        buf.fill(0);
        if at < self.src_len {
            let n = usize::try_from((self.src_len - at).min(buf.len() as u64)).expect("small");
            self.src.seek(SeekFrom::Start(self.base + at))?;
            self.src.read_exact(&mut buf[..n])?;
        }
        Ok(())
    }

    fn changed_bytes(&self) -> u64 {
        self.chunks.len() as u64 * CHUNK
    }

    /// The saved partition, for reading from its start.
    fn rewound(mut self) -> Self {
        self.pos = 0;
        self
    }
}

impl std::io::Write for Overlay {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
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

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl std::io::Read for Overlay {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
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

impl std::io::Seek for Overlay {
    fn seek(&mut self, to: std::io::SeekFrom) -> std::io::Result<u64> {
        self.pos = match to {
            std::io::SeekFrom::Start(p) => p,
            std::io::SeekFrom::End(d) => self.len.saturating_add_signed(d),
            std::io::SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
        };
        Ok(self.pos)
    }
}

/// Saves the handler's game into an overlay over the game partition of the original game at `path`.
fn save(path: &PathBuf, h: &Gen7Handler) -> Overlay {
    let mut overlay = Overlay::new(path);
    h.save(&mut overlay).expect("saves");
    overlay
}

/// The species after `n`, wrapping around.
fn next_species(n: DexNumber) -> DexNumber {
    n % 807 + 1
}

/// Changes something in every part of the game data the handler writes.
fn scramble(d: &mut GameData) {
    let bulbasaur = &mut d.species[0];
    bulbasaur.name = "Bulba".into();
    bulbasaur.stats.hp = 99;
    bulbasaur.secondary_type = Some(Type::Fire);
    bulbasaur.catch_rate = 3;
    bulbasaur.abilities.rotate_left(1);
    d.type_chart.set(Type::Normal, Type::Ghost, Effectiveness::Neutral);
    let level_up = d
        .evolutions
        .iter_mut()
        .find(|e| e.from == 1)
        .expect("Bulbasaur evolves");
    level_up.update_method(level_up.kind, 20);
    // Nincada into Charmander, and Squirtle as the extra Pokémon instead of Shedinja.
    let nincada: Vec<&mut ppr_model::Evolution> = d.evolutions.iter_mut().filter(|e| e.from == 290).collect();
    assert_eq!(nincada.len(), 2, "Ninjask and Shedinja");
    for (e, to) in nincada.into_iter().zip([4, 7]) {
        e.to = to;
    }
    let tackle = d.moves.iter_mut().find(|m| m.number == 33).expect("Tackle");
    tackle.power = 90;
    tackle.pp = 5;
    tackle.accuracy = 70;
    tackle.move_type = Some(Type::Dragon);
    if let Some(l) = d.learnsets.get_mut(&1) {
        l[0].move_id = 394;
    }
    if let Some(e) = d.egg_moves.get_mut(&1) {
        e[0] = 394;
    }
    d.tm_moves[0] = 394;
    d.tutor_moves[0] = 394;
    if let Some(c) = d.tm_hm_compat.get_mut(&4) {
        c[0] = !c[0];
    }
    if let Some(c) = d.tutor_compat.get_mut(&4) {
        c[0] = !c[0];
    }
    d.starters = vec![1, 4, 7];
    d.starter_held_items = vec![Some(1), None, Some(234)];
    for set in &mut d.encounters {
        for e in &mut set.encounters {
            e.species = next_species(e.species);
            e.forme = 0;
            e.max_level = e.max_level.saturating_add(1).min(100);
        }
    }
    // The assembled Zygarde, the last, holds nothing.
    let held = d.statics.len() - 1;
    for s in &mut d.statics[..held] {
        s.held_item = s.held_item.map_or(Some(1), |_| None);
    }
    for s in &mut d.statics {
        s.species = next_species(s.species);
        s.forme = 0;
    }
    for t in &mut d.totems {
        t.species = next_species(t.species);
        t.forme = 0;
        t.level = t.level.saturating_add(1).min(100);
        t.held_item = Some(158);
        t.aura = ppr_model::Aura::from_byte(t.aura.to_byte() % 18 + 1);
        for a in &mut t.allies {
            a.species = next_species(a.species);
            a.forme = 0;
            a.level = a.level.saturating_sub(1).max(1);
            a.held_item = None;
        }
    }
    for t in &mut d.trades {
        t.given = next_species(t.given);
        t.requested = t.requested.map(next_species);
        t.nickname = "Potato".into();
        t.ot_id = 3_000_000_000;
    }
    d.shops[0].items.reverse();
    d.item_prices.insert(17, 1230);
    d.item_prices.insert(328, 100_000);
    d.field_items.rotate_left(1);
    d.pickup_items.reverse();
    for t in &mut d.trainers {
        for p in &mut t.pokemon {
            p.species = next_species(p.species);
            p.forme = 0;
            p.level = p.level.saturating_add(1).min(100);
            p.held_item = Some(234);
            p.moves = [33, 394, 0, 0];
            p.shiny = !p.shiny;
        }
    }
}

/// Fails with the name of each part of the game data that differs. Estimated evolution levels are left out: they
/// are estimated from the game as loaded.
fn assert_same(ours: &GameData, theirs: &GameData) {
    let without_estimates = |d: &GameData| {
        let mut d = d.clone();
        d.evolution_level_samples.clear();
        for e in &mut d.evolutions {
            e.estimated_level = 0;
        }
        d
    };
    let (ours, theirs) = (&without_estimates(ours), &without_estimates(theirs));
    macro_rules! fields {
        ($($f:ident),* $(,)?) => {{
            let mut differ = Vec::new();
            $(if ours.$f != theirs.$f { differ.push(stringify!($f)); })*
            differ
        }};
    }
    let differ = fields!(
        types,
        type_chart,
        species,
        evolutions,
        starters,
        starter_held_items,
        items,
        wild_held_items,
        encounters,
        statics,
        totems,
        trades,
        moves,
        learnsets,
        egg_moves,
        tm_moves,
        tutor_moves,
        tm_hm_compat,
        tutor_compat,
        field_items,
        shops,
        item_prices,
        pickup_items,
        trainers,
        trainer_class_names,
    );
    let statics: Vec<(usize, &ppr_model::StaticEncounter, &ppr_model::StaticEncounter)> = ours
        .statics
        .iter()
        .zip(&theirs.statics)
        .enumerate()
        .filter(|(_, (a, b))| a != b)
        .map(|(i, (a, b))| (i, a, b))
        .collect();
    assert!(differ.is_empty(), "these differ: {differ:?}; statics {statics:?}");
    assert_eq!(ours, theirs);
}

/// An unchanged game saves byte for byte, also after writing the game data back unchanged.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_saves_unchanged_identically() {
    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let mut h = load(&path);
    let overlay = save(&path, &h);
    assert_eq!((overlay.len, overlay.changed_bytes()), (overlay.src_len, 0));

    let data = h.game_data().clone();
    h.apply(&data).expect("writes");
    let overlay = save(&path, &h);
    assert_eq!((overlay.len, overlay.changed_bytes()), (overlay.src_len, 0));
}

/// What is written reads back from the saved game, as a full image and as a LayeredFS folder.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_writes_read_back() {
    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let mut h = load(&path);
    let mut data = h.game_data().clone();
    scramble(&mut data);
    // Too high, and between tens: written as they fit.
    data.item_prices.insert(329, 700_000);
    data.item_prices.insert(330, 4_567);
    let personal = &h.layout.personal;
    // Alolan Raticate in the wild and Midnight Lycanroc in a team, which have entries of their own, and a cosmetic
    // forme, which shares its species' entry.
    let alolan_raticate = personal.species_of(20, 1);
    data.encounters[0].encounters[0].species = alolan_raticate;
    data.encounters[0].encounters[0].forme = 1;
    let midnight = personal.species_of(745, 1);
    data.trainers[0].pokemon[0].species = midnight;
    data.trainers[0].pokemon[0].forme = 1;
    let (cosmetic, forme) = data
        .base_species()
        .find(|sp| personal.cosmetic_counts.contains_key(&sp.number))
        .map(|sp| (sp.number, personal.cosmetic_counts[&sp.number] - 1))
        .expect("a species with cosmetic formes");
    data.trainers[1].pokemon[0].species = cosmetic;
    data.trainers[1].pokemon[0].forme = forme;
    // Alternate formes as a starter, gifts, static encounters, a totem and an ally, and a cosmetic forme of a
    // static encounter and an ally.
    let alolan_raichu = personal.species_of(26, 1);
    data.starters[1] = alolan_raichu;
    for (i, (species, forme)) in [(alolan_raticate, 1), (midnight, 1), (cosmetic, forme)]
        .into_iter()
        .enumerate()
    {
        data.statics[i].species = species;
        data.statics[i].forme = forme;
    }
    let last = data.statics.len() - 2;
    data.statics[last].species = alolan_raichu;
    data.statics[last].forme = 1;
    data.totems[0].species = midnight;
    data.totems[0].forme = 1;
    data.totems[1].allies[0].species = cosmetic;
    data.totems[1].allies[0].forme = forme;
    data.totems[2].allies[0].species = alolan_raticate;
    data.totems[2].allies[0].forme = 1;
    // A Mega Evolution's data, level-up moves and compatibility.
    let mega_venusaur = personal.species_of(3, 1);
    let at = data
        .species
        .iter()
        .position(|sp| sp.number == mega_venusaur)
        .expect("Mega Venusaur");
    data.species[at].stats.speed = 101;
    data.species[at].abilities = vec![65, 0, 34];
    data.learnsets
        .insert(mega_venusaur, vec![ppr_model::LevelUpMove { level: 1, move_id: 22 }]);
    if let Some(f) = data.tm_hm_compat.get_mut(&mega_venusaur) {
        f[3] = !f[3];
    }
    h.apply(&data).expect("writes");
    data.item_prices.insert(329, 655_350);
    data.item_prices.insert(330, 4_560);
    h.set_seed(1234);

    let overlay = save(&path, &h);
    let mut reopened = Gen7Handler::open(Box::new(overlay.rewound()), identity_of(&path)).expect("reopens");
    assert_same(reopened.game_data(), &data);
    let smdh = reopened.files.get_mut().game.smdh().expect("icon").clone();
    assert_eq!(smdh.publisher(1).as_deref(), Some(PUBLISHER));
    let description = smdh.short_description(1).expect("description");
    assert!(description.ends_with("randomizer seed: 1234"), "{description}");

    // A game saved before takes changes again; Nincada's patched code only gets the new extra species.
    let mut again = data.clone();
    for e in again.evolutions.iter_mut().filter(|e| e.from == 290 && e.to == 7) {
        e.to = 10;
    }
    reopened.apply(&again).expect("writes");
    let overlay = save(&path, &reopened);
    let third = Gen7Handler::open(Box::new(overlay.rewound()), identity_of(&path)).expect("reopens");
    assert_same(third.game_data(), &again);

    // The LayeredFS folder laid over the original game gives the same game.
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = h.save_layered_fs(dir.path()).expect("saves");
    let layered = lay_over(&path, &root);
    assert_same(layered.game_data(), &data);
}

/// The original game with a LayeredFS folder's files laid over it.
fn lay_over(path: &PathBuf, root: &std::path::Path) -> Gen7Handler {
    fn walk(dir: &std::path::Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
        for e in std::fs::read_dir(dir).expect("readable") {
            let p = e.expect("entry").path();
            let name = format!("{prefix}{}", p.file_name().expect("name").to_string_lossy());
            if p.is_dir() {
                walk(&p, &format!("{name}/"), out);
            } else {
                out.push((name, p));
            }
        }
    }
    let file = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(path).expect("readable"));
    let mut base: Image = crate::ctr::open(Box::new(file) as Box<dyn super::files::Source>).expect("opens");
    let mut files = Vec::new();
    walk(&root.join("romfs"), "", &mut files);
    for (name, p) in files {
        base.replace_file(&name, std::fs::read(p).expect("readable"))
            .expect("replaces");
    }
    if let Ok(code) = std::fs::read(root.join("code.bin")) {
        base.set_code(code).expect("sets");
    }
    Gen7Handler::load(base, None, identity_of(path)).expect("loads")
}

/// A game update replaces files of the game; its changes are read, and changes made on top of it are saved as a
/// LayeredFS folder only.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_with_an_update() {
    use ppr_formats::ctr::testing::{Cxi, cxi};

    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let mut h = load(&path);
    let (title, code, stats_path, mut stats) = {
        let files = h.files.get_mut();
        let title = u64::from_str_radix(&files.game.identity().title_id, 16).expect("hex");
        let code = files.code.clone();
        let stats_path = h.entry.path("PokemonStats").expect("entry");
        let stats = Garc::parse(files.read(&stats_path).expect("readable"), &Decompression::None).expect("parses");
        (title, code, stats_path, stats)
    };
    // The update raises Bulbasaur's HP to 77.
    let mut bulbasaur = stats.file(1).expect("Bulbasaur");
    bulbasaur[0] = 77;
    stats.set_file(1, bulbasaur).expect("exists");
    let stats = stats.to_bytes().expect("builds");
    let update = cxi(&Cxi {
        product_code: "CTR-U-A2AP",
        title_id: (title & 0xFFFF_FFFF) | 0x0004_000E_0000_0000,
        code: &code,
        compress_code: false,
        files: &[(stats_path.as_str(), &stats)],
        decrypted: true,
    });
    h.load_update(Box::new(std::io::Cursor::new(update)))
        .expect("loads the update");
    assert_eq!(h.game_data().species[0].stats.hp, 77);

    // Unchanged, there is nothing to lay over the game.
    let data = h.game_data().clone();
    h.apply(&data).expect("writes");
    let dir = tempfile::tempdir().expect("temporary folder");
    let root = h.save_layered_fs(dir.path()).expect("saves");
    assert!(!root.join("code.bin").exists());
    assert_eq!(std::fs::read_dir(root.join("romfs")).expect("folder").count(), 0);

    // Changed, the folder holds the update's files with the changes; a full image is refused.
    let mut changed = data.clone();
    scramble(&mut changed);
    h.apply(&changed).expect("writes");
    assert!(matches!(h.save(&mut std::io::sink()), Err(RomError::Unsupported(_))));
    let root = h.save_layered_fs(dir.path()).expect("saves");
    let layered = lay_over(&path, &root);
    assert_same(layered.game_data(), &changed);

    // An image of another game is not an update of this one.
    let other = cxi(&Cxi {
        product_code: "CTR-P-XXXX",
        title_id: 0x0004_0000_0005_5D00,
        code: &[0; 16],
        compress_code: false,
        files: &[],
        decrypted: true,
    });
    assert!(matches!(
        h.load_update(Box::new(std::io::Cursor::new(other))),
        Err(RomError::Unsupported(_))
    ));
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_PRICES {
        assert_eq!(ppr_model::PriceFormat::TENS.check(price), Ok(()), "item {id}");
    }
}

/// The encounter sets as the harness's `TimedEncounters` prints them.
fn timed_dump(sets: &[ppr_model::EncounterSet]) -> String {
    let mut s = String::from("# encounters\n");
    for set in sets {
        let _ = write!(
            s,
            "{}|{}|{:?}|{}|{}|{}|",
            set.name,
            set.rate,
            set.kind,
            set.map.map_or_else(|| "-".to_owned(), |m| m.to_string()),
            set.location.as_deref().unwrap_or("null"),
            set.post_game
        );
        for e in &set.encounters {
            let _ = write!(s, "{}/{}/{},", e.species, e.min_level, e.max_level);
        }
        s.push('\n');
    }
    s
}

/// The harness's dump of the sets by time of day, with alternate formes counted as their species and no map index.
fn normalize_timed_reference(dump: &str, base: &dyn Fn(usize) -> usize) -> String {
    let mut out = String::new();
    for line in dump.lines() {
        let mut parts: Vec<String> = line.split('|').map(str::to_owned).collect();
        if parts.len() > 6 {
            "-".clone_into(&mut parts[3]);
            parts[6] = parts[6]
                .split(',')
                .map(|e| {
                    let mut f: Vec<String> = e.split('/').map(str::to_owned).collect();
                    if let Some(first) = f.first_mut() {
                        *first = map_species(first, base);
                    }
                    f.join("/")
                })
                .collect::<Vec<_>>()
                .join(",");
        }
        out.push_str(&parts.join("|"));
        out.push('\n');
    }
    out
}

/// The tables (pairs of a day and a night set) whose night set differs from their day set.
fn differ(sets: &[ppr_model::EncounterSet]) -> usize {
    sets.chunks(2).filter(|p| p[0].encounters != p[1].encounters).count()
}

/// The species of each set.
fn set_species(sets: &[ppr_model::EncounterSet]) -> Vec<Vec<DexNumber>> {
    sets.iter()
        .map(|s| s.encounters.iter().map(|e| e.species).collect())
        .collect()
}

/// With `PPR_TEST_ROMS` (and `PPR_REFERENCE_DUMPS` with the harness's `TimedEncounters` dump, named after the game
/// file with `.timed.txt`, for the comparison): by time of day, every table of Ultra Sun is a day set and a night
/// set, as the reference reads them. Each half is written from its own set and reads back, and changing only the
/// day sets leaves the night halves alone.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_time_of_day_encounters() {
    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let mut h = load(&path);
    let untimed = h.game_data().encounters.clone();
    h.use_time_of_day(true).expect("reads");
    let timed = h.game_data().encounters.clone();
    assert_eq!(timed.len(), 2 * untimed.len());
    for (i, set) in timed.iter().enumerate() {
        let table = &untimed[i / 2];
        let suffix = if i % 2 == 0 { " (Day)" } else { " (Night)" };
        assert_eq!(set.name, format!("{}{suffix}", table.name));
        assert_eq!(
            (set.kind, &set.location, set.post_game),
            (table.kind, &table.location, table.post_game)
        );
        if i % 2 == 0 {
            assert_eq!(set.encounters, table.encounters, "{}", set.name);
        }
    }
    let night_tables = differ(&timed);
    assert!(night_tables > 100, "{night_tables} tables differ by night");

    if let Some(dir) = std::env::var_os("PPR_REFERENCE_DUMPS") {
        let name = format!("{}.timed.txt", path.file_name().expect("file").to_string_lossy());
        if let Ok(reference) = std::fs::read_to_string(PathBuf::from(dir).join(name)) {
            let personal = &h.layout.personal;
            let base = |n: usize| personal.base_of_number(n).map_or(n, usize::from);
            let theirs = normalize_timed_reference(&reference, &base);
            // The harness dumps the sets by time of day with base species, so ours count as their base species too.
            let ours = normalize_timed_reference(&timed_dump(&timed), &base);
            let differences: Vec<String> = ours
                .lines()
                .zip(theirs.lines())
                .enumerate()
                .filter(|(_, (a, b))| a != b)
                .map(|(i, (a, b))| format!("line {}:\n  ours   {a}\n  theirs {b}", i + 1))
                .collect();
            assert!(
                differences.is_empty(),
                "{} differences, the first:\n{}",
                differences.len(),
                differences.iter().take(10).cloned().collect::<Vec<_>>().join("\n")
            );
            assert_eq!(ours.lines().count(), theirs.lines().count());
        } else {
            eprintln!("skipped the comparison: no TimedEncounters dump of the game");
        }
    }

    // Unchanged, nothing is written in either view.
    let data = h.game_data().clone();
    h.apply(&data).expect("writes");
    let overlay = save(&path, &h);
    assert_eq!(overlay.changed_bytes(), 0);

    // By time of day: the day sets one way, the night sets another; both read back.
    let mut data = h.game_data().clone();
    for (i, set) in data.encounters.iter_mut().enumerate() {
        for e in &mut set.encounters {
            e.species = next_species(e.species);
            e.forme = 0;
            if i % 2 == 1 {
                e.species = next_species(e.species);
                e.max_level = e.max_level.saturating_add(1).min(100);
            }
        }
    }
    h.apply(&data).expect("writes");
    let mut reopened = Gen7Handler::open(Box::new(save(&path, &h).rewound()), identity_of(&path)).expect("reopens");
    let day: Vec<_> = data.encounters.iter().step_by(2).cloned().collect();
    assert_eq!(set_species(&reopened.game_data().encounters), set_species(&day));
    reopened.use_time_of_day(true).expect("reads");
    assert_eq!(reopened.game_data().encounters, data.encounters);

    // By time of day, changing only the day sets leaves every night half as it was.
    let mut h = load(&path);
    h.use_time_of_day(true).expect("reads");
    let mut data = h.game_data().clone();
    for set in data.encounters.iter_mut().step_by(2) {
        for e in &mut set.encounters {
            e.species = next_species(e.species);
            e.forme = 0;
        }
    }
    h.apply(&data).expect("writes");
    let mut reopened = Gen7Handler::open(Box::new(save(&path, &h).rewound()), identity_of(&path)).expect("reopens");
    reopened.use_time_of_day(true).expect("reads");
    let back = &reopened.game_data().encounters;
    assert_eq!(back, &data.encounters);
    let nights =
        |sets: &[ppr_model::EncounterSet]| set_species(sets).into_iter().skip(1).step_by(2).collect::<Vec<_>>();
    assert_eq!(nights(back), nights(&timed));
}

/// With `PPR_TEST_ROMS`: without time of day, the night halves of the tables written get the day's Pokémon.
#[test]
#[ignore = "needs a real ROM"]
fn real_ultra_sun_without_time_of_day_writes_the_day_by_night() {
    let Some(path) = real_rom("Ultra Sun") else {
        return;
    };
    let mut h = load(&path);
    let mut data = h.game_data().clone();
    for set in &mut data.encounters {
        for e in &mut set.encounters {
            e.species = next_species(e.species);
            e.forme = 0;
        }
    }
    h.apply(&data).expect("writes");
    let mut reopened = Gen7Handler::open(Box::new(save(&path, &h).rewound()), identity_of(&path)).expect("reopens");
    assert_eq!(reopened.game_data().encounters, data.encounters);
    reopened.use_time_of_day(true).expect("reads");
    let back = &reopened.game_data().encounters;
    assert_eq!(differ(back), 0, "every night half is its day half");
    let day: Vec<_> = back.iter().step_by(2).map(|s| s.encounters.clone()).collect();
    let written: Vec<_> = data.encounters.iter().map(|s| s.encounters.clone()).collect();
    assert_eq!(day, written);
}
