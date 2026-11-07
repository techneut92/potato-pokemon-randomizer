//! The game files a Gen 6 handler reads and writes: the executable, the code modules (`.cro`) and the GARC
//! archives of the RomFS, each read once and kept until it is written back into the image. With a game update,
//! the image reads each file from the update when the update has it.
//!
//! Every file is read through one cache, so two names for the same file (the field module is both `Field` and
//! `StaticPokemon`) share their changes. [`Rom::flush`] hands only what differs from the source to the image, so an
//! unchanged game saves byte for byte.

use std::collections::BTreeMap;
use std::io::{Read, Seek};

use ppr_formats::ctr::{Decompression, Game, Garc};

use crate::RomError;
use crate::ctr_text;
use crate::entries::RomEntry;

/// A source the image is read from: a file or bytes in memory.
pub(crate) trait Source: Read + Seek + Send {}

impl<T: Read + Seek + Send> Source for T {}

/// The boxed source, so the handler is the same type for files and bytes.
pub(crate) struct BoxedSource(pub(crate) Box<dyn Source>);

impl std::fmt::Debug for BoxedSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BoxedSource")
    }
}

impl Read for BoxedSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl Seek for BoxedSource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

/// The image of a game, with its update when one is loaded.
pub(crate) type Image = Game<BoxedSource>;

/// A parsed GARC with the way it was read.
#[derive(Debug)]
struct CachedGarc {
    garc: Garc,
    changed: bool,
}

#[derive(Debug)]
struct CachedFile {
    data: Vec<u8>,
    changed: bool,
}

/// The image and the game files read from it.
#[derive(Debug)]
pub(crate) struct Rom {
    image: Image,
    code: Vec<u8>,
    code_changed: bool,
    garcs: BTreeMap<String, CachedGarc>,
    files: BTreeMap<String, CachedFile>,
}

/// The path of a `File<key>` entry, e.g. `a/2/1/8` from `<a/2/1/8, [7834E6AE, 7834E6AE]>`.
pub(crate) fn file_path(entry: &RomEntry, key: &str) -> Result<String, RomError> {
    let raw = entry
        .value(&format!("File<{key}>"))
        .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no file {key}", entry.name)))?;
    let inner = raw.trim().trim_start_matches('<').trim_end_matches('>');
    let path = inner.split(',').next().unwrap_or_default().trim();
    if path.is_empty() {
        return Err(RomError::Malformed(format!(
            "ROM entry [{}] has an empty file {key}",
            entry.name
        )));
    }
    Ok(path.to_owned())
}

impl Rom {
    pub(crate) fn new(mut image: Image) -> Result<Self, RomError> {
        let code = image.code()?.to_vec();
        Ok(Self {
            image,
            code,
            code_changed: false,
            garcs: BTreeMap::new(),
            files: BTreeMap::new(),
        })
    }

    /// Gives back the image, dropping what was read from it.
    pub(crate) fn into_image(self) -> Image {
        self.image
    }

    pub(crate) fn image(&self) -> &Image {
        &self.image
    }

    pub(crate) fn image_mut(&mut self) -> &mut Image {
        &mut self.image
    }

    /// The executable, decompressed.
    pub(crate) fn code(&self) -> &[u8] {
        &self.code
    }

    /// The executable for changing.
    pub(crate) fn code_mut(&mut self) -> &mut Vec<u8> {
        self.code_changed = true;
        &mut self.code
    }

    /// A GARC by path. `decompress` decompresses every compressed file, as the wild encounters need; the first read
    /// of a path decides it.
    pub(crate) fn garc(&mut self, path: &str, decompress: bool) -> Result<&Garc, RomError> {
        self.load_garc(path, decompress)?;
        Ok(&self.garcs[path].garc)
    }

    /// A GARC by path, for changing.
    pub(crate) fn garc_mut(&mut self, path: &str, decompress: bool) -> Result<&mut Garc, RomError> {
        self.load_garc(path, decompress)?;
        let cached = self
            .garcs
            .get_mut(path)
            .ok_or_else(|| RomError::Malformed(path.to_owned()))?;
        cached.changed = true;
        Ok(&mut cached.garc)
    }

    fn load_garc(&mut self, path: &str, decompress: bool) -> Result<(), RomError> {
        if !self.garcs.contains_key(path) {
            let data = self.image.read_file(path)?;
            let mode = if decompress {
                Decompression::All
            } else {
                Decompression::None
            };
            let garc = Garc::parse(data, &mode)?;
            self.garcs.insert(path.to_owned(), CachedGarc { garc, changed: false });
        }
        Ok(())
    }

    /// File `index` of a GARC.
    pub(crate) fn garc_file(&mut self, path: &str, decompress: bool, index: usize) -> Result<Vec<u8>, RomError> {
        Ok(self.garc(path, decompress)?.file(index)?)
    }

    /// Replaces file `index` of a GARC, when it differs.
    pub(crate) fn set_garc_file(
        &mut self,
        path: &str,
        decompress: bool,
        index: usize,
        data: Vec<u8>,
    ) -> Result<(), RomError> {
        if self.garc_file(path, decompress, index)? != data {
            self.garc_mut(path, decompress)?.set_file(index, data)?;
        }
        Ok(())
    }

    /// A whole RomFS file, such as a code module.
    pub(crate) fn file(&mut self, path: &str) -> Result<&[u8], RomError> {
        self.load_file(path)?;
        Ok(&self.files[path].data)
    }

    /// A whole RomFS file, for changing.
    pub(crate) fn file_mut(&mut self, path: &str) -> Result<&mut Vec<u8>, RomError> {
        self.load_file(path)?;
        let cached = self
            .files
            .get_mut(path)
            .ok_or_else(|| RomError::Malformed(path.to_owned()))?;
        cached.changed = true;
        Ok(&mut cached.data)
    }

    fn load_file(&mut self, path: &str) -> Result<(), RomError> {
        if !self.files.contains_key(path) {
            let data = self.image.read_file(path)?;
            self.files.insert(path.to_owned(), CachedFile { data, changed: false });
        }
        Ok(())
    }

    /// The texts of file `index` of a text GARC.
    pub(crate) fn texts(&mut self, path: &str, index: usize) -> Result<Vec<String>, RomError> {
        let raw = self.garc_file(path, false, index)?;
        ctr_text::read_texts(&raw, true)
    }

    /// Replaces the texts of file `index` of a text GARC.
    pub(crate) fn set_texts(&mut self, path: &str, index: usize, texts: &[String]) -> Result<(), RomError> {
        let raw = self.garc_file(path, false, index)?;
        if ctr_text::read_texts(&raw, true)? == texts {
            return Ok(());
        }
        let new = ctr_text::write_texts(Some(&raw), texts, true)?;
        self.set_garc_file(path, false, index, new)
    }

    /// Hands every changed file to the image; files changed back to their original contents are left out.
    pub(crate) fn flush(&mut self) -> Result<(), RomError> {
        if self.code_changed {
            self.image.set_code(self.code.clone())?;
            self.code_changed = false;
        }
        for (path, cached) in &mut self.garcs {
            if cached.changed {
                self.image.replace_file(path, cached.garc.to_bytes()?)?;
                cached.changed = false;
            }
        }
        for (path, cached) in &mut self.files {
            if cached.changed {
                self.image.replace_file(path, cached.data.clone())?;
                cached.changed = false;
            }
        }
        Ok(())
    }
}

/// A non-negative integer the ROM entry must have.
pub(crate) fn need(entry: &RomEntry, key: &str) -> Result<usize, RomError> {
    entry
        .int(key)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no {key}", entry.name)))
}

/// Little-endian reads and writes inside a file's bytes; out-of-range access is an error, not a panic.
pub(crate) trait Bytes {
    fn u8_at(&self, at: usize) -> Result<u8, RomError>;
    fn u16_at(&self, at: usize) -> Result<u16, RomError>;
    fn u32_at(&self, at: usize) -> Result<u32, RomError>;
}

pub(crate) trait BytesMut {
    fn set_u8(&mut self, at: usize, value: u8) -> Result<(), RomError>;
    fn set_u16(&mut self, at: usize, value: u16) -> Result<(), RomError>;
    fn set_u32(&mut self, at: usize, value: u32) -> Result<(), RomError>;
}

fn out_of_range(at: usize, len: usize) -> RomError {
    RomError::Malformed(format!("offset {at:#x} is past the end of a file of {len:#x} bytes"))
}

impl Bytes for [u8] {
    fn u8_at(&self, at: usize) -> Result<u8, RomError> {
        self.get(at).copied().ok_or_else(|| out_of_range(at, self.len()))
    }

    fn u16_at(&self, at: usize) -> Result<u16, RomError> {
        let b = self.get(at..at + 2).ok_or_else(|| out_of_range(at, self.len()))?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn u32_at(&self, at: usize) -> Result<u32, RomError> {
        let b = self.get(at..at + 4).ok_or_else(|| out_of_range(at, self.len()))?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}

impl BytesMut for [u8] {
    fn set_u8(&mut self, at: usize, value: u8) -> Result<(), RomError> {
        let len = self.len();
        *self.get_mut(at).ok_or_else(|| out_of_range(at, len))? = value;
        Ok(())
    }

    fn set_u16(&mut self, at: usize, value: u16) -> Result<(), RomError> {
        let len = self.len();
        self.get_mut(at..at + 2)
            .ok_or_else(|| out_of_range(at, len))?
            .copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    fn set_u32(&mut self, at: usize, value: u32) -> Result<(), RomError> {
        let len = self.len();
        self.get_mut(at..at + 4)
            .ok_or_else(|| out_of_range(at, len))?
            .copy_from_slice(&value.to_le_bytes());
        Ok(())
    }
}

/// The offset of the only match of `hex` in `data`, counting matches that do not overlap; `None` when there is no
/// match, more than one, or the only one is at offset 0 (which every caller treats as not found).
pub(crate) fn find(data: &[u8], hex: &str) -> Option<usize> {
    let needle = crate::gba::hex(hex).ok()?;
    if needle.is_empty() {
        return None;
    }
    let mut found = None;
    let mut at = 0;
    while at + needle.len() <= data.len() {
        if data[at..].starts_with(&needle) {
            if found.is_some() {
                return None;
            }
            found = Some(at);
            at += needle.len();
        } else {
            at += 1;
        }
    }
    found.filter(|&at| at > 0)
}

/// The offset just past the only match of `hex`, see [`find`].
pub(crate) fn find_after(data: &[u8], hex: &str) -> Option<usize> {
    find(data, hex).map(|at| at + hex.len() / 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_wants_one_match_past_the_start() {
        assert_eq!(find(&[1, 2, 3, 4], "0304"), Some(2));
        assert_eq!(find(&[1, 2, 3, 4], "0102"), None);
        assert_eq!(find(&[0, 3, 4, 3, 4], "0304"), None);
        assert_eq!(find(&[0, 1, 1, 1], "0101"), Some(1));
        assert_eq!(find_after(&[0, 9, 9, 5], "0909"), Some(3));
    }

    #[test]
    fn bytes_are_little_endian_and_checked() {
        let mut d = [0u8; 6];
        d.set_u16(0, 0x1234).expect("fits");
        d.set_u32(2, 0xAABB_CCDD).expect("fits");
        assert_eq!(d.u16_at(0).expect("fits"), 0x1234);
        assert_eq!(d.u32_at(2).expect("fits"), 0xAABB_CCDD);
        assert!(d.u32_at(3).is_err());
        assert!(d.set_u8(6, 1).is_err());
    }
}
