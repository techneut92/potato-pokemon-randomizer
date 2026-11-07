//! The LZ3 compression of Game Boy Color games (also known as `LC_LZ3`). The compressor tries a fixed set of
//! strategies (copy only, fills only, and 72 single-pass variants) and keeps the first shortest result, so its
//! output is deterministic down to the byte.
//!
//! A chunk starts with a header byte `CCCLLLLL` (command, length − 1), or for longer chunks two bytes
//! `111CCCLL LLLLLLLL`. Commands: 0 copies bytes, 1 fills with a byte, 2 alternates two bytes, 3 fills with zeros,
//! 4 repeats earlier output, 5 repeats it with the bits of each byte reversed, 6 repeats it backwards. A repeat's
//! source is one byte `1DDDDDDD` (D + 1 bytes back) or two bytes of absolute position. `0xFF` ends the data.
//!
//! Deliberate choices:
//! - A long header holds the high bits of the length − 1, so chunks of 256, 512 and 768 bytes come out right.
//! - Copies longer than 1024 bytes are split, as one header can't hold them.
//! - The shortest data of the strategies is taken even when it is not shorter than twice the input.

use crate::FormatError;

const SHORT_COUNT: usize = 32;
const MAX_COUNT: usize = 1024;
const LOOKBACK: usize = 128;
const MAX_INPUT: usize = 0x8000;
/// The longest chunk the copy and fill strategies write, with a one-byte header.
const MAX_SHORT_CHUNK: usize = 31;
const END: u8 = 0xFF;

const FORMAT: &str = "LZ3";

fn truncated(offset: usize) -> FormatError {
    FormatError::Truncated { offset: offset as u64 }
}

/// The length of the LZ3 data at `offset` of `data`, the end byte included.
///
/// # Errors
/// When the data runs past the end of `data`.
pub fn compressed_length(data: &[u8], offset: usize) -> Result<usize, FormatError> {
    let byte = |at: usize| data.get(at).copied().ok_or_else(|| truncated(at));
    let mut at = offset;
    loop {
        let head = byte(at)?;
        if head == END {
            return Ok(at + 1 - offset);
        }
        let (command, length) = if head >> 5 == 7 {
            let length = usize::from(head & 3) * 0x100 + usize::from(byte(at + 1)?) + 1;
            at += 2;
            ((head >> 2) & 7, length)
        } else {
            at += 1;
            (head >> 5, usize::from(head & 0x1F) + 1)
        };
        at += match command {
            0 => length,
            1 => 1,
            2 => 2,
            3 => 0,
            _ if byte(at)? >= 0x80 => 1,
            _ => 2,
        };
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Copy = 0,
    ByteFill = 1,
    WordFill = 2,
    ZeroFill = 3,
    Repeat = 4,
    FlippedRepeat = 5,
    BackwardsRepeat = 6,
}

impl Command {
    const fn is_repeat(self) -> bool {
        matches!(self, Self::Repeat | Self::FlippedRepeat | Self::BackwardsRepeat)
    }
}

/// One command: for fills the byte or word, for repeats the source (negative: that far back from the current
/// position).
#[derive(Debug, Clone, Copy)]
struct Chunk {
    command: Command,
    count: usize,
    value: i32,
}

impl Chunk {
    const fn new(command: Command, count: usize, value: i32) -> Self {
        Self { command, count, value }
    }

    /// The size the strategies estimate for the chunk: a two-byte header from 33 on.
    fn size(&self) -> usize {
        let header = if self.count > SHORT_COUNT { 2 } else { 1 };
        if self.command.is_repeat() {
            return header + 1 + usize::from(self.value >= 0);
        }
        header
            + match self.command {
                Command::Copy => self.count,
                Command::ByteFill => 1,
                Command::WordFill => 2,
                _ => 0,
            }
    }

    #[expect(clippy::cast_possible_wrap, reason = "sizes are far below i64::MAX")]
    fn savings(&self) -> i64 {
        self.count as i64 - self.size() as i64
    }
}

/// The chunk that saves the most bytes; the first of equals; `None` when there is none.
fn best(chunks: &[Option<Chunk>]) -> Option<Chunk> {
    let mut best: Option<Chunk> = None;
    for c in chunks.iter().flatten() {
        if best.is_none_or(|b| c.savings() > b.savings()) {
            best = Some(*c);
        }
    }
    best
}

/// A fill at `pos` of at most `limit` bytes: zeros, a byte or two alternating bytes.
fn fill(data: &[u8], pos: usize, limit: usize) -> Chunk {
    if pos + 1 >= data.len() {
        return if data[pos] == 0 {
            Chunk::new(Command::ZeroFill, 1, 0)
        } else {
            Chunk::new(Command::ByteFill, 1, i32::from(data[pos]))
        };
    }
    let value = [data[pos], data[pos + 1]];
    let limit = (data.len() - pos).min(limit);
    let mut count = 2;
    while count < limit && data[pos + count] == value[count & 1] {
        count += 1;
    }
    if value[0] != value[1] {
        if value[0] == 0 && count < 3 {
            Chunk::new(Command::ZeroFill, 1, 0)
        } else {
            Chunk::new(Command::WordFill, count, i32::from(value[0]) << 8 | i32::from(value[1]))
        }
    } else if value[0] != 0 {
        Chunk::new(Command::ByteFill, count, i32::from(value[0]))
    } else {
        Chunk::new(Command::ZeroFill, count, 0)
    }
}

/// The source of a repeat from `found`: relative when it is at most 128 bytes back.
fn source(found: usize, pos: usize) -> i32 {
    let found = i32::try_from(found).unwrap_or(i32::MAX);
    let pos = i32::try_from(pos).unwrap_or(i32::MAX);
    if found + i32::try_from(LOOKBACK).unwrap_or(0) >= pos {
        found - pos
    } else {
        found
    }
}

/// The longest earlier run of `source` (the data, or its bit-reversed bytes) that matches the data at `pos`.
fn scan_forwards(data: &[u8], source_data: &[u8], pos: usize, command: Command) -> Option<Chunk> {
    let limit = source_data.len() - pos;
    let (mut found, mut length) = (0, 0);
    for from in 0..pos {
        if source_data[from] != data[pos] {
            continue;
        }
        let mut n = 0;
        while n < limit && source_data[from + n] == data[pos + n] {
            n += 1;
        }
        let n = n.min(MAX_COUNT);
        if n > length {
            (found, length) = (from, n);
        }
    }
    (length > 0).then(|| Chunk::new(command, length, source(found, pos)))
}

/// The longest earlier run, read backwards, that matches the data at `pos`.
fn scan_backwards(data: &[u8], pos: usize) -> Option<Chunk> {
    let limit = (data.len() - pos).min(pos);
    let (mut found, mut length) = (0, 0);
    for from in 0..pos {
        if data[from] != data[pos] {
            continue;
        }
        let mut n = 0;
        while n <= from && n < limit && data[from - n] == data[pos + n] {
            n += 1;
        }
        let n = n.min(MAX_COUNT);
        if n > length {
            (found, length) = (from, n);
        }
    }
    (length > 0).then(|| Chunk::new(Command::BackwardsRepeat, length, source(found, pos)))
}

/// Which repeat wins a tie.
#[derive(Debug, Clone, Copy)]
enum RepeatOrder {
    /// Normal, backwards, flipped.
    Nrf,
    /// Backwards, flipped, normal.
    Rfn,
    /// Flipped, backwards, normal.
    Frn,
}

/// One single-pass strategy.
#[derive(Debug, Clone, Copy)]
struct SinglePass {
    /// A fill wins a tie with a repeat.
    prefer_fill: bool,
    /// No fill or repeat that saves nothing right after a copy of an odd length.
    avoid_fill_or_repeat: bool,
    /// Repeats copy at most 32 bytes.
    avoid_long_repeat: bool,
    /// How many single bytes are copied after each other command.
    max_scan_delay: usize,
    order: RepeatOrder,
}

impl SinglePass {
    fn all() -> impl Iterator<Item = Self> {
        [false, true].into_iter().flat_map(|prefer_fill| {
            [false, true].into_iter().flat_map(move |avoid_fill_or_repeat| {
                [false, true].into_iter().flat_map(move |avoid_long_repeat| {
                    (0..3).flat_map(move |max_scan_delay| {
                        [RepeatOrder::Nrf, RepeatOrder::Rfn, RepeatOrder::Frn]
                            .into_iter()
                            .map(move |order| Self {
                                prefer_fill,
                                avoid_fill_or_repeat,
                                avoid_long_repeat,
                                max_scan_delay,
                                order,
                            })
                    })
                })
            })
        })
    }

    fn repeat(&self, data: &[u8], flipped: &[u8], pos: usize) -> Option<Chunk> {
        let simple = scan_forwards(data, data, pos, Command::Repeat);
        let flip = scan_forwards(data, flipped, pos, Command::FlippedRepeat);
        let backwards = scan_backwards(data, pos);
        let mut chunk = match self.order {
            RepeatOrder::Nrf => best(&[simple, backwards, flip]),
            RepeatOrder::Rfn => best(&[backwards, flip, simple]),
            RepeatOrder::Frn => best(&[flip, backwards, simple]),
        };
        if self.avoid_long_repeat
            && let Some(c) = &mut chunk
            && c.count > SHORT_COUNT
        {
            c.count = SHORT_COUNT;
        }
        chunk
    }

    fn compress(&self, data: &[u8], flipped: &[u8]) -> Vec<u8> {
        let mut chunks = Vec::new();
        let (mut pos, mut previous_copy, mut scan_delay) = (0, 0, 0);
        while pos < data.len() {
            let fill = Some(fill(data, pos, MAX_COUNT));
            let repeat = self.repeat(data, flipped, pos);
            let other = if self.prefer_fill {
                best(&[fill, repeat])
            } else {
                best(&[repeat, fill])
            };
            let direct = Chunk::new(Command::Copy, 1, 0);
            let mut chunk = best(&[Some(direct), other]).unwrap_or(direct);
            if self.avoid_fill_or_repeat
                && chunk.size() == chunk.count
                && previous_copy != 0
                && previous_copy != SHORT_COUNT
                && previous_copy != MAX_COUNT
            {
                chunk = direct;
            }
            if self.max_scan_delay != 0 {
                if scan_delay >= self.max_scan_delay {
                    scan_delay = 0;
                } else if chunk.command != Command::Copy {
                    scan_delay += 1;
                    chunk = direct;
                }
            }
            if chunk.command == Command::Copy {
                previous_copy += chunk.count;
            } else {
                previous_copy = 0;
            }
            pos += chunk.count;
            chunks.push(chunk);
        }
        to_bytes(&merge_copies(&chunks), data)
    }
}

/// Neighbouring copies as one, at most 1024 bytes each.
fn merge_copies(chunks: &[Chunk]) -> Vec<Chunk> {
    let mut merged = Vec::new();
    let mut copy = 0;
    let flush = |merged: &mut Vec<Chunk>, copy: &mut usize| {
        while *copy > 0 {
            let n = (*copy).min(MAX_COUNT);
            merged.push(Chunk::new(Command::Copy, n, 0));
            *copy -= n;
        }
    };
    for c in chunks {
        if c.command == Command::Copy {
            copy += c.count;
        } else {
            flush(&mut merged, &mut copy);
            merged.push(*c);
        }
    }
    flush(&mut merged, &mut copy);
    merged
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "bytes of checked values"
)]
fn to_bytes(chunks: &[Chunk], data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut pos = 0;
    for c in chunks {
        let bits = c.command as usize;
        if c.count < SHORT_COUNT {
            out.push(((bits << 5) + ((c.count - 1) & 0x1F)) as u8);
        } else {
            out.push((0b1110_0000 + (bits << 2) + ((c.count - 1) >> 8)) as u8);
            out.push(((c.count - 1) & 0xFF) as u8);
        }
        match c.command {
            Command::Copy => out.extend_from_slice(&data[pos..pos + c.count]),
            Command::ByteFill => out.push(c.value as u8),
            Command::WordFill => out.extend_from_slice(&[(c.value >> 8) as u8, c.value as u8]),
            Command::ZeroFill => {}
            _ if c.value < 0 => out.push((c.value ^ 127) as u8),
            _ => out.extend_from_slice(&[(c.value >> 8) as u8, c.value as u8]),
        }
        pos += c.count;
    }
    out.push(END);
    out
}

/// Copies only, 31 bytes at a time.
fn copy_only(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for chunk in data.chunks(MAX_SHORT_CHUNK) {
        out.push(u8::try_from(chunk.len() - 1).unwrap_or(0));
        out.extend_from_slice(chunk);
    }
    out.push(END);
    out
}

/// Fills only, at most 31 bytes each.
fn fills_only(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    let mut pos = 0;
    while pos < data.len() {
        let c = fill(data, pos, MAX_SHORT_CHUNK);
        let head = ((c.command as usize) << 5) + ((c.count - 1) & 0x1F);
        out.push(u8::try_from(head).unwrap_or(0));
        let [_, _, hi, lo] = c.value.to_be_bytes();
        match c.command {
            Command::ByteFill => out.push(lo),
            Command::WordFill => out.extend_from_slice(&[hi, lo]),
            _ => {}
        }
        pos += c.count;
    }
    out.push(END);
    out
}

/// Compresses `data`: the first shortest result of the strategies.
///
/// # Errors
/// When `data` is longer than 32 KiB, as far as repeats can reach.
pub fn compress(data: &[u8]) -> Result<Vec<u8>, FormatError> {
    if data.len() > MAX_INPUT {
        return Err(FormatError::WrongFormat {
            format: FORMAT,
            reason: "more than 32 KiB to compress",
        });
    }
    let flipped: Vec<u8> = data.iter().map(|b| b.reverse_bits()).collect();
    let mut shortest = copy_only(data);
    let candidates = std::iter::once(fills_only(data)).chain(SinglePass::all().map(|s| s.compress(data, &flipped)));
    for c in candidates {
        if c.len() < shortest.len() {
            shortest = c;
        }
    }
    Ok(shortest)
}

/// Decompresses the LZ3 data at `offset` of `data`.
///
/// # Errors
/// When the data runs past the end of `data` or repeats from outside the output.
pub fn decompress(data: &[u8], offset: usize) -> Result<Vec<u8>, FormatError> {
    let byte = |at: usize| data.get(at).copied().ok_or_else(|| truncated(at));
    let bad = || FormatError::WrongFormat {
        format: FORMAT,
        reason: "a repeat from outside the output",
    };
    let mut out: Vec<u8> = Vec::new();
    let mut at = offset;
    loop {
        let head = byte(at)?;
        if head == END {
            return Ok(out);
        }
        let (command, length) = if head >> 5 == 7 {
            let length = usize::from(head & 3) * 0x100 + usize::from(byte(at + 1)?) + 1;
            at += 2;
            ((head >> 2) & 7, length)
        } else {
            at += 1;
            (head >> 5, usize::from(head & 0x1F) + 1)
        };
        match command {
            0 => {
                out.extend_from_slice(data.get(at..at + length).ok_or_else(|| truncated(at))?);
                at += length;
            }
            1 => {
                out.extend(std::iter::repeat_n(byte(at)?, length));
                at += 1;
            }
            2 => {
                let pair = [byte(at)?, byte(at + 1)?];
                out.extend((0..length).map(|i| pair[i & 1]));
                at += 2;
            }
            3 => out.extend(std::iter::repeat_n(0, length)),
            _ => {
                let first = byte(at)?;
                let from = if first >= 0x80 {
                    at += 1;
                    out.len().checked_sub(usize::from(first & 0x7F) + 1).ok_or_else(bad)?
                } else {
                    at += 2;
                    usize::from(first) << 8 | usize::from(byte(at - 1)?)
                };
                for i in 0..length {
                    let b = match command {
                        6 => from.checked_sub(i).and_then(|s| out.get(s)).copied(),
                        _ => out.get(from + i).copied(),
                    }
                    .ok_or_else(bad)?;
                    out.push(if command == 5 { b.reverse_bits() } else { b });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(data: &[u8]) -> Vec<u8> {
        let packed = compress(data).expect("compresses");
        assert_eq!(compressed_length(&packed, 0).expect("length"), packed.len());
        assert_eq!(decompress(&packed, 0).expect("decompresses"), data);
        packed
    }

    #[test]
    fn data_round_trips() {
        round_trip(&[]);
        round_trip(&[7]);
        round_trip(&(0..=255).collect::<Vec<u8>>());
        let pattern: Vec<u8> = (0..600u32).map(|i| u8::try_from(i * 37 % 11).unwrap_or(0)).collect();
        round_trip(&pattern);
        let mut sprite = vec![0u8; 576];
        for (i, b) in sprite.iter_mut().enumerate().skip(100).take(300) {
            *b = u8::try_from(i % 5).unwrap_or(0).reverse_bits();
        }
        assert!(round_trip(&sprite).len() < 200);
    }

    #[test]
    fn long_chunks_hold_their_length() {
        // 512 zeros: one long zero fill, whose header holds 511.
        assert_eq!(round_trip(&[0; 512]), [0b1110_1101, 0xFF, END]);
    }

    #[test]
    fn copies_and_fills_are_written_as_documented() {
        assert_eq!(copy_only(&[1, 2]), [1, 1, 2, END]);
        assert_eq!(fills_only(&[5, 5, 5, 0]), [0b0010_0010, 5, 0b0110_0000, END]);
        assert!(compress(&vec![0; MAX_INPUT + 1]).is_err());
    }
}
