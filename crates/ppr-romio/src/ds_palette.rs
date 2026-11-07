//! The palette files (NCLR) of the Pokémon graphics archives of Gen 4 and Gen 5: a fixed header, then 16 colors.

use ppr_model::Color;

/// Each palette file starts with this header; 16 colors follow.
const HEADER: [u8; 40] = [
    0x52, 0x4C, 0x43, 0x4E, 0xFF, 0xFE, 0x00, 0x01, 0x48, 0x00, 0x00, 0x00, 0x10, 0x00, 0x01, 0x00, 0x54, 0x54, 0x4C,
    0x50, 0x38, 0x00, 0x00, 0x00, 0x04, 0x00, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x10, 0x00,
    0x00, 0x00,
];

/// The colors of a palette file: every color word after the header.
pub(crate) fn read(data: &[u8]) -> Vec<Color> {
    data.get(HEADER.len()..)
        .unwrap_or_default()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&w| Color::from_word(u16::from_le_bytes(w)))
        .collect()
}

/// A palette file of these colors.
pub(crate) fn to_bytes(colors: &[Color]) -> Vec<u8> {
    let mut out = HEADER.to_vec();
    for c in colors {
        out.extend_from_slice(&c.to_word().to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_convert_both_ways() {
        let colors = vec![Color::rgb(255, 0, 0), Color::rgb(8, 16, 255)];
        let bytes = to_bytes(&colors);
        assert_eq!(bytes.len(), HEADER.len() + 4);
        assert_eq!(&bytes[HEADER.len()..], [0x1F, 0x00, 0x41, 0x7C]);
        assert_eq!(read(&bytes), colors);
        assert_eq!(read(&bytes[..10]), Vec::<Color>::new());
    }
}
