//! Text of the Game Boy games: text tables, reading, encoding and measuring strings.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::RomError;

/// Ends a string.
pub(crate) const TERMINATOR: u8 = 0x50;
/// Ends printed text.
const PRINTED_END: u8 = 0x57;
/// Ends printed text with a prompt.
const PROMPT_END: u8 = 0x58;

/// The Japanese base table and a ROM's extra table on top.
#[derive(Debug, Clone)]
pub(crate) struct GbText {
    decode: Vec<Option<String>>,
    encode: HashMap<String, u8>,
    /// The longest text one byte stands for, in characters.
    longest: usize,
}

fn table(name: &str) -> Result<&'static str, RomError> {
    Ok(match name {
        "gameboy_jpn" => include_str!("../../data/texttables/gameboy_jpn.tbl"),
        "rby_english" => include_str!("../../data/texttables/rby_english.tbl"),
        "rby_espita" => include_str!("../../data/texttables/rby_espita.tbl"),
        "rby_freger" => include_str!("../../data/texttables/rby_freger.tbl"),
        "green_translation" => include_str!("../../data/texttables/green_translation.tbl"),
        "gsc_english" => include_str!("../../data/texttables/gsc_english.tbl"),
        "gsc_espita" => include_str!("../../data/texttables/gsc_espita.tbl"),
        "gsc_freger" => include_str!("../../data/texttables/gsc_freger.tbl"),
        other => return Err(RomError::Malformed(format!("unknown text table {other}"))),
    })
}

impl GbText {
    /// `gameboy_jpn` with the ROM entry's `ExtraTableFile` on top.
    pub(crate) fn for_entry(extra: Option<&str>) -> Result<Self, RomError> {
        let mut text = Self {
            decode: vec![None; 256],
            encode: HashMap::new(),
            longest: 0,
        };
        text.read(table("gameboy_jpn")?)?;
        if let Some(extra) = extra.filter(|e| !e.eq_ignore_ascii_case("none")) {
            text.read(table(extra)?)?;
        }
        Ok(text)
    }

    /// A later line replaces an earlier one for the same byte, and its text then encodes
    /// to this byte.
    fn read(&mut self, file: &str) -> Result<(), RomError> {
        for line in file.lines().filter(|l| !l.trim().is_empty()) {
            let (key, value) = line
                .split_once('=')
                .ok_or_else(|| RomError::Malformed(format!("text table line {line:?}")))?;
            let byte =
                u8::from_str_radix(key, 16).map_err(|_| RomError::Malformed(format!("text table key {key:?}")))?;
            if let Some(old) = self.decode[usize::from(byte)].take()
                && self.encode.get(&old) == Some(&byte)
            {
                self.encode.remove(&old);
            }
            self.decode[usize::from(byte)] = Some(value.to_owned());
            self.longest = self.longest.max(value.chars().count());
            self.encode.insert(value.to_owned(), byte);
        }
        Ok(())
    }

    /// Up to `max_len` bytes; the terminator ends it, and in text engine mode so do `\r` and
    /// `\e`. Unknown bytes become `\xNN`.
    pub(crate) fn read_string(&self, bytes: &[u8], max_len: usize, text_engine: bool) -> String {
        let mut out = String::new();
        for &b in bytes.iter().take(max_len) {
            if let Some(s) = &self.decode[usize::from(b)] {
                out.push_str(s);
                if text_engine && (s == "\\r" || s == "\\e") {
                    break;
                }
            } else if b == TERMINATOR {
                break;
            } else {
                let _ = write!(out, "\\x{b:02X}");
            }
        }
        out
    }

    /// The texts that encode to one byte, sorted.
    pub(crate) fn tokens(&self) -> Vec<String> {
        let mut tokens: Vec<String> = self.encode.keys().cloned().collect();
        tokens.sort();
        tokens
    }

    /// `\xNN` is that byte, anything else the longest table text that matches; a
    /// character nothing matches is dropped. No terminator is added.
    pub(crate) fn encode(&self, text: &str) -> Vec<u8> {
        let chars: Vec<char> = text.chars().collect();
        let mut out = Vec::with_capacity(chars.len());
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '\\' && chars.get(i + 1) == Some(&'x') {
                let hex: String = chars.iter().skip(i + 2).take(2).collect();
                out.push(u8::from_str_radix(&hex, 16).unwrap_or_default());
                i += 4;
                continue;
            }
            let rest = chars.len() - i;
            let longest = (1..=self.longest.min(rest)).rev().find_map(|len| {
                let token: String = chars[i..i + len].iter().collect();
                self.encode.get(&token).map(|&b| (b, len))
            });
            match longest {
                Some((b, len)) => {
                    out.push(b);
                    i += len;
                }
                None => i += 1,
            }
        }
        out
    }

    /// The text, cut to `length` bytes and padded with terminators.
    pub(crate) fn fixed(&self, text: &str, length: usize) -> Vec<u8> {
        let mut bytes = self.encode(text);
        bytes.truncate(length);
        bytes.resize(length, TERMINATOR);
        bytes
    }

    /// The text with its terminator.
    pub(crate) fn terminated(&self, text: &str) -> Vec<u8> {
        let mut bytes = self.encode(text);
        bytes.push(TERMINATOR);
        bytes
    }
}

/// Text that was encoded to UTF-8 twice, the second time read as Windows-1252 (`PokÃ©` for `Poké`, `â€¦` for
/// `…`), as some ROM entry texts are, decoded once more; any other text as it is.
pub(crate) fn repair_double_encoding(s: &str) -> String {
    /// The characters Windows-1252 has at 0x80 to 0x9F, by byte (`None` where it has none).
    const CP1252_HIGH: [Option<char>; 32] = [
        Some('€'),
        None,
        Some('‚'),
        Some('ƒ'),
        Some('„'),
        Some('…'),
        Some('†'),
        Some('‡'),
        Some('ˆ'),
        Some('‰'),
        Some('Š'),
        Some('‹'),
        Some('Œ'),
        None,
        Some('Ž'),
        None,
        None,
        Some('‘'),
        Some('’'),
        Some('“'),
        Some('”'),
        Some('•'),
        Some('–'),
        Some('—'),
        Some('˜'),
        Some('™'),
        Some('š'),
        Some('›'),
        Some('œ'),
        None,
        Some('ž'),
        Some('Ÿ'),
    ];
    if s.is_ascii() {
        return s.to_owned();
    }
    let bytes: Option<Vec<u8>> = s
        .chars()
        .map(|ch| {
            CP1252_HIGH
                .iter()
                .position(|&c| c == Some(ch))
                .map(|i| 0x80 + u8::try_from(i).unwrap_or_default())
                .or_else(|| u8::try_from(u32::from(ch)).ok().filter(|b| !(0x80..0xA0).contains(b)))
        })
        .collect();
    bytes
        .and_then(|b| String::from_utf8(b).ok())
        .unwrap_or_else(|| s.to_owned())
}

/// Text in the place of the old text at `offset`, if it fits there (terminator included); false, with a warning,
/// when it doesn't.
pub(crate) fn write_in_place(rom: &mut super::GbRom, text: &GbText, offset: usize, s: &str) -> bool {
    let bytes = text.encode(s);
    let space = length_of_string_at(rom.bytes(), offset, true);
    if bytes.len() <= space && rom.write(offset, &bytes).is_ok() {
        return true;
    }
    tracing::warn!("text {s:?} doesn't fit in {space} bytes at {offset:#x}; kept the old one");
    false
}

/// Bytes up to and including the terminator; in text engine mode the printed-text ends
/// count too.
pub(crate) fn length_of_string_at(bytes: &[u8], offset: usize, text_engine: bool) -> usize {
    let at = |i: usize| bytes.get(offset + i).copied().unwrap_or(TERMINATOR);
    let mut len = 0;
    while at(len) != TERMINATOR && (!text_engine || (at(len) != PRINTED_END && at(len) != PROMPT_END)) {
        len += 1;
    }
    if text_engine && (at(len) == PRINTED_END || at(len) == PROMPT_END) {
        len += 1;
    }
    len + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_red_blue_text_round_trips() {
        let t = GbText::for_entry(Some("rby_english")).expect("tables");
        let bytes = t.terminated("BULBASAUR");
        assert_eq!(bytes.last(), Some(&TERMINATOR));
        assert_eq!(t.read_string(&bytes, 20, false), "BULBASAUR");
        assert_eq!(length_of_string_at(&bytes, 0, false), bytes.len());
        assert_eq!(t.fixed("MEW", 5), [t.encode("MEW"), vec![TERMINATOR; 2]].concat());
    }

    #[test]
    fn text_encoded_twice_is_repaired() {
        assert_eq!(repair_double_encoding("[POKÃ©]MON"), "[POKé]MON");
        assert_eq!(repair_double_encoding("Oooohâ€¦ ZURÃœCK"), "Ooooh… ZURÜCK");
        assert_eq!(
            repair_double_encoding("Poké Mart"),
            "Poké Mart",
            "text encoded once stays"
        );
        assert_eq!(repair_double_encoding("CANCEL"), "CANCEL");
    }
}
