//! Graphics packs: their `info.ini` entries, and a reader for their images and palettes, from files or from
//! one sheet image.

use std::collections::HashMap;

use ppr_model::{Color, GraphicsPackCategory, PlayerGraphicsPack};

use super::png::{self, Source};

/// Every file of the packs, by path below `data/player_graphics`.
const FILES: &[(&str, &[u8])] = include!(concat!(env!("OUT_DIR"), "/player_graphics.rs"));

fn file(path: &str) -> Option<&'static [u8]> {
    FILES.iter().find(|(p, _)| *p == path).map(|(_, bytes)| *bytes)
}

/// One `[Name]` entry of a pack's `info.ini`.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// The pack's folder.
    pub(crate) dir: &'static str,
    /// Identifies the entry among those of its `RomType`: the folder, followed by `/2`, `/3` and so on for the
    /// second and later entry of the same type in one folder.
    pub(crate) id: String,
    pub(crate) name: String,
    values: HashMap<String, String>,
    description: String,
    from: String,
    creator: String,
    adapter: Option<String>,
    category: Option<GraphicsPackCategory>,
}

impl Entry {
    /// The value of `key`; empty when the key is missing.
    pub(crate) fn value(&self, key: &str) -> &str {
        self.values.get(key).map_or("", String::as_str)
    }

    pub(crate) fn put(&mut self, key: &str, value: &str) {
        self.values.insert(key.to_owned(), value.to_owned());
    }

    pub(crate) fn info(&self) -> PlayerGraphicsPack {
        PlayerGraphicsPack {
            id: self.id.clone(),
            name: self.name.clone(),
            description: self.description.clone(),
            from: self.from.clone(),
            creator: self.creator.clone(),
            adapter: self.adapter.clone(),
            category: self.category,
            rom_type: self.value("RomType").to_owned(),
        }
    }
}

/// Parses an `info.ini`: `[Name]` starts an entry, `Key=Value` lines fill it, `//`
/// starts a comment.
fn parse(dir: &'static str, text: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();
    for line in text.lines() {
        let line = match line.find("//") {
            Some(i) => line[..i].trim(),
            None => line,
        };
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') && line.len() >= 2 {
            entries.push(Entry {
                dir,
                id: dir.to_owned(),
                name: line[1..line.len() - 1].to_owned(),
                values: HashMap::new(),
                description: String::new(),
                from: String::new(),
                creator: String::new(),
                adapter: None,
                category: None,
            });
            continue;
        }
        let (Some(entry), Some((key, value))) = (entries.last_mut(), line.split_once('=')) else {
            continue;
        };
        let (key, value) = (key.trim(), value.trim().to_owned());
        match key {
            "Description" => entry.description = value,
            "From" => entry.from = value,
            "Creator" => entry.creator = value,
            "Adapter" => entry.adapter = Some(value),
            "Category" => {
                entry.category = match value.as_str() {
                    "POKEMON" => Some(GraphicsPackCategory::Pokemon),
                    "GAMES" => Some(GraphicsPackCategory::Games),
                    "OTHER" => Some(GraphicsPackCategory::Other),
                    _ => None,
                };
            }
            _ => {
                entry.values.insert(key.to_owned(), value);
            }
        }
    }
    entries
}

/// Every entry of every pack, unsorted.
pub(crate) fn all_entries() -> Vec<Entry> {
    FILES
        .iter()
        .filter_map(|(path, bytes)| {
            let dir = path.strip_suffix("/info.ini")?;
            let mut entries = parse(dir, &String::from_utf8_lossy(bytes));
            number_same_types(&mut entries);
            Some(entries)
        })
        .flatten()
        .collect()
}

/// Gives the second and later entry of a `RomType` in one folder an id of its own.
fn number_same_types(entries: &mut [Entry]) {
    for i in 0..entries.len() {
        let rom_type = entries[i].value("RomType").to_ascii_uppercase();
        let earlier = entries[..i]
            .iter()
            .filter(|e| e.value("RomType").eq_ignore_ascii_case(&rom_type))
            .count();
        if earlier > 0 {
            entries[i].id = format!("{}/{}", entries[i].dir, earlier + 1);
        }
    }
}

/// The entries whose `RomType` is `rom_type`, sorted by
/// category, then source, then name.
pub(crate) fn entries_for(rom_type: &str) -> Vec<Entry> {
    let mut entries: Vec<Entry> = all_entries()
        .into_iter()
        .filter(|e| e.value("RomType").eq_ignore_ascii_case(rom_type))
        .collect();
    let rank = |c: Option<GraphicsPackCategory>| match c {
        Some(GraphicsPackCategory::Pokemon) => 0,
        Some(GraphicsPackCategory::Games) => 1,
        Some(GraphicsPackCategory::Other) => 2,
        None => 3,
    };
    entries.sort_by(|a, b| {
        rank(a.category)
            .cmp(&rank(b.category))
            .then_with(|| a.from.cmp(&b.from))
            .then_with(|| a.name.cmp(&b.name))
    });
    entries
}

/// Reads a JASC palette.
fn jasc(bytes: &[u8]) -> Option<Vec<Color>> {
    let text = String::from_utf8_lossy(bytes);
    let mut lines = text.lines();
    if lines.next()? != "JASC-PAL" {
        return None;
    }
    lines.next()?;
    let length: usize = lines.next()?.parse().ok()?;
    (0..length).map(|_| color(lines.next()?)).collect()
}

/// Parses a color: six hex digits anywhere, otherwise the first three numbers.
fn color(s: &str) -> Option<Color> {
    let bytes = s.as_bytes();
    if let Some(start) = (0..bytes.len().saturating_sub(5)).find(|&i| bytes[i..i + 6].iter().all(u8::is_ascii_hexdigit))
    {
        return Some(Color::hex(u32::from_str_radix(&s[start..start + 6], 16).ok()?));
    }
    let mut numbers = s
        .split(|c: char| !c.is_ascii_digit())
        .filter(|n| !n.is_empty())
        .map(|n| n.parse::<u8>().ok());
    Some(Color::rgb(numbers.next()??, numbers.next()??, numbers.next()??))
}

/// Where a pack's sheet holds one image: frames of `frame_width` by `frame_height` pixels from (`x`, `y`), with
/// `margin` pixels between them; the frames listed by column and row are stacked into one image.
pub(crate) struct SheetImage {
    pub(crate) key: &'static str,
    pub(crate) x: usize,
    pub(crate) y: usize,
    pub(crate) frame_width: usize,
    pub(crate) frame_height: usize,
    pub(crate) margin: usize,
    pub(crate) frames: &'static [(usize, usize)],
}

/// Where a single image lies on a sheet.
pub(crate) const fn single(key: &'static str, x: usize, y: usize, width: usize, height: usize) -> SheetImage {
    SheetImage {
        key,
        x,
        y,
        frame_width: width,
        frame_height: height,
        margin: 0,
        frames: &[(0, 0)],
    }
}

/// Where frames one pixel apart lie on a sheet.
pub(crate) const fn framed(
    key: &'static str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    frames: &'static [(usize, usize)],
) -> SheetImage {
    SheetImage {
        key,
        x,
        y,
        frame_width: width,
        frame_height: height,
        margin: 1,
        frames,
    }
}

/// Where a pack's sheet holds a palette: 16 colors in a row of 2 by 2 pixel squares from (`x`, `y`).
pub(crate) struct SheetPalette {
    pub(crate) key: &'static str,
    pub(crate) x: usize,
    pub(crate) y: usize,
}

/// Reads an entry's images and palettes, from its sheet when it has one.
pub(crate) struct Reader {
    pub(crate) entry: Entry,
    sheet_images: HashMap<&'static str, Source>,
    sheet_palettes: HashMap<&'static str, Vec<Color>>,
}

impl Reader {
    /// A reader for `entry`, with its sheet cut into images and palettes; later descriptions of the same key win.
    pub(crate) fn new(entry: Entry, images: &[&[SheetImage]], palettes: &[&[SheetPalette]]) -> Self {
        let mut reader = Self {
            entry,
            sheet_images: HashMap::new(),
            sheet_palettes: HashMap::new(),
        };
        if reader.entry.value("Sheet").is_empty() {
            return reader;
        }
        let Some(sheet) = reader.read_file_image("Sheet") else {
            return reader;
        };
        for desc in images.iter().flat_map(|list| list.iter()) {
            let frames: Option<Vec<Source>> = desc
                .frames
                .iter()
                .map(|&(fx, fy)| {
                    sheet.crop(
                        desc.x + fx * (desc.frame_width + desc.margin),
                        desc.y + fy * (desc.frame_height + desc.margin),
                        desc.frame_width,
                        desc.frame_height,
                    )
                })
                .collect();
            // An image that does not fit on the sheet is left out.
            if let Some(image) = frames.and_then(|f| Source::stack(&f)) {
                reader.sheet_images.insert(desc.key, image);
            }
        }
        for desc in palettes.iter().flat_map(|list| list.iter()) {
            if desc.x + 32 <= sheet.width && desc.y + 2 <= sheet.height {
                let colors = (0..16).map(|i| sheet.color(desc.x + 2 * i, desc.y)).collect();
                reader.sheet_palettes.insert(desc.key, colors);
            }
        }
        reader
    }

    fn path(&self, key: &str) -> Option<String> {
        let name = self.entry.value(key);
        (!name.is_empty()).then(|| format!("{}/{name}", self.entry.dir))
    }

    fn read_file_image(&self, key: &str) -> Option<Source> {
        png::decode(file(&self.path(key)?)?)
    }

    /// The image `key`, from the sheet or its own file.
    pub(crate) fn image(&self, key: &str) -> Option<Source> {
        match self.sheet_images.get(key) {
            Some(image) => Some(image.clone()),
            None => self.read_file_image(key),
        }
    }

    /// The palette `key`, from the sheet or its own JASC file.
    pub(crate) fn palette(&self, key: &str) -> Option<Vec<Color>> {
        match self.sheet_palettes.get(key) {
            Some(palette) => Some(palette.clone()),
            None => jasc(file(&self.path(key)?)?),
        }
    }

    /// Whether the entry's images or palettes come from a sheet.
    pub(crate) fn uses_sheet(&self) -> bool {
        !(self.sheet_images.is_empty() && self.sheet_palettes.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_parse_with_comments_and_sections() {
        let entries = parse(
            "x",
            "[A]\r\nRomType=RSE\r\nAdapter=Someone // https://example.com\r\nCategory=GAMES\r\nFrontImage = front.png\r\n\r\n[B]\r\nRomType=FRLG\r\n",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].adapter.as_deref(), Some("Someone"));
        assert_eq!(entries[0].category, Some(GraphicsPackCategory::Games));
        assert_eq!(entries[0].value("FrontImage"), "front.png");
        assert_eq!(entries[1].value("RomType"), "FRLG");
        assert_eq!(entries[1].value("Missing"), "");
        let mut same = parse("x", "[A]\nRomType=Gen2\n[B]\nRomType=Gen1\n[C]\nRomType=gen2\n");
        number_same_types(&mut same);
        let ids: Vec<&str> = same.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["x", "x", "x/2"]);
    }

    #[test]
    fn jasc_palettes_and_colors() {
        assert_eq!(color("148 230 230"), Some(Color::rgb(148, 230, 230)));
        assert_eq!(color("#94E6E6"), Some(Color::rgb(0x94, 0xE6, 0xE6)));
        assert_eq!(color("1 2"), None);
        assert_eq!(
            jasc(b"JASC-PAL\r\n0100\r\n2\r\n0 0 0\r\n255 255 255\r\n"),
            Some(vec![Color::rgb(0, 0, 0), Color::WHITE])
        );
        assert_eq!(jasc(b"JASC-PAL\r\n0100\r\n3\r\n0 0 0\r\n"), None);
    }

    #[test]
    fn every_gen3_pack_is_listed() {
        let rse: Vec<String> = entries_for("RSE").iter().map(|e| e.name.clone()).collect();
        let frlg: Vec<String> = entries_for("frlg").iter().map(|e| e.name.clone()).collect();
        assert_eq!(rse.len() + frlg.len(), 16, "{rse:?} {frlg:?}");
        assert!(frlg.contains(&"May (Emerald)".to_owned()));
        assert!(rse.contains(&"May (Emerald)".to_owned()));
    }
}
