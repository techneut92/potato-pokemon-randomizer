//! The picture compression of Gen 1: a 2-bit image as two bitplanes, each delta coded row by row and written in
//! column pairs as alternating runs of zero pairs and literal pairs. The second plane may be stored as the XOR of
//! the two. The compressor tries the three modes with both plane orders and keeps the first shortest result.

use crate::RomError;

/// The most tiles a side may have.
const MAX_TILES: usize = 15;

/// Writes bits, most significant first.
#[derive(Default)]
struct BitWriter {
    bytes: Vec<u8>,
    bit: u8,
}

impl BitWriter {
    fn bit(&mut self, bit: u8) {
        if self.bit == 8 || self.bytes.is_empty() {
            self.bit = 0;
            self.bytes.push(0);
        }
        if let Some(last) = self.bytes.last_mut() {
            *last |= (bit & 1) << (7 - self.bit);
        }
        self.bit += 1;
    }

    fn pair(&mut self, pair: u8) {
        self.bit(pair >> 1);
        self.bit(pair);
    }
}

/// A bitplane, indexed `[x][y]`.
type Plane = Vec<Vec<u8>>;

fn delta(plane: &Plane, width: usize, height: usize) -> Plane {
    let mut out = vec![vec![0; height]; width];
    for y in 0..height {
        let mut previous = 0;
        for x in 0..width {
            out[x][y] = plane[x][y] ^ previous;
            previous = plane[x][y];
        }
    }
    out
}

fn write_plane(plane: &Plane, width: usize, height: usize, w: &mut BitWriter) {
    let mut pairs = Vec::with_capacity(width * height / 2);
    for x in (0..width).step_by(2) {
        for (&left, &right) in plane[x].iter().zip(&plane[x + 1]) {
            pairs.push((left << 1) | right);
        }
    }
    let mut rle = pairs.first() == Some(&0);
    w.bit(u8::from(!rle));
    let mut i = 0;
    while i < pairs.len() {
        if rle {
            let start = i;
            while i < pairs.len() && pairs[i] == 0 {
                i += 1;
            }
            let length = i - start;
            // The number of bits of length + 1, without its leading 1.
            let mut bits = 1;
            while length > (1 << bits) - 2 {
                bits += 1;
            }
            bits -= 1;
            for _ in 1..bits {
                w.bit(1);
            }
            w.bit(0);
            for j in (1..=bits).rev() {
                w.bit(u8::from((length + 1) >> (j - 1) & 1 == 1));
            }
        } else {
            loop {
                w.pair(pairs[i]);
                i += 1;
                if i >= pairs.len() || pairs[i] == 0 {
                    break;
                }
            }
            if i != pairs.len() {
                w.pair(0);
            }
        }
        rle = !rle;
    }
}

/// Compresses a `width` × `height` pixel image of color indices 0 to 3, row by row.
///
/// # Errors
/// When the size is not whole tiles of at most 15 a side.
pub(crate) fn compress(width: usize, height: usize, indices: &[u8]) -> Result<Vec<u8>, RomError> {
    if !width.is_multiple_of(8)
        || !height.is_multiple_of(8)
        || width / 8 > MAX_TILES
        || height / 8 > MAX_TILES
        || indices.len() != width * height
    {
        return Err(RomError::Malformed(format!(
            "a {width}×{height} picture can't be compressed"
        )));
    }
    let plane = |bit: u8| -> Plane {
        (0..width)
            .map(|x| (0..height).map(|y| indices[y * width + x] >> bit & 1).collect())
            .collect()
    };
    let planes = [plane(0), plane(1)];
    let mut shortest: Option<Vec<u8>> = None;
    for mode in 0..=2u8 {
        for order in [false, true] {
            let (first, second) = if order {
                (&planes[1], &planes[0])
            } else {
                (&planes[0], &planes[1])
            };
            let mut second = second.clone();
            if mode != 0 {
                for x in 0..width {
                    for y in 0..height {
                        second[x][y] ^= first[x][y];
                    }
                }
            }
            let first = delta(first, width, height);
            if mode != 1 {
                second = delta(&second, width, height);
            }
            let mut w = BitWriter::default();
            let size = u8::try_from(((width / 8) << 4) | (height / 8)).unwrap_or(0);
            for i in (0..8).rev() {
                w.bit(size >> i & 1);
            }
            w.bit(u8::from(order));
            write_plane(&first, width, height, &mut w);
            if mode == 0 {
                w.bit(0);
            } else {
                w.bit(1);
                w.bit(!mode & 1);
            }
            write_plane(&second, width, height, &mut w);
            if shortest.as_ref().is_none_or(|s| w.bytes.len() < s.len()) {
                shortest = Some(w.bytes);
            }
        }
    }
    Ok(shortest.unwrap_or_default())
}

/// Reads bits, most significant first.
struct BitReader<'a> {
    data: &'a [u8],
    /// The byte the last bit came from.
    at: usize,
    left: u8,
}

impl BitReader<'_> {
    fn bit(&mut self) -> Result<u8, RomError> {
        if self.left == 0 {
            self.at += 1;
            self.left = 8;
        }
        let byte = *self
            .data
            .get(self.at)
            .ok_or_else(|| RomError::Malformed("a compressed picture runs past the end of the ROM".into()))?;
        self.left -= 1;
        Ok(byte >> self.left & 1)
    }

    fn int(&mut self, bits: usize) -> Result<usize, RomError> {
        let mut n = 0;
        for _ in 0..bits {
            n = n << 1 | usize::from(self.bit()?);
        }
        Ok(n)
    }
}

/// The length of the compressed picture at `offset`, in bytes.
///
/// # Errors
/// When `offset` is 0 or the data runs past the end of `data`.
pub(crate) fn compressed_length(data: &[u8], offset: usize) -> Result<usize, RomError> {
    if offset == 0 {
        return Err(RomError::Malformed("a compressed picture at offset 0".into()));
    }
    let mut r = BitReader {
        data,
        at: offset.wrapping_sub(1),
        left: 0,
    };
    let width = r.int(4)? * 8;
    let height = r.int(4)?;
    let pairs = width * height * 4;
    let plane = |r: &mut BitReader<'_>| -> Result<(), RomError> {
        let mut rle = r.bit()? == 0;
        let mut written = 0;
        while written < pairs {
            if rle {
                let mut i = 0;
                while r.bit()? == 1 {
                    i += 1;
                    if i >= 16 {
                        return Err(RomError::Malformed("a compressed picture with a run too long".into()));
                    }
                }
                written += (2 << i) - 1 + r.int(i + 1)?;
            } else {
                loop {
                    if r.int(2)? == 0 {
                        break;
                    }
                    written += 1;
                    if written >= pairs {
                        break;
                    }
                }
            }
            rle = !rle;
        }
        Ok(())
    };
    // The plane order.
    r.bit()?;
    plane(&mut r)?;
    if r.bit()? == 1 {
        r.bit()?;
    }
    plane(&mut r)?;
    Ok(r.at + 1 - offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pictures_compress_to_their_measured_length() {
        let mut indices = vec![0u8; 56 * 56];
        for (i, p) in indices.iter_mut().enumerate() {
            let (x, y) = (i % 56, i / 56);
            if (10..40).contains(&x) && (5..50).contains(&y) {
                *p = u8::try_from((x / 3 + y / 7) % 4).unwrap_or(0);
            }
        }
        let packed = compress(56, 56, &indices).expect("compresses");
        assert_eq!(packed[0], 0x77);
        let mut rom = vec![0xAA; 3];
        rom.extend_from_slice(&packed);
        rom.extend_from_slice(&[0xFF; 8]);
        assert_eq!(compressed_length(&rom, 3).expect("length"), packed.len());
        assert!(compress(56, 60, &indices).is_err());
        // All white: one run of zero pairs per plane.
        assert_eq!(compress(8, 8, &[0; 64]).expect("compresses"), [0x11, 0x3C, 0x13, 0xC1]);
    }
}
