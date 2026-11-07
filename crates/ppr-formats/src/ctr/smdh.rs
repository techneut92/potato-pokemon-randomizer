//! SMDH: the icon file of a 3DS title (`icon` in the ExeFS), with the title, description and publisher per
//! language that the HOME Menu and emulators show.

const MAGIC: &[u8; 4] = b"SMDH";
const LANGUAGES: usize = 12;
const TITLE_LEN: usize = 0x200;
const SHORT_DESCRIPTION: (usize, usize) = (0x08, 0x80);
const LONG_DESCRIPTION: (usize, usize) = (0x88, 0x100);
const PUBLISHER: (usize, usize) = (0x188, 0x80);

/// The icon file of a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Smdh {
    data: Vec<u8>,
}

impl Smdh {
    /// Wraps the bytes of an icon file. Data without the `SMDH` magic is kept but cannot be edited.
    pub fn new(data: Vec<u8>) -> Self {
        Self { data }
    }

    /// Whether the data starts with the `SMDH` magic and holds all language entries.
    pub fn is_valid(&self) -> bool {
        self.data.starts_with(MAGIC) && self.data.len() >= 0x08 + LANGUAGES * TITLE_LEN
    }

    /// The file as it is now.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    fn field(&self, language: usize, (offset, len): (usize, usize)) -> Option<String> {
        if !self.is_valid() || language >= LANGUAGES {
            return None;
        }
        let at = offset + language * TITLE_LEN;
        let units: Vec<u16> = self.data[at..at + len]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&c| u16::from_le_bytes(c))
            .collect();
        Some(
            String::from_utf16_lossy(&units)
                .trim_matches(|c: char| c <= ' ')
                .to_owned(),
        )
    }

    /// The short description (the game's name) in language `language` (0 to 11).
    pub fn short_description(&self, language: usize) -> Option<String> {
        self.field(language, SHORT_DESCRIPTION)
    }

    /// The long description in language `language` (0 to 11).
    pub fn long_description(&self, language: usize) -> Option<String> {
        self.field(language, LONG_DESCRIPTION)
    }

    /// The publisher in language `language` (0 to 11).
    pub fn publisher(&self, language: usize) -> Option<String> {
        self.field(language, PUBLISHER)
    }

    fn set_all(&mut self, text: &str, fields: &[(usize, usize)], limit: usize) -> bool {
        let bytes: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        if !self.is_valid() || bytes.len() > limit {
            return false;
        }
        for language in 0..LANGUAGES {
            for &(offset, len) in fields {
                let at = offset + language * TITLE_LEN;
                let field = &mut self.data[at..at + len];
                field.fill(0);
                field[..bytes.len()].copy_from_slice(&bytes);
            }
        }
        true
    }

    /// Sets the short and long description of every language to `text`. Does nothing and returns `false` when
    /// the file is not valid or `text` does not fit the short description (64 UTF-16 units).
    pub fn set_all_descriptions(&mut self, text: &str) -> bool {
        self.set_all(text, &[SHORT_DESCRIPTION, LONG_DESCRIPTION], SHORT_DESCRIPTION.1)
    }

    /// Sets the publisher of every language to `text`. Does nothing and returns `false` when the file is not
    /// valid or `text` does not fit (64 UTF-16 units).
    pub fn set_all_publishers(&mut self, text: &str) -> bool {
        self.set_all(text, &[PUBLISHER], PUBLISHER.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        let mut data = vec![0u8; 0x36C0];
        data[..4].copy_from_slice(MAGIC);
        for language in 0..LANGUAGES {
            let at = 0x08 + language * TITLE_LEN;
            let name: Vec<u8> = "Game".encode_utf16().flat_map(u16::to_le_bytes).collect();
            data[at..at + name.len()].copy_from_slice(&name);
        }
        data
    }

    #[test]
    fn reads_and_sets_texts() {
        let mut smdh = Smdh::new(sample());
        assert_eq!(smdh.short_description(1).as_deref(), Some("Game"));
        assert!(smdh.set_all_descriptions("Seed 42"));
        assert!(smdh.set_all_publishers("Someone"));
        for language in 0..LANGUAGES {
            assert_eq!(smdh.short_description(language).as_deref(), Some("Seed 42"));
            assert_eq!(smdh.long_description(language).as_deref(), Some("Seed 42"));
            assert_eq!(smdh.publisher(language).as_deref(), Some("Someone"));
        }
        assert_eq!(smdh.as_bytes().len(), 0x36C0);
    }

    #[test]
    fn refuses_texts_that_do_not_fit() {
        let mut smdh = Smdh::new(sample());
        let before = smdh.clone();
        assert!(!smdh.set_all_descriptions(&"x".repeat(65)));
        assert!(smdh.set_all_descriptions(&"x".repeat(64)));
        let mut invalid = Smdh::new(vec![0; 0x36C0]);
        assert!(!invalid.set_all_publishers("Someone"));
        assert_ne!(before, smdh);
    }
}
