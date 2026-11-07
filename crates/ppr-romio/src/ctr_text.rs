//! Text files of the Gen 6 and Gen 7 games: one file per entry of the text GARCs (`TextStrings`, `StoryText`),
//! holding a list of lines.
//!
//! A file has one section: a header (section count 1, line count, section length, key 0, section offset 0x10), then
//! the section length again, a table of (offset, length in UTF-16 units) per line and the lines. Each line is
//! UTF-16LE ending in 0, XOR-ed with a 16-bit key that starts at 0x7C89 + 0x2983 × line number and rotates left by 3
//! bits after every unit.
//!
//! Lines are read into strings with escapes:
//!
//! * `\n` for a line break, `\\` for a backslash, `\[` for a bracket, `\r` and `\c` for the "scroll" and "clear"
//!   commands;
//! * `[VAR NAME]` or `[VAR NAME(ARG1,ARG2)]` for a placeholder (arguments in hex), `[WAIT n]` for a pause and
//!   `[~ n]` for an empty-line marker.
//!
//! With `remap`, the game's private-use characters for the narrow space, ellipsis, ♂ and ♀ are read as those Unicode
//! characters and written back.
//!
//! All Gen 6 and Gen 7 games share the placeholder names. Deliberate choices: a placeholder without a name is read
//! as its code in 4 hex digits (`[VAR 01A4]`) and that is parsed back as hex, so it survives a round trip; each
//! line's attribute word (the 16 bits after its length in the table) is carried over from the file being replaced;
//! `[~ n]` and `[WAIT n]` are written with an argument count of 2 (the command and its value), as the games store
//! them. So rewriting unchanged lines gives the original file. A file whose header does not match the layout above
//! reads as no lines.

use std::fmt::Write as _;

use crate::RomError;

const KEY_BASE: u16 = 0x7C89;
const KEY_ADVANCE: u16 = 0x2983;
const TERMINATOR: u16 = 0x0000;
const VARIABLE: u16 = 0x0010;
const TEXT_RETURN: u16 = 0xBE00;
const TEXT_CLEAR: u16 = 0xBE01;
const TEXT_WAIT: u16 = 0xBE02;
const TEXT_NULL: u16 = 0xBDFF;

/// Placeholder codes and their names.
const VARIABLES: [(u16, &str); 48] = [
    (0xFF00, "COLOR"),
    (0x0100, "TRNAME"),
    (0x0101, "PKNAME"),
    (0x0102, "PKNICK"),
    (0x0103, "TYPE"),
    (0x0105, "LOCATION"),
    (0x0106, "ABILITY"),
    (0x0107, "MOVE"),
    (0x0108, "ITEM1"),
    (0x0109, "ITEM2"),
    (0x010A, "sTRBAG"),
    (0x010B, "BOX"),
    (0x010D, "EVSTAT"),
    (0x0110, "OPOWER"),
    (0x0127, "RIBBON"),
    (0x0134, "MIINAME"),
    (0x013E, "WEATHER"),
    (0x0189, "TRNICK"),
    (0x018A, "1stchrTR"),
    (0x018B, "SHOUTOUT"),
    (0x018E, "BERRY"),
    (0x018F, "REMFEEL"),
    (0x0190, "REMQUAL"),
    (0x0191, "WEBSITE"),
    (0x019C, "CHOICECOS"),
    (0x01A1, "GSYNCID"),
    (0x0192, "PRVIDSAY"),
    (0x0193, "BTLTEST"),
    (0x0195, "GENLOC"),
    (0x0199, "CHOICEFOOD"),
    (0x019A, "HOTELITEM"),
    (0x019B, "TAXISTOP"),
    (0x019F, "MAISTITLE"),
    (0x1000, "ITEMPLUR0"),
    (0x1001, "ITEMPLUR1"),
    (0x1100, "GENDBR"),
    (0x1101, "NUMBRNCH"),
    (0x1302, "iCOLOR2"),
    (0x1303, "iCOLOR3"),
    (0x0200, "NUM1"),
    (0x0201, "NUM2"),
    (0x0202, "NUM3"),
    (0x0203, "NUM4"),
    (0x0204, "NUM5"),
    (0x0205, "NUM6"),
    (0x0206, "NUM7"),
    (0x0207, "NUM8"),
    (0x0208, "NUM9"),
];

/// The game's private-use characters and the Unicode characters they show.
const REMAP: [(u16, u16); 4] = [(0xE07F, 0x202F), (0xE08D, 0x2026), (0xE08E, 0x2642), (0xE08F, 0x2640)];

fn malformed(what: &str) -> RomError {
    RomError::Malformed(format!("text file: {what}"))
}

fn key_of(line: usize) -> u16 {
    // The key advances by a constant per line, modulo 2^16.
    let steps = u16::try_from(line % 0x1_0000).unwrap_or(0);
    KEY_BASE.wrapping_add(KEY_ADVANCE.wrapping_mul(steps))
}

/// XORs `units` with the key stream that starts at `key`.
fn crypt(units: &mut [u16], mut key: u16) {
    for u in units {
        *u ^= key;
        key = key.rotate_left(3);
    }
}

/// The lines of a text file.
///
/// # Errors
/// When the line table or a line points outside the file, or a placeholder is cut off.
pub fn read_texts(data: &[u8], remap: bool) -> Result<Vec<String>, RomError> {
    let u16_at = |at: usize| {
        data.get(at..at + 2)
            .map(|b| u16::from_le_bytes([b[0], b[1]]))
            .ok_or_else(|| malformed("truncated"))
    };
    let u32_at = |at: usize| {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| malformed("truncated"))
    };
    let sections = u16_at(0)?;
    let lines = usize::from(u16_at(2)?);
    let total = u32_at(4)?;
    let initial_key = u32_at(8)?;
    let section = u32_at(0x0C)? as usize;
    if sections != 1 || initial_key != 0 || u32_at(section)? != total {
        return Ok(Vec::new());
    }
    let mut texts = Vec::with_capacity(lines);
    for line in 0..lines {
        let offset = u32_at(section + 4 + line * 8)? as usize + section;
        let len = usize::from(u16_at(section + 8 + line * 8)?);
        let bytes = data
            .get(offset..offset + len * 2)
            .ok_or_else(|| malformed("line out of bounds"))?;
        let mut units: Vec<u16> = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&b| u16::from_le_bytes(b))
            .collect();
        crypt(&mut units, key_of(line));
        texts.push(decode(&units, remap)?);
    }
    Ok(texts)
}

fn push_str(out: &mut Vec<u16>, s: &str) {
    out.extend(s.encode_utf16());
}

fn decode(units: &[u16], remap: bool) -> Result<String, RomError> {
    let mut out = Vec::with_capacity(units.len());
    let mut i = 0;
    while let Some(&unit) = units.get(i) {
        if unit == TERMINATOR {
            break;
        }
        i += 1;
        match unit {
            VARIABLE => push_str(&mut out, &decode_variable(units, &mut i)?),
            0x0A => push_str(&mut out, "\\n"),
            0x5C => push_str(&mut out, "\\\\"),
            0x5B => push_str(&mut out, "\\["),
            _ => out.push(if remap { unmap(unit) } else { unit }),
        }
    }
    Ok(String::from_utf16_lossy(&out))
}

/// A placeholder whose code `0x0010` was just read; `i` points at its argument count.
fn decode_variable(units: &[u16], i: &mut usize) -> Result<String, RomError> {
    let mut next = || {
        let u = units.get(*i).copied().ok_or_else(|| malformed("placeholder cut off"));
        *i += 1;
        u
    };
    let mut count = next()?;
    let variable = next()?;
    match variable {
        TEXT_RETURN => return Ok("\\r".into()),
        TEXT_CLEAR => return Ok("\\c".into()),
        TEXT_WAIT => return Ok(format!("[WAIT {}]", next()?)),
        TEXT_NULL => return Ok(format!("[~ {}]", next()?)),
        _ => {}
    }
    let mut s = match VARIABLES.iter().find(|(code, _)| *code == variable) {
        Some((_, name)) => format!("[VAR {name}"),
        None => format!("[VAR {variable:04X}"),
    };
    if count > 1 {
        s.push('(');
        while count > 1 && *i < units.len() {
            let arg = units[*i];
            *i += 1;
            let _ = write!(s, "{arg:04X}");
            count -= 1;
            if count == 1 || *i >= units.len() {
                break;
            }
            s.push(',');
        }
        s.push(')');
    }
    s.push(']');
    Ok(s)
}

fn unmap(unit: u16) -> u16 {
    REMAP
        .iter()
        .find(|(game, _)| *game == unit)
        .map_or(unit, |&(_, shown)| shown)
}

fn map(unit: u16) -> u16 {
    REMAP
        .iter()
        .find(|(_, shown)| *shown == unit)
        .map_or(unit, |&(game, _)| game)
}

/// The 16-bit attribute word after each line's length in the line table of `data`, if its header is valid.
fn line_attributes(data: &[u8]) -> Vec<[u8; 2]> {
    let word = |at: usize| {
        data.get(at..at + 4)
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let (Some(section), Some(0), Some(total)) = (word(0x0C), word(0x08), word(0x04)) else {
        return Vec::new();
    };
    let section = section as usize;
    if data.get(..2) != Some(&[1, 0]) || word(section) != Some(total) {
        return Vec::new();
    }
    let lines = data
        .get(2..4)
        .map_or(0, |b| usize::from(u16::from_le_bytes([b[0], b[1]])));
    (0..lines)
        .map_while(|i| {
            let at = section + 10 + i * 8;
            data.get(at..at + 2).map(|b| [b[0], b[1]])
        })
        .collect()
}

/// A text file holding `texts`. Each line is trimmed of surrounding spaces and control characters first. Lines
/// keep the attribute word of the same line in `original`, the file being replaced; other lines get 0.
///
/// # Errors
/// When a line has a malformed escape or placeholder.
pub fn write_texts(original: Option<&[u8]>, texts: &[impl AsRef<str>], remap: bool) -> Result<Vec<u8>, RomError> {
    let attributes = original.map(line_attributes).unwrap_or_default();
    let count = u16::try_from(texts.len()).map_err(|_| malformed("more than 65535 lines"))?;
    let table_len = 4 + texts.len() * 8;
    let mut table = Vec::with_capacity(texts.len() * 8);
    let mut lines = Vec::new();
    let mut key = KEY_BASE;
    for (line, text) in texts.iter().enumerate() {
        let mut units = encode(text.as_ref(), remap)?;
        crypt(&mut units, key);
        let offset = u32::try_from(table_len + lines.len()).map_err(|_| malformed("too long"))?;
        let len = u16::try_from(units.len()).map_err(|_| malformed("line too long"))?;
        table.extend_from_slice(&offset.to_le_bytes());
        table.extend_from_slice(&len.to_le_bytes());
        table.extend_from_slice(&attributes.get(line).copied().unwrap_or_default());
        lines.extend(units.iter().flat_map(|u| u.to_le_bytes()));
        if units.len() % 2 == 1 {
            lines.extend_from_slice(&[0, 0]);
        }
        key = key.wrapping_add(KEY_ADVANCE);
    }
    let section_len = u32::try_from(table_len + lines.len()).map_err(|_| malformed("too long"))?;
    let mut out = Vec::with_capacity(0x14 + table.len() + lines.len());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&section_len.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0x10u32.to_le_bytes());
    out.extend_from_slice(&section_len.to_le_bytes());
    out.extend_from_slice(&table);
    out.extend_from_slice(&lines);
    Ok(out)
}

/// The units of one line, with escapes and placeholders resolved, ending in 0.
fn encode(text: &str, remap: bool) -> Result<Vec<u16>, RomError> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let start = units.iter().position(|&u| u > 0x20).unwrap_or(units.len());
    let end = units.iter().rposition(|&u| u > 0x20).map_or(start, |e| e + 1);
    let units = &units[start..end];
    let mut out = Vec::with_capacity(units.len() + 1);
    let mut i = 0;
    while i < units.len() {
        let unit = if remap { map(units[i]) } else { units[i] };
        i += 1;
        match unit {
            0x5B => {
                let close = units[i..]
                    .iter()
                    .position(|&u| u == 0x5D)
                    .ok_or_else(|| malformed(&format!("placeholder not closed in {text:?}")))?;
                let inner = String::from_utf16_lossy(&units[i..i + close]);
                out.extend(encode_variable(&inner)?);
                i += close + 1;
            }
            0x5C => {
                let escape = units
                    .get(i)
                    .copied()
                    .ok_or_else(|| malformed(&format!("lone backslash in {text:?}")))?;
                i += 1;
                match escape {
                    0x6E => out.push(0x0A),
                    0x5C | 0x5B => out.push(escape),
                    0x72 => out.extend([VARIABLE, 1, TEXT_RETURN]),
                    0x63 => out.extend([VARIABLE, 1, TEXT_CLEAR]),
                    _ => return Err(malformed(&format!("unknown escape in {text:?}"))),
                }
            }
            _ => out.push(unit),
        }
    }
    out.push(TERMINATOR);
    Ok(out)
}

/// Splits like a regular-expression split that drops trailing empty pieces; an empty input gives one empty piece.
fn split_dropping_trailing(s: &str, separator: char) -> Vec<&str> {
    if s.is_empty() {
        return vec![""];
    }
    let mut parts: Vec<&str> = s.split(separator).collect();
    while parts.last() == Some(&"") {
        parts.pop();
    }
    parts
}

/// The low 16 bits of a number in `radix`, with an optional sign.
fn number(s: &str, radix: u32) -> Result<u16, RomError> {
    i64::from_str_radix(s, radix)
        .ok()
        .filter(|v| i32::try_from(*v).is_ok())
        .map(i64::to_le_bytes)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| malformed(&format!("bad number {s:?}")))
}

/// The units of a placeholder, from the text between its brackets.
fn encode_variable(inner: &str) -> Result<Vec<u16>, RomError> {
    let parts = split_dropping_trailing(inner, ' ');
    if parts.len() < 2 {
        return Err(malformed(&format!("bad placeholder [{inner}]")));
    }
    let mut out = vec![VARIABLE];
    match parts[0] {
        "~" => out.extend([2, TEXT_NULL, number(parts[1], 10)?]),
        "WAIT" => out.extend([2, TEXT_WAIT, number(parts[1], 10)?]),
        "VAR" => {
            let text = parts[1];
            let (name, args) = match text.find('(') {
                None => (text, None),
                Some(open) => {
                    let inside = text.get(open + 1..text.len().saturating_sub(1)).unwrap_or("");
                    (&text[..open], Some(split_dropping_trailing(inside, ',')))
                }
            };
            let code = match VARIABLES.iter().find(|(_, n)| *n == name) {
                Some(&(code, _)) => code,
                None => number(name, 16)?,
            };
            match args {
                None => out.extend([1, code]),
                Some(args) => {
                    let count = u16::try_from(args.len() + 1).map_err(|_| malformed("too many arguments"))?;
                    out.extend([count, code]);
                    for arg in args {
                        out.push(number(arg, 16)?);
                    }
                }
            }
        }
        _ => return Err(malformed(&format!("unknown placeholder [{inner}]"))),
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypts_with_the_line_key() {
        let file = write_texts(None, &["A"], false).expect("valid");
        // Header, section length, table (offset 0x0C, 2 units), then 'A' ^ 0x7C89 and 0 ^ 0x7C89 rotated by 3.
        assert_eq!(&file[..0x10], &[1, 0, 1, 0, 0x10, 0, 0, 0, 0, 0, 0, 0, 0x10, 0, 0, 0]);
        assert_eq!(&file[0x10..0x1C], &[0x10, 0, 0, 0, 0x0C, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(&file[0x1C..], &[0xC8, 0x7C, 0x4B, 0xE4]);
        assert_eq!(read_texts(&file, false).expect("valid"), ["A"]);
    }

    #[test]
    fn round_trips_escapes_and_placeholders() {
        let texts = [
            "Bulbasaur",
            "",
            "Line one\\nline two\\rscroll\\cclear",
            "[VAR PKNAME(0001)] used [VAR MOVE(0002,0003)]![WAIT 30]",
            "A \\[bracket] and a \\\\ backslash[~ 7]",
            "Unnamed [VAR 01A4] and [VAR COLOR(0002)]",
            "Nidoran\u{2642} \u{2026}",
        ];
        for remap in [false, true] {
            let file = write_texts(None, &texts, remap).expect("valid");
            assert_eq!(read_texts(&file, remap).expect("valid"), texts, "remap {remap}");
        }
    }

    #[test]
    fn remaps_the_private_use_characters() {
        let file = write_texts(None, &["\u{2640}"], true).expect("valid");
        assert_eq!(read_texts(&file, false).expect("valid"), ["\u{E08F}"]);
    }

    #[test]
    fn keys_advance_per_line() {
        let texts: Vec<String> = (0..300).map(|i| format!("line {i}")).collect();
        let file = write_texts(None, &texts, true).expect("valid");
        assert_eq!(read_texts(&file, true).expect("valid"), texts);
        assert_eq!(key_of(1), 0xA60C);
    }

    #[test]
    fn keeps_the_line_attributes_of_the_replaced_file() {
        let mut original = write_texts(None, &["one", "two", "three"], false).expect("valid");
        // Line 1's attribute word sits after its offset and length in the table at 0x14.
        original[0x14 + 8 + 6] = 4;
        let rewritten = write_texts(Some(&original), &["one", "two", "three"], false).expect("valid");
        assert_eq!(rewritten, original);
        let grown = write_texts(Some(&original), &["uno", "dos", "tres", "cuatro"], false).expect("valid");
        assert_eq!(grown[0x14 + 8 + 6], 4);
        assert_eq!(grown[0x14 + 24 + 6], 0, "a new line has no attributes");
    }

    #[test]
    fn trims_lines_before_writing() {
        let file = write_texts(None, &["  padded \n"], false).expect("valid");
        assert_eq!(read_texts(&file, false).expect("valid"), ["padded"]);
    }

    #[test]
    fn rejects_malformed_lines() {
        for bad in [
            "[VAR PKNAME",
            "trailing \\",
            "\\q",
            "[NOPE 1]",
            "[VAR]",
            "[WAIT x]",
            "[VAR NOTANAME]",
        ] {
            assert!(write_texts(None, &[bad], false).is_err(), "{bad}");
        }
    }

    #[test]
    fn other_layouts_read_as_no_lines() {
        let mut file = write_texts(None, &["A"], false).expect("valid");
        file[0] = 2;
        assert_eq!(read_texts(&file, false).expect("readable"), Vec::<String>::new());
        assert!(read_texts(&[1, 0], false).is_err());
    }
}
