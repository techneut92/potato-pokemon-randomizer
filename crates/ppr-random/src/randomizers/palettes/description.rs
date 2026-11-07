//! Palette descriptions: which slots of a species' palette make up each colored part, from the
//! `pokePalettes*.txt` files.
//!
//! A line is `Name [part/part/…] note`. A part lists 1-based slots from brightest to darkest, "0" for a shade
//! that is made but not used; `;` starts sibling slots that end with `-` and the slot they share; a leading `A`
//! makes the first slot the average of the others. Suffixes: `L` light, `D` dark, `LN` not light, `DN` not dark,
//! `B` neither, `E` the darkest shade darker still.

/// Whether a part's shades lean light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LightDark {
    Default,
    Light,
    Dark,
}

/// The light/dark suffix of a part.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Suffix {
    Any,
    Light,
    Dark,
    Base,
    NoLight,
    NoDark,
}

/// Where the slots being read go.
#[derive(PartialEq)]
enum Target {
    Slots,
    Siblings,
    Average,
}

/// One part of a palette description. Slots are 0-based; -1 is a shade made but not used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PartDescription {
    pub(super) blank: bool,
    pub(super) slots: Vec<i32>,
    pub(super) sibling_slots: Vec<i32>,
    pub(super) shared_slot: Option<i32>,
    average_slots: Vec<i32>,
    suffix: Suffix,
    pub(super) end_darkened: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharKind {
    Digit,
    Letter,
    Sibling,
    Ignore,
}

fn kind(c: char) -> CharKind {
    if c.is_ascii_digit() {
        CharKind::Digit
    } else if c.is_alphabetic() {
        CharKind::Letter
    } else if c == ';' {
        CharKind::Sibling
    } else {
        CharKind::Ignore
    }
}

/// Runs of digits, of letters and of `;`; anything else separates them.
fn tokens(unparsed: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut token = String::new();
    let mut last = ' ';
    for c in unparsed.chars() {
        let k = kind(c);
        if k != CharKind::Ignore {
            if k != kind(last) {
                out.push(std::mem::take(&mut token));
            }
            token.push(c);
        }
        last = c;
    }
    out.push(token);
    out
}

impl PartDescription {
    /// Parses one part.
    pub(super) fn parse(unparsed: &str) -> Self {
        let mut d = Self {
            blank: unparsed.chars().all(char::is_whitespace),
            slots: Vec::new(),
            sibling_slots: Vec::new(),
            shared_slot: None,
            average_slots: Vec::new(),
            suffix: Suffix::Any,
            end_darkened: false,
        };
        let mut target = Target::Slots;
        for token in tokens(unparsed) {
            if !token.is_empty() && token.bytes().all(|b| b.is_ascii_digit()) {
                let slot = token.parse::<i32>().unwrap_or(0) - 1;
                match target {
                    Target::Slots => d.slots.push(slot),
                    Target::Siblings => d.sibling_slots.push(slot),
                    Target::Average => d.average_slots.push(slot),
                }
                continue;
            }
            match token.as_str() {
                "L" => d.suffix = Suffix::Light,
                "D" => d.suffix = Suffix::Dark,
                "LN" => d.suffix = Suffix::NoLight,
                "DN" => d.suffix = Suffix::NoDark,
                "B" => d.suffix = Suffix::Base,
                "E" => d.end_darkened = true,
                "A" => target = Target::Average,
                ";" => target = Target::Siblings,
                _ => {}
            }
        }
        if target == Target::Siblings {
            d.shared_slot = d.sibling_slots.pop();
        }
        d
    }

    /// The parts of a description body, split at `/`: trailing empty parts are dropped,
    /// but an empty body is one blank part.
    pub(super) fn all_from(body: &str) -> Vec<Self> {
        let mut parts: Vec<&str> = body.split('/').collect();
        while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
            parts.pop();
        }
        if parts.len() == 1 && parts[0].is_empty() && body.contains('/') {
            parts.clear();
        }
        parts.into_iter().map(Self::parse).collect()
    }

    pub(super) fn has_sibling(&self) -> bool {
        !self.sibling_slots.is_empty()
    }

    pub(super) fn is_average(&self) -> bool {
        !self.average_slots.is_empty()
    }

    /// The slot that becomes the average, and the slots it is the average of.
    pub(super) fn average(&self) -> (i32, &[i32]) {
        (self.average_slots[0], &self.average_slots[1..])
    }

    /// The mode the suffix allows, light for dark and dark for light so the share of
    /// either stays the same.
    pub(super) fn correct(&self, mode: LightDark) -> LightDark {
        match (self.suffix, mode) {
            (Suffix::Base, _) => LightDark::Default,
            (Suffix::Dark, _) | (Suffix::NoLight, LightDark::Light) => LightDark::Dark,
            (Suffix::Light, _) | (Suffix::NoDark, LightDark::Dark) => LightDark::Light,
            _ => mode,
        }
    }
}

/// The body between `[` and `]` of one line, whitespace other than spaces removed.
pub(super) fn body(line: &str) -> String {
    let mut body = None;
    let mut current = String::new();
    for c in line.chars() {
        match c {
            '[' => current.clear(),
            ']' => body = Some(std::mem::take(&mut current)),
            c if !c.is_whitespace() || c == ' ' => current.push(c),
            _ => {}
        }
    }
    body.unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_parse_into_parts() {
        assert_eq!(
            body("Charmeleon\t[11,12,13,14//5,6,7;6,9,8-6]"),
            "11,12,13,14//5,6,7;6,9,8-6"
        );
        let parts = PartDescription::all_from(&body("Charmeleon\t[11,12,13,14//5,6,7;6,9,8-6]"));
        assert_eq!(parts.len(), 3);
        assert!(parts[1].blank);
        assert_eq!(parts[2].slots, [4, 5, 6]);
        assert_eq!(parts[2].sibling_slots, [5, 8, 7]);
        assert_eq!(parts[2].shared_slot, Some(5));
        let dark = PartDescription::parse("16,15,14-LN");
        assert_eq!(dark.slots, [15, 14, 13]);
        assert_eq!(dark.correct(LightDark::Light), LightDark::Dark);
        assert_eq!(dark.correct(LightDark::Default), LightDark::Default);
        let avg = PartDescription::parse("A2,3,4");
        assert!(avg.is_average());
        assert_eq!(avg.average(), (1, &[2, 3][..]));
        let zero = PartDescription::parse("0,3,4E");
        assert_eq!(zero.slots, [-1, 2, 3]);
        assert!(zero.end_darkened);
        // Trailing empty parts are dropped.
        assert_eq!(PartDescription::all_from("14,13,12,11/2,7,6/").len(), 2);
        assert_eq!(PartDescription::all_from("").len(), 1);
        assert!(PartDescription::all_from("")[0].blank);
    }
}
