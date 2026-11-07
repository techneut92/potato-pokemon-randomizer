//! The LZ77 variant of the GBA and DS BIOS ("LZ10"). The compressor's output is deterministic down to the byte,
//! quirks included (see [`compress`]), so recompressed data that didn't change keeps its length and its place.

use crate::FormatError;

/// The type byte that starts LZ10 data.
const LZ10: u8 = 0x10;
/// The longest run one back-reference copies.
const MAX_RUN: usize = 0x12;
/// How far back a back-reference reaches.
const WINDOW: usize = 0x1000;
/// The shortest run worth a back-reference.
const MIN_RUN: usize = 3;

const FORMAT: &str = "LZ10";

fn truncated(offset: usize) -> FormatError {
    FormatError::Truncated { offset: offset as u64 }
}

/// Decompresses LZ10 data starting at `offset` of `data`. A length of 0 in the header means a 32-bit big-endian
/// length follows.
///
/// # Errors
/// When `data` holds no LZ10 data at `offset`, ends early or refers back before the start of the output.
pub fn decompress(data: &[u8], offset: usize) -> Result<Vec<u8>, FormatError> {
    let byte = |at: usize| data.get(at).copied().ok_or_else(|| truncated(at));
    if byte(offset)? != LZ10 {
        return Err(FormatError::WrongFormat {
            format: FORMAT,
            reason: "no LZ10 type byte",
        });
    }
    let mut at = offset + 1;
    let mut length = usize::from(byte(at)?) | usize::from(byte(at + 1)?) << 8 | usize::from(byte(at + 2)?) << 16;
    at += 3;
    if length == 0 {
        let word = data.get(at..at + 4).ok_or_else(|| truncated(at))?;
        length =
            usize::try_from(u32::from_be_bytes([word[0], word[1], word[2], word[3]])).map_err(|_| truncated(at))?;
        at += 4;
    }
    // A forged header can claim gigabytes; real data expands at most 144 bytes per 17 (a flag byte and eight
    // back-references), so reserve no more than that.
    let mut out = Vec::with_capacity(length.min(data.len().saturating_sub(at).saturating_mul(9)));
    while out.len() < length {
        let flags = byte(at)?;
        at += 1;
        for i in 0..8 {
            if out.len() >= length {
                break;
            }
            if flags & (0x80 >> i) == 0 {
                out.push(byte(at)?);
                at += 1;
            } else {
                let b = byte(at)?;
                let n = usize::from(b >> 4) + MIN_RUN;
                let disp = usize::from(b & 0x0F) << 8 | usize::from(byte(at + 1)?);
                at += 2;
                if disp >= out.len() {
                    return Err(FormatError::WrongFormat {
                        format: FORMAT,
                        reason: "a back-reference before the start",
                    });
                }
                let from = out.len() - disp - 1;
                for j in 0..n {
                    if out.len() >= length {
                        break;
                    }
                    out.push(out[from + j]);
                }
            }
        }
    }
    Ok(out)
}

/// Compresses `data` to LZ10. Data longer than 2^24 - 1 bytes gets the long header. Back-references never reach just
/// one byte back.
///
/// Empty data gets a length of 0, which [`decompress`] reads as the start of a long header, so it doesn't read back.
/// Nothing compresses empty data.
pub fn compress(data: &[u8]) -> Vec<u8> {
    let mut out = vec![LZ10];
    let len = data.len().to_le_bytes();
    if data.len() > 0xFF_FFFF {
        out.extend([0, 0, 0]);
        out.extend(&len[..4]);
    } else {
        out.extend(&len[..3]);
    }
    let mut block = vec![0u8];
    let mut blocks = 0;
    let mut current = 0;
    while current < data.len() {
        if blocks == 8 {
            out.append(&mut block);
            block.push(0);
            blocks = 0;
        }
        let (run, disp) = longest_match(data, current, (data.len() - current).min(MAX_RUN), current.min(WINDOW));
        if run < MIN_RUN {
            block.push(data[current]);
            current += 1;
        } else {
            block[0] |= 1 << (7 - blocks);
            let back = disp - 1;
            // Both fit: `run - 3` is at most 15 and `back` below 0x1000.
            block.push(u8::try_from((run - MIN_RUN) << 4 | back >> 8).unwrap_or_default());
            block.push(u8::try_from(back & 0xFF).unwrap_or_default());
            current += run;
        }
        blocks += 1;
    }
    if blocks > 0 {
        out.append(&mut block);
    }
    out
}

/// The longest earlier run matching the data at `current`, of at most `pending` bytes, starting within `finished`
/// bytes back, the farthest first. It never looks just one byte back.
fn longest_match(data: &[u8], current: usize, pending: usize, finished: usize) -> (usize, usize) {
    let mut length = 0;
    let mut disp = 0;
    for i in 0..finished.saturating_sub(1) {
        let start = current - finished + i;
        let run = (0..pending)
            .take_while(|&j| data[start + j] == data[current + j])
            .count();
        if run > length {
            length = run;
            disp = finished - i;
            if length == pending {
                break;
            }
        }
    }
    (length, disp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_forged_length_fails_without_reserving_it() {
        // Long header claiming 4 GiB, then one literal.
        let forged = [LZ10, 0, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x41];
        assert!(matches!(decompress(&forged, 0), Err(FormatError::Truncated { .. })));
    }

    #[test]
    fn round_trips() {
        let palette: Vec<u8> = (0..32u8).map(|i| i % 7).collect();
        // Empty data cannot round-trip: a length of 0 means a long length follows.
        for data in [vec![5], palette, vec![0xAB; 300], (0..=255).collect()] {
            let packed = compress(&data);
            assert_eq!(decompress(&packed, 0).expect("valid"), data);
        }
    }

    #[test]
    fn never_refers_one_byte_back() {
        // No back-reference reaches one byte back, so a run of one value starts with two literals.
        assert_eq!(compress(&[7; 6]), [0x10, 6, 0, 0, 0b0010_0000, 7, 7, 0x10, 0x01]);
        assert!(decompress(&[0x11, 0, 0, 0], 0).is_err());
        assert!(decompress(&[0x10, 4, 0, 0, 0x80, 0x00, 0x00], 0).is_err());
    }
}
