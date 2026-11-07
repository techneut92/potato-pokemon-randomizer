//! The game partition of a 3DS image (an NCCH executable, "CXI"): its ExeFS with the executable (`.code`) and the
//! icon, and its RomFS with the game files.
//!
//! [`Ncch::open`] finds the executable in a CCI, CIA or bare CXI and reads only headers and tables; file contents are
//! read when asked for, so multi-gigabyte images are never loaded whole. Changes (a new executable, replaced RomFS
//! files, an edited icon) are kept in memory until they are written in one of two ways:
//!
//! * [`Ncch::write_cxi`] writes a complete decrypted CXI, streaming everything that did not change from the source.
//!   Without changes it is a byte-for-byte copy of the game partition. With changes, the signature is zeroed (it no
//!   longer matches), the crypto flags are set to "no crypto", the ExeFS is rebuilt if the executable or icon
//!   changed (the executable compressed again if it was stored compressed), the RomFS is rebuilt as described in
//!   its module, and every hash and size in the header is updated.
//! * [`Ncch::write_layered_fs`] writes only what changed, as a folder for a mod loader: `<title id>/code.bin` (the
//!   decompressed executable, if it changed) and `<title id>/romfs/<path>` for every replaced file.
//!
//! A game with an update is read through [`Game`](super::Game), which lays the update's partition over this one.
//!
//! Deliberate choices for the CXI:
//!
//! * The extended header, logo and plain region stay where they were (they must precede the ExeFS), and the RomFS
//!   keeps its offset unless a grown ExeFS needs the room; it then starts at the next 4 KiB boundary.
//! * ExeFS files are laid out in their original order, the executable first, each at a 512-byte boundary, so a
//!   grown executable never overlaps the files after it.
//! * The content size in the header is updated to the new length.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::romfs::{Rebuilt, RomFs};
use super::smdh::Smdh;
use super::{Container, CtrHeader, cia_title_version, layered_fs};
use crate::io::{align_up, copy_range, read_at, read_vec_at, write_zeros};
use crate::{FormatError, RomSource, blz};

const FORMAT: &str = "NCCH";
const MEDIA_UNIT: u64 = 0x200;
const HEADER_LEN: usize = 0x200;
const EXEFS_HEADER_LEN: u64 = EXEFS_HEADER_BYTES as u64;
const EXEFS_HEADER_BYTES: usize = 0x200;
const MEDIA_UNIT_BYTES: usize = 0x200;
const EXEFS_SLOTS: usize = 10;
const ROMFS_ALIGN: u64 = 0x1000;
/// Bit of flag byte 7 that marks an image as not encrypted.
const NO_CRYPTO: u8 = 0x04;
const FLAGS: usize = 0x188;
/// System control info flags in the extended header; bit 0 marks a compressed executable.
const SCI_FLAGS: u64 = 0x20D;
const CODE: &str = ".code";
const ICON: &str = "icon";
/// Largest ExeFS file accepted; real executables are a few MiB.
const MAX_EXEFS_FILE: u64 = 64 << 20;

fn malformed(reason: &'static str) -> FormatError {
    FormatError::WrongFormat { format: FORMAT, reason }
}

fn unencodable(reason: &'static str) -> FormatError {
    FormatError::Unencodable { format: FORMAT, reason }
}

fn le32(h: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([h[at], h[at + 1], h[at + 2], h[at + 3]])
}

/// A header field in media units, as bytes.
fn units(h: &[u8], at: usize) -> u64 {
    u64::from(le32(h, at)) * MEDIA_UNIT
}

fn set_units(h: &mut [u8], at: usize, bytes: u64) -> Result<(), FormatError> {
    let n = u32::try_from(bytes / MEDIA_UNIT).map_err(|_| unencodable("image too large"))?;
    h[at..at + 4].copy_from_slice(&n.to_le_bytes());
    Ok(())
}

fn to_usize(n: u64) -> Result<usize, FormatError> {
    usize::try_from(n).map_err(|_| malformed("size out of range"))
}

#[derive(Debug, Clone)]
struct ExefsEntry {
    name: String,
    raw_name: [u8; 8],
    offset: u64,
    size: u64,
}

/// The game partition of a 3DS image, with pending changes.
#[derive(Debug)]
pub struct Ncch<R> {
    src: R,
    identity: CtrHeader,
    /// The title version from a CIA's metadata; 0 for other containers.
    version: u16,
    header: [u8; HEADER_LEN],
    exefs_offset: u64,
    exefs_size: u64,
    exefs: Vec<ExefsEntry>,
    code_compressed: bool,
    original_code: Option<Vec<u8>>,
    new_code: Option<Vec<u8>>,
    /// The icon as it is now and as it was.
    icon: Option<(Smdh, Vec<u8>)>,
    romfs_offset: u64,
    romfs_size: u64,
    romfs: RomFs,
    /// Replaced RomFS files: file index → contents.
    changes: BTreeMap<usize, Vec<u8>>,
}

impl<R: RomSource> Ncch<R> {
    /// Opens the game partition of a CCI, CIA or CXI and reads its file system tables.
    ///
    /// # Errors
    /// [`FormatError::Encrypted`] when the image is not marked as decrypted and its file systems cannot be read;
    /// other errors when it is no 3DS image or its structures are broken.
    pub fn open(mut src: R) -> Result<Self, FormatError> {
        let identity = CtrHeader::read(&mut src)?;
        let version = match identity.container {
            Container::Cia => cia_title_version(&mut src)?,
            Container::Cci | Container::Cxi => 0,
        };
        let base = identity.ncch_offset;
        let header = read_at::<HEADER_LEN>(&mut src, base)?;
        let marked_decrypted = header[FLAGS + 7] & NO_CRYPTO != 0;
        match Self::read_file_systems(&mut src, base, &header) {
            Ok(parts) => Ok(Self {
                src,
                identity,
                version,
                header,
                exefs_offset: parts.exefs_offset,
                exefs_size: parts.exefs_size,
                exefs: parts.exefs,
                code_compressed: parts.code_compressed,
                original_code: None,
                new_code: None,
                icon: parts.icon.map(|bytes| (Smdh::new(bytes.clone()), bytes)),
                romfs_offset: parts.romfs_offset,
                romfs_size: parts.romfs_size,
                romfs: parts.romfs,
                changes: BTreeMap::new(),
            }),
            Err(FormatError::Io(e)) => Err(FormatError::Io(e)),
            // Garbage where the tables should be: the image is most likely still encrypted.
            Err(_) if !marked_decrypted => Err(FormatError::Encrypted),
            Err(e) => Err(e),
        }
    }

    fn read_file_systems(src: &mut R, base: u64, h: &[u8; HEADER_LEN]) -> Result<Parts, FormatError> {
        let exefs_offset = units(h, 0x1A0);
        let exefs_size = units(h, 0x1A4);
        let romfs_offset = units(h, 0x1B0);
        let romfs_size = units(h, 0x1B4);
        if exefs_size < EXEFS_HEADER_LEN {
            return Err(malformed("no ExeFS"));
        }
        if romfs_size == 0 {
            return Err(malformed("no RomFS"));
        }
        let code_compressed = read_at::<1>(src, base + SCI_FLAGS)?[0] & 1 != 0;
        let table = read_at::<EXEFS_HEADER_BYTES>(src, base + exefs_offset)?;
        let mut exefs = Vec::new();
        for slot in table.as_chunks::<0x10>().0.iter().take(EXEFS_SLOTS) {
            let mut raw_name = [0u8; 8];
            raw_name.copy_from_slice(&slot[..8]);
            let name = String::from_utf8_lossy(&raw_name)
                .trim_matches(|c: char| c <= ' ')
                .to_owned();
            let offset = u64::from(le32(slot, 8));
            let size = u64::from(le32(slot, 12));
            if name.is_empty() || size == 0 {
                continue;
            }
            if size > MAX_EXEFS_FILE || EXEFS_HEADER_LEN + offset + size > exefs_size {
                return Err(malformed("ExeFS file out of bounds"));
            }
            exefs.push(ExefsEntry {
                name,
                raw_name,
                offset,
                size,
            });
        }
        let icon = match exefs.iter().find(|e| e.name == ICON) {
            Some(e) => Some(read_vec_at(
                src,
                base + exefs_offset + EXEFS_HEADER_LEN + e.offset,
                to_usize(e.size)?,
            )?),
            None => None,
        };
        let romfs = RomFs::parse(src, base + romfs_offset, romfs_size)?;
        Ok(Parts {
            exefs_offset,
            exefs_size,
            exefs,
            code_compressed,
            icon,
            romfs_offset,
            romfs_size,
            romfs,
        })
    }

    /// Where the game partition is and what it is.
    pub fn identity(&self) -> &CtrHeader {
        &self.identity
    }

    /// The title version a CIA's metadata gives, e.g. 5232 for version 1.5 of X; 0 for a CCI or CXI, which carry
    /// none.
    pub fn title_version(&self) -> u16 {
        self.version
    }

    /// Whether the header marks the image as not encrypted. Some badly decrypted dumps lack the mark but open
    /// fine; callers may accept those on other evidence.
    pub fn is_marked_decrypted(&self) -> bool {
        self.header[FLAGS + 7] & NO_CRYPTO != 0
    }

    /// Whether the executable is stored compressed.
    pub fn is_code_compressed(&self) -> bool {
        self.code_compressed
    }

    fn base(&self) -> u64 {
        self.identity.ncch_offset
    }

    fn read_exefs_file(&mut self, entry: &ExefsEntry) -> Result<Vec<u8>, FormatError> {
        let at = self.base() + self.exefs_offset + EXEFS_HEADER_LEN + entry.offset;
        read_vec_at(&mut self.src, at, to_usize(entry.size)?)
    }

    fn load_code(&mut self) -> Result<(), FormatError> {
        if self.original_code.is_some() {
            return Ok(());
        }
        let entry = self
            .exefs
            .iter()
            .find(|e| e.name == CODE)
            .cloned()
            .ok_or_else(|| FormatError::NotFound { what: CODE.into() })?;
        let raw = self.read_exefs_file(&entry)?;
        self.original_code = Some(if self.code_compressed {
            blz::decompress(&raw)?
        } else {
            raw
        });
        Ok(())
    }

    /// The executable (`.code`), decompressed, with the changes made through [`set_code`](Self::set_code).
    ///
    /// # Errors
    /// When there is no executable or it does not decompress.
    pub fn code(&mut self) -> Result<&[u8], FormatError> {
        self.load_code()?;
        Ok(self
            .new_code
            .as_deref()
            .or(self.original_code.as_deref())
            .unwrap_or_default())
    }

    /// Replaces the executable (decompressed). Setting the original contents undoes the change.
    ///
    /// # Errors
    /// When there is no executable to replace.
    pub fn set_code(&mut self, code: Vec<u8>) -> Result<(), FormatError> {
        self.load_code()?;
        self.new_code = (self.original_code.as_ref() != Some(&code)).then_some(code);
        Ok(())
    }

    /// The paths of all RomFS files, sorted, e.g. `a/0/1/6` or `DllBattle.cro`.
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        self.romfs.index.keys().map(String::as_str)
    }

    /// Whether the RomFS has a file at `path`.
    pub fn has_file(&self, path: &str) -> bool {
        self.romfs.index.contains_key(path)
    }

    fn file_index(&self, path: &str) -> Result<usize, FormatError> {
        self.romfs
            .index
            .get(path)
            .copied()
            .ok_or_else(|| FormatError::NotFound {
                what: format!("RomFS file {path}"),
            })
    }

    /// Length of the RomFS file at `path`, with changes.
    pub fn file_len(&self, path: &str) -> Option<u64> {
        let i = *self.romfs.index.get(path)?;
        Some(self.changes.get(&i).map_or(self.romfs.files[i].len, |d| d.len() as u64))
    }

    fn read_original(&mut self, i: usize) -> Result<Vec<u8>, FormatError> {
        let entry = &self.romfs.files[i];
        let at = self.base() + self.romfs_offset + self.romfs.data_offset(entry);
        let len = to_usize(entry.len)?;
        read_vec_at(&mut self.src, at, len)
    }

    /// The RomFS file at `path`, with changes.
    ///
    /// # Errors
    /// When there is no such file or it cannot be read.
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, FormatError> {
        let i = self.file_index(path)?;
        match self.changes.get(&i) {
            Some(data) => Ok(data.clone()),
            None => self.read_original(i),
        }
    }

    /// Replaces the RomFS file at `path`. Replacing a file with its original contents undoes the change.
    ///
    /// # Errors
    /// When there is no such file or the original cannot be read for the comparison.
    pub fn replace_file(&mut self, path: &str, data: Vec<u8>) -> Result<(), FormatError> {
        let i = self.file_index(path)?;
        if data.len() as u64 == self.romfs.files[i].len && self.read_original(i)? == data {
            self.changes.remove(&i);
        } else {
            self.changes.insert(i, data);
        }
        Ok(())
    }

    /// The RomFS files that were replaced, with their new contents, by path.
    pub fn changed_files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.changes
            .iter()
            .map(|(&i, data)| (self.romfs.files[i].path.as_str(), data.as_slice()))
    }

    /// Whether the executable was replaced.
    pub fn is_code_changed(&self) -> bool {
        self.new_code.is_some()
    }

    /// The icon file, if the ExeFS has one.
    pub fn smdh(&self) -> Option<&Smdh> {
        self.icon.as_ref().map(|(smdh, _)| smdh)
    }

    /// The icon file for editing; the edit is written by [`write_cxi`](Self::write_cxi).
    pub fn smdh_mut(&mut self) -> Option<&mut Smdh> {
        self.icon.as_mut().map(|(smdh, _)| smdh)
    }

    fn is_icon_changed(&self) -> bool {
        self.icon
            .as_ref()
            .is_some_and(|(smdh, original)| smdh.as_bytes() != original.as_slice())
    }

    /// Whether anything differs from the source.
    pub fn is_modified(&self) -> bool {
        self.new_code.is_some() || !self.changes.is_empty() || self.is_icon_changed()
    }

    /// Writes the changed executable and RomFS files below `dir/<title id>/`, which is emptied first:
    /// `code.bin` and `romfs/<path>`. Returns the `<title id>` folder.
    ///
    /// # Errors
    /// When writing fails or a changed file has a path that cannot be a relative file path.
    pub fn write_layered_fs(&self, dir: &Path) -> Result<PathBuf, FormatError> {
        layered_fs::write(
            dir,
            &self.identity.title_id,
            self.new_code.as_deref(),
            self.changed_files(),
        )
    }

    /// Builds the ExeFS: the executable first, then the other files in their original order.
    fn build_exefs(&mut self) -> Result<Vec<u8>, FormatError> {
        let mut entries = self.exefs.clone();
        entries.sort_by_key(|e| e.name != CODE);
        let mut out = vec![0u8; to_usize(EXEFS_HEADER_LEN)?];
        let mut offset = 0usize;
        for (slot, entry) in entries.iter().enumerate() {
            let data = match entry.name.as_str() {
                CODE if self.new_code.is_some() => {
                    let code = self.new_code.as_deref().unwrap_or_default();
                    if self.code_compressed {
                        blz::compress(code, 0, blz::Mode::Best)
                    } else {
                        code.to_vec()
                    }
                }
                ICON if self.icon.is_some() => self.smdh().map(|s| s.as_bytes().to_vec()).unwrap_or_default(),
                _ => self.read_exefs_file(entry)?,
            };
            let at = slot * 0x10;
            out[at..at + 8].copy_from_slice(&entry.raw_name);
            let offset32 = u32::try_from(offset).map_err(|_| unencodable("ExeFS too large"))?;
            let size32 = u32::try_from(data.len()).map_err(|_| unencodable("ExeFS too large"))?;
            out[at + 8..at + 12].copy_from_slice(&offset32.to_le_bytes());
            out[at + 12..at + 16].copy_from_slice(&size32.to_le_bytes());
            let hash_at = 0x200 - (slot + 1) * 0x20;
            out[hash_at..hash_at + 0x20].copy_from_slice(&Sha256::digest(&data));
            out.resize(0x200 + offset, 0);
            out.extend_from_slice(&data);
            offset = (offset + data.len()).next_multiple_of(MEDIA_UNIT_BYTES);
        }
        out.resize(out.len().next_multiple_of(MEDIA_UNIT_BYTES), 0);
        Ok(out)
    }

    /// Writes the game partition as a decrypted CXI with all changes, and returns its length.
    ///
    /// # Errors
    /// When reading or writing fails, or the partition is laid out in a way this cannot rebuild (a logo or plain
    /// region after the ExeFS).
    pub fn write_cxi(&mut self, out: &mut (impl Write + ?Sized)) -> Result<u64, FormatError> {
        let base = self.base();
        if !self.is_modified() {
            let len = units(&self.header, 0x104);
            copy_range(&mut self.src, base, len, out)?;
            return Ok(len);
        }
        let h0 = self.header;
        for (offset, size) in [(0x190, 0x194), (0x198, 0x19C)] {
            if units(&h0, size) > 0 && units(&h0, offset) + units(&h0, size) > self.exefs_offset {
                return Err(unencodable("logo or plain region after the ExeFS"));
            }
        }
        if self.exefs_offset < HEADER_LEN as u64 || self.exefs_offset + self.exefs_size > self.romfs_offset {
            return Err(unencodable("ExeFS not between the header and the RomFS"));
        }

        let exefs = if self.new_code.is_some() || self.is_icon_changed() {
            Some(self.build_exefs()?)
        } else {
            None
        };
        let exefs_size = exefs.as_ref().map_or(self.exefs_size, |e| e.len() as u64);
        let exefs_end = self.exefs_offset + exefs_size;
        let romfs_offset = if exefs_end <= self.romfs_offset {
            self.romfs_offset
        } else {
            align_up(exefs_end, ROMFS_ALIGN)
        };
        let romfs: Option<Rebuilt<'_>> = if self.changes.is_empty() {
            None
        } else {
            Some(
                self.romfs
                    .rebuild(&mut self.src, base + self.romfs_offset, &self.changes)?,
            )
        };
        let romfs_size = romfs.as_ref().map_or(self.romfs_size, Rebuilt::len);

        let mut h = h0;
        h[..0x100].fill(0);
        h[FLAGS + 3] = 0;
        h[FLAGS + 7] = NO_CRYPTO;
        set_units(&mut h, 0x104, romfs_offset + romfs_size)?;
        set_units(&mut h, 0x1A4, exefs_size)?;
        set_units(&mut h, 0x1B0, romfs_offset)?;
        set_units(&mut h, 0x1B4, romfs_size)?;
        if let Some(exefs) = &exefs {
            let hashed = to_usize(units(&h0, 0x1A8))?.min(exefs.len());
            h[0x1C0..0x1E0].copy_from_slice(&Sha256::digest(&exefs[..hashed]));
        }
        if let Some(romfs) = &romfs {
            let hashed = units(&h0, 0x1B8).max(align_up(romfs.hashed_len(), MEDIA_UNIT));
            set_units(&mut h, 0x1B8, hashed)?;
            let head = romfs.head(&mut self.src, to_usize(hashed)?)?;
            h[0x1E0..0x200].copy_from_slice(&Sha256::digest(&head));
        }

        out.write_all(&h)?;
        copy_range(
            &mut self.src,
            base + HEADER_LEN as u64,
            self.exefs_offset - HEADER_LEN as u64,
            out,
        )?;
        match &exefs {
            Some(exefs) => {
                out.write_all(exefs)?;
                write_zeros(out, romfs_offset - exefs_end)?;
            }
            None => copy_range(
                &mut self.src,
                base + self.exefs_offset,
                self.romfs_offset - self.exefs_offset,
                out,
            )?,
        }
        match &romfs {
            Some(romfs) => romfs.write(&mut self.src, out)?,
            None => copy_range(&mut self.src, base + self.romfs_offset, self.romfs_size, out)?,
        }
        Ok(romfs_offset + romfs_size)
    }

    /// Checks every hash of the source image: the ExeFS file hashes, both superblock hashes in the header and the
    /// whole RomFS hash tree. This reads the entire RomFS.
    ///
    /// # Errors
    /// When the image cannot be read.
    pub fn verify(&mut self) -> Result<bool, FormatError> {
        let base = self.base();
        let exefs_at = base + self.exefs_offset;
        let table = read_vec_at(&mut self.src, exefs_at, to_usize(EXEFS_HEADER_LEN)?)?;
        for (slot, entry) in self.exefs.clone().iter().enumerate() {
            let data = self.read_exefs_file(entry)?;
            let hash_at = 0x200 - (slot + 1) * 0x20;
            if table[hash_at..hash_at + 0x20] != Sha256::digest(&data)[..] {
                return Ok(false);
            }
        }
        let regions = [(exefs_at, 0x1A8, 0x1C0), (base + self.romfs_offset, 0x1B8, 0x1E0)];
        for (at, size, hash) in regions {
            let data = read_vec_at(&mut self.src, at, to_usize(units(&self.header, size))?)?;
            if self.header[hash..hash + 0x20] != Sha256::digest(&data)[..] {
                return Ok(false);
            }
        }
        self.romfs.verify(&mut self.src, base + self.romfs_offset)
    }

    /// Gives back the source.
    pub fn into_inner(self) -> R {
        self.src
    }
}

struct Parts {
    exefs_offset: u64,
    exefs_size: u64,
    exefs: Vec<ExefsEntry>,
    code_compressed: bool,
    icon: Option<Vec<u8>>,
    romfs_offset: u64,
    romfs_size: u64,
    romfs: RomFs,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;

    use super::*;
    use crate::ctr::testing::{Cxi, cci, cxi};

    fn code() -> Vec<u8> {
        (0..20_000u32)
            .map(|i| [0xE5, 0x9F, (i / 32 % 200) as u8, 0][(i % 4) as usize])
            .collect()
    }

    const FILES: &[(&str, &[u8])] = &[
        ("a/0/0/0", b"first file"),
        ("a/0/1/6", &[3; 5000]),
        ("DllBattle.cro", b"battle code"),
    ];

    fn spec(code: &[u8], compress_code: bool, decrypted: bool) -> Cxi<'_> {
        Cxi {
            product_code: "CTR-P-EKJA",
            title_id: 0x0004_0000_0005_5D00,
            code,
            compress_code,
            files: FILES,
            decrypted,
        }
    }

    fn open(img: &[u8]) -> Ncch<Cursor<&[u8]>> {
        Ncch::open(Cursor::new(img)).expect("opens")
    }

    fn save(ncch: &mut Ncch<Cursor<&[u8]>>) -> Vec<u8> {
        let mut out = Vec::new();
        let len = ncch.write_cxi(&mut out).expect("writes");
        assert_eq!(len, out.len() as u64);
        out
    }

    #[test]
    fn opens_a_cci_and_reads_code_and_files() {
        let code = code();
        let img = cci(&cxi(&spec(&code, true, true)));
        let mut ncch = open(&img);
        assert_eq!(ncch.identity().title_id, "0004000000055D00");
        assert!(ncch.is_code_compressed());
        assert_eq!(ncch.code().expect("code"), code);
        assert_eq!(
            ncch.file_names().collect::<Vec<_>>(),
            ["DllBattle.cro", "a/0/0/0", "a/0/1/6"]
        );
        assert_eq!(ncch.read_file("a/0/1/6").expect("exists"), [3; 5000]);
        assert!(ncch.read_file("a/9/9/9").is_err());
        assert!(ncch.verify().expect("reads"));
    }

    #[test]
    fn unchanged_save_copies_the_partition() {
        let code = code();
        let part = cxi(&spec(&code, true, true));
        let img = cci(&part);
        let mut ncch = open(&img);
        // Writing back what was read is no change.
        let same = ncch.read_file("a/0/0/0").expect("exists");
        ncch.replace_file("a/0/0/0", same).expect("exists");
        ncch.set_code(code.clone()).expect("has code");
        assert!(!ncch.is_modified());
        assert_eq!(save(&mut ncch), part);
    }

    #[test]
    fn saves_replaced_files_and_code() {
        let code = code();
        let img = cxi(&spec(&code, true, true));
        let mut ncch = open(&img);
        let mut new_code = code.clone();
        new_code.extend_from_slice(&[0x11; 3000]);
        ncch.set_code(new_code.clone()).expect("has code");
        ncch.replace_file("a/0/0/0", b"changed".to_vec()).expect("exists");
        ncch.replace_file("a/0/1/6", vec![4; 9000]).expect("exists");
        assert!(ncch.smdh_mut().expect("icon").set_all_descriptions("seed 1"));
        let out = save(&mut ncch);

        let mut saved = open(&out);
        assert!(saved.verify().expect("reads"), "every hash is valid");
        assert_eq!(saved.code().expect("code"), new_code);
        assert_eq!(saved.read_file("a/0/0/0").expect("exists"), b"changed");
        assert_eq!(saved.read_file("a/0/1/6").expect("exists"), [4; 9000]);
        assert_eq!(saved.read_file("DllBattle.cro").expect("exists"), b"battle code");
        assert_eq!(
            saved.smdh().expect("icon").short_description(0).as_deref(),
            Some("seed 1")
        );
        assert_eq!(&out[..0x100], &[0; 0x100], "signature zeroed");
        assert_eq!(&out[0xA00..0xA0C], b"plain region");
        assert_eq!(units(&out, 0x104), out.len() as u64);
    }

    #[test]
    fn grown_exefs_moves_the_romfs() {
        let code = code();
        let img = cxi(&spec(&code, false, true));
        let mut ncch = open(&img);
        let big: Vec<u8> = (0..40_000u32).map(|i| (i % 253) as u8).collect();
        ncch.set_code(big.clone()).expect("has code");
        let out = save(&mut ncch);
        let mut saved = open(&out);
        assert!(saved.verify().expect("reads"));
        assert_eq!(saved.code().expect("code"), big);
        assert_eq!(saved.read_file("a/0/1/6").expect("exists"), [3; 5000]);
        assert!(units(&out, 0x1B0) > units(&img, 0x1B0));
        assert_eq!(units(&out, 0x1B0) % ROMFS_ALIGN, 0);
    }

    #[test]
    fn encrypted_images_are_refused() {
        let code = code();
        let mut img = cxi(&spec(&code, false, false));
        // Encrypted file systems look like noise.
        let exefs = to_usize(units(&img, 0x1A0)).expect("small");
        for (i, b) in img.iter_mut().enumerate().skip(exefs) {
            *b ^= (i * 131 % 251).to_le_bytes()[0] | 0x40;
        }
        assert!(matches!(Ncch::open(Cursor::new(&img)), Err(FormatError::Encrypted)));
        // A decrypted image without the mark opens, and says so.
        let unmarked = cxi(&spec(&code, false, false));
        assert!(!open(&unmarked).is_marked_decrypted());
    }

    #[test]
    fn writes_only_changes_as_layered_fs() {
        let code = code();
        let img = cxi(&spec(&code, true, true));
        let mut ncch = open(&img);
        let dir = tempfile::tempdir().expect("temp dir");
        let root = ncch.write_layered_fs(dir.path()).expect("writes");
        assert_eq!(root, dir.path().join("0004000000055D00"));
        assert_eq!(
            fs::read_dir(&root).expect("exists").count(),
            1,
            "only an empty romfs folder"
        );
        ncch.replace_file("a/0/1/6", b"new".to_vec()).expect("exists");
        ncch.set_code(vec![1, 2, 3]).expect("has code");
        let root = ncch.write_layered_fs(dir.path()).expect("writes");
        assert_eq!(fs::read(root.join("code.bin")).expect("written"), [1, 2, 3]);
        assert_eq!(fs::read(root.join("romfs/a/0/1/6")).expect("written"), b"new");
        assert!(!root.join("romfs/a/0/0/0").exists());
    }
}
