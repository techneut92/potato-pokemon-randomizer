//! CRC-32 (IEEE), used for settings strings and ROM checks, and the CRC-16 of the DS header.

const TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        #[expect(clippy::cast_possible_truncation, reason = "i < 256")]
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 == 1 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            k += 1;
        }
        table[i] = c;
        i += 1;
    }
    table
};

/// The CRC-32 of `bytes`.
#[must_use]
pub fn crc32(bytes: &[u8]) -> u32 {
    !update(!0, bytes)
}

fn update(crc: u32, bytes: &[u8]) -> u32 {
    bytes
        .iter()
        .fold(crc, |crc, &b| TABLE[usize::from(crc.to_le_bytes()[0] ^ b)] ^ (crc >> 8))
}

/// The CRC-32 of everything `reader` gives, read in pieces, for files too large to hold in memory.
///
/// # Errors
/// When reading fails.
pub fn crc32_of(reader: &mut impl std::io::Read) -> std::io::Result<u32> {
    let mut crc = !0;
    let mut buf = vec![0u8; 1 << 16];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => return Ok(!crc),
            Ok(n) => crc = update(crc, &buf[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) => return Err(e),
        }
    }
}

/// CRC-16/MODBUS (reflected polynomial 0xA001, start value 0xFFFF), as used by the DS cartridge header.
#[must_use]
pub fn crc16(bytes: &[u8]) -> u16 {
    bytes.iter().fold(0xFFFF_u16, |crc, &b| {
        (0..8).fold(
            crc ^ u16::from(b),
            |c, _| if c & 1 == 1 { (c >> 1) ^ 0xA001 } else { c >> 1 },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_matches_the_check_value() {
        assert_eq!(crc16(b"123456789"), 0x4B37);
    }

    #[test]
    fn matches_the_check_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn reading_in_pieces_gives_the_same_crc() {
        let data: Vec<u8> = (0..200_000u32).map(|i| (i * 7 % 251) as u8).collect();
        assert_eq!(crc32_of(&mut data.as_slice()).expect("reads"), crc32(&data));
        assert_eq!(crc32_of(&mut b"123456789".as_slice()).expect("reads"), 0xCBF4_3926);
    }
}
