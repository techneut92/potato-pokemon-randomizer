//! Text tables of the GBA games: decoding and encoding Gen 3 text.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::RomError;

/// Ends a string.
const TERMINATOR: u8 = 0xFF;
/// Followed by one byte naming a variable (player name, …).
const VARIABLE: u8 = 0xFD;

/// One `.tbl` file.
#[derive(Debug, Clone)]
pub(crate) struct TextTable {
    decode: Vec<Option<String>>,
    /// Text → byte; for text that several bytes decode to, the last line of the table wins.
    encode: HashMap<String, u8>,
}

/// The longest text one table byte stands for, in characters.
const MAX_TOKEN: usize = 4;

impl TextTable {
    /// Parses `XX=value` lines. Values are kept exactly, including a lone space.
    pub(crate) fn parse(text: &str) -> Result<Self, RomError> {
        let mut decode = vec![None; 256];
        let mut encode = HashMap::new();
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| RomError::Malformed(format!("text table line {line:?}")))?;
            let byte =
                u8::from_str_radix(key, 16).map_err(|_| RomError::Malformed(format!("text table key {key:?}")))?;
            decode[usize::from(byte)] = Some(value.to_owned());
            encode.insert(value.to_owned(), byte);
        }
        Ok(Self { decode, encode })
    }

    /// The table a Gen 3 ROM entry names in `TableFile=`.
    pub(crate) fn named(name: &str) -> Result<Self, RomError> {
        match name {
            "gba_english" => Self::parse(include_str!("../data/texttables/gba_english.tbl")),
            "gba_jpn" => Self::parse(include_str!("../data/texttables/gba_jpn.tbl")),
            other => Err(RomError::Malformed(format!("unknown text table {other}"))),
        }
    }

    /// Decodes up to `max_len` bytes. Bytes the table does not know become `\xNN`, variables `\vNN`; the
    /// terminator ends the string.
    pub(crate) fn decode(&self, bytes: &[u8], max_len: usize) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < max_len.min(bytes.len()) {
            let b = bytes[i];
            if let Some(s) = &self.decode[usize::from(b)] {
                out.push_str(s);
            } else if b == TERMINATOR {
                break;
            } else if b == VARIABLE {
                let next = bytes.get(i + 1).copied().unwrap_or_default();
                let _ = write!(out, "\\v{next:02X}");
                i += 1;
            } else {
                let _ = write!(out, "\\x{b:02X}");
            }
            i += 1;
        }
        out
    }
}

impl TextTable {
    /// Encodes `text`: `\xNN` is that byte, `\vNN` a variable, anything else the longest table entry
    /// (up to four characters) that matches. A character no entry matches is dropped, except that an ASCII
    /// apostrophe becomes `’` (rather than being dropped, which would turn "It's" into "Its"). No terminator is added.
    pub(crate) fn encode(&self, text: &str) -> Vec<u8> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::with_capacity(chars.len());
        let mut i = 0;
        while i < chars.len() {
            let escape = (chars[i] == '\\')
                .then(|| chars.get(i + 1).copied())
                .flatten()
                .filter(|c| *c == 'x' || *c == 'v');
            if let Some(kind) = escape {
                let hex: String = chars.iter().skip(i + 2).take(2).collect();
                if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                    if kind == 'v' {
                        out.push(VARIABLE);
                    }
                    out.push(byte);
                    i += 4;
                    continue;
                }
            }
            let longest = (1..=MAX_TOKEN.min(chars.len() - i)).rev().find_map(|len| {
                let token: String = chars[i..i + len].iter().collect();
                self.encode.get(&token).map(|&b| (b, len))
            });
            if let Some((byte, len)) = longest {
                out.push(byte);
                i += len;
            } else {
                // The ROM entries type "It's" with an ASCII apostrophe the tables lack.
                if chars[i] == '\''
                    && let Some(&b) = self.encode.get("’")
                {
                    out.push(b);
                }
                i += 1;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn english() -> TextTable {
        TextTable::named("gba_english").expect("embedded table")
    }

    #[test]
    fn decodes_names_up_to_the_terminator() {
        // "TREECKO" in the English table, then the terminator and padding.
        let bytes = [0xCE, 0xCC, 0xBF, 0xBF, 0xBD, 0xC5, 0xC9, 0xFF, 0x00, 0x00, 0x00];
        assert_eq!(english().decode(&bytes, 11), "TREECKO");
        assert_eq!(english().decode(&bytes, 3), "TRE");
    }

    #[test]
    fn unknown_bytes_and_variables_are_escaped() {
        let t = TextTable::parse("00= \nBB=A").expect("valid");
        assert_eq!(t.decode(&[0xBB, 0x00, 0x99, 0xFD, 0x01, 0xFF], 10), "A \\x99\\v01");
    }

    #[test]
    fn encodes_text_variables_and_apostrophes() {
        let t = english();
        let treecko = [0xCE, 0xCC, 0xBF, 0xBF, 0xBD, 0xC5, 0xC9];
        assert_eq!(t.encode("TREECKO"), treecko);
        assert_eq!(
            t.decode(&t.encode("So, \\v01, you want POKéMON?"), 64),
            "So, \\v01, you want POKéMON?"
        );
        assert_eq!(t.encode("\\x99A"), [0x99, 0xBB]);
        // A character the table lacks is dropped, but an ASCII apostrophe becomes ’.
        assert_eq!(t.encode("A\u{2603}A"), [0xBB, 0xBB]);
        assert_eq!(t.encode("A'"), [0xBB, 0xB4]);
    }

    #[test]
    fn both_tables_parse() {
        assert!(TextTable::named("gba_jpn").is_ok());
        assert!(TextTable::named("other").is_err());
    }
}
