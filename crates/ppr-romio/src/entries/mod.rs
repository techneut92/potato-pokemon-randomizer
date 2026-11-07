//! Typed access to the embedded ROM entries (ADR 0003).

pub mod ini;

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ppr_model::Generation;

/// How `CopyFrom=` names the entry to copy from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyFromMode {
    /// By section name, case-insensitive (generations 1–4).
    Name,
    /// By game code (generations 5–7).
    RomCode,
}

/// Keys that every reader handles specially, so `CopyFrom` never overwrites them.
const IDENTITY_KEYS: [&str; 7] = [
    "Game",
    "Version",
    "Type",
    "NonJapanese",
    "TitleId",
    "Acronym",
    "CopyFrom",
];

/// One supported ROM, as described by a section of `genN_offsets.ini`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RomEntry {
    /// Section name, e.g. `Platinum (U)`.
    pub name: String,
    /// `Game=`: the title signature (Gen 1), game code (Gen 2–5) or product code (Gen 6–7).
    pub code: String,
    /// `Version=`, 0 when absent.
    pub version: i64,
    /// `Type=`: the game family, e.g. `Plat`.
    pub family: String,
    /// `NonJapanese=` (Gen 1–2), 0 when absent.
    pub non_japanese: i64,
    /// `TitleId=` (Gen 6–7).
    pub title_id: Option<String>,
    values: BTreeMap<String, String>,
    lists: BTreeMap<String, Vec<String>>,
}

impl RomEntry {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            code: String::new(),
            version: 0,
            family: String::new(),
            non_japanese: 0,
            title_id: None,
            values: BTreeMap::new(),
            lists: BTreeMap::new(),
        }
    }

    /// Raw value of a plain key, after `CopyFrom` was applied.
    pub fn value(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Sets a plain key to an integer, for values detected in the ROM.
    pub(crate) fn set_int(&mut self, key: &str, value: usize) {
        self.values.insert(key.to_owned(), value.to_string());
    }

    /// The plain keys starting with `prefix`, with their values, e.g. `UnusedChunk<…>` entries.
    pub fn keys_with_prefix<'a>(&'a self, prefix: &'a str) -> impl Iterator<Item = (&'a str, &'a str)> {
        self.values
            .range(prefix.to_owned()..)
            .take_while(move |(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Integer value of a plain key.
    pub fn int(&self, key: &str) -> Option<i64> {
        self.value(key).and_then(ini::parse_int)
    }

    /// Integer array value of a plain key.
    pub fn array(&self, key: &str) -> Option<Vec<i64>> {
        self.value(key).and_then(ini::parse_array)
    }

    /// All values of a repeated `Key{}` or `Key[]` key, in file order. These are not inherited through
    /// `CopyFrom`; each generation's handler decides that.
    pub fn list(&self, key: &str) -> &[String] {
        self.lists.get(key).map_or(&[], Vec::as_slice)
    }
}

/// Where one static encounter's species and levels are stored (Gen 1 to 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaticSpot {
    pub(crate) species: Vec<usize>,
    pub(crate) levels: Vec<usize>,
}

impl StaticSpot {
    /// Parses `{Species=[0x1, 0x2], Level=[0x3]}`; offsets that don't parse are skipped.
    pub(crate) fn parse(s: &str) -> Self {
        let mut spot = Self {
            species: Vec::new(),
            levels: Vec::new(),
        };
        let mut rest = s;
        while let Some(eq) = rest.find("=[") {
            let key = rest[..eq]
                .rsplit(|ch: char| !ch.is_ascii_alphabetic())
                .next()
                .unwrap_or_default();
            let Some(close) = rest[eq..].find(']') else { break };
            let offsets = rest[eq + 2..eq + close]
                .split(',')
                .filter_map(|o| ini::parse_int(o.trim()))
                .filter_map(|o| usize::try_from(o).ok())
                .collect();
            match key {
                "Species" => spot.species = offsets,
                "Level" => spot.levels = offsets,
                _ => {}
            }
            rest = &rest[eq + close..];
        }
        spot
    }
}

/// The ROM entries of one generation.
#[derive(Debug)]
pub struct Catalog {
    /// The generation these entries belong to.
    pub generation: Generation,
    /// Entries in file order.
    pub entries: Vec<RomEntry>,
    /// Problems found while reading: invalid lines and unresolved `CopyFrom` targets.
    pub problems: Vec<String>,
}

impl Catalog {
    fn read(generation: Generation, text: &str) -> Self {
        let mode = if generation.number() <= 4 {
            CopyFromMode::Name
        } else {
            CopyFromMode::RomCode
        };
        let (sections, invalid) = ini::parse(text);
        let mut problems: Vec<String> = invalid.into_iter().map(|(n, l)| format!("line {n}: {l}")).collect();
        let mut entries: Vec<RomEntry> = Vec::with_capacity(sections.len());
        for section in sections {
            let mut entry = RomEntry::new(&section.name);
            for (key, value) in section.pairs {
                match key.as_str() {
                    "Game" => entry.code.clone_from(&value),
                    "Version" => entry.version = ini::parse_int(&value).unwrap_or_default(),
                    "Type" => entry.family.clone_from(&value),
                    "NonJapanese" => entry.non_japanese = ini::parse_int(&value).unwrap_or_default(),
                    "TitleId" => entry.title_id = Some(value),
                    "CopyFrom" => {
                        let matches = |e: &&RomEntry| match mode {
                            CopyFromMode::Name => e.name.eq_ignore_ascii_case(&value),
                            CopyFromMode::RomCode => e.code == value,
                        };
                        let mut found = false;
                        // Every earlier match is copied in order, and copied values overwrite the ones read so far.
                        // Gen 3's CopyStaticPokemon is a flag of the entry's own, read before anything is copied.
                        let copy_statics = (1..=3).contains(&generation.number())
                            && entry.values.get("CopyStaticPokemon").and_then(|v| ini::parse_int(v)) == Some(1);
                        let gen5_flag = |key: &str| entry.values.get(key).and_then(|v| ini::parse_int(v)) == Some(1);
                        let gen5_copies = gen5_copied_lists(generation, gen5_flag);
                        let copy_story_texts = gen5_flag("CopyStoryText");
                        for source in entries.iter().filter(matches) {
                            // The Game Boy entries keep their own header checksum; Gen 5 story texts only come along
                            // when asked for.
                            entry.values.extend(
                                source
                                    .values
                                    .iter()
                                    .filter(|(k, _)| k.as_str() != "CRCInHeader")
                                    .filter(|(k, _)| copy_story_texts || !gen5_story_text(generation, k))
                                    .map(|(k, v)| (k.clone(), v.clone())),
                            );
                            if let Some(copies) = &gen5_copies {
                                copy_gen5_lists(&mut entry, source, copies);
                            }
                            if (1..=3).contains(&generation.number()) {
                                // Gen 1-3: statics only come along when asked, and support follows.
                                let support = if copy_statics { "1" } else { "0" };
                                entry
                                    .values
                                    .insert("StaticPokemonSupport".to_owned(), support.to_owned());
                            }
                            // Gen 3: lists come along only when a flag before CopyFrom asks for them.
                            let flag = |key: &str| entry.values.get(key).and_then(|v| ini::parse_int(v)) == Some(1);
                            let mut copied: Vec<&str> = Vec::new();
                            if flag("CopyStarterText") {
                                copied.push("StarterText[]");
                            }
                            if flag("CopyTMText") {
                                copied.extend(["TMText[]", "MoveTutorText[]"]);
                            }
                            if copy_statics {
                                copied.extend([
                                    "StaticPokemon{}",
                                    "RoamingPokemon{}",
                                    "StaticPokemonGhostMarowak{}",
                                    "StaticPokemonGameCorner{}",
                                ]);
                            }
                            if generation.number() == 2 && !copy_statics {
                                // Gen 2: no statics, so no Odd Egg either.
                                entry.values.remove("StaticPokemonOddEggOffset");
                                entry.values.remove("StaticPokemonOddEggDataSize");
                            }
                            for key in copied {
                                if let Some(list) = source.lists.get(key) {
                                    entry
                                        .lists
                                        .entry(key.to_owned())
                                        .or_default()
                                        .extend(list.iter().cloned());
                                }
                            }
                            found = true;
                        }
                        if !found {
                            problems.push(format!("[{}] CopyFrom={value}: no such entry", entry.name));
                        }
                    }
                    k if k.ends_with("{}") || k.ends_with("[]") => entry.lists.entry(key).or_default().push(value),
                    k if IDENTITY_KEYS.contains(&k) => {}
                    _ => {
                        entry.values.insert(key, value);
                    }
                }
            }
            entries.push(entry);
        }
        for p in &problems {
            tracing::warn!(%generation, "ROM entries: {p}");
        }
        Self {
            generation,
            entries,
            problems,
        }
    }
}

/// A story text key of a Gen 5 entry (`StoryText<Starter>`), which `CopyFrom` only copies with `CopyStoryText=1`.
fn gen5_story_text(generation: Generation, key: &str) -> bool {
    generation.number() == 5 && key.starts_with("StoryText<")
}

/// Copies the lists of `copies` from `source` into a Gen 5 entry, and sets its static support.
fn copy_gen5_lists(entry: &mut RomEntry, source: &RomEntry, (support, lists): &(u8, Vec<&'static str>)) {
    entry
        .values
        .insert("StaticPokemonSupport".to_owned(), support.to_string());
    for &key in lists {
        if let Some(list) = source.lists.get(key) {
            entry
                .lists
                .entry(key.to_owned())
                .or_default()
                .extend(list.iter().cloned());
        }
    }
}

/// For a Gen 5 entry: its static support after `CopyFrom` and the lists that come along, as its copy flags ask.
fn gen5_copied_lists(generation: Generation, flag: impl Fn(&str) -> bool) -> Option<(u8, Vec<&'static str>)> {
    if generation.number() != 5 {
        return None;
    }
    let mut lists = Vec::new();
    let statics = flag("CopyStaticPokemon");
    if statics {
        lists.extend(["StaticPokemon{}", "StaticPokemonFakeBall{}"]);
    }
    if flag("CopyRoamingPokemon") {
        lists.push("RoamingPokemon{}");
    }
    if flag("CopyTradeScripts") {
        lists.push("TradeScript[]");
    }
    Some((u8::from(statics), lists))
}

macro_rules! embedded {
    ($($n:literal),*) => {
        [$((
            Generation::new($n).expect("1..=7"),
            include_str!(concat!("../../data/romentries/gen", $n, "_offsets.ini")),
        )),*]
    };
}

static CATALOGS: LazyLock<Vec<Catalog>> = LazyLock::new(|| {
    embedded!(1, 2, 3, 4, 5, 6, 7)
        .into_iter()
        .map(|(g, text)| Catalog::read(g, text))
        .collect()
});

/// The parsed ROM entries of every generation, oldest first.
pub fn catalogs() -> &'static [Catalog] {
    &CATALOGS
}

/// The parsed ROM entries of one generation.
pub fn catalog(generation: Generation) -> &'static Catalog {
    let i = usize::from(generation.number() - 1);
    &CATALOGS[i]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_embedded_file_reads_cleanly() {
        for c in catalogs() {
            assert!(c.problems.is_empty(), "{}: {:?}", c.generation, c.problems);
            assert!(!c.entries.is_empty(), "{} has no entries", c.generation);
            for e in &c.entries {
                assert!(!e.code.is_empty(), "{} [{}] has no Game=", c.generation, e.name);
            }
        }
    }

    #[test]
    fn copy_from_inherits_values_but_not_identity() {
        let c = Catalog::read(
            Generation::new(4).expect("gen 4"),
            "[A]\nGame=AAAA\nType=X\nOffset=0x10\nKeep=1\n[B]\nGame=BBBB\nKeep=2\nCopyFrom=a\nOwn=3\n",
        );
        let b = &c.entries[1];
        assert_eq!((b.code.as_str(), b.family.as_str()), ("BBBB", ""));
        assert_eq!(b.int("Offset"), Some(0x10));
        // Values read before CopyFrom are overwritten by the copied ones.
        assert_eq!(b.int("Keep"), Some(1));
        assert_eq!(b.int("Own"), Some(3));
    }

    #[test]
    fn game_boy_entries_keep_their_own_header_checksum() {
        let c = Catalog::read(
            Generation::new(1).expect("gen 1"),
            "[A]\nGame=POKEMON RED\nCRCInHeader=0x1111\nX=1\n[B]\nGame=POKEMON BLUE\nCRCInHeader=0x2222\nCopyFrom=A\n",
        );
        let b = &c.entries[1];
        assert_eq!((b.int("CRCInHeader"), b.int("X")), (Some(0x2222), Some(1)));
    }

    #[test]
    fn copy_from_by_rom_code_from_gen_5() {
        let c = Catalog::read(
            Generation::new(5).expect("gen 5"),
            "[Black]\nGame=IRBO\nX=1\n[White]\nGame=IRAO\nCopyFrom=IRBO\n",
        );
        assert_eq!(c.problems, Vec::<String>::new());
        assert_eq!(c.entries[1].int("X"), Some(1));
    }

    #[test]
    fn gen_5_copies_lists_and_story_texts_only_when_asked() {
        let gen5 = catalog(Generation::new(5).expect("gen 5"));
        let entry = |name: &str| gen5.entries.iter().find(|e| e.name == name).expect(name);
        let white = entry("White (U)");
        assert_eq!(
            white.list("StaticPokemon{}").len(),
            entry("Black (U)").list("StaticPokemon{}").len()
        );
        assert!(!white.list("RoamingPokemon{}").is_empty() && !white.list("TradeScript[]").is_empty());
        assert!(white.value("StoryText<Cheren1>").is_some());
        // Black (J) copies Black (U) without its story texts.
        assert!(entry("Black (J)").value("StoryText<Starter>").is_none());
        assert_eq!(entry("Black 2 (U)").int("StaticPokemonSupport"), Some(1));
        assert_eq!(entry("Black 2 (U)").list("RoamingPokemon{}").len(), 0);
    }

    #[test]
    fn known_entries_are_present() {
        let plat = catalog(Generation::new(4).expect("gen 4"))
            .entries
            .iter()
            .find(|e| e.name == "Platinum (U)");
        let plat = plat.expect("Platinum (U)");
        assert_eq!(
            (plat.code.as_str(), plat.family.as_str(), plat.version),
            ("CPUE", "Plat", 0)
        );
        let sun = catalog(Generation::new(7).expect("gen 7"))
            .entries
            .iter()
            .find(|e| e.name == "Sun")
            .expect("Sun");
        assert_eq!(sun.title_id.as_deref(), Some("0004000000164800"));
    }

    #[test]
    fn copy_flags_bring_event_texts_along() {
        let gen3 = catalog(Generation::new(3).expect("gen 3"));
        let texts = |name: &str| {
            gen3.entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| e.list("StarterText[]").len())
                .expect(name)
        };
        assert_eq!(texts("Fire Red (U) 1.0"), 3);
        assert_eq!(texts("Leaf Green (U) 1.0"), 3, "CopyStarterText=1");
        assert_eq!(texts("Fire Red (U) 1.1"), 3);
        assert_eq!(texts("Emerald (U)"), 0);
        let statics = |name: &str| {
            gen3.entries
                .iter()
                .find(|e| e.name == name)
                .map(|e| {
                    (
                        e.list("StaticPokemon{}").len(),
                        e.list("RoamingPokemon{}").len(),
                        e.int("StaticPokemonSupport"),
                    )
                })
                .expect(name)
        };
        assert_eq!(statics("Ruby (U)"), (18, 1, Some(1)));
        assert_eq!(statics("Ruby (U, E) 1.2"), (18, 1, Some(1)), "CopyStaticPokemon=1");
        // Copied without the flag, then given its own statics and support again.
        assert_eq!(statics("Ruby (E)").2, Some(1));
    }
}
