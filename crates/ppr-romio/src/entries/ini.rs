//! Reader for the `genN_offsets.ini` ROM entry syntax.

/// One `[Name]` section with its `Key=Value` lines in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// Text between the brackets.
    pub name: String,
    /// Trimmed key/value pairs, comments removed. Keys may repeat (`StaticPokemon{}`).
    pub pairs: Vec<(String, String)>,
}

/// Lines that are neither a section header nor `Key=Value`, with their 1-based line numbers.
pub type Invalid = Vec<(usize, String)>;

/// Splits an ini file into sections: `//` starts a comment anywhere on a
/// line, `[Name]` starts a section, and lines before the first section are ignored.
pub fn parse(text: &str) -> (Vec<Section>, Invalid) {
    let mut sections: Vec<Section> = Vec::new();
    let mut invalid = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = raw.find("//").map_or(raw, |at| &raw[..at]).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            sections.push(Section {
                name: name.to_owned(),
                pairs: Vec::new(),
            });
        } else if let Some(section) = sections.last_mut() {
            match line.split_once('=') {
                Some((k, v)) => section.pairs.push((k.trim().to_owned(), v.trim().to_owned())),
                None => invalid.push((i + 1, line.to_owned())),
            }
        }
    }
    (sections, invalid)
}

/// Parses an integer: decimal, or hex with `0x` or `&h`.
pub fn parse_int(s: &str) -> Option<i64> {
    let s = s.trim().to_ascii_lowercase();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("&h")) {
        Some(hex) => i64::from_str_radix(hex, 16).ok(),
        None => s.parse().ok(),
    }
}

/// Parses `[a, b, c]` into integers; `[]` is empty.
pub fn parse_array(s: &str) -> Option<Vec<i64>> {
    let inner = s.trim().strip_prefix('[')?.strip_suffix(']')?;
    if inner.trim().is_empty() {
        return Some(Vec::new());
    }
    inner.split(',').map(parse_int).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_comments_and_repeats() {
        let (sections, invalid) = parse(
            "ignored=1\n[Red (U)]\nGame=POKEMON RED // the title\nStaticPokemon{}=a\nStaticPokemon{}=b\n\n[Blue (U)]\noops\n",
        );
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].pairs[0], ("Game".into(), "POKEMON RED".into()));
        assert_eq!(
            sections[0].pairs.iter().filter(|(k, _)| k == "StaticPokemon{}").count(),
            2
        );
        assert_eq!(invalid, [(8, "oops".to_owned())]);
    }

    #[test]
    fn parses_decimal_and_hex_numbers() {
        assert_eq!(parse_int("0xF57E"), Some(0xF57E));
        assert_eq!(parse_int("&H10"), Some(16));
        assert_eq!(parse_int(" 42 "), Some(42));
        assert_eq!(parse_int("x"), None);
        assert_eq!(parse_array("[1, 0x10]"), Some(vec![1, 16]));
        assert_eq!(parse_array("[ ]"), Some(vec![]));
        assert_eq!(parse_array("1, 2"), None);
    }
}
