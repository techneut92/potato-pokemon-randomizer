//! A Game Boy image: 2 bits per pixel in 8 × 8 tiles, with 4 colors, built from a PNG.

use std::cmp::Ordering;

use ppr_model::Color;

use super::image::closest;
use super::png::{Pixels, Source};

const TILE: usize = 8;

/// The colors of an image without its own: white, light gray, dark gray, black.
pub(crate) const DEFAULT_PALETTE: [Color; 4] = [
    Color::WHITE,
    Color::hex(0xAA_AA_AA),
    Color::hex(0x66_66_66),
    Color::rgb(0, 0, 0),
];

/// A tiled 2-bit image. In column mode its tiles are stored column by column, otherwise row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GbcImage {
    width: usize,
    height: usize,
    indices: Vec<u8>,
    palette: [Color; 4],
    column_mode: bool,
}

/// Hue (0 to 360), saturation and value (0 to 1).
fn hsv(c: Color) -> [f64; 3] {
    let (r, g, b) = (f64::from(c.r) / 255.0, f64::from(c.g) / 255.0, f64::from(c.b) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    #[expect(clippy::float_cmp, reason = "which component is the largest, exactly")]
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * (((g - b) / delta) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    let saturation = if max == 0.0 { 0.0 } else { delta / max };
    [hue, saturation, max]
}

/// Brightest first: by value down, then saturation up, then hue up.
fn brightest_first(a: Color, b: Color) -> Ordering {
    let (a, b) = (hsv(a), hsv(b));
    b[2].total_cmp(&a[2])
        .then_with(|| a[1].total_cmp(&b[1]))
        .then_with(|| a[0].total_cmp(&b[0]))
}

/// The 4 colors of an image: the default ones, with those of its pixels, brightest first, put in their place; white
/// stays first and black last. `None` when it has more than 4 colors (a transparent and an opaque pixel of the same
/// color count as two).
fn prepare_palette(source: &Source) -> Option<[Color; 4]> {
    let mut seen: Vec<(Color, u8)> = Vec::new();
    // Column by column, each top to bottom.
    for x in 0..source.width {
        for y in 0..source.height {
            let alpha = source.alpha.as_ref().map_or(u8::MAX, |a| a[y * source.width + x]);
            let c = (source.color(x, y), alpha);
            if !seen.contains(&c) {
                seen.push(c);
            }
        }
    }
    if seen.len() > 4 {
        return None;
    }
    let mut colors: Vec<Color> = seen.into_iter().map(|(c, _)| c).collect();
    colors.sort_by(|&a, &b| brightest_first(a, b));
    let mut palette = DEFAULT_PALETTE;
    for (i, c) in colors.into_iter().enumerate() {
        if c != Color::WHITE && c != DEFAULT_PALETTE[3] {
            palette[i] = c;
        }
    }
    Some(palette)
}

/// The palette color for a pixel: the last one that is the same Game Boy Color color (15 bits, 5 a component), so
/// a near-black that only differs in the low bits becomes black; otherwise the closest.
fn palette_index(palette: &[Color; 4], c: Color) -> u8 {
    let gbc = |c: Color| (c.r >> 3, c.g >> 3, c.b >> 3);
    palette
        .iter()
        .rposition(|&p| gbc(p) == gbc(c))
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or_else(|| closest(palette, c))
}

impl GbcImage {
    /// The image of `source`: its colors become the palette, each pixel the closest of them; a transparent pixel
    /// the first. `None` when the size is not whole tiles or the image has more than 4 colors.
    pub(crate) fn build(source: &Source, column_mode: bool) -> Option<Self> {
        if source.width == 0
            || source.height == 0
            || !source.width.is_multiple_of(TILE)
            || !source.height.is_multiple_of(TILE)
        {
            return None;
        }
        let palette = prepare_palette(source)?;
        let mut indices = Vec::with_capacity(source.width * source.height);
        for y in 0..source.height {
            for x in 0..source.width {
                indices.push(if source.transparent(x, y) {
                    0
                } else {
                    palette_index(&palette, source.color(x, y))
                });
            }
        }
        Some(Self {
            width: source.width,
            height: source.height,
            indices,
            palette,
            column_mode,
        })
    }

    /// The image of a source of these colors and indices.
    pub(crate) fn source(&self) -> Source {
        Source {
            width: self.width,
            height: self.height,
            pixels: Pixels::Rgb(self.indices.iter().map(|&i| self.palette[usize::from(i)]).collect()),
            alpha: None,
        }
    }

    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn width_in_tiles(&self) -> usize {
        self.width / TILE
    }

    pub(crate) fn height_in_tiles(&self) -> usize {
        self.height / TILE
    }

    pub(crate) fn tiles(&self) -> usize {
        self.width_in_tiles() * self.height_in_tiles()
    }

    pub(crate) fn palette(&self) -> [Color; 4] {
        self.palette
    }

    /// The color indices, row by row.
    pub(crate) fn indices(&self) -> &[u8] {
        &self.indices
    }

    /// The same image rebuilt in `column_mode`.
    pub(crate) fn rebuilt(&self, column_mode: bool) -> Option<Self> {
        Self::build(&self.source(), column_mode)
    }

    /// The `w` × `h` tiles from tile (`x`, `y`), rebuilt, in the same mode.
    pub(crate) fn tile_rect(&self, x: usize, y: usize, w: usize, h: usize) -> Option<Self> {
        Self::build(
            &self.source().crop(x * TILE, y * TILE, w * TILE, h * TILE)?,
            self.column_mode,
        )
    }

    /// The tiles `from` to `to` (not included) in a row. Tiles are counted across the rows, also in column mode.
    pub(crate) fn tile_range(&self, from: usize, to: usize) -> Source {
        let w = self.width_in_tiles();
        let mut colors = vec![Color::default(); (to - from) * TILE * TILE];
        let row = (to - from) * TILE;
        for (k, tile) in (from..to).enumerate() {
            let (tx, ty) = if self.column_mode {
                (tile / w, tile % w)
            } else {
                (tile % w, tile / w)
            };
            for y in 0..TILE {
                for x in 0..TILE {
                    let (sx, sy) = (tx * TILE + x, ty * TILE + y);
                    if sx < self.width && sy < self.height {
                        colors[y * row + k * TILE + x] = self.palette[usize::from(self.indices[sy * self.width + sx])];
                    }
                }
            }
        }
        Source {
            width: row,
            height: TILE,
            pixels: Pixels::Rgb(colors),
            alpha: None,
        }
    }

    /// The 2-bit tile data: per tile row the low bits, then the high bits, the leftmost pixel in the top bit.
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let (w, h) = (self.width_in_tiles(), self.height_in_tiles());
        let mut out = Vec::with_capacity(w * h * 16);
        for tile in 0..w * h {
            let (tx, ty) = if self.column_mode {
                (tile / h, tile % h)
            } else {
                (tile % w, tile / w)
            };
            for y in 0..TILE {
                let (mut low, mut high) = (0u8, 0u8);
                for x in 0..TILE {
                    let i = self.indices[(ty * TILE + y) * self.width + tx * TILE + x];
                    low |= (i & 1) << (7 - x);
                    high |= (i >> 1 & 1) << (7 - x);
                }
                out.push(low);
                out.push(high);
            }
        }
        out
    }

    /// The image as an indexed PNG in `palette` (its own by default), the first color transparent.
    pub(crate) fn to_png(&self, palette: Option<[Color; 4]>) -> Vec<u8> {
        let palette = palette.unwrap_or(self.palette);
        let mut out = Vec::new();
        let (Ok(width), Ok(height)) = (u32::try_from(self.width), u32::try_from(self.height)) else {
            return out;
        };
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(palette.iter().flat_map(|c| [c.r, c.g, c.b]).collect::<Vec<u8>>());
        encoder.set_trns(vec![0]);
        let written = encoder
            .write_header()
            .and_then(|mut w| w.write_image_data(&self.indices).and_then(|()| w.finish()));
        if written.is_err() {
            out.clear();
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgb(width: usize, height: usize, colors: Vec<Color>, alpha: Option<Vec<u8>>) -> Source {
        Source {
            width,
            height,
            pixels: Pixels::Rgb(colors),
            alpha,
        }
    }

    #[test]
    fn colors_go_brightest_first_with_white_and_black_in_place() {
        let (red, dark) = (Color::rgb(200, 40, 40), Color::rgb(60, 20, 20));
        let mut colors = vec![Color::WHITE; 64];
        colors[1] = dark;
        colors[2] = red;
        let image = GbcImage::build(&rgb(8, 8, colors.clone(), None), false).expect("whole tiles");
        assert_eq!(image.palette(), [Color::WHITE, red, dark, Color::rgb(0, 0, 0)]);
        assert_eq!(&image.to_bytes()[..2], [0b0010_0000, 0b0100_0000]);
        // Without white, the brightest color is first.
        colors[0] = red;
        let only_two = rgb(
            8,
            8,
            colors
                .iter()
                .map(|&c| if c == Color::WHITE { red } else { c })
                .collect(),
            None,
        );
        assert_eq!(
            GbcImage::build(&only_two, false).expect("builds").palette()[..2],
            [red, dark]
        );
        // A transparent pixel is the first color.
        let mut alpha = vec![u8::MAX; 64];
        alpha[1] = 0;
        let image = GbcImage::build(&rgb(8, 8, colors, Some(alpha)), false).expect("builds");
        assert_eq!(image.indices()[1], 0);
        let five: Vec<Color> = (0..64u8).map(|i| Color::rgb(i % 5 * 40, 0, 0)).collect();
        assert!(GbcImage::build(&rgb(8, 8, five, None), false).is_none());
    }

    #[test]
    fn a_near_black_pixel_is_black() {
        let near_black = Color::rgb(0, 0, 1);
        let mut colors = vec![Color::rgb(0, 255, 255); 64];
        colors[0] = near_black;
        let image = GbcImage::build(&rgb(8, 8, colors, None), false).expect("builds");
        // The near-black keeps its place in the palette, but the pixel takes black, the same 15-bit color.
        assert_eq!(image.palette()[1], near_black);
        assert_eq!(image.indices()[0], 3);
    }

    #[test]
    fn column_mode_stores_tiles_down_the_columns() {
        let colors: Vec<Color> = (0..16 * 16)
            .map(|i| {
                if (i % 16) / 8 + (i / 16) / 8 * 2 == 1 {
                    Color::rgb(0, 0, 0)
                } else {
                    Color::WHITE
                }
            })
            .collect();
        let rows = GbcImage::build(&rgb(16, 16, colors.clone(), None), false).expect("builds");
        let columns = GbcImage::build(&rgb(16, 16, colors, None), true).expect("builds");
        // The black tile is the top right one: second by rows, third by columns.
        assert_eq!(rows.to_bytes()[16], 0xFF);
        assert_eq!(columns.to_bytes()[32], 0xFF);
        assert_eq!(columns.tile_rect(1, 0, 1, 1).expect("fits").to_bytes(), [0xFF; 16]);
        assert_eq!(rows.tile_range(1, 2).color(0, 0), Color::rgb(0, 0, 0));
    }
}
