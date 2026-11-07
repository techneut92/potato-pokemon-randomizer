//! Nintendo 3DS (CTR) images: CCI (`.3ds`), CXI and CIA, their file systems and the archive formats inside them.

mod game;
pub mod garc;
mod layered_fs;
pub mod mini;
mod ncch;
mod romfs;
pub mod smdh;
#[cfg(any(test, feature = "testing"))]
pub mod testing;

use std::fmt::Write as _;

use crate::io::{ascii_field, read_at};
use crate::{FormatError, RomSource};

pub use game::Game;
pub use garc::{Decompression, Garc};
pub use ncch::Ncch;
pub use smdh::Smdh;

const MAGIC_OFFSET: u64 = 0x100;
const NCCH_MAGIC: [u8; 4] = *b"NCCH";
const NCSD_MAGIC: [u8; 4] = *b"NCSD";
const CIA_HEADER_SIZE: u32 = 0x2020;
const CCI_FIRST_PARTITION: u64 = 0x4000;
const PROGRAM_ID: u64 = 0x118;
const PRODUCT_CODE: u64 = 0x150;

/// How the executable (CXI) is wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// A bare NCCH executable.
    Cxi,
    /// A cartridge image; the executable is the first partition.
    Cci,
    /// An installable archive; the executable is the first content.
    Cia,
}

/// The fields of the NCCH header that identify a game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtrHeader {
    /// Wrapper around the executable.
    pub container: Container,
    /// Offset of the executable's NCCH header in the file.
    pub ncch_offset: u64,
    /// Product code, e.g. `CTR-P-BNDA` for Sun.
    pub product_code: String,
    /// Title ID as 16 upper-case hex digits, e.g. `0004000000164800`.
    pub title_id: String,
}

impl CtrHeader {
    /// Finds the executable (the CXI) and reads its identity.
    pub fn read(src: &mut (impl RomSource + ?Sized)) -> Result<Self, FormatError> {
        let (container, ncch_offset) = locate_cxi(src)?;
        let product_code = ascii_field(&read_at::<0x10>(src, ncch_offset + PRODUCT_CODE)?);
        let mut program_id = read_at::<8>(src, ncch_offset + PROGRAM_ID)?;
        program_id.reverse();
        let title_id = program_id.iter().fold(String::with_capacity(16), |mut hex, b| {
            let _ = write!(hex, "{b:02X}");
            hex
        });
        Ok(Self {
            container,
            ncch_offset,
            product_code,
            title_id,
        })
    }
}

/// The title ID of the updates of the game with `title_id`: the same, but with `E` as its eighth digit (the
/// update title type), e.g. `0004000E00055D00` for `0004000000055D00`.
#[must_use]
pub fn update_title_id(title_id: &str) -> String {
    title_id
        .char_indices()
        .map(|(i, c)| if i == 7 { 'E' } else { c })
        .collect()
}

fn align64(n: u64) -> u64 {
    n.div_ceil(64) * 64
}

/// The offset of a CIA's title metadata, which follows the header, the certificate chain and the ticket.
fn cia_tmd_offset(src: &mut (impl RomSource + ?Sized)) -> Result<u64, FormatError> {
    let le32 = |b: [u8; 4]| u64::from(u32::from_le_bytes(b));
    let cert = le32(read_at(src, 0x08)?);
    let ticket = le32(read_at(src, 0x0C)?);
    let cert_offset = align64(u64::from(CIA_HEADER_SIZE));
    let ticket_offset = align64(cert_offset + cert);
    Ok(align64(ticket_offset + ticket))
}

/// The title version in a CIA's title metadata, e.g. 5232 for version 1.5 of X; 0 when the metadata has a
/// signature type this does not know, as its header cannot be found then.
pub(crate) fn cia_title_version(src: &mut (impl RomSource + ?Sized)) -> Result<u16, FormatError> {
    let tmd = cia_tmd_offset(src)?;
    // The signature (and its padding) before the header has a length that depends on its type.
    let signature = match u32::from_be_bytes(read_at(src, tmd)?) {
        0x0001_0003 => 0x200 + 0x3C,
        0x0001_0004 => 0x100 + 0x3C,
        0x0001_0005 => 0x3C + 0x40,
        _ => return Ok(0),
    };
    Ok(u16::from_be_bytes(read_at(src, tmd + 4 + signature + 0x9C)?))
}

fn locate_cxi(src: &mut (impl RomSource + ?Sized)) -> Result<(Container, u64), FormatError> {
    let le32 = |b: [u8; 4]| u64::from(u32::from_le_bytes(b));
    let header_size = read_at::<4>(src, 0).map(u32::from_le_bytes)?;
    if header_size == CIA_HEADER_SIZE {
        let tmd = le32(read_at(src, 0x10)?);
        let content_offset = align64(cia_tmd_offset(src)? + tmd);
        if read_at::<4>(src, content_offset + MAGIC_OFFSET).ok() == Some(NCCH_MAGIC) {
            return Ok((Container::Cia, content_offset));
        }
    }
    // A CXI or CCI may start with bytes that look like a CIA header, so this is not an `else`.
    match read_at::<4>(src, MAGIC_OFFSET)? {
        NCCH_MAGIC => Ok((Container::Cxi, 0)),
        NCSD_MAGIC => Ok((Container::Cci, CCI_FIRST_PARTITION)),
        _ => Err(FormatError::WrongFormat {
            format: "3DS",
            reason: "no NCCH, NCSD or CIA header",
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    fn ncch(product: &str, title_id: u64) -> Vec<u8> {
        let mut n = vec![0u8; 0x200];
        n[0x100..0x104].copy_from_slice(&NCCH_MAGIC);
        n[0x118..0x120].copy_from_slice(&title_id.to_le_bytes());
        n[0x150..0x150 + product.len()].copy_from_slice(product.as_bytes());
        n
    }

    #[test]
    fn reads_bare_cxi() {
        let h = CtrHeader::read(&mut Cursor::new(ncch("CTR-P-BNDA", 0x0004_0000_0016_4800))).expect("cxi");
        assert_eq!(h.container, Container::Cxi);
        assert_eq!(h.product_code, "CTR-P-BNDA");
        assert_eq!(h.title_id, "0004000000164800");
    }

    #[test]
    fn reads_cci_first_partition() {
        let mut img = vec![0u8; 0x4000];
        img[0x100..0x104].copy_from_slice(&NCSD_MAGIC);
        img.extend(ncch("CTR-P-A2BA", 0x0004_0000_001B_5100));
        let h = CtrHeader::read(&mut Cursor::new(img)).expect("cci");
        assert_eq!((h.container, h.ncch_offset), (Container::Cci, 0x4000));
        assert_eq!(h.title_id, "00040000001B5100");
    }

    #[test]
    fn reads_cia_content() {
        let (cert, ticket, tmd) = (0xA00_u32, 0x350_u32, 0xB34_u32);
        let mut img = vec![0u8; 0x2020];
        img[0..4].copy_from_slice(&CIA_HEADER_SIZE.to_le_bytes());
        img[0x08..0x0C].copy_from_slice(&cert.to_le_bytes());
        img[0x0C..0x10].copy_from_slice(&ticket.to_le_bytes());
        img[0x10..0x14].copy_from_slice(&tmd.to_le_bytes());
        let content = align64(align64(align64(align64(0x2020) + u64::from(cert)) + u64::from(ticket)) + u64::from(tmd));
        img.resize(usize::try_from(content).expect("small"), 0);
        img.extend(ncch("CTR-P-EKJA", 0x0004_0000_0005_5D00));
        let h = CtrHeader::read(&mut Cursor::new(img)).expect("cia");
        assert_eq!((h.container, h.ncch_offset), (Container::Cia, content));
        assert_eq!(h.product_code, "CTR-P-EKJA");
    }

    #[test]
    fn reads_the_title_version_of_a_cia() {
        let (cert, ticket) = (0xA00_u32, 0x350_u32);
        let mut img = vec![0u8; 0x2020];
        img[0..4].copy_from_slice(&CIA_HEADER_SIZE.to_le_bytes());
        img[0x08..0x0C].copy_from_slice(&cert.to_le_bytes());
        img[0x0C..0x10].copy_from_slice(&ticket.to_le_bytes());
        let tmd =
            usize::try_from(align64(align64(align64(0x2020) + u64::from(cert)) + u64::from(ticket))).expect("small");
        img.resize(tmd + 0x400, 0);
        // RSA-2048 with SHA-256: a 0x100-byte signature and 0x3C bytes of padding.
        img[tmd..tmd + 4].copy_from_slice(&0x0001_0004_u32.to_be_bytes());
        let version = tmd + 4 + 0x100 + 0x3C + 0x9C;
        img[version..version + 2].copy_from_slice(&5232_u16.to_be_bytes());
        assert_eq!(cia_title_version(&mut Cursor::new(&img)).expect("reads"), 5232);
        img[tmd..tmd + 4].copy_from_slice(&0x0001_0009_u32.to_be_bytes());
        assert_eq!(
            cia_title_version(&mut Cursor::new(&img)).expect("reads"),
            0,
            "unknown signature type"
        );
    }

    #[test]
    fn update_title_ids_have_the_update_type() {
        assert_eq!(update_title_id("0004000000055D00"), "0004000E00055D00");
        assert_eq!(update_title_id("000400000011C400"), "0004000E0011C400");
    }

    #[test]
    fn rejects_other_files() {
        assert!(CtrHeader::read(&mut Cursor::new(vec![0u8; 0x400])).is_err());
    }
}
