//! Bottom-up LZ ("BLZ"): the compression of DS overlays and ARM9 binaries, and of 3DS code.
//!
//! The data is compressed from its end towards its start, so the console can decompress it in place. A compressed
//! file is an uncompressed head, the compressed body and an 8-byte (or longer, padded) footer:
//!
//! - bytes −8..−5: the compressed length including the footer (24 bits) and the footer length (8 bits);
//! - bytes −4..−1: how many bytes decompressing adds.
//!
//! A footer whose last word is 0 marks data that is not compressed at all.
//!
//! The compressor's output is deterministic down to the byte, so recompressed data that didn't change keeps its
//! length. Two deviations from the classic encoder, both in cases it never finished:
//!
//! - data that does not get smaller is stored uncompressed (the data, zeros up to a multiple of 4 and a zero word)
//!   instead of hanging;
//! - data that would grow by no more than its footer takes is stored uncompressed too, instead of getting a footer
//!   whose added length is 0 (which reads as uncompressed) or negative (which nothing can decompress).

use crate::FormatError;

const FORMAT: &str = "BLZ";

/// How much a back-reference copies at least, minus one.
const THRESHOLD: usize = 2;
/// How far back a back-reference reaches.
const WINDOW: usize = 0x1002;
/// The longest run one back-reference copies.
const MAX_RUN: usize = 0x12;
/// The largest decompressed size accepted.
const RAW_MAX: usize = 0x00FF_FFFF;

/// The bytes at the start of an ARM9 binary that stay uncompressed (the secure area and the code that
/// decompresses the rest).
pub const ARM9_HEAD: usize = 0x4000;

/// How hard the compressor looks for matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Always takes the longest match.
    Normal,
    /// Looks one match ahead and takes a literal when that gives a better pair ("LZ-CUE").
    Best,
}

fn bad(reason: &'static str) -> FormatError {
    FormatError::WrongFormat { format: FORMAT, reason }
}

/// A 31-bit little-endian value; the top bit is ignored.
fn read31(data: &[u8], at: usize) -> usize {
    usize::from(data[at])
        | usize::from(data[at + 1]) << 8
        | usize::from(data[at + 2]) << 16
        | usize::from(data[at + 3] & 0x7F) << 24
}

/// Decompresses BLZ data. Data marked as uncompressed (a last word of 0) comes back unchanged, footer included.
///
/// # Errors
/// When the footer is malformed, the decompressed size is implausible or a back-reference points before the start.
pub fn decompress(data: &[u8]) -> Result<Vec<u8>, FormatError> {
    let total = data.len();
    if total < 4 {
        return Err(FormatError::Truncated { offset: 0 });
    }
    let inc_len = read31(data, total - 4);
    if inc_len == 0 {
        return Ok(data.to_vec());
    }
    if total < 8 {
        return Err(bad("no footer"));
    }
    let hdr_len = usize::from(data[total - 5]);
    if !(8..=0xB).contains(&hdr_len) {
        return Err(bad("footer length out of range"));
    }
    if total <= hdr_len {
        return Err(bad("shorter than its footer"));
    }
    let enc_len = read31(data, total - 8) & 0x00FF_FFFF;
    if enc_len > total || enc_len < hdr_len {
        return Err(bad("compressed length out of range"));
    }
    let dec_len = total - enc_len;
    let pak_len = enc_len - hdr_len;
    let raw_len = dec_len + enc_len + inc_len;
    if raw_len > RAW_MAX {
        return Err(bad("decompressed length out of range"));
    }

    let mut raw_buf = vec![0u8; raw_len];
    raw_buf[..dec_len].copy_from_slice(&data[..dec_len]);
    let mut pak_buf: Vec<u8> = data[dec_len..dec_len + pak_len].to_vec();
    pak_buf.reverse();

    let mut pak = 0;
    let mut raw = dec_len;
    let raw_end = raw_len;
    let mut mask = 0u8;
    let mut flags = 0u8;
    while raw < raw_end {
        mask >>= 1;
        if mask == 0 {
            let Some(&f) = pak_buf.get(pak) else { break };
            flags = f;
            pak += 1;
            mask = 0x80;
        }
        if flags & mask == 0 {
            let Some(&b) = pak_buf.get(pak) else { break };
            raw_buf[raw] = b;
            raw += 1;
            pak += 1;
        } else {
            if pak + 1 >= pak_buf.len() {
                break;
            }
            let pair = usize::from(pak_buf[pak]) << 8 | usize::from(pak_buf[pak + 1]);
            pak += 2;
            let len = ((pair >> 12) + THRESHOLD + 1).min(raw_end - raw);
            let pos = (pair & 0xFFF) + 3;
            if pos > raw {
                return Err(bad("a back-reference before the start"));
            }
            for _ in 0..len {
                raw_buf[raw] = raw_buf[raw - pos];
                raw += 1;
            }
        }
    }
    raw_buf[dec_len..].reverse();
    raw_buf.truncate(raw);
    Ok(raw_buf)
}

/// Compresses `data`, keeping the first `keep` bytes uncompressed ([`ARM9_HEAD`] for an ARM9 binary, 0 otherwise).
pub fn compress(data: &[u8], keep: usize, mode: Mode) -> Vec<u8> {
    let raw_len = data.len();
    let mut raw_buf: Vec<u8> = data.iter().rev().copied().collect();
    let mut pak_buf: Vec<u8> = Vec::with_capacity(raw_len + raw_len.div_ceil(8) + 11);
    let raw_end = raw_len.saturating_sub(keep);
    let mut finder = Finder::new(&raw_buf);

    let mut pak_tmp = 0;
    let mut raw_tmp = raw_len;
    let mut raw = 0;
    let mut mask = 0u8;
    let mut flg = 0;
    let (mut pos_best, mut pos_next, mut pos_post) = (0, 0, 0);
    while raw < raw_end {
        mask >>= 1;
        if mask == 0 {
            flg = pak_buf.len();
            pak_buf.push(0);
            mask = 0x80;
        }
        let (mut len_best, p) = finder.search(pos_best, raw, raw_end);
        pos_best = p;
        if mode == Mode::Best && len_best > THRESHOLD && raw + len_best < raw_end {
            let (mut len_next, p) = finder.search(pos_next, raw + len_best, raw_end);
            pos_next = p;
            let (mut len_post, p) = finder.search(pos_post, raw + 1, raw_end);
            pos_post = p;
            if len_next <= THRESHOLD {
                len_next = 1;
            }
            if len_post <= THRESHOLD {
                len_post = 1;
            }
            if len_best + len_next <= 1 + len_post {
                len_best = 1;
            }
        }
        pak_buf[flg] <<= 1;
        if len_best > THRESHOLD {
            raw += len_best;
            pak_buf[flg] |= 1;
            let back = pos_best - 3;
            // Both fit: the run is at most 18 and `back` below 0x1000.
            pak_buf.push(u8::try_from((len_best - (THRESHOLD + 1)) << 4 | back >> 8).unwrap_or_default());
            pak_buf.push(u8::try_from(back & 0xFF).unwrap_or_default());
        } else {
            pak_buf.push(raw_buf[raw]);
            raw += 1;
        }
        if pak_buf.len() + raw_len - raw < pak_tmp + raw_tmp {
            pak_tmp = pak_buf.len();
            raw_tmp = raw_len - raw;
        }
    }
    while mask > 0 && mask != 1 {
        mask >>= 1;
        pak_buf[flg] <<= 1;
    }
    let pak_len = pak_buf.len();
    raw_buf.reverse();
    pak_buf.reverse();

    let inc_len = raw_len - pak_tmp - raw_tmp;
    let padded = (pak_tmp + raw_tmp + 3) & !3;
    let hdr_len = 8 + (padded - pak_tmp - raw_tmp);
    if pak_tmp == 0 || raw_len + 4 < padded + 8 || inc_len <= hdr_len {
        let mut out = data.to_vec();
        out.resize(raw_len.next_multiple_of(4) + 4, 0);
        return out;
    }
    let mut out = Vec::with_capacity(padded + 8);
    out.extend_from_slice(&raw_buf[..raw_tmp]);
    out.extend_from_slice(&pak_buf[pak_len - pak_tmp..]);
    out.resize(padded, 0xFF);
    let enc = u32::try_from(pak_tmp + hdr_len).unwrap_or_default().to_le_bytes();
    out.extend_from_slice(&enc[..3]);
    out.push(u8::try_from(hdr_len).unwrap_or_default());
    let inc = u32::try_from(inc_len - hdr_len).unwrap_or_default() & 0x7FFF_FFFF;
    out.extend_from_slice(&inc.to_le_bytes());
    out
}

/// Finds the nearest longest earlier match, like a scan of every distance from 3 to [`WINDOW`] would, but only
/// visits the positions that share the next three bytes.
struct Finder<'a> {
    buf: &'a [u8],
    /// The latest position with each 3-byte hash.
    head: Vec<u32>,
    /// The previous position with the same hash.
    prev: Vec<u32>,
    /// Positions below this are in the chains.
    inserted: usize,
}

const NONE: u32 = u32::MAX;
const HASH_BITS: u32 = 16;

impl<'a> Finder<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self {
            buf,
            head: vec![NONE; 1 << HASH_BITS],
            prev: vec![NONE; buf.len()],
            inserted: 0,
        }
    }

    fn hash(&self, at: usize) -> usize {
        let key = u32::from(self.buf[at]) << 16 | u32::from(self.buf[at + 1]) << 8 | u32::from(self.buf[at + 2]);
        (key.wrapping_mul(0x9E37_79B1) >> (32 - HASH_BITS)) as usize
    }

    /// Adds every position at least 3 bytes before `raw` to the chains.
    fn catch_up(&mut self, raw: usize) {
        while self.inserted + 3 <= raw && self.inserted + 2 < self.buf.len() {
            let h = self.hash(self.inserted);
            self.prev[self.inserted] = self.head[h];
            // Buffers are far below 4 GiB (BLZ caps them at 16 MiB).
            self.head[h] = u32::try_from(self.inserted).unwrap_or(NONE);
            self.inserted += 1;
        }
    }

    /// The longest match for `raw` (at least 3 bytes, else `THRESHOLD` and `p` unchanged) and its distance; the
    /// nearest wins a tie. A match never runs past `raw_end` and is never longer than its distance.
    fn search(&mut self, p: usize, raw: usize, raw_end: usize) -> (usize, usize) {
        let mut l = THRESHOLD;
        let mut p = p;
        if raw + THRESHOLD >= raw_end {
            return (l, p);
        }
        self.catch_up(raw);
        let max = raw.min(WINDOW);
        let mut q = self.head[self.hash(raw)];
        while q != NONE {
            let at = q as usize;
            q = self.prev[at];
            if at + 3 > raw {
                continue;
            }
            let pos = raw - at;
            if pos > max {
                break;
            }
            let mut len = 0;
            while len < MAX_RUN && raw + len != raw_end && len < pos && self.buf[raw + len] == self.buf[raw + len - pos]
            {
                len += 1;
            }
            if len > l {
                p = pos;
                l = len;
                if l == MAX_RUN {
                    break;
                }
            }
        }
        (l, p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The classic search: every distance from 3 up, keeping the first longest.
    fn slow_search(buf: &[u8], p: usize, raw: usize, raw_end: usize) -> (usize, usize) {
        let mut l = THRESHOLD;
        let mut p = p;
        for pos in 3..=raw.min(WINDOW) {
            let mut len = 0;
            while len < MAX_RUN && raw + len != raw_end && len < pos && buf[raw + len] == buf[raw + len - pos] {
                len += 1;
            }
            if len > l {
                p = pos;
                l = len;
                if l == MAX_RUN {
                    break;
                }
            }
        }
        (l, p)
    }

    fn sample(len: usize) -> Vec<u8> {
        // Code-like data: repeated words with small variations, and some runs.
        let mut x: u32 = 12345;
        (0..len)
            .map(|i| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
                if i % 97 < 20 {
                    0
                } else {
                    u8::try_from((x >> 16) % 7).unwrap_or(0) + u8::from(i % 3 == 0) * 0x40
                }
            })
            .collect()
    }

    #[test]
    fn the_search_matches_a_scan_of_every_distance() {
        let buf = sample(9000);
        let mut finder = Finder::new(&buf);
        for raw in 0..buf.len() {
            assert_eq!(
                finder.search(7, raw, buf.len()),
                slow_search(&buf, 7, raw, buf.len()),
                "at {raw}"
            );
        }
        // Searching earlier again (as the look-ahead does) still works.
        let mut finder = Finder::new(&buf);
        finder.search(0, 5000, buf.len());
        assert_eq!(finder.search(0, 4990, buf.len()), slow_search(&buf, 0, 4990, buf.len()));
    }

    #[test]
    fn round_trips() {
        for mode in [Mode::Normal, Mode::Best] {
            for data in [sample(20_000), vec![0u8; 5000], (0..=255).cycle().take(3000).collect()] {
                let packed = compress(&data, 0, mode);
                assert!(packed.len() < data.len());
                assert_eq!(decompress(&packed).expect("own output"), data);
            }
        }
    }

    #[test]
    fn keeps_the_arm9_head_uncompressed() {
        let data = sample(ARM9_HEAD + 10_000);
        let packed = compress(&data, ARM9_HEAD, Mode::Normal);
        assert_eq!(packed[..ARM9_HEAD], data[..ARM9_HEAD]);
        assert_eq!(decompress(&packed).expect("own output"), data);
    }

    #[test]
    fn stores_incompressible_data_uncompressed() {
        let data: Vec<u8> = (0..=255).collect();
        let packed = compress(&data[..13], 0, Mode::Normal);
        assert_eq!(packed.len(), 20);
        assert_eq!(packed[..13], data[..13]);
        assert_eq!(packed[13..], [0; 7]);
        // Uncompressed data decompresses to itself, footer included.
        assert_eq!(decompress(&packed).expect("valid"), packed);
        assert_eq!(compress(&[], 0, Mode::Normal), [0, 0, 0, 0]);
        // Compressing would save exactly the footer's length, which a footer cannot express.
        let mut barely = vec![0x40];
        barely.extend([0xFF; 19]);
        let packed = compress(&barely, 0, Mode::Normal);
        assert_eq!(decompress(&packed).expect("valid")[..20], barely);
    }

    #[test]
    fn rejects_malformed_footers() {
        assert!(decompress(&[1, 2]).is_err());
        // Footer length 7.
        assert!(decompress(&[0, 0, 0, 0, 8, 0, 0, 7, 1, 0, 0, 0]).is_err());
        // Compressed length larger than the data.
        assert!(decompress(&[0xFF, 0, 0, 8, 1, 0, 0, 0]).is_err());
        // A back-reference before the start: flag 0x80, pair 0x0000 (distance 3) with nothing decoded yet.
        assert!(decompress(&[0x00, 0x00, 0x80, 0xFF, 11, 0, 0, 8, 3, 0, 0, 0]).is_err());
    }
}
