//! Text that people on a map say, rewritten when what they talk about changes.
//!
//! An entry such as `StarterText[]=[0,4,3,5,[0x28;0x18;0x2],I see! [starter] is your choice. …]` names an id, a
//! map bank and number, a person on that map (1-based), a path of relative offsets from the person's script to
//! the pointer to the text, and a template.
//!
//! Deliberate choice: names are kept in one piece by writing their spaces as `_` while lines are broken, and the
//! underscores are turned back into spaces afterwards, so "MR. MIME" stays "MR. MIME".

use super::space::{Space, pointer_copies};
use crate::RomError;
use crate::entries::ini::parse_int;
use crate::gba::GbaRom;
use crate::text::TextTable;
use crate::textfmt::{Breaks, format_with_replacements};

/// Characters per line of the regular text box.
const CHARS_PER_LINE: usize = 36;
const BREAKS: Breaks<'static> = Breaks {
    newline: "\\n",
    extraline: "\\l",
    newpara: "\\p",
};
const PERSON_SIZE: usize = 24;
const PERSON_SCRIPT: usize = 16;
const MAP_EVENTS: usize = 4;
const EVENT_PEOPLE: usize = 4;

/// One entry, with the offset of its text pointer once found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EventText {
    pub(super) id: usize,
    bank: usize,
    map: usize,
    person: usize,
    relative: Vec<usize>,
    template: String,
    /// Where the pointer to the text is; `None` when the path from the map headers did not lead to it.
    pointer: Option<usize>,
}

#[cfg(any(test, feature = "test-fixtures"))]
impl EventText {
    pub(crate) fn location(&self) -> (usize, usize) {
        (self.bank, self.map)
    }

    /// 0-based.
    pub(crate) fn person(&self) -> usize {
        self.person
    }

    pub(crate) fn relative(&self) -> &[usize] {
        &self.relative
    }

    #[cfg(test)]
    pub(crate) fn pointer_offset(&self) -> Option<usize> {
        self.pointer
    }
}

/// Parses the entry's event text lines. Lines that cannot be read are skipped.
pub(crate) fn parse(lines: &[String]) -> Vec<EventText> {
    lines.iter().filter_map(|line| parse_one(line)).collect()
}

fn parse_one(line: &str) -> Option<EventText> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?;
    let parts: Vec<&str> = inner.splitn(6, ',').collect();
    let [id, bank, map, person, offsets, template] = <[&str; 6]>::try_from(parts).ok()?;
    let int = |s: &str| parse_int(s.trim()).and_then(|v| usize::try_from(v).ok());
    let relative = match offsets.trim().strip_prefix('[').and_then(|o| o.strip_suffix(']')) {
        Some(list) => list.split(';').map(int).collect::<Option<Vec<_>>>()?,
        None => vec![int(offsets)?],
    };
    Some(EventText {
        id: int(id)?,
        bank: int(bank)?,
        map: int(map)?,
        person: int(person)?.checked_sub(1)?,
        relative,
        template: template.to_owned(),
        pointer: None,
    })
}

/// Finds each entry's text pointer through the map headers.
pub(super) fn resolve(rom: &GbaRom, map_headers: usize, texts: &mut [EventText]) {
    for text in texts {
        text.pointer = pointer_offset(rom, map_headers, text);
    }
}

fn pointer_offset(rom: &GbaRom, map_headers: usize, text: &EventText) -> Option<usize> {
    let bank = rom.pointer(map_headers + 4 * text.bank).ok()?;
    let header = rom.pointer(bank + 4 * text.map).ok()?;
    let events = rom.pointer(header + MAP_EVENTS).ok()?;
    if rom.slice(events, 1).ok()?[0] == 0 {
        return None;
    }
    let people = rom.pointer(events + EVENT_PEOPLE).ok()?;
    let mut script = rom.pointer(people + text.person * PERSON_SIZE + PERSON_SCRIPT).ok()?;
    let (last, path) = text.relative.split_last()?;
    for step in path {
        script = rom.pointer(script + step).ok()?;
    }
    Some(script + last)
}

/// Fills each template with `values(id)` for `targets`, wraps it to the text box and
/// writes it to free space. `what` names the texts in errors.
pub(super) fn write(
    space: &mut Space<'_>,
    table: &TextTable,
    texts: &[EventText],
    targets: &[&str],
    values: impl Fn(usize) -> Option<Vec<String>>,
    what: &str,
) -> Result<(), RomError> {
    let error = |detail: String| RomError::Malformed(format!("couldn't insert new {what} text: {detail}"));
    for text in texts {
        let values = values(text.id).ok_or_else(|| error(format!("nothing to say for entry {}", text.id)))?;
        // Spaces would let a name be split over two lines.
        let joined: Vec<String> = values.iter().map(|v| v.replace(' ', "_")).collect();
        let replacements: Vec<(&str, &str)> = targets.iter().copied().zip(joined.iter().map(String::as_str)).collect();
        let mut new_text = format_with_replacements(&text.template, &replacements, BREAKS, CHARS_PER_LINE, |w| {
            table.encode(w).len()
        });
        for (value, joined) in values.iter().zip(&joined) {
            new_text = new_text.replace(joined.as_str(), value);
        }
        let pointer = text
            .pointer
            .ok_or_else(|| error(format!("entry {} is not on its map", text.id)))?;
        let copies = pointer_copies(space.rom, pointer)?;
        let mut data = table.encode(&new_text);
        data.push(super::space::FREE_SPACE_BYTE);
        space
            .rewrite_string(pointer, &data, &copies)
            .map_err(|e| error(e.to_string()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_entries_with_and_without_offset_paths() {
        let lines = [
            "[2,4,3,6,[0x28;0x23;0x2],Hm! [starter] is your choice. It's one worth raising.\\pSo, \\v01, you've \
             decided on the [type] POKéMON [starter]?]"
                .to_owned(),
            "[1,4,3,7,0x28,Short, with a comma]".to_owned(),
            "not an entry".to_owned(),
        ];
        let texts = parse(&lines);
        assert_eq!(texts.len(), 2);
        assert_eq!(
            (texts[0].id, texts[0].bank, texts[0].map, texts[0].person),
            (2, 4, 3, 5)
        );
        assert_eq!(texts[0].relative, [0x28, 0x23, 0x2]);
        assert!(texts[0].template.starts_with("Hm! [starter]") && texts[0].template.ends_with("[starter]?"));
        assert_eq!(texts[1].relative, [0x28]);
        assert_eq!(texts[1].template, "Short, with a comma");
    }
}
