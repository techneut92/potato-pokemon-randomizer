//! Pokémon palettes and their colors.

use serde::{Deserialize, Serialize};

/// An RGB color, 0 to 255 per component.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Color {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Color {
    /// White, the default color.
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    /// A color from its components.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// A color from `0xRRGGBB`.
    pub const fn hex(hex: u32) -> Self {
        let [_, r, g, b] = hex.to_be_bytes();
        Self { r, g, b }
    }

    /// A 15-bit GBA/DS color word (5 bits each of red, green, blue from the low bit up), each component scaled by 8.25
    /// and rounded down.
    pub fn from_word(word: u16) -> Self {
        // 31 * 33 / 4 = 255, so every component fits.
        let scale = |c: u16| u8::try_from((c & 0x1F) * 33 / 4).unwrap_or(u8::MAX);
        Self::rgb(scale(word), scale(word >> 5), scale(word >> 10))
    }

    /// The 15-bit color word: each component divided by 8, rounded down.
    pub fn to_word(self) -> u16 {
        u16::from(self.r / 8) | u16::from(self.g / 8) << 5 | u16::from(self.b / 8) << 10
    }

    /// Component `i`: 0 red, 1 green, 2 blue.
    pub fn comp(self, i: usize) -> u8 {
        match i {
            0 => self.r,
            1 => self.g,
            _ => self.b,
        }
    }

    /// Sets component `i`: 0 red, 1 green, 2 blue.
    pub fn set_comp(&mut self, i: usize, value: u8) {
        match i {
            0 => self.r = value,
            1 => self.g = value,
            _ => self.b = value,
        }
    }
}

/// A species' normal and shiny palettes: 16 colors each in Gen 3, the first the transparent background; 2 in Gen 2,
/// the colors between white and black. Gen 1 has no colors of its own, only the number of a shared palette.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeciesPalettes {
    /// The palette of the normal form.
    pub normal: Vec<Color>,
    /// The palette of the shiny form.
    pub shiny: Vec<Color>,
    /// In Gen 1, the number of the Super Game Boy palette the species uses; `None` where it has colors of its own.
    #[serde(default)]
    pub id: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_convert_both_ways() {
        assert_eq!(Color::from_word(0x7FFF), Color::WHITE);
        assert_eq!(Color::from_word(0x001F), Color::rgb(255, 0, 0));
        assert_eq!(Color::from_word(0x0001), Color::rgb(8, 0, 0));
        for word in 0..0x8000u16 {
            assert_eq!(Color::from_word(word).to_word(), word);
        }
        // Bit 15 is not a color bit.
        assert_eq!(Color::from_word(0x8000).to_word(), 0);
        assert_eq!(Color::hex(0x12_34_56), Color::rgb(0x12, 0x34, 0x56));
    }
}
