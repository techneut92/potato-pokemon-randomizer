//! Gen 4 text archives.
//!
//! A file is a count and a key (16 bits each), then an encrypted table of (offset, length in 16-bit codes) pairs,
//! then the encrypted strings. A string may be packed 9 bits per code (it then starts with `F100`). Codes become text
//! through the table; the rest are written as escapes:
//!
//! - `\xNNNN`: a code without text;
//! - `\vNNNN` then `\zNNNN` per argument: a variable (`\x0000` when it has no arguments);
//! - `\n`, `\p`, `\l`: new line, new box, scroll (from the table, and accepted when encoding);
//! - `\and`: the "and" in some names.
//!
//! Encoding takes the longest table text (up to 6 characters) that matches, drops characters no text matches, and
//! skips unknown escape letters. Saved files use the key `D00E`, like the games' own files do.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::OnceLock;

use super::{hex4, parse_table};
use crate::RomError;

/// Starts a string packed 9 bits per code.
const PACKED: u16 = 0xF100;
/// Ends a string.
const END: u16 = 0xFFFF;
/// Starts a variable.
const VARIABLE: u16 = 0xFFFE;
/// Ends a packed string.
const PACKED_END: u32 = 0x1FF;
/// The key of saved files.
const SAVE_KEY: u16 = 0xD00E;
/// The longest text one code stands for, in characters.
const MAX_TOKEN: usize = 6;

struct Table {
    decode: HashMap<u16, String>,
    encode: HashMap<String, u16>,
}

fn table() -> &'static Table {
    static TABLE: OnceLock<Table> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut decode = HashMap::new();
        let mut encode = HashMap::new();
        for (code, text) in parse_table(include_str!("../../data/texttables/gen4.tbl")) {
            decode.insert(code, text.to_owned());
            encode.insert(text.to_owned(), code);
        }
        Table { decode, encode }
    })
}

/// The strings of one text file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gen4Text {
    /// The strings, in order.
    pub strings: Vec<String>,
    /// Whether any string was packed 9 bits per code.
    pub compressed: bool,
}

fn malformed(what: &str) -> RomError {
    RomError::Malformed(format!("Gen 4 text file: {what}"))
}

fn read16(data: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([data[at], data[at + 1]])
}

fn read32(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn xor32(data: &mut [u8], at: usize, key: u32) {
    let value = read32(data, at) ^ key;
    data[at..at + 4].copy_from_slice(&value.to_le_bytes());
}

/// Encrypts or decrypts (the same thing) the pointer table.
fn crypt_pointers(data: &mut [u8], count: usize, key: u16) {
    let key = u32::from(key).wrapping_mul(0x2FD) & 0xFFFF;
    for i in 0..count {
        let key2 = key.wrapping_mul(u32::try_from(i + 1).unwrap_or_default()) & 0xFFFF;
        let full = key2 | key2 << 16;
        xor32(data, 4 + i * 8, full);
        xor32(data, 8 + i * 8, full);
    }
}

/// Encrypts or decrypts string `index` (counting from 0): `len` codes at `at`.
fn crypt_string(data: &mut [u8], index: usize, at: usize, len: usize) {
    let mut key = 0x91BD3_u32.wrapping_mul(u32::try_from(index + 1).unwrap_or_default()) & 0xFFFF;
    for i in 0..len {
        let pos = at + i * 2;
        let value = read16(data, pos) ^ u16::try_from(key).unwrap_or_default();
        data[pos..pos + 2].copy_from_slice(&value.to_le_bytes());
        key = (key + 0x493D) & 0xFFFF;
    }
}

/// Decodes a text file.
///
/// # Errors
/// When a table entry points outside the file or a string is empty.
pub fn decode(data: &[u8]) -> Result<Gen4Text, RomError> {
    if data.len() < 4 {
        return Err(malformed("no header"));
    }
    let mut data = data.to_vec();
    let count = usize::from(read16(&data, 0));
    let key = read16(&data, 2);
    if data.len() < 4 + count * 8 {
        return Err(malformed("the table runs past the end"));
    }
    crypt_pointers(&mut data, count, key);
    let mut strings = Vec::with_capacity(count);
    let mut compressed = false;
    for i in 0..count {
        let at = read32(&data, 4 + i * 8) as usize;
        let len = read32(&data, 8 + i * 8) as usize;
        if len
            .checked_mul(2)
            .and_then(|n| n.checked_add(at))
            .is_none_or(|end| end > data.len())
        {
            return Err(malformed("a string runs past the end"));
        }
        crypt_string(&mut data, i, at, len);
        let mut codes: Vec<u16> = (0..len).map(|k| read16(&data, at + k * 2)).collect();
        match codes.first() {
            None => return Err(malformed("an empty string")),
            Some(&PACKED) => {
                compressed = true;
                codes = unpack(&codes);
            }
            Some(_) => {}
        }
        strings.push(to_text(&codes));
    }
    Ok(Gen4Text { strings, compressed })
}

/// Unpacks 9-bit codes, 15 bits of each 16-bit word, after the `F100` marker, up to the `1FF` end code.
fn unpack(words: &[u16]) -> Vec<u16> {
    let mut out = Vec::new();
    let mut j = 1;
    let mut shift = 0_u32;
    let mut carry = 0_u32;
    while let Some(&word) = words.get(j) {
        let word = u32::from(word);
        if shift >= 0xF {
            shift -= 0xF;
            if shift > 0 {
                let code = carry | ((word << (9 - shift)) & 0x1FF);
                if code == PACKED_END {
                    break;
                }
                out.push(u16::try_from(code).unwrap_or_default());
            }
        } else {
            let code = (word >> shift) & 0x1FF;
            if code == PACKED_END {
                break;
            }
            out.push(u16::try_from(code).unwrap_or_default());
            shift += 9;
            if shift < 0xF {
                carry = (word >> shift) & 0x1FF;
                shift += 9;
            }
            j += 1;
        }
    }
    out
}

fn to_text(codes: &[u16]) -> String {
    let table = table();
    let mut out = String::new();
    let mut i = 0;
    while let Some(&code) = codes.get(i) {
        if let Some(text) = table.decode.get(&code) {
            out.push_str(text);
        } else if code == VARIABLE {
            let (Some(&kind), Some(&args)) = (codes.get(i + 1), codes.get(i + 2)) else {
                break;
            };
            let _ = write!(out, "\\v{kind:04X}");
            i += 2;
            if args == 0 {
                out.push_str("\\x0000");
            }
            for _ in 0..args {
                i += 1;
                let Some(&arg) = codes.get(i) else { break };
                let _ = write!(out, "\\z{arg:04X}");
            }
        } else if code == END {
            break;
        } else {
            let _ = write!(out, "\\x{code:04X}");
        }
        i += 1;
    }
    out
}

/// Encodes `strings` into a text file, packing them 9 bits per code if `compressed` (which keeps only the low 9
/// bits of each code, so only for plain characters, as in trainer names).
///
/// # Errors
/// When an escape has no 4 hex digits after it.
pub fn encode(strings: &[impl AsRef<str>], compressed: bool) -> Result<Vec<u8>, RomError> {
    let encoded = strings
        .iter()
        .map(|s| to_codes(s.as_ref(), compressed))
        .collect::<Result<Vec<_>, _>>()?;
    let count = encoded.len();
    let mut data = Vec::new();
    data.extend(u16::try_from(count).unwrap_or(u16::MAX).to_le_bytes());
    data.extend(SAVE_KEY.to_le_bytes());
    let mut at = count * 8 + 4;
    for codes in &encoded {
        data.extend(u32::try_from(at).unwrap_or(u32::MAX).to_le_bytes());
        data.extend(u32::try_from(codes.len()).unwrap_or(u32::MAX).to_le_bytes());
        at += codes.len() * 2;
    }
    let mut starts = Vec::with_capacity(count);
    for codes in &encoded {
        starts.push(data.len());
        data.extend(codes.iter().flat_map(|c| c.to_le_bytes()));
    }
    for (i, codes) in encoded.iter().enumerate() {
        crypt_string(&mut data, i, starts[i], codes.len());
    }
    crypt_pointers(&mut data, count, SAVE_KEY);
    Ok(data)
}

fn bad_escape(text: &str) -> RomError {
    RomError::Malformed(format!("Gen 4 text {text:?}: an escape without 4 hex digits"))
}

/// The codes of one string, ending with `FFFF`.
fn to_codes(text: &str, compressed: bool) -> Result<Vec<u16>, RomError> {
    let table = table();
    let chars: Vec<char> = text.chars().collect();
    let hex_at = |at: usize| -> Result<u16, RomError> {
        hex4(chars.get(at..).unwrap_or_default().iter().copied()).ok_or_else(|| bad_escape(text))
    };
    let mut codes = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' {
            match chars.get(i + 1) {
                Some('x') => {
                    codes.push(hex_at(i + 2)?);
                    i += 6;
                }
                Some('v') => {
                    codes.push(VARIABLE);
                    codes.push(hex_at(i + 2)?);
                    i += 6;
                }
                Some('z') => {
                    let mut args = Vec::new();
                    while chars.get(i) == Some(&'\\') && chars.get(i + 1) == Some(&'z') {
                        args.push(hex_at(i + 2)?);
                        i += 6;
                    }
                    codes.push(u16::try_from(args.len()).unwrap_or(u16::MAX));
                    codes.extend(args);
                }
                Some('n') => {
                    codes.push(0xE000);
                    i += 2;
                }
                Some('p') => {
                    codes.push(0x25BC);
                    i += 2;
                }
                Some('l') => {
                    codes.push(0x25BD);
                    i += 2;
                }
                Some('a') if chars.get(i + 2..i + 4) == Some(&['n', 'd']) => {
                    codes.push(0x1C2);
                    i += 4;
                }
                _ => i += 2,
            }
        } else {
            let longest = (1..=MAX_TOKEN.min(chars.len() - i)).rev().find_map(|n| {
                let token: String = chars[i..i + n].iter().collect();
                table.encode.get(&token).map(|&code| (n, code))
            });
            match longest {
                Some((n, code)) => {
                    codes.push(code);
                    i += n;
                }
                None => i += 1,
            }
        }
    }
    if compressed {
        codes = pack(codes);
    }
    codes.push(END);
    Ok(codes)
}

/// Packs codes 9 bits each into 15 bits of each word, after an `F100` marker; a `1FF` end code is added unless the
/// codes fill the words exactly.
fn pack(mut codes: Vec<u16>) -> Vec<u16> {
    if !codes.len().is_multiple_of(5) || codes.is_empty() {
        codes.push(0x1FF);
    }
    let bits: Vec<u16> = codes.iter().flat_map(|&c| (0..9).map(move |j| (c >> j) & 1)).collect();
    let mut out = vec![PACKED];
    for chunk in bits.chunks(15) {
        out.push(chunk.iter().enumerate().fold(0, |word, (k, &bit)| word | bit << k));
    }
    out
}
