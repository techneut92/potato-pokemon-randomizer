//! The extended LZ77 variant of the DS BIOS ("LZ11"): like LZ10, but back-references copy up to 65 808 bytes. It is
//! also the compression of 3DS GARC entries.
//!
//! [`compress`] picks, at every position, the longest earlier run (the farthest one on a tie) and never refers one
//! byte back. Its output is deterministic.

use crate::FormatError;

/// The type byte that starts LZ11 data.
pub(crate) const LZ11: u8 = 0x11;

const FORMAT: &str = "LZ11";

fn truncated(offset: usize) -> FormatError {
    FormatError::Truncated { offset: offset as u64 }
}

/// Decompresses LZ11 data starting at `offset` of `data`. A length of 0 in the header means a 32-bit big-endian
/// length follows.
///
/// # Errors
/// When `data` holds no LZ11 data at `offset`, ends early or refers back before the start of the output.
pub fn decompress(data: &[u8], offset: usize) -> Result<Vec<u8>, FormatError> {
    let byte = |at: usize| data.get(at).copied().ok_or_else(|| truncated(at));
    if byte(offset)? != LZ11 {
        return Err(FormatError::WrongFormat {
            format: FORMAT,
            reason: "no LZ11 type byte",
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
    // A forged header can claim gigabytes; don't reserve more than the input could expand to.
    let mut out = Vec::with_capacity(length.min(data.len().saturating_sub(at).saturating_mul(0x400)));
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
                continue;
            }
            let b1 = usize::from(byte(at)?);
            at += 1;
            let (len, disp) = match b1 >> 4 {
                0 => {
                    let (b2, b3) = (usize::from(byte(at)?), usize::from(byte(at + 1)?));
                    at += 2;
                    ((b1 << 4 | b2 >> 4) + 0x11, (b2 & 0x0F) << 8 | b3)
                }
                1 => {
                    let (b2, b3, b4) = (
                        usize::from(byte(at)?),
                        usize::from(byte(at + 1)?),
                        usize::from(byte(at + 2)?),
                    );
                    at += 3;
                    (((b1 & 0x0F) << 12 | b2 << 4 | b3 >> 4) + 0x111, (b3 & 0x0F) << 8 | b4)
                }
                n => {
                    let b2 = usize::from(byte(at)?);
                    at += 1;
                    (n + 1, (b1 & 0x0F) << 8 | b2)
                }
            };
            if disp >= out.len() {
                return Err(FormatError::WrongFormat {
                    format: FORMAT,
                    reason: "a back-reference before the start",
                });
            }
            let from = out.len() - disp - 1;
            for j in 0..len {
                if out.len() >= length {
                    break;
                }
                out.push(out[from + j]);
            }
        }
    }
    Ok(out)
}

/// How far back a back-reference reaches.
const WINDOW: usize = 0x1000;
/// The longest run one back-reference copies.
const MAX_RUN: usize = 0x10110;
/// The shortest run worth a back-reference.
const MIN_RUN: usize = 3;
/// The largest size the 24-bit header holds.
const MAX_LEN: usize = 0x00FF_FFFF;

/// The longest earlier run matching `data[at..]`: `(length, distance)`. Of equally long runs the farthest wins, and
/// runs one byte back are not considered.
fn longest_run(data: &[u8], at: usize, head: &[u32], prev: &[u32], candidates: &mut Vec<usize>) -> (usize, usize) {
    let max = (data.len() - at).min(MAX_RUN);
    if max < MIN_RUN {
        return (0, 0);
    }
    let lowest = at.saturating_sub(WINDOW);
    candidates.clear();
    let mut j = head[hash3(data, at)];
    while j != NONE && (j as usize) >= lowest {
        let jj = j as usize;
        if jj + 1 < at {
            candidates.push(jj);
        }
        j = prev[jj];
    }
    let (mut best, mut disp) = (0, 0);
    // Farthest first, so a later run must be strictly longer to win.
    for &start in candidates.iter().rev() {
        let len = data[at..at + max]
            .iter()
            .zip(&data[start..])
            .take_while(|(a, b)| a == b)
            .count();
        if len > best {
            best = len;
            disp = at - start;
            if best == max {
                break;
            }
        }
    }
    (best, disp)
}

const NONE: u32 = u32::MAX;

/// The low 8 bits of `n`.
fn low_byte(n: usize) -> u8 {
    n.to_le_bytes()[0]
}

fn hash3(data: &[u8], i: usize) -> usize {
    let v = u32::from(data[i]) << 16 | u32::from(data[i + 1]) << 8 | u32::from(data[i + 2]);
    (v.wrapping_mul(0x9E37_79B1) >> 16) as usize
}

/// Compresses `data` with a 24-bit length header. Empty data gets a zero header followed by a zero 32-bit length.
///
/// # Errors
/// When `data` is larger than the 24-bit header can describe.
pub fn compress(data: &[u8]) -> Result<Vec<u8>, FormatError> {
    if data.len() > MAX_LEN {
        return Err(FormatError::Unencodable {
            format: FORMAT,
            reason: "more than 16 MiB of data",
        });
    }
    let len = u32::try_from(data.len()).unwrap_or(0).to_le_bytes();
    let mut out = vec![LZ11, len[0], len[1], len[2]];
    if data.is_empty() {
        out.extend_from_slice(&[0; 4]);
        return Ok(out);
    }
    out.reserve(data.len() + data.len() / 8);
    let mut head = vec![NONE; 1 << 16];
    let mut prev = vec![NONE; data.len()];
    let mut inserted = 0;
    let mut candidates = Vec::new();
    let mut at = 0;
    let mut flag_at = 0;
    let mut blocks = 8;
    while at < data.len() {
        if blocks == 8 {
            flag_at = out.len();
            out.push(0);
            blocks = 0;
        }
        // Index every position before `at` that has 3 bytes to hash.
        while inserted < at && inserted + 2 < data.len() {
            let h = hash3(data, inserted);
            prev[inserted] = head[h];
            head[h] = u32::try_from(inserted).unwrap_or(NONE);
            inserted += 1;
        }
        let (run, disp) = longest_run(data, at, &head, &prev, &mut candidates);
        if run < MIN_RUN {
            out.push(data[at]);
            at += 1;
        } else {
            out[flag_at] |= 0x80 >> blocks;
            let d = disp - 1;
            if run > 0x110 {
                let n = run - 0x111;
                out.extend_from_slice(&[
                    0x10 | low_byte(n >> 12),
                    low_byte(n >> 4),
                    low_byte((n & 0x0F) << 4 | d >> 8),
                ]);
            } else if run > 0x10 {
                let n = run - 0x11;
                out.extend_from_slice(&[low_byte(n >> 4), low_byte((n & 0x0F) << 4 | d >> 8)]);
            } else {
                out.push(low_byte((run - 1) << 4 | d >> 8));
            }
            out.push(low_byte(d));
            at += run;
        }
        blocks += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_all_three_reference_sizes() {
        // "ab", then a 3-byte run (short form, len 3 = 2 + 1), a 0x11 run (medium form) and a 0x111 run (long form),
        // all repeating "ab".
        let data = [
            LZ11,
            0x26,
            0x01,
            0x00, // 2 + 3 + 0x11 + 0x111 = 0x126 bytes
            0b0011_1000,
            b'a',
            b'b',
            0x20,
            0x01, // len 3, disp 2
            0x00,
            0x00,
            0x01, // len 0x11, disp 2
            0x10,
            0x00,
            0x00,
            0x01, // len 0x111, disp 2
        ];
        let out = decompress(&data, 0).expect("valid");
        assert_eq!(out.len(), 0x126);
        assert!(out.chunks(2).all(|c| c == b"ab"));
    }

    #[test]
    fn rejects_bad_data() {
        assert!(decompress(&[0x10, 1, 0, 0, 0, 5], 0).is_err());
        assert!(decompress(&[LZ11, 4, 0, 0, 0x80, 0x20, 0x00], 0).is_err());
        assert!(matches!(
            decompress(&[LZ11, 2, 0, 0, 0x00, 1], 0),
            Err(FormatError::Truncated { .. })
        ));
    }

    #[test]
    fn round_trips_runs_of_every_size() {
        let mut data = Vec::new();
        for (i, run) in [1usize, 5, 16, 17, 100, 0x110, 0x111, 0x2000].into_iter().enumerate() {
            data.extend(std::iter::repeat_n(u8::try_from(i).expect("small"), run));
            data.extend_from_slice(b"pattern-");
        }
        data.extend(std::iter::repeat_n(0u8, 70_000));
        let packed = compress(&data).expect("fits");
        assert!(packed.len() < data.len() / 10);
        assert_eq!(decompress(&packed, 0).expect("decodes"), data);
    }

    #[test]
    fn never_refers_one_byte_back() {
        let packed = compress(&[7; 10]).expect("fits");
        // 7, 7 as literals, then a run of 8 at distance 2.
        assert_eq!(packed, [0x11, 10, 0, 0, 0x20, 7, 7, 0x70, 0x01]);
        assert_eq!(decompress(&packed, 0).expect("decodes"), [7; 10]);
    }

    #[test]
    fn prefers_the_farthest_of_equal_runs() {
        let data = b"abcdXabcdYabcd";
        let packed = compress(data).expect("fits");
        assert_eq!(decompress(&packed, 0).expect("decodes"), data);
        // The last run copies "abcd" from offset 0, 10 bytes back, not from offset 5.
        assert_eq!(&packed[packed.len() - 2..], [0x30, 0x09]);
    }

    #[test]
    fn empty_data_has_an_extended_zero_header() {
        let packed = compress(&[]).expect("fits");
        assert_eq!(packed, [0x11, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(decompress(&packed, 0).expect("decodes"), Vec::<u8>::new());
    }
}
