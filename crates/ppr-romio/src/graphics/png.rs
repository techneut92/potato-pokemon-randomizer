//! PNG decoding: indexed images keep their raw indices and color map, grayscale images below 16 bits become indexed
//! with a map of grays, anything else becomes RGB, with the alpha kept apart.

use std::io::Cursor;

use png::{BitDepth, ColorType, Decoder, Transformations};
use ppr_model::Color;

/// A decoded image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Source {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) pixels: Pixels,
    /// The alpha of every pixel, for images that have one.
    pub(crate) alpha: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pixels {
    /// Indices into a color map of 2^bit-depth colors, black past the PNG's own.
    Indexed { indices: Vec<u8>, palette: Vec<Color> },
    /// One color per pixel.
    Rgb(Vec<Color>),
}

impl Source {
    /// The color at (x, y), without the alpha.
    pub(crate) fn color(&self, x: usize, y: usize) -> Color {
        let i = y * self.width + x;
        match &self.pixels {
            Pixels::Indexed { indices, palette } => palette.get(usize::from(indices[i])).copied().unwrap_or_default(),
            Pixels::Rgb(colors) => colors[i],
        }
    }

    /// The `width` × `height` part at (x, y); `None` when it does not fit.
    pub(crate) fn crop(&self, x: usize, y: usize, width: usize, height: usize) -> Option<Self> {
        if x + width > self.width || y + height > self.height {
            return None;
        }
        let rows = (y..y + height).flat_map(|row| (row * self.width + x)..(row * self.width + x + width));
        let pixels = match &self.pixels {
            Pixels::Indexed { indices, palette } => Pixels::Indexed {
                indices: rows.clone().map(|i| indices[i]).collect(),
                palette: palette.clone(),
            },
            Pixels::Rgb(colors) => Pixels::Rgb(rows.clone().map(|i| colors[i]).collect()),
        };
        let alpha = self.alpha.as_ref().map(|a| rows.map(|i| a[i]).collect());
        Some(Self {
            width,
            height,
            pixels,
            alpha,
        })
    }

    /// Whether the pixel at (x, y) is fully transparent.
    pub(crate) fn transparent(&self, x: usize, y: usize) -> bool {
        self.alpha.as_ref().is_some_and(|a| a[y * self.width + x] == 0)
    }

    /// The images one below the other. Images that all share
    /// a color map keep their indices, in an 8-bit image; otherwise the result is RGB, with an alpha when one of
    /// the images has one (the empty pixels transparent).
    pub(crate) fn stack(images: &[Self]) -> Option<Self> {
        let width = images.iter().map(|s| s.width).max()?;
        let height = images.iter().map(|s| s.height).sum();
        let shared = match &images[0].pixels {
            Pixels::Indexed { palette, .. } => images
                .iter()
                .all(|s| matches!(&s.pixels, Pixels::Indexed { palette: p, .. } if p == palette))
                .then(|| palette.clone()),
            Pixels::Rgb(_) => None,
        };
        let pixels = if let Some(mut palette) = shared {
            palette.resize(256, Color::default());
            let mut indices = vec![0; width * height];
            let mut top = 0;
            for s in images {
                let Pixels::Indexed { indices: from, .. } = &s.pixels else {
                    return None;
                };
                for (row, line) in from.chunks(s.width).enumerate() {
                    let at = (top + row) * width;
                    indices[at..at + s.width].copy_from_slice(line);
                }
                top += s.height;
            }
            Pixels::Indexed { indices, palette }
        } else {
            // Empty pixels: transparent black.
            let mut colors = vec![Color::default(); width * height];
            let mut top = 0;
            for s in images {
                for y in 0..s.height {
                    for x in 0..s.width {
                        colors[(top + y) * width + x] = s.color(x, y);
                    }
                }
                top += s.height;
            }
            Pixels::Rgb(colors)
        };
        let alpha = images.iter().any(|s| s.alpha.is_some()).then(|| {
            let mut alpha = vec![0; width * height];
            let mut top = 0;
            for s in images {
                for y in 0..s.height {
                    for x in 0..s.width {
                        alpha[(top + y) * width + x] = s.alpha.as_ref().map_or(u8::MAX, |a| a[y * s.width + x]);
                    }
                }
                top += s.height;
            }
            alpha
        });
        Some(Self {
            width,
            height,
            pixels,
            alpha,
        })
    }
}

/// Decodes a PNG; `None` when it is broken, or not indexed, grayscale below 16 bits, or 8-bit RGB or RGBA, the
/// formats the packs use. The transparency of indexed images is not read: the packs' are all opaque.
pub(crate) fn decode(bytes: &[u8]) -> Option<Source> {
    let mut decoder = Decoder::new(Cursor::new(bytes));
    decoder.set_transformations(Transformations::IDENTITY);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let frame = reader.next_frame(&mut buf).ok()?;
    let info = reader.info();
    let (width, height) = (usize::try_from(info.width).ok()?, usize::try_from(info.height).ok()?);
    let line = frame.line_size;
    let mut alpha = None;
    let pixels = match (frame.color_type, frame.bit_depth) {
        (ColorType::Indexed, depth) => {
            let bits = depth as usize;
            if bits > 8 {
                return None;
            }
            let per_byte = 8 / bits;
            let mask = u8::try_from((1u16 << bits) - 1).ok()?;
            let mut indices = Vec::with_capacity(width * height);
            for row in buf[..line * height].chunks(line) {
                for x in 0..width {
                    let byte = row[x / per_byte];
                    let shift = 8 - bits * (x % per_byte + 1);
                    indices.push((byte >> shift) & mask);
                }
            }
            let plte = info.palette.as_deref()?;
            let mut palette: Vec<Color> = plte
                .as_chunks::<3>()
                .0
                .iter()
                .map(|&[r, g, b]| Color::rgb(r, g, b))
                .collect();
            palette.resize(1 << bits, Color::default());
            Pixels::Indexed { indices, palette }
        }
        (ColorType::Grayscale, depth) => {
            let bits = depth as usize;
            if bits > 8 {
                return None;
            }
            let per_byte = 8 / bits;
            let max = (1u16 << bits) - 1;
            let mask = u8::try_from(max).ok()?;
            let mut indices = Vec::with_capacity(width * height);
            for row in buf[..line * height].chunks(line) {
                for x in 0..width {
                    let shift = 8 - bits * (x % per_byte + 1);
                    indices.push((row[x / per_byte] >> shift) & mask);
                }
            }
            let palette = (0..=max)
                .map(|v| {
                    let g = u8::try_from(v * 255 / max).unwrap_or(u8::MAX);
                    Color::rgb(g, g, g)
                })
                .collect();
            Pixels::Indexed { indices, palette }
        }
        (ColorType::Rgba, BitDepth::Eight) => {
            let mut colors = Vec::with_capacity(width * height);
            let mut alphas = Vec::with_capacity(width * height);
            for row in buf[..line * height].chunks(line) {
                for &[r, g, b, a] in row[..width * 4].as_chunks::<4>().0 {
                    colors.push(Color::rgb(r, g, b));
                    alphas.push(a);
                }
            }
            alpha = Some(alphas);
            Pixels::Rgb(colors)
        }
        (ColorType::Rgb, BitDepth::Eight) => {
            let mut colors = Vec::with_capacity(width * height);
            for row in buf[..line * height].chunks(line) {
                colors.extend(
                    row[..width * 3]
                        .as_chunks::<3>()
                        .0
                        .iter()
                        .map(|&[r, g, b]| Color::rgb(r, g, b)),
                );
            }
            Pixels::Rgb(colors)
        }
        _ => return None,
    };
    Some(Source {
        width,
        height,
        pixels,
        alpha,
    })
}
