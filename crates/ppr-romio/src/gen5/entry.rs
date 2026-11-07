//! What the Gen 5 ROM entries say beyond plain numbers: file paths (`File<Key>=<path, CRC>`), offsets inside the
//! files of a NARC (`854:0x58B`), static and roaming encounters, trade scripts and story texts.

use crate::RomError;
use crate::entries::RomEntry;
use crate::entries::ini::parse_int;

/// The game family (`Type=`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    /// Black and White.
    Bw1,
    /// Black 2 and White 2.
    Bw2,
}

impl Family {
    pub(super) fn parse(entry: &RomEntry) -> Result<Self, RomError> {
        match entry.family.to_ascii_uppercase().as_str() {
            "BW1" => Ok(Self::Bw1),
            "BW2" => Ok(Self::Bw2),
            other => Err(RomError::Malformed(format!("unknown Gen 5 game type {other}"))),
        }
    }

    /// `bw1` in Black and White, `bw2` in Black 2 and White 2.
    pub(super) fn pick<T>(self, bw1: T, bw2: T) -> T {
        match self {
            Self::Bw1 => bw1,
            Self::Bw2 => bw2,
        }
    }

    /// The number of alternate formes with data of their own.
    pub(super) fn forme_count(self) -> u16 {
        self.pick(18, 24)
    }

    /// Alternate formes' files come this many files after their number (Black 2 and White 2 have other files
    /// between).
    pub(super) fn forme_file_offset(self) -> usize {
        self.pick(0, 35)
    }

    /// The numbers of the species and the alternate formes with data of their own: 1 to 649, then the formes from
    /// 650.
    pub(super) fn species_numbers(self) -> std::ops::RangeInclusive<u16> {
        1..=super::species::SPECIES_COUNT + self.forme_count()
    }

    /// The file of species or alternate forme `number` in the archives kept per species (stats, level-up moves).
    pub(super) fn species_file(self, number: u16) -> usize {
        let n = usize::from(number);
        if number > super::species::SPECIES_COUNT {
            n + self.forme_file_offset()
        } else {
            n
        }
    }
}

/// An offset inside one file of a NARC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct InFile {
    pub(super) file: usize,
    pub(super) offset: usize,
}

fn number(s: &str) -> Option<usize> {
    parse_int(s.trim()).and_then(|v| usize::try_from(v).ok())
}

impl InFile {
    /// Parses `854:0x58B`.
    fn parse(s: &str) -> Option<Self> {
        let (file, offset) = s.split_once(':')?;
        Some(Self {
            file: number(file)?,
            offset: number(offset)?,
        })
    }
}

/// The items of a bracketed list, `[a, b]`; empty for anything else.
fn bracketed(s: &str) -> Vec<&str> {
    let s = s.trim();
    s.strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .map(|inner| inner.split(',').map(str::trim).filter(|p| !p.is_empty()).collect())
        .unwrap_or_default()
}

/// `[854:0x58B, 854:0x590]`; items that don't parse are skipped.
pub(super) fn in_file_list(s: &str) -> Vec<InFile> {
    bracketed(s).into_iter().filter_map(InFile::parse).collect()
}

/// The `Key=[…]` parts of `{Species=[…], Level=[…]}`, by key.
fn keyed_lists(s: &str) -> Vec<(&str, &str)> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(eq) = rest.find("=[") {
        let key = rest[..eq]
            .rsplit(|ch: char| !ch.is_ascii_alphabetic())
            .next()
            .unwrap_or_default();
        let Some(close) = rest[eq..].find(']') else { break };
        out.push((key, &rest[eq + 1..=eq + close]));
        rest = &rest[eq + close..];
    }
    out
}

/// Where a static encounter's species, formes and levels are kept: in script files, and for the fake item balls
/// the levels are in map files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct StaticSpec {
    pub(super) species: Vec<InFile>,
    pub(super) formes: Vec<InFile>,
    pub(super) levels: Vec<InFile>,
}

impl StaticSpec {
    /// Parses `{Species=[…], Level=[…], Forme=[…]}`.
    pub(super) fn parse(s: &str) -> Self {
        let mut spec = Self::default();
        for (key, list) in keyed_lists(s) {
            match key {
                "Species" => spec.species = in_file_list(list),
                "Level" => spec.levels = in_file_list(list),
                "Forme" => spec.formes = in_file_list(list),
                _ => {}
            }
        }
        spec
    }
}

/// A roaming Pokémon of Black and White: its species and levels in the roamer overlay, and its species in scripts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Roamer {
    pub(super) species: Vec<usize>,
    pub(super) levels: Vec<usize>,
    pub(super) scripts: Vec<InFile>,
}

impl Roamer {
    /// Parses `{Species=[0x…], Level=[0x…], Script=[…:0x…]}`.
    pub(super) fn parse(s: &str) -> Self {
        let mut roamer = Self::default();
        for (key, list) in keyed_lists(s) {
            match key {
                "Species" => roamer.species = bracketed(list).into_iter().filter_map(number).collect(),
                "Level" => roamer.levels = bracketed(list).into_iter().filter_map(number).collect(),
                "Script" => roamer.scripts = in_file_list(list),
                _ => {}
            }
        }
        roamer
    }
}

/// The requested and given species of a trade in its script file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TradeScript {
    pub(super) file: usize,
    pub(super) requested: Vec<usize>,
    pub(super) given: Vec<usize>,
}

impl TradeScript {
    /// Parses `[file:requested:given, …]`; the last item's file counts, as all items share it.
    pub(super) fn parse(s: &str) -> Self {
        let mut script = Self::default();
        for item in bracketed(s) {
            let parts: Vec<Option<usize>> = item.split(':').map(number).collect();
            if let [Some(file), Some(requested), Some(given)] = parts[..] {
                script.file = file;
                script.requested.push(requested);
                script.given.push(given);
            }
        }
        script
    }
}

/// The path of the file the entry names `File<key>`.
pub(super) fn file_path(entry: &RomEntry, key: &str) -> Result<String, RomError> {
    optional_file_path(entry, key).ok_or_else(|| RomError::Malformed(format!("{}: no file {key}", entry.name)))
}

/// [`file_path`], or `None` when the entry names none.
pub(super) fn optional_file_path(entry: &RomEntry, key: &str) -> Option<String> {
    let value = entry.value(&format!("File<{key}>"))?;
    let inner = value.trim().trim_start_matches('<').trim_end_matches('>');
    let path = inner.split(',').next()?.trim();
    (!path.is_empty()).then(|| path.to_owned())
}

/// A number the entry must have.
pub(super) fn need(entry: &RomEntry, key: &str) -> Result<usize, RomError> {
    entry
        .int(key)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed(format!("{}: no {key}", entry.name)))
}

/// A number of the entry, 0 when it has none.
pub(super) fn int_or_zero(entry: &RomEntry, key: &str) -> usize {
    entry.int(key).and_then(|v| usize::try_from(v).ok()).unwrap_or_default()
}

/// The numbers of an array of the entry, empty when it has none.
pub(super) fn indices(entry: &RomEntry, key: &str) -> Vec<usize> {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| usize::try_from(v).ok())
        .collect()
}

/// A story text of the entry (`StoryText<key>`), empty when it has none.
pub(super) fn story_text<'a>(entry: &'a RomEntry, key: &str) -> &'a str {
    entry.value(&format!("StoryText<{key}>")).unwrap_or_default()
}

/// The starter's script offsets (`StarterOffsets1` to `3`).
pub(super) fn starter_offsets(entry: &RomEntry, starter: usize) -> Vec<InFile> {
    entry
        .value(&format!("StarterOffsets{}", starter + 1))
        .map(in_file_list)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_offsets_and_lists() {
        assert_eq!(
            in_file_list("[854:0x58B, 854:0x590]"),
            [
                InFile {
                    file: 854,
                    offset: 0x58B
                },
                InFile {
                    file: 854,
                    offset: 0x590
                }
            ]
        );
        let spec = StaticSpec::parse("{Species=[1208:0xD8B, 1208:0xD97], Level=[1208:0xD99], Forme=[1208:0xD8D]}");
        assert_eq!((spec.species.len(), spec.levels.len(), spec.formes.len()), (2, 1, 1));
        let roamer = Roamer::parse("{Species=[0x1A, 0x2B], Level=[0x3C], Script=[12:0x40]}");
        assert_eq!(
            (roamer.species, roamer.levels, roamer.scripts.len()),
            (vec![0x1A, 0x2B], vec![0x3C], 1)
        );
        let trade = TradeScript::parse("[300:0x10:0x20, 300:0x30:0x40]");
        assert_eq!(
            (trade.file, trade.requested, trade.given),
            (300, vec![0x10, 0x30], vec![0x20, 0x40])
        );
    }
}
