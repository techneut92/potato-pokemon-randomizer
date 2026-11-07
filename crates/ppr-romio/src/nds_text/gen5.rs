//! Gen 5 text archives.
//!
//! A file is a header (section count, string count, …, section offsets) and sections. The first section (the only
//! one in most releases) holds a table of (offset, length in 16-bit codes, extra word) per string and the strings.
//! Each string is encrypted with its own key, which rotates by 3 bits per code and turns the final `FFFF` into a
//! known value, so the key can be recovered from the last code. A string may be packed 9 bits per code (it then
//! starts with `F100`).
//!
//! Codes are UTF-16, except a few symbols the table names (`×`, `\[angry]`, …). Codes 0–20, codes above `FFF0`,
//! the unassigned `2FE0`–`2FEF` and surrogates are written `\xNNNN`, so every string is valid text and encodes back to
//! the same codes. The terminator is not part of the string.
//!
//! Encoding needs the original file: each string keeps the key and extra word of the string it replaces. Strings are
//! never packed. With fewer strings than the file has, the file is left unchanged; extra strings are ignored.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use super::{hex4, parse_table};
use crate::RomError;

/// Starts a string packed 9 bits per code.
const PACKED: u16 = 0xF100;
/// Ends a string.
const END: u16 = 0xFFFF;
/// The most sections a file may have.
const MAX_SECTIONS: usize = 3;

struct Table {
    decode: HashMap<u16, String>,
    /// Table text and the escape it encodes to, longest text first.
    encode: Vec<(Vec<char>, String)>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut decode = HashMap::new();
        let mut encode: HashMap<String, String> = HashMap::new();
        for (code, text) in parse_table(include_str!("../../data/texttables/gen5.tbl")) {
            decode.insert(code, text.to_owned());
            encode.insert(text.to_owned(), format!("\\x{code:04X}"));
        }
        let mut encode: Vec<(Vec<char>, String)> = encode.into_iter().map(|(t, e)| (t.chars().collect(), e)).collect();
        encode.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Table { decode, encode }
    })
}

fn malformed(what: &str) -> RomError {
    RomError::Malformed(format!("Gen 5 text file: {what}"))
}

fn read16(data: &[u8], at: usize) -> Result<u16, RomError> {
    data.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| malformed("runs past the end"))
}

fn read32(data: &[u8], at: usize) -> Result<usize, RomError> {
    data.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .ok_or_else(|| malformed("runs past the end"))
}

fn write32(data: &mut [u8], at: usize, value: usize) -> Result<(), RomError> {
    let bytes = u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes();
    data.get_mut(at..at + 4)
        .ok_or_else(|| malformed("runs past the end"))?
        .copy_from_slice(&bytes);
    Ok(())
}

/// A decoded file: the strings and, per string, the key of its first code and its extra word.
struct Parsed {
    strings: Vec<String>,
    keys: Vec<u16>,
    extras: Vec<u16>,
}

fn section_offsets(data: &[u8]) -> Result<Vec<usize>, RomError> {
    let count = usize::from(read16(data, 0)?);
    if count > MAX_SECTIONS {
        return Err(malformed("too many sections"));
    }
    (0..count).map(|z| read32(data, 12 + z * 4)).collect()
}

fn parse(data: &[u8]) -> Result<Parsed, RomError> {
    let entries = usize::from(read16(data, 2)?);
    let sections = section_offsets(data)?;
    let mut parsed = Parsed {
        strings: Vec::new(),
        keys: Vec::new(),
        extras: Vec::new(),
    };
    let Some(&section) = sections.first() else {
        return Ok(parsed);
    };
    let mut table_entries = Vec::with_capacity(entries);
    for j in 0..entries {
        let at = section + 4 + j * 8;
        table_entries.push((
            read32(data, at)?,
            usize::from(read16(data, at + 4)?),
            read16(data, at + 6)?,
        ));
        parsed.extras.push(read16(data, at + 6)?);
    }
    for (offset, len, _) in table_entries {
        let at = section + offset;
        let mut codes = (0..len)
            .map(|k| read16(data, at + k * 2))
            .collect::<Result<Vec<_>, _>>()?;
        let Some(&last) = codes.last() else {
            return Err(malformed("an empty string"));
        };
        let mut key = last ^ 0xFFFF;
        for code in codes.iter_mut().rev() {
            *code ^= key;
            key = key.rotate_right(3);
        }
        parsed.keys.push(key.rotate_left(3));
        if codes[0] == PACKED {
            codes = unpack(&codes);
        }
        parsed.strings.push(to_text(&codes));
    }
    Ok(parsed)
}

/// Unpacks 9-bit codes, all 16 bits of each word, after the `F100` marker, up to a code whose low byte is `FF`.
/// Codes 0 and 1 are skipped.
fn unpack(words: &[u16]) -> Vec<u16> {
    let mut out = Vec::new();
    let mut push = |code: u32| {
        if code > 1 {
            out.push(u16::try_from(code).unwrap_or_default());
        }
    };
    let mut j = 1;
    let mut shift = 0_u32;
    let mut carry = 0_u32;
    while let Some(&word) = words.get(j) {
        let word = u32::from(word);
        if shift >= 0x10 {
            shift -= 0x10;
            if shift > 0 {
                let code = carry | ((word << (9 - shift)) & 0x1FF);
                if code & 0xFF == 0xFF {
                    break;
                }
                push(code);
            }
        } else {
            let code = (word >> shift) & 0x1FF;
            if code & 0xFF == 0xFF {
                break;
            }
            push(code);
            shift += 9;
            if shift < 0x10 {
                carry = (word >> shift) & 0x1FF;
                shift += 9;
            }
            j += 1;
        }
    }
    out
}

/// Whether a code stands for itself as a character.
fn is_plain(code: u16) -> bool {
    code > 20 && code <= 0xFFF0 && !(0x2FE0..=0x2FEF).contains(&code) && !(0xD800..=0xDFFF).contains(&code)
}

fn to_text(codes: &[u16]) -> String {
    let table = table();
    let mut out = String::new();
    for &code in codes {
        if code == END {
            continue;
        }
        match (table.decode.get(&code), char::from_u32(u32::from(code))) {
            (Some(text), _) if is_plain(code) => out.push_str(text),
            (_, Some(c)) if is_plain(code) => out.push(c),
            _ => {
                let _ = write!(out, "\\x{code:04X}");
            }
        }
    }
    out
}

/// Decodes a text file.
///
/// # Errors
/// When the header or table points outside the file, or a string is empty.
pub fn decode(data: &[u8]) -> Result<Vec<String>, RomError> {
    parse(data).map(|p| p.strings)
}

/// Encodes `strings` into a copy of `original`, the file they replace (see the module docs).
///
/// # Errors
/// When `original` does not decode, has no sections, or a string has an escape without 4 hex digits after it.
pub fn encode(original: &[u8], strings: &[impl AsRef<str>]) -> Result<Vec<u8>, RomError> {
    let parsed = parse(original)?;
    let entries = usize::from(read16(original, 2)?);
    if strings.len() < entries {
        return Ok(original.to_vec());
    }
    let sections = section_offsets(original)?;
    let Some(&first) = sections.first() else {
        return Err(malformed("no sections"));
    };
    let mut encoded = Vec::with_capacity(entries);
    for (i, s) in strings.iter().take(entries).enumerate() {
        encoded.push(to_codes(&replace_symbols(s.as_ref()), parsed.keys[i])?);
    }
    let section = make_section(encoded, &parsed);

    let sizes = sections
        .iter()
        .map(|&at| read32(original, at))
        .collect::<Result<Vec<_>, _>>()?;
    let new_len = (original.len() + section.len())
        .checked_sub(sizes[0])
        .ok_or_else(|| malformed("a section larger than the file"))?;
    let mut out = vec![0u8; new_len];
    let keep = original.len().min(new_len);
    out[..keep].copy_from_slice(&original[..keep]);
    write32(&mut out, 4, section.len())?;
    let second = first + section.len();
    if sections.len() == 2 {
        write32(&mut out, 0x10, second)?;
    }
    out.get_mut(first..first + section.len())
        .ok_or_else(|| malformed("the first section runs past the end"))?
        .copy_from_slice(&section);
    if sections.len() == 2 {
        let old = original
            .get(sections[1]..sections[1] + sizes[1])
            .ok_or_else(|| malformed("the second section runs past the end"))?;
        out.get_mut(second..second + sizes[1])
            .ok_or_else(|| malformed("the second section runs past the end"))?
            .copy_from_slice(old);
    }
    Ok(out)
}

/// Table texts turned into their `\xNNNN` escapes, the longest first at each position.
fn replace_symbols(text: &str) -> String {
    let table = table();
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    'outer: while i < chars.len() {
        for (token, escape) in &table.encode {
            if chars[i..].starts_with(token) {
                out.push_str(escape);
                i += token.len();
                continue 'outer;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The encrypted codes of one string, ending with the terminator; `key` is the key of the first code. A backslash
/// starts a 4-digit escape after one letter, unless a `{` follows that letter.
fn to_codes(text: &str, key: u16) -> Result<Vec<u16>, RomError> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let mut codes = Vec::with_capacity(units.len() + 1);
    let mut i = 0;
    while i < units.len() {
        if units[i] != u16::from(b'\\') || units.get(i + 2) == Some(&u16::from(b'{')) {
            codes.push(units[i]);
        } else {
            let digits = units
                .get(i + 2..)
                .unwrap_or_default()
                .iter()
                .map(|&u| char::from_u32(u32::from(u)).unwrap_or('?'));
            let code = hex4(digits)
                .ok_or_else(|| RomError::Malformed(format!("Gen 5 text {text:?}: an escape without 4 hex digits")))?;
            codes.push(code);
            i += 5;
        }
        i += 1;
    }
    codes.push(END);
    let mut key = key;
    for code in &mut codes {
        *code ^= key;
        key = key.rotate_left(3);
    }
    Ok(codes)
}

/// The first section for `encoded` strings: size, table, strings. Its size is kept a multiple of 4 by adding a
/// code to the last string that decrypts to a second terminator.
fn make_section(mut encoded: Vec<Vec<u16>>, parsed: &Parsed) -> Vec<u8> {
    let entries = encoded.len();
    let mut size: usize = encoded.iter().map(|c| c.len() * 2).sum();
    if size % 4 == 2 {
        size += 2;
        let last = encoded.len() - 1;
        let key = parsed.keys[last].rotate_left(u32::try_from(encoded[last].len() % 16).unwrap_or_default() * 3);
        encoded[last].push(END ^ key);
    }
    let mut offset = 4 + 8 * entries;
    size += offset;
    let mut section = Vec::with_capacity(size);
    section.extend(u32::try_from(size).unwrap_or(u32::MAX).to_le_bytes());
    for (codes, &extra) in encoded.iter().zip(&parsed.extras) {
        section.extend(u32::try_from(offset).unwrap_or(u32::MAX).to_le_bytes());
        section.extend(u16::try_from(codes.len()).unwrap_or(u16::MAX).to_le_bytes());
        section.extend(extra.to_le_bytes());
        offset += codes.len() * 2;
    }
    for codes in &encoded {
        section.extend(codes.iter().flat_map(|c| c.to_le_bytes()));
    }
    section
}
