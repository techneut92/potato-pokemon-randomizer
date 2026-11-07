//! A GBA image: a 4-bit tiled image with a 16-color palette, built from a PNG.

use ppr_model::Color;

use super::png::{Pixels, Source};

/// Pixels per tile side.
pub(crate) const TILE: usize = 8;
/// Colors in a GBA palette.
pub(crate) const PALETTE_SIZE: usize = 16;

/// A tiled image. Pixels keep whatever index they were given, even past 15.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GbaImage {
    width: usize,
    height: usize,
    indices: Vec<u8>,
    palette: Vec<Color>,
    /// Frame width and height in tiles, once set.
    frame: Option<(usize, usize)>,
}

/// The first `size` colors, white past the end.
pub(crate) fn sized(palette: &[Color], size: usize) -> Vec<Color> {
    let mut out: Vec<Color> = palette.iter().copied().take(size).collect();
    out.resize(size, Color::WHITE);
    out
}

/// The 16-color palette of an image: its own color map, or for RGB the colors in the order they are first seen.
fn image_palette(source: &Source) -> Vec<Color> {
    match &source.pixels {
        Pixels::Indexed { palette, .. } => sized(palette, PALETTE_SIZE),
        Pixels::Rgb(_) => {
            // Column by column, each top to bottom.
            let mut seen: Vec<Color> = Vec::new();
            for x in 0..source.width {
                for y in 0..source.height {
                    let c = source.color(x, y);
                    if !seen.contains(&c) {
                        seen.push(c);
                    }
                }
            }
            sized(&seen, PALETTE_SIZE)
        }
    }
}

/// The palette index for an opaque color: the first exact match, otherwise the closest
/// color; for an all-gray palette, the closest gray.
pub(super) fn closest(palette: &[Color], c: Color) -> u8 {
    let index = |i: usize| u8::try_from(i).unwrap_or(u8::MAX);
    let gray = |p: &Color| p.r == p.g && p.g == p.b;
    if palette.iter().all(gray) {
        let target = (i32::from(c.r) * 77 + i32::from(c.g) * 150 + i32::from(c.b) * 29 + 128) / 256;
        let mut best = (0, 256);
        for (i, p) in palette.iter().enumerate() {
            let d = (i32::from(p.r) - target).abs();
            if d < best.1 {
                best = (i, d);
                if d == 0 {
                    break;
                }
            }
        }
        return index(best.0);
    }
    if let Some(i) = palette.iter().position(|&p| p == c) {
        return index(i);
    }
    let distance = |p: &Color| {
        let d = |a: u8, b: u8| (i32::from(a) - i32::from(b)).pow(2);
        d(p.r, c.r) + d(p.g, c.g) + d(p.b, c.b)
    };
    let mut best = (0, i32::MAX);
    for (i, p) in palette.iter().enumerate() {
        let e = distance(p);
        if e < best.1 {
            best = (i, e);
        }
    }
    index(best.0)
}

impl GbaImage {
    /// The image built from `source`, with an optional pre-prepared palette. `None` when the size is not whole
    /// tiles, or the image has an alpha.
    pub(crate) fn build(source: &Source, prepared: Option<&[Color]>) -> Option<Self> {
        if source.alpha.is_some()
            || !source.width.is_multiple_of(TILE)
            || !source.height.is_multiple_of(TILE)
            || source.width == 0
            || source.height == 0
        {
            return None;
        }
        let palette = prepared.map_or_else(|| image_palette(source), <[Color]>::to_vec);
        let indices = match &source.pixels {
            Pixels::Indexed { indices, .. } => indices.clone(),
            Pixels::Rgb(colors) => colors.iter().map(|&c| closest(&palette, c)).collect(),
        };
        Some(Self {
            width: source.width,
            height: source.height,
            indices,
            palette,
            frame: None,
        })
    }

    /// This image as an indexed source, for stitching.
    pub(crate) fn source(&self) -> Source {
        Source {
            width: self.width,
            height: self.height,
            pixels: Pixels::Indexed {
                indices: self.indices.clone(),
                palette: self.palette.clone(),
            },
            alpha: None,
        }
    }

    pub(crate) fn width_in_tiles(&self) -> usize {
        self.width / TILE
    }

    pub(crate) fn height_in_tiles(&self) -> usize {
        self.height / TILE
    }

    pub(crate) fn palette(&self) -> &[Color] {
        &self.palette
    }

    /// The image split into frames of `width` × `height` tiles; `None` when the image does not split into such frames.
    pub(crate) fn with_frames(mut self, width: usize, height: usize) -> Option<Self> {
        if width == 0
            || height == 0
            || !self.width_in_tiles().is_multiple_of(width)
            || !self.height_in_tiles().is_multiple_of(height)
        {
            return None;
        }
        self.frame = Some((width, height));
        Some(self)
    }

    /// The number of frames: 0 before the frame size is set.
    pub(crate) fn frame_count(&self) -> usize {
        self.frame
            .map_or(0, |(w, h)| self.width_in_tiles() * self.height_in_tiles() / (w * h))
    }

    /// Frame `index`: frames run left to right, then down.
    pub(crate) fn frame(&self, index: usize) -> Self {
        let (width, height) = self.frame.unwrap_or((self.width_in_tiles(), self.height_in_tiles()));
        self.frame_sized(index, width, height)
    }

    /// Frame `index` of frames `width` × `height`, sizes in tiles.
    pub(crate) fn frame_sized(&self, index: usize, width: usize, height: usize) -> Self {
        let column = (index * width) % self.width_in_tiles();
        let row = (index * width) / self.width_in_tiles() * height;
        let (px, py, pw, ph) = (column * TILE, row * TILE, width * TILE, height * TILE);
        let mut indices = Vec::with_capacity(pw * ph);
        for row in py..py + ph {
            let at = row * self.width + px;
            indices.extend_from_slice(&self.indices[at..at + pw]);
        }
        Self {
            width: pw,
            height: ph,
            indices,
            palette: self.palette.clone(),
            frame: None,
        }
    }

    /// The 4-bit tile data: tiles row by row, two pixels per byte, the first in the low nibble.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.width * self.height / 2);
        for ty in 0..self.height_in_tiles() {
            for tx in 0..self.width_in_tiles() {
                for y in 0..TILE {
                    let at = (ty * TILE + y) * self.width + tx * TILE;
                    for pair in self.indices[at..at + TILE].as_chunks::<2>().0 {
                        out.push((pair[1] << 4).wrapping_add(pair[0]));
                    }
                }
            }
        }
        out
    }
}

impl GbaImage {
    /// The image as an indexed PNG with its first color transparent, as samples are shown. Indices past the
    /// palette show its first color.
    pub(crate) fn to_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let (Ok(width), Ok(height)) = (u32::try_from(self.width), u32::try_from(self.height)) else {
            return out;
        };
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(self.palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect::<Vec<u8>>());
        encoder.set_trns(vec![0]);
        let pixels: Vec<u8> = self
            .indices
            .iter()
            .map(|&i| if usize::from(i) < self.palette.len() { i } else { 0 })
            .collect();
        let written = encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(&pixels).and_then(|()| w.finish()));
        if written.is_err() {
            out.clear();
        }
        out
    }
}

/// The palette as little-endian GBA color words.
pub(crate) fn palette_bytes(palette: &[Color]) -> Vec<u8> {
    palette.iter().flat_map(|c| c.to_word().to_le_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(width: usize, height: usize, colors: Vec<Color>) -> Source {
        Source {
            width,
            height,
            pixels: Pixels::Rgb(colors),
            alpha: None,
        }
    }

    #[test]
    fn rgb_images_get_their_colors_in_column_order() {
        let (a, b) = (Color::rgb(10, 0, 0), Color::rgb(0, 10, 0));
        // b appears first in column order although a is first in the first row.
        let mut colors = vec![b; 64];
        colors[1] = a;
        let image = GbaImage::build(&rgb(8, 8, colors), None).expect("whole tiles");
        assert_eq!(image.palette()[..3], [b, a, Color::WHITE]);
        assert_eq!(image.to_bytes()[0], 0x10);
    }

    #[test]
    fn closest_color_is_an_exact_match_or_the_nearest() {
        let palette = [Color::rgb(0, 0, 0), Color::rgb(200, 0, 0), Color::rgb(0, 200, 0)];
        assert_eq!(closest(&palette, Color::rgb(150, 20, 0)), 1);
        assert_eq!(closest(&palette, Color::rgb(0, 200, 0)), 2);
        let grays = [
            Color::rgb(0, 0, 0),
            Color::rgb(128, 128, 128),
            Color::rgb(255, 255, 255),
        ];
        assert_eq!(closest(&grays, Color::rgb(140, 120, 130)), 1);
    }

    #[test]
    fn frames_run_across_then_down() {
        let source = Source {
            width: 32,
            height: 16,
            pixels: Pixels::Indexed {
                indices: (0..32 * 16)
                    .map(|i| u8::try_from((i % 32) / 8 + (i / 32) / 8 * 4).unwrap_or(0))
                    .collect(),
                palette: vec![Color::default(); 16],
            },
            alpha: None,
        };
        let image = GbaImage::build(&source, None)
            .and_then(|i| i.with_frames(2, 1))
            .expect("frames");
        assert_eq!(image.frame_count(), 4);
        assert_eq!(image.frame(3).to_bytes()[0], 0x66);
        assert_eq!(image.frame(3).to_bytes()[32], 0x77);
        assert_eq!(image.frame(1).to_bytes()[0], 0x22);
    }
}
