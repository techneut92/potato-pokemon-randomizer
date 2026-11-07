//! AMX scripts: the compiled event scripts of the Gen 6 and Gen 7 games, stored with their instructions packed into
//! a variable-length encoding.
//!
//! A script has a header (total length, magic, a pointer table position and count, where the instructions start,
//! where the movement data starts, where the decompressed instructions end, the memory to allocate), extra data up
//! to the instructions, and the packed instructions. Each 32-bit instruction is packed big-end first into 7-bit
//! groups; a set top bit means another group follows, and bit 6 of the first group is the sign.

use crate::RomError;
use crate::gen6::rom::Bytes;

const MAGIC: u32 = 0x0A0A_F1E0;
const HEADER_LEN: usize = 0x1C;

/// An unpacked script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Amx {
    /// The unpacked instructions and data, for reading and changing.
    pub(crate) data: Vec<u8>,
    /// Where the script starts in the file it was found in (see [`Amx::find`]).
    pub(crate) offset: usize,
    header: [u8; HEADER_LEN],
    extra: Vec<u8>,
}

fn malformed(why: &str) -> RomError {
    RomError::Malformed(format!("AMX script: {why}"))
}

impl Amx {
    /// The script that a file holds.
    pub(crate) fn parse(file: &[u8]) -> Result<Self, RomError> {
        let length = usize::try_from(file.u32_at(0)?).map_err(|_| malformed("length"))?;
        if file.u32_at(4)? != MAGIC {
            return Err(malformed("no magic"));
        }
        let instr_start = usize::try_from(file.u32_at(0xC)?).map_err(|_| malformed("start"))?;
        let final_offset = usize::try_from(file.u32_at(0x14)?).map_err(|_| malformed("end"))?;
        if instr_start < HEADER_LEN || instr_start > length || length > file.len() || final_offset < instr_start {
            return Err(malformed("bad header"));
        }
        let data = unpack(&file[instr_start..length], final_offset - instr_start)?;
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&file[..HEADER_LEN]);
        Ok(Self {
            data,
            offset: 0,
            header,
            extra: file[HEADER_LEN..instr_start].to_vec(),
        })
    }

    /// Script number `n` (from 0) inside a larger file, found by its magic; its length is the word before.
    pub(crate) fn find(file: &[u8], n: usize) -> Result<Self, RomError> {
        let mut found = 0;
        for i in 0..file.len().saturating_sub(3) {
            if file.u32_at(i)? == MAGIC {
                if found == n {
                    let start = i.checked_sub(4).ok_or_else(|| malformed("magic at the start"))?;
                    let length = usize::try_from(file.u32_at(start)?).map_err(|_| malformed("length"))?;
                    let end = start.checked_add(length).filter(|&e| e <= file.len());
                    let end = end.ok_or_else(|| malformed("length past the end"))?;
                    let mut script = Self::parse(&file[start..end])?;
                    script.offset = start;
                    return Ok(script);
                }
                found += 1;
            }
        }
        Err(malformed("not found"))
    }

    /// The script packed again; its length field is the new total.
    pub(crate) fn to_bytes(&self) -> Result<Vec<u8>, RomError> {
        if !self.data.len().is_multiple_of(4) {
            return Err(malformed("data not in whole words"));
        }
        let mut out = self.header.to_vec();
        out.extend_from_slice(&self.extra);
        for word in self.data.as_chunks::<4>().0 {
            pack(u32::from_le_bytes([word[0], word[1], word[2], word[3]]), &mut out);
        }
        let total = u32::try_from(out.len()).map_err(|_| malformed("too long"))?;
        out[..4].copy_from_slice(&total.to_le_bytes());
        Ok(out)
    }
}

/// Unpacks `len` bytes of instructions. A last instruction without its final group is dropped, as is a word that
/// does not fit.
fn unpack(packed: &[u8], len: usize) -> Result<Vec<u8>, RomError> {
    let mut out = vec![0u8; len];
    let (mut i, mut groups, mut x) = (0usize, 0usize, 0u32);
    let mut bytes = packed.iter();
    while i < out.len() {
        let b = *bytes.next().ok_or_else(|| malformed("packed data too short"))?;
        let v = u32::from(b & 0x7F);
        groups += 1;
        x = if groups == 1 {
            // Bit 6 of the first group is the sign.
            if v & 0x40 != 0 { 0xFFFF_FFC0 | v } else { v }
        } else {
            (x << 7) | v
        };
        if b & 0x80 != 0 {
            continue;
        }
        for byte in x.to_le_bytes() {
            if i < out.len() {
                out[i] = byte;
            }
            i += 1;
        }
        groups = 0;
    }
    Ok(out)
}

/// Packs one instruction: the fewest 7-bit groups that keep its sign, at most five.
fn pack(instruction: u32, out: &mut Vec<u8>) {
    let negative = instruction & 0x8000_0000 != 0;
    let mut value = u64::from(instruction);
    let mut shadow = u64::from(if negative { !instruction } else { instruction });
    let mut groups: Vec<u8> = Vec::with_capacity(5);
    loop {
        let mut byte = u8::try_from(value & 0x7F).unwrap_or(0);
        if !groups.is_empty() {
            byte |= 0x80;
        }
        groups.push(byte);
        value >>= 7;
        shadow >>= 7;
        if shadow == 0 {
            break;
        }
    }
    if groups.len() < 5 {
        let sign = if negative { 0x40 } else { 0 };
        if groups.last().is_some_and(|&g| g & 0x40 != sign) {
            groups.push(if negative { 0xFF } else { 0x80 });
        }
    }
    groups.reverse();
    out.extend_from_slice(&groups);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(words: &[u32]) -> Vec<u8> {
        let mut data = Vec::new();
        for &w in words {
            data.extend_from_slice(&w.to_le_bytes());
        }
        let mut header = [0u8; HEADER_LEN];
        header[4..8].copy_from_slice(&MAGIC.to_le_bytes());
        let start = u32::try_from(HEADER_LEN + 4).expect("small");
        header[0xC..0x10].copy_from_slice(&start.to_le_bytes());
        let end = start + u32::try_from(data.len()).expect("small");
        header[0x14..0x18].copy_from_slice(&end.to_le_bytes());
        let amx = Amx {
            data,
            offset: 0,
            header,
            extra: vec![1, 2, 3, 4],
        };
        amx.to_bytes().expect("packs")
    }

    #[test]
    fn words_survive_packing() {
        let words = [
            0,
            1,
            0x3F,
            0x40,
            0x7F,
            0x80,
            0x1234_5678,
            0xFFFF_FFFF,
            0xFFFF_FFC0,
            0x8000_0000,
            0x7FFF_FFFF,
        ];
        let bytes = script(&words);
        let amx = Amx::parse(&bytes).expect("parses");
        let back: Vec<u32> = amx
            .data
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
            .collect();
        assert_eq!(back, words);
        assert_eq!(amx.extra, [1, 2, 3, 4]);
        assert_eq!(amx.to_bytes().expect("packs"), bytes);
    }

    #[test]
    fn small_values_take_one_group_and_signs_are_kept() {
        let mut out = Vec::new();
        pack(5, &mut out);
        assert_eq!(out, [5]);
        out.clear();
        pack(0x40, &mut out);
        assert_eq!(out, [0x80, 0x40]);
        out.clear();
        pack(0xFFFF_FFFF, &mut out);
        assert_eq!(out, [0x7F]);
        out.clear();
        pack(0xFFFF_FF80, &mut out);
        assert_eq!(out, [0xFF, 0x00]);
    }

    #[test]
    fn scripts_are_found_by_number() {
        let one = script(&[7]);
        let two = script(&[9, 10]);
        let mut file = vec![0xEE; 3];
        file.extend_from_slice(&one);
        file.extend_from_slice(&two);
        let found = Amx::find(&file, 1).expect("second");
        assert_eq!(found.offset, 3 + one.len());
        assert_eq!(found.data, [9, 0, 0, 0, 10, 0, 0, 0]);
        assert!(Amx::find(&file, 2).is_err());
    }
}
