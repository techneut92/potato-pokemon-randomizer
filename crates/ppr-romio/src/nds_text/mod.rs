//! Text archives of the DS games: one file of a message NARC holds a list of strings, encrypted.
//!
//! - [`gen4`]: Diamond, Pearl, Platinum, `HeartGold` and `SoulSilver`. Characters are game codes, turned into text by
//!   a table (`data/texttables/gen4.tbl`).
//! - [`gen5`]: Black, White, Black 2 and White 2. Characters are UTF-16, except for a few symbols a table
//!   (`data/texttables/gen5.tbl`) names.
//!
//! In both, codes without a text form are written `\xNNNN`, and a decoded string encodes back to the same codes.

pub mod gen4;
pub mod gen5;

#[cfg(test)]
mod tests;

/// One `XXXX=text` line per code; blank lines are skipped. A later line for the same code or text wins.
fn parse_table(text: &str) -> Vec<(u16, &str)> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| {
            let (code, value) = l.split_once('=')?;
            Some((u16::from_str_radix(code, 16).ok()?, value))
        })
        .collect()
}

/// The value of the first 4 characters when they are hex digits, as after `\x` in `\xNNNN`.
fn hex4(chars: impl IntoIterator<Item = char>) -> Option<u16> {
    let digits: String = chars.into_iter().take(4).collect();
    if digits.len() == 4 && digits.chars().all(|c| c.is_ascii_hexdigit()) {
        u16::from_str_radix(&digits, 16).ok()
    } else {
        None
    }
}
