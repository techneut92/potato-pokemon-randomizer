//! AMX scripts: a header and the script's 32-bit cells, each stored in 1 to 5 bytes of 7 bits, high bits first,
//! the top bit marking a byte that more bytes follow.

use crate::RomError;

const MAGIC: u32 = 0x0A0A_F1E0;

fn u32_at(d: &[u8], at: usize) -> Result<u32, RomError> {
    d.get(at..at + 4)
        .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or_else(|| RomError::Malformed("AMX script cut off".into()))
}

/// A decompressed script.
#[derive(Debug, Clone)]
pub(super) struct Amx {
    /// The header after the length, up to where the cells start.
    header: Vec<u8>,
    /// The cells, as bytes.
    pub(super) data: Vec<u8>,
}

impl Amx {
    pub(super) fn parse(enc: &[u8]) -> Result<Self, RomError> {
        let length = u32_at(enc, 0)? as usize;
        if u32_at(enc, 4)? != MAGIC {
            return Err(RomError::Malformed("not an AMX script".into()));
        }
        let start = u32_at(enc, 0xC)? as usize;
        let end = u32_at(enc, 0x14)? as usize;
        let compressed = enc
            .get(start..length)
            .ok_or_else(|| RomError::Malformed("AMX script cut off".into()))?;
        let header = enc
            .get(4..start)
            .ok_or_else(|| RomError::Malformed("AMX header cut off".into()))?
            .to_vec();
        let data = decompress(compressed, end.saturating_sub(start))?;
        Ok(Self { header, data })
    }

    /// The script, compressed again.
    pub(super) fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; 4];
        out.extend_from_slice(&self.header);
        for &cell in self.data.as_chunks::<4>().0 {
            out.extend(compress(u32::from_le_bytes(cell)));
        }
        let len = u32::try_from(out.len()).unwrap_or(u32::MAX);
        out[..4].copy_from_slice(&len.to_le_bytes());
        out
    }
}

fn decompress(data: &[u8], length: usize) -> Result<Vec<u8>, RomError> {
    let mut code = Vec::with_capacity(length);
    let mut first = true;
    let mut x: i32 = 0;
    let mut bytes = data.iter();
    while code.len() < length {
        let b = *bytes
            .next()
            .ok_or_else(|| RomError::Malformed("AMX script data cut off".into()))?;
        let v = i32::from(b & 0x7F);
        if first {
            // The first byte's bit 6 is the sign.
            x = if v & 0x40 != 0 { v - 0x80 } else { v };
            first = false;
        } else {
            x = (x << 7) | v;
        }
        if b & 0x80 != 0 {
            continue;
        }
        code.extend_from_slice(&x.to_le_bytes());
        first = true;
    }
    code.truncate(length);
    Ok(code)
}

fn compress(cell: u32) -> Vec<u8> {
    let negative = cell & 0x8000_0000 != 0;
    let mut value = cell;
    let mut shadow = if negative { !cell } else { cell };
    let mut bytes = Vec::new();
    loop {
        let mut b = u8::try_from(value & 0x7F).unwrap_or_default();
        if !bytes.is_empty() {
            b |= 0x80;
        }
        bytes.push(b);
        value >>= 7;
        shadow >>= 7;
        if shadow == 0 {
            break;
        }
    }
    if bytes.len() < 5 {
        let sign = if negative { 0x40 } else { 0 };
        if bytes.last().is_some_and(|&b| b & 0x40 != sign) {
            bytes.push(if negative { 0xFF } else { 0x80 });
        }
    }
    bytes.reverse();
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_survive_a_round_trip() {
        for cell in [
            0u32,
            1,
            0x3F,
            0x40,
            0x7F,
            0x80,
            0x1234,
            0xFFFF_FFFF,
            0x8000_0000,
            0xFFFF_FF80,
            0x7FFF_FFFF,
        ] {
            let c = compress(cell);
            let d = decompress(&c, 4).expect("decompresses");
            assert_eq!(d, cell.to_le_bytes(), "{cell:#x}: {c:02X?}");
        }
    }

    #[test]
    fn scripts_survive_a_round_trip() {
        let cells: Vec<u8> = [5u32, 0xFFFF_FFFE, 300, 0x1234_5678]
            .iter()
            .flat_map(|c| c.to_le_bytes())
            .collect();
        let packed: Vec<u8> = cells
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|&c| compress(u32::from_le_bytes(c)))
            .collect();
        let start = 0x1C_u32;
        let mut enc = Vec::new();
        enc.extend_from_slice(&(start + u32::try_from(packed.len()).expect("small")).to_le_bytes());
        enc.extend_from_slice(&MAGIC.to_le_bytes());
        enc.extend_from_slice(&[0; 4]);
        enc.extend_from_slice(&start.to_le_bytes());
        enc.extend_from_slice(&[0; 4]);
        enc.extend_from_slice(&(start + 16).to_le_bytes());
        enc.extend_from_slice(&[0; 4]);
        enc.extend_from_slice(&packed);
        let amx = Amx::parse(&enc).expect("parses");
        assert_eq!(amx.data, cells);
        assert_eq!(amx.to_bytes(), enc);
    }
}
