//! The Gen 4 parts of the ROM entries that the shared reader keeps as plain lists: static encounters (with the
//! Game Corner prizes among them, in file order), roamers, TM texts, the Battle Frontier TM tutors and the Marill
//! cry scripts. They follow `CopyFrom` by their own rules:
//!
//! - static encounters are copied when `CopyStaticPokemon=1` came before `CopyFrom`, and support follows that flag;
//!   `IgnoreGameCornerStatics=1` then drops the Game Corner prizes;
//! - roamers are copied with `CopyRoamingPokemon=1`, TM texts with `CopyText=1`;
//! - Marill cry scripts are always copied.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ppr_model::Generation;

use super::constants::Family;
use crate::RomError;
use crate::entries::{RomEntry, catalog, ini};

/// An offset in a file of a NARC: `file:offset` in the ROM entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InFile {
    pub(super) file: usize,
    pub(super) offset: usize,
}

/// What kind of static encounter an entry describes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StaticKind {
    /// A plain one, perhaps with its forme stored too.
    Plain { forme: Vec<InFile> },
    /// A Game Corner prize, whose name is in these texts (`text file:string number`).
    GameCorner { text: Vec<InFile> },
}

/// A static encounter: where its species and levels are written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StaticSlot {
    pub(super) species: Vec<InFile>,
    pub(super) levels: Vec<InFile>,
    pub(super) kind: StaticKind,
}

/// A roamer: its species and level in the ARM9 code, and its species and gender in scripts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Roamer {
    pub(super) species_code: Vec<usize>,
    pub(super) level_code: Vec<usize>,
    pub(super) script: Vec<InFile>,
    pub(super) gender: Vec<InFile>,
}

/// A text of the game: string `string` of text file `file`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TextRef {
    pub(super) file: usize,
    pub(super) string: usize,
}

/// One Gen 4 ROM entry: the plain values and the lists.
#[derive(Debug, Clone)]
pub(super) struct Gen4Entry {
    pub(super) entry: RomEntry,
    pub(super) family: Family,
    pub(super) static_support: bool,
    pub(super) statics: Vec<StaticSlot>,
    pub(super) roamers: Vec<Roamer>,
    /// Texts naming a TM's move, by TM number.
    pub(super) tm_texts: BTreeMap<usize, Vec<TextRef>>,
    /// The Game Corner's TM prize names, by TM number.
    pub(super) tm_texts_game_corner: BTreeMap<usize, TextRef>,
    /// Where the Battle Frontier script names a TM's move, by TM number.
    pub(super) frontier_tm_scripts: BTreeMap<usize, usize>,
    /// The Battle Frontier's TM prize names (strings of `MiscUITextOffset`), by TM number.
    pub(super) frontier_tm_texts: BTreeMap<usize, usize>,
    pub(super) marill_cry_scripts: Vec<InFile>,
}

impl Gen4Entry {
    /// The entry named `name`.
    pub(super) fn named(name: &str) -> Result<Self, RomError> {
        let entry = catalog(generation())
            .entries
            .iter()
            .find(|e| e.name == name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {name}")))?;
        let family = Family::parse(&entry.family)
            .ok_or_else(|| RomError::Malformed(format!("unknown Gen 4 game type {}", entry.family)))?;
        let lists = LISTS
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, l)| l.clone())
            .unwrap_or_default();
        Ok(Self {
            entry,
            family,
            static_support: lists.static_support,
            statics: lists.statics,
            roamers: lists.roamers,
            tm_texts: lists.tm_texts,
            tm_texts_game_corner: lists.tm_texts_game_corner,
            frontier_tm_scripts: lists.frontier_tm_scripts,
            frontier_tm_texts: lists.frontier_tm_texts,
            marill_cry_scripts: lists.marill_cry_scripts,
        })
    }

    /// The section name, e.g. `Platinum (U)`.
    pub(super) fn name(&self) -> &str {
        &self.entry.name
    }

    /// An integer value; 0 when absent, as the games' code expects.
    pub(super) fn int(&self, key: &str) -> usize {
        self.entry
            .int(key)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or_default()
    }

    /// An integer value that must be there.
    pub(super) fn need(&self, key: &str) -> Result<usize, RomError> {
        self.entry
            .int(key)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no valid {key}", self.name())))
    }

    /// A string value; empty when absent.
    pub(super) fn string(&self, key: &str) -> &str {
        self.entry.value(key).unwrap_or_default()
    }

    /// An array value; empty when absent.
    pub(super) fn array(&self, key: &str) -> Vec<usize> {
        self.entry
            .array(key)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| usize::try_from(v).ok())
            .collect()
    }

    /// Whether the entry names a patch for `key` (`…Tweak=`).
    pub(super) fn has_tweak(&self, key: &str) -> bool {
        self.entry.value(key).is_some()
    }

    /// The path of the file `File<key>` names.
    pub(super) fn file(&self, key: &str) -> Result<String, RomError> {
        let value = self
            .entry
            .value(&format!("File<{key}>"))
            .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] names no file {key}", self.name())))?;
        let inner = value.trim().trim_start_matches('<').trim_end_matches('>');
        Ok(inner.split(',').next().unwrap_or_default().trim().to_owned())
    }

    /// Whether the entry names the file `File<key>`.
    pub(super) fn has_file(&self, key: &str) -> bool {
        self.entry.value(&format!("File<{key}>")).is_some()
    }
}

fn generation() -> Generation {
    Generation::new(4).unwrap_or(Generation::ALL[3])
}

/// The lists of one entry, after `CopyFrom`.
#[derive(Debug, Clone, Default)]
struct Lists {
    static_support: bool,
    statics: Vec<StaticSlot>,
    roamers: Vec<Roamer>,
    tm_texts: BTreeMap<usize, Vec<TextRef>>,
    tm_texts_game_corner: BTreeMap<usize, TextRef>,
    frontier_tm_scripts: BTreeMap<usize, usize>,
    frontier_tm_texts: BTreeMap<usize, usize>,
    marill_cry_scripts: Vec<InFile>,
}

/// The lists of every entry, by section name, read from the embedded file once.
static LISTS: LazyLock<Vec<(String, Lists)>> = LazyLock::new(|| {
    let (sections, _) = ini::parse(include_str!("../../data/romentries/gen4_offsets.ini"));
    let mut done: Vec<(String, Lists)> = Vec::new();
    for section in sections {
        let mut lists = Lists::default();
        let (mut copy_statics, mut copy_roamers, mut copy_text, mut ignore_game_corner) = (false, false, false, false);
        let flag = |v: &str| ini::parse_int(v).is_some_and(|n| n > 0);
        for (key, value) in &section.pairs {
            match key.as_str() {
                "StaticPokemonSupport" => lists.static_support = flag(value),
                "CopyStaticPokemon" => copy_statics = flag(value),
                "CopyRoamingPokemon" => copy_roamers = flag(value),
                "CopyText" => copy_text = flag(value),
                "IgnoreGameCornerStatics" => ignore_game_corner = flag(value),
                "StaticPokemon{}" => lists.statics.push(parse_static(value, false)),
                "StaticPokemonGameCorner{}" => lists.statics.push(parse_static(value, true)),
                "RoamingPokemon{}" => lists.roamers.push(parse_roamer(value)),
                "TMText{}" => parse_tm_text(value, &mut lists.tm_texts),
                "TMTextGameCorner{}" => parse_tm_text_game_corner(value, &mut lists.tm_texts_game_corner),
                "FrontierScriptTMOffsets{}" => parse_number_pairs(value, &mut lists.frontier_tm_scripts),
                "FrontierTMText{}" => parse_number_pairs(value, &mut lists.frontier_tm_texts),
                "CopyFrom" => {
                    for (_, other) in done.iter().filter(|(n, _)| n.eq_ignore_ascii_case(value)) {
                        if copy_statics {
                            lists.statics.extend(other.statics.iter().cloned());
                            if ignore_game_corner {
                                lists.statics.retain(|s| matches!(s.kind, StaticKind::Plain { .. }));
                            }
                        }
                        lists.static_support = copy_statics;
                        if copy_roamers {
                            lists.roamers.extend(other.roamers.iter().cloned());
                        }
                        if copy_text {
                            lists
                                .tm_texts
                                .extend(other.tm_texts.iter().map(|(k, v)| (*k, v.clone())));
                            lists.tm_texts_game_corner.extend(other.tm_texts_game_corner.clone());
                            lists.frontier_tm_scripts.extend(other.frontier_tm_scripts.clone());
                            lists.frontier_tm_texts.extend(other.frontier_tm_texts.clone());
                        }
                        lists
                            .marill_cry_scripts
                            .extend(other.marill_cry_scripts.iter().copied());
                    }
                }
                k if k.ends_with("MarillCryScripts") => lists.marill_cry_scripts = parse_in_files(value),
                _ => {}
            }
        }
        done.push((section.name, lists));
    }
    done
});

/// The text between the first `[` and the `]` after it.
fn bracketed(s: &str) -> Option<&str> {
    let start = s.find('[')? + 1;
    let end = start + s[start..].find(']')?;
    Some(&s[start..end])
}

/// `a:b, c:d` as offsets in files; parts that don't parse are skipped.
fn parse_in_files(s: &str) -> Vec<InFile> {
    let inner = bracketed(s).unwrap_or(s);
    inner
        .split(',')
        .filter_map(|part| {
            let (file, offset) = part.split_once(':')?;
            Some(InFile {
                file: usize::try_from(ini::parse_int(file)?).ok()?,
                offset: usize::try_from(ini::parse_int(offset)?).ok()?,
            })
        })
        .collect()
}

/// `0x1, 0x2` as numbers.
fn parse_numbers(s: &str) -> Vec<usize> {
    s.split(',')
        .filter_map(|p| ini::parse_int(p).and_then(|n| usize::try_from(n).ok()))
        .collect()
}

/// `Key=[…]` groups of a value like `{Species=[1:0x2], Level=[1:0x3]}`, in order.
fn groups(s: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(eq) = rest.find("=[") {
        let key = rest[..eq]
            .rsplit(|ch: char| !ch.is_ascii_alphabetic())
            .next()
            .unwrap_or_default();
        let Some(close) = rest[eq..].find(']') else { break };
        out.push((key, &rest[eq + 2..eq + close]));
        rest = &rest[eq + close..];
    }
    out
}

fn parse_static(s: &str, game_corner: bool) -> StaticSlot {
    let (mut species, mut levels, mut forme, mut text) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for (key, inner) in groups(s) {
        let entries = parse_in_files(&format!("[{inner}]"));
        match key {
            "Species" => species = entries,
            "Level" => levels = entries,
            "Forme" if !game_corner => forme = entries,
            "Text" if game_corner => text = entries,
            _ => {}
        }
    }
    StaticSlot {
        species,
        levels,
        kind: if game_corner {
            StaticKind::GameCorner { text }
        } else {
            StaticKind::Plain { forme }
        },
    }
}

fn parse_roamer(s: &str) -> Roamer {
    let mut roamer = Roamer::default();
    for (key, inner) in groups(s) {
        match key {
            "Species" => roamer.species_code = parse_numbers(inner),
            "Level" => roamer.level_code = parse_numbers(inner),
            "Script" => roamer.script = parse_in_files(&format!("[{inner}]")),
            "Gender" => roamer.gender = parse_in_files(&format!("[{inner}]")),
            _ => {}
        }
    }
    roamer
}

/// `{27=[561:10], 56=[517:0, 517:1]}`: the texts naming each TM's move.
fn parse_tm_text(s: &str, out: &mut BTreeMap<usize, Vec<TextRef>>) {
    let mut rest = s;
    while let Some(eq) = rest.find("=[") {
        let number = rest[..eq]
            .rsplit(|ch: char| !ch.is_ascii_digit())
            .next()
            .and_then(|n| n.parse::<usize>().ok());
        let Some(close) = rest[eq..].find(']') else { break };
        let texts = parse_in_files(&rest[eq + 1..=eq + close])
            .into_iter()
            .map(|f| TextRef {
                file: f.file,
                string: f.offset,
            })
            .collect();
        if let Some(n) = number {
            out.insert(n, texts);
        }
        rest = &rest[eq + close..];
    }
}

/// `{90=[603:14], 75=[603:15]}`: one text per TM.
fn parse_tm_text_game_corner(s: &str, out: &mut BTreeMap<usize, TextRef>) {
    let mut texts = BTreeMap::new();
    parse_tm_text(s, &mut texts);
    for (n, t) in texts {
        if let Some(&first) = t.first() {
            out.insert(n, first);
        }
    }
}

/// `{40=0xC5C, 31=0xC7A}`.
fn parse_number_pairs(s: &str, out: &mut BTreeMap<usize, usize>) {
    let inner = s.trim().trim_start_matches('{').trim_end_matches('}');
    for part in inner.split(',') {
        if let Some((k, v)) = part.split_once('=')
            && let (Some(k), Some(v)) = (ini::parse_int(k), ini::parse_int(v))
            && let (Ok(k), Ok(v)) = (usize::try_from(k), usize::try_from(v))
        {
            out.insert(k, v);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entry_reads_with_its_lists() {
        for e in &catalog(generation()).entries {
            let g4 = Gen4Entry::named(&e.name).expect(&e.name);
            if g4.static_support {
                assert!(!g4.statics.is_empty(), "{}", e.name);
            }
        }
        let plat = Gen4Entry::named("Platinum (U)").expect("Platinum (U)");
        assert_eq!(plat.family, Family::Platinum);
        assert!(plat.static_support);
        assert_eq!(plat.statics.len(), 20);
        assert_eq!(
            plat.statics[0].species[..2],
            [
                InFile {
                    file: 291,
                    offset: 0x43
                },
                InFile {
                    file: 291,
                    offset: 0x52
                }
            ]
        );
        assert_eq!(plat.roamers.len(), 5);
        assert_eq!(plat.roamers[0].species_code, [0x0010_24B0, 0x0010_2514]);
        assert_eq!(
            plat.tm_texts[&56],
            [TextRef { file: 517, string: 0 }, TextRef { file: 517, string: 1 }]
        );
        assert_eq!(
            plat.file("PokemonStats").expect("file"),
            "poketool/personal/pl_personal.narc"
        );
        // Copied from Diamond (U), whose statics it doesn't take.
        assert_eq!(plat.int("StarterPokemonOvlNumber"), 78);
        let plat_e = Gen4Entry::named("Platinum (E)").expect("Platinum (E)");
        assert_eq!(plat_e.statics, plat.statics);
        assert_eq!(plat_e.roamers, plat.roamers);
    }

    #[test]
    fn heartgold_keeps_game_corner_prizes_in_file_order() {
        let hg = Gen4Entry::named("HeartGold (U)").expect("HeartGold (U)");
        let first_prize = hg
            .statics
            .iter()
            .position(|s| matches!(s.kind, StaticKind::GameCorner { .. }))
            .expect("prizes");
        assert_eq!(first_prize, 38);
        assert!(matches!(&hg.statics[first_prize].kind, StaticKind::GameCorner { text } if text.len() == 1));
        assert_eq!(hg.marill_cry_scripts.len(), 4);
        assert_eq!(hg.tm_texts_game_corner[&90], TextRef { file: 603, string: 14 });
        assert_eq!(hg.frontier_tm_scripts[&40], 0xC5C);
        assert_eq!(hg.frontier_tm_texts[&4], 395);
        let ss = Gen4Entry::named("SoulSilver (U)").expect("SoulSilver (U)");
        assert_eq!(ss.statics.len(), hg.statics.len());
        assert_eq!(ss.tm_texts, hg.tm_texts);
        assert_eq!(ss.marill_cry_scripts.len(), 4);
    }
}
