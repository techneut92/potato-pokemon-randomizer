//! GARC: the archive format of the Gen 6 and Gen 7 RomFS (`a/0/1/6` and so on).
//!
//! An archive has a header, a FATO table (offsets into FATB), a FATB table (per file a bit vector of the sub-files
//! that exist, and per sub-file its start, end and length in FIMB) and FIMB, the data. Most files have one
//! sub-file, number 0. Sub-files may be LZ11-compressed; whether they are decompressed on reading is chosen when
//! the archive is parsed ([`Decompression`]), and decompressed sub-files are compressed again on writing.
//!
//! Deliberate choices:
//!
//! * Each sub-file keeps its own compression flag and its own FATB triple, so files with several sub-files survive
//!   a rebuild.
//! * A replaced sub-file is compressed again when the original was compressed and decompressed on reading,
//!   whether or not it was read before it was replaced. Sub-files that were not replaced keep their original bytes,
//!   so rebuilding an unchanged archive gives the original data.

use std::collections::BTreeMap;

use crate::{FormatError, lz11};

const FORMAT: &str = "GARC";
const VER_4: u16 = 0x0400;
const VER_6: u16 = 0x0600;
const HEADER_LEN_4: usize = 0x1C;
const HEADER_LEN_6: usize = 0x24;
const FRAME_COUNT: u32 = 4;

fn malformed(reason: &'static str) -> FormatError {
    FormatError::WrongFormat { format: FORMAT, reason }
}

/// Which compressed sub-files are decompressed when they are read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decompression {
    /// None: compressed sub-files are read and written as stored.
    None,
    /// Every compressed sub-file.
    All,
    /// The compressed sub-files of the files whose index is `true`; missing indexes count as `false`.
    Only(Vec<bool>),
}

#[derive(Debug, Clone)]
struct SubFile {
    /// Range of the stored bytes in the original data.
    start: usize,
    end: usize,
    /// Whether the stored bytes are compressed and decompressed on reading.
    compressed: bool,
    /// New contents (decompressed), if replaced.
    replaced: Option<Vec<u8>>,
}

/// A parsed GARC archive. It keeps the original data and the replaced sub-files.
#[derive(Debug, Clone)]
pub struct Garc {
    data: Vec<u8>,
    version: u16,
    pad_to: u32,
    fato_header_len: usize,
    fato_padding: u16,
    fatb_header_len: usize,
    fimb_header_len: usize,
    /// Per file: sub-file number → sub-file.
    files: Vec<BTreeMap<u8, SubFile>>,
}

/// A little-endian reader over the archive header and tables.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn bytes<const N: usize>(&mut self) -> Result<[u8; N], FormatError> {
        let b = self
            .data
            .get(self.at..self.at + N)
            .ok_or(FormatError::Truncated { offset: self.at as u64 })?;
        self.at += N;
        let mut out = [0u8; N];
        out.copy_from_slice(b);
        Ok(out)
    }

    fn u16(&mut self) -> Result<u16, FormatError> {
        self.bytes().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, FormatError> {
        self.bytes().map(u32::from_le_bytes)
    }

    fn usize(&mut self) -> Result<usize, FormatError> {
        self.u32().map(|v| v as usize)
    }

    /// A frame magic, stored reversed (`CRAG` for `GARC`).
    fn magic(&mut self, magic: [u8; 4], reason: &'static str) -> Result<(), FormatError> {
        let mut m = self.bytes::<4>()?;
        m.reverse();
        if m == magic { Ok(()) } else { Err(malformed(reason)) }
    }
}

impl Garc {
    /// Parses an archive. The sub-file data is only looked at when it is read.
    ///
    /// # Errors
    /// When the frames are missing or inconsistent.
    pub fn parse(data: Vec<u8>, decompression: &Decompression) -> Result<Self, FormatError> {
        if data.is_empty() {
            return Err(malformed("empty"));
        }
        let mut r = Reader { data: &data, at: 0 };
        r.magic(*b"GARC", "no GARC magic")?;
        let _header_len = r.u32()?;
        let _endianness = r.u16()?;
        let version = r.u16()?;
        if r.u32()? != FRAME_COUNT {
            return Err(malformed("not 4 frames"));
        }
        let data_offset = r.usize()?;
        let _file_size = r.u32()?;
        let pad_to = match version {
            VER_4 => {
                let _largest = r.u32()?;
                4
            }
            VER_6 => {
                let _largest_padded = r.u32()?;
                let _largest = r.u32()?;
                r.u32()?
            }
            _ => return Err(malformed("unknown version")),
        };
        if pad_to == 0 {
            return Err(malformed("zero padding unit"));
        }

        r.magic(*b"FATO", "no FATO frame")?;
        let fato_header_len = r.usize()?;
        let entry_count = usize::from(r.u16()?);
        let fato_padding = r.u16()?;
        r.at += entry_count * 4;

        r.magic(*b"FATB", "no FATB frame")?;
        let table_header_len = r.usize()?;
        let file_count = r.usize()?;
        if file_count > data.len() / 4 {
            return Err(malformed("more files than the archive can hold"));
        }
        let mut files = Vec::with_capacity(file_count);
        for _ in 0..file_count {
            let vector = r.u32()?;
            let mut subs = BTreeMap::new();
            for bit in 0..32u8 {
                if vector >> bit & 1 == 0 {
                    continue;
                }
                let start = r.usize()?;
                let _end = r.usize()?;
                let len = r.usize()?;
                let (start, end) = data_offset
                    .checked_add(start)
                    .and_then(|s| Some((s, s.checked_add(len)?)))
                    .filter(|&(_, e)| e <= data.len())
                    .ok_or_else(|| malformed("sub-file out of bounds"))?;
                let decompress = match decompression {
                    Decompression::None => false,
                    Decompression::All => true,
                    Decompression::Only(which) => which.get(files.len()).copied().unwrap_or(false),
                };
                let compressed = decompress && end > start && data[start] == lz11::LZ11;
                subs.insert(
                    bit,
                    SubFile {
                        start,
                        end,
                        compressed,
                        replaced: None,
                    },
                );
            }
            files.push(subs);
        }

        r.magic(*b"FIMB", "no FIMB frame")?;
        let fimb_header_len = r.usize()?;
        if fimb_header_len < 12 {
            return Err(malformed("FIMB header too short"));
        }
        Ok(Self {
            data,
            version,
            pad_to,
            fato_header_len,
            fato_padding,
            fatb_header_len: table_header_len,
            fimb_header_len,
            files,
        })
    }

    /// Number of files.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the archive has no files.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The sub-file numbers of file `index`, in order.
    pub fn sub_files(&self, index: usize) -> impl Iterator<Item = u8> + '_ {
        self.files.get(index).into_iter().flat_map(|subs| subs.keys().copied())
    }

    /// Sub-file 0 of file `index`, decompressed if chosen at parsing.
    ///
    /// # Errors
    /// When the file or its sub-file 0 does not exist or does not decompress.
    pub fn file(&self, index: usize) -> Result<Vec<u8>, FormatError> {
        self.sub_file(index, 0)
    }

    /// Sub-file `sub` of file `index`, decompressed if chosen at parsing.
    ///
    /// # Errors
    /// When the sub-file does not exist or does not decompress.
    pub fn sub_file(&self, index: usize, sub: u8) -> Result<Vec<u8>, FormatError> {
        let s = self.get(index, sub)?;
        if let Some(data) = &s.replaced {
            return Ok(data.clone());
        }
        let stored = &self.data[s.start..s.end];
        if s.compressed {
            lz11::decompress(stored, 0)
        } else {
            Ok(stored.to_vec())
        }
    }

    /// Replaces sub-file 0 of file `index`, adding it if the file has none; other sub-files stay.
    ///
    /// # Errors
    /// When the file does not exist.
    pub fn set_file(&mut self, index: usize, data: Vec<u8>) -> Result<(), FormatError> {
        self.set_sub_file(index, 0, data)
    }

    /// Replaces sub-file `sub` of file `index`, adding it if it does not exist.
    ///
    /// # Errors
    /// When the file does not exist or `sub` is not below 32.
    pub fn set_sub_file(&mut self, index: usize, sub: u8, data: Vec<u8>) -> Result<(), FormatError> {
        if sub >= 32 {
            return Err(FormatError::NotFound {
                what: format!("GARC sub-file {sub}"),
            });
        }
        let subs = self.files.get_mut(index).ok_or_else(|| FormatError::NotFound {
            what: format!("GARC file {index}"),
        })?;
        subs.entry(sub)
            .or_insert(SubFile {
                start: 0,
                end: 0,
                compressed: false,
                replaced: None,
            })
            .replaced = Some(data);
        Ok(())
    }

    fn get(&self, index: usize, sub: u8) -> Result<&SubFile, FormatError> {
        self.files
            .get(index)
            .and_then(|subs| subs.get(&sub))
            .ok_or_else(|| FormatError::NotFound {
                what: format!("GARC file {index} sub-file {sub}"),
            })
    }

    /// The archive with the replaced sub-files.
    ///
    /// # Errors
    /// When a replaced sub-file that has to be compressed is too large for LZ11, or the archive grows past 4 GiB.
    pub fn to_bytes(&self) -> Result<Vec<u8>, FormatError> {
        let too_large = || FormatError::Unencodable {
            format: FORMAT,
            reason: "archive larger than 4 GiB",
        };
        let u32_of = |n: usize| u32::try_from(n).map_err(|_| too_large());
        let header_len = if self.version == VER_4 {
            HEADER_LEN_4
        } else {
            HEADER_LEN_6
        };
        let pad_to = self.pad_to as usize;
        let pad_byte = self.fato_padding.to_le_bytes()[0];

        let mut offsets = Vec::with_capacity(self.fato_header_len.max(12 + self.files.len() * 4));
        offsets.extend_from_slice(b"OTAF");
        offsets.extend_from_slice(&u32_of(self.fato_header_len)?.to_le_bytes());
        offsets.extend_from_slice(&u16::try_from(self.files.len()).map_err(|_| too_large())?.to_le_bytes());
        offsets.extend_from_slice(&self.fato_padding.to_le_bytes());
        let mut fatb = Vec::with_capacity(self.fatb_header_len);
        fatb.extend_from_slice(b"BTAF");
        fatb.extend_from_slice(&u32_of(self.fatb_header_len)?.to_le_bytes());
        fatb.extend_from_slice(&u32_of(self.files.len())?.to_le_bytes());

        let mut fimb_data = Vec::new();
        let (mut largest, mut largest_padded) = (0, 0);
        for subs in &self.files {
            offsets.extend_from_slice(&u32_of(fatb.len() - 12)?.to_le_bytes());
            let vector = subs.keys().fold(0u32, |v, &bit| v | 1 << bit);
            fatb.extend_from_slice(&vector.to_le_bytes());
            for s in subs.values() {
                let stored = match &s.replaced {
                    None => std::borrow::Cow::Borrowed(&self.data[s.start..s.end]),
                    Some(data) if s.compressed => std::borrow::Cow::Owned(lz11::compress(data)?),
                    Some(data) => std::borrow::Cow::Borrowed(data.as_slice()),
                };
                let start = fimb_data.len();
                fimb_data.extend_from_slice(&stored);
                let padding = (pad_to - stored.len() % pad_to) % pad_to;
                fimb_data.resize(fimb_data.len() + padding, pad_byte);
                largest = largest.max(stored.len());
                largest_padded = largest_padded.max(stored.len() + padding);
                fatb.extend_from_slice(&u32_of(start)?.to_le_bytes());
                fatb.extend_from_slice(&u32_of(fimb_data.len())?.to_le_bytes());
                fatb.extend_from_slice(&u32_of(stored.len())?.to_le_bytes());
            }
        }
        // The header lengths of the tables are kept as they were, as is the archive's own custom.
        let mut out =
            Vec::with_capacity(header_len + offsets.len() + fatb.len() + self.fimb_header_len + fimb_data.len());
        let data_offset = header_len + offsets.len() + fatb.len() + self.fimb_header_len;
        out.extend_from_slice(b"CRAG");
        out.extend_from_slice(&u32_of(header_len)?.to_le_bytes());
        out.extend_from_slice(&0xFEFFu16.to_le_bytes());
        out.extend_from_slice(&self.version.to_le_bytes());
        out.extend_from_slice(&FRAME_COUNT.to_le_bytes());
        out.extend_from_slice(&u32_of(data_offset)?.to_le_bytes());
        out.extend_from_slice(&u32_of(data_offset + fimb_data.len())?.to_le_bytes());
        if self.version == VER_4 {
            out.extend_from_slice(&u32_of(largest)?.to_le_bytes());
        } else {
            out.extend_from_slice(&u32_of(largest_padded)?.to_le_bytes());
            out.extend_from_slice(&u32_of(largest)?.to_le_bytes());
            out.extend_from_slice(&self.pad_to.to_le_bytes());
        }
        out.extend_from_slice(&offsets);
        out.extend_from_slice(&fatb);
        out.extend_from_slice(b"BMIF");
        out.extend_from_slice(&u32_of(self.fimb_header_len)?.to_le_bytes());
        out.extend_from_slice(&u32_of(fimb_data.len())?.to_le_bytes());
        out.resize(data_offset, 0);
        out.extend_from_slice(&fimb_data);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_and_rebuilds_unchanged() {
        let img = crate::ctr::testing::garc(&[b"zero", b"one!!", b"", b"three"]);
        let g = Garc::parse(img.clone(), &Decompression::None).expect("parses");
        assert_eq!(g.len(), 4);
        assert_eq!(g.file(1).expect("exists"), b"one!!");
        assert_eq!(g.file(2).expect("exists"), b"");
        assert_eq!(g.to_bytes().expect("builds"), img);
    }

    #[test]
    fn version_4_rebuilds_unchanged() {
        let img = crate::ctr::testing::garc_with(&[vec![(0, b"abc".to_vec())], vec![(0, b"defgh".to_vec())]], 0x0400);
        let g = Garc::parse(img.clone(), &Decompression::None).expect("parses");
        assert_eq!(g.file(1).expect("exists"), b"defgh");
        assert_eq!(g.to_bytes().expect("builds"), img);
    }

    #[test]
    fn replaces_files() {
        let img = crate::ctr::testing::garc(&[b"zero", b"one", b"two"]);
        let mut g = Garc::parse(img, &Decompression::None).expect("parses");
        g.set_file(1, b"a longer replacement".to_vec()).expect("exists");
        let rebuilt = Garc::parse(g.to_bytes().expect("builds"), &Decompression::None).expect("parses");
        assert_eq!(rebuilt.file(0).expect("exists"), b"zero");
        assert_eq!(rebuilt.file(1).expect("exists"), b"a longer replacement");
        assert_eq!(rebuilt.file(2).expect("exists"), b"two");
        assert!(g.set_file(3, Vec::new()).is_err());
    }

    #[test]
    fn decompresses_and_compresses_again_as_chosen() {
        let plain = vec![5u8; 100];
        let packed = lz11::compress(&plain).expect("fits");
        let img = crate::ctr::testing::garc(&[&packed, &packed]);
        let raw = Garc::parse(img.clone(), &Decompression::None).expect("parses");
        assert_eq!(raw.file(0).expect("exists"), packed);
        let only = Garc::parse(img.clone(), &Decompression::Only(vec![false, true])).expect("parses");
        assert_eq!(only.file(0).expect("exists"), packed);
        assert_eq!(only.file(1).expect("exists"), plain);

        let mut all = Garc::parse(img.clone(), &Decompression::All).expect("parses");
        assert_eq!(all.file(0).expect("exists"), plain);
        assert_eq!(
            all.to_bytes().expect("builds"),
            img,
            "unchanged sub-files keep their bytes"
        );
        all.set_file(1, vec![6u8; 50]).expect("exists");
        let rebuilt = Garc::parse(all.to_bytes().expect("builds"), &Decompression::None).expect("parses");
        assert_eq!(
            lz11::decompress(&rebuilt.file(1).expect("exists"), 0).expect("compressed"),
            [6u8; 50]
        );
    }

    #[test]
    fn keeps_every_sub_file() {
        let img = crate::ctr::testing::garc_with(
            &[
                vec![(0, b"main".to_vec()), (3, b"extra".to_vec())],
                vec![(1, b"only one".to_vec())],
            ],
            0x0600,
        );
        let mut g = Garc::parse(img.clone(), &Decompression::None).expect("parses");
        assert_eq!(g.sub_files(0).collect::<Vec<_>>(), [0, 3]);
        assert_eq!(g.sub_file(0, 3).expect("exists"), b"extra");
        assert!(g.file(1).is_err());
        assert_eq!(g.to_bytes().expect("builds"), img);
        g.set_sub_file(0, 3, b"changed".to_vec()).expect("exists");
        let rebuilt = Garc::parse(g.to_bytes().expect("builds"), &Decompression::None).expect("parses");
        assert_eq!(rebuilt.sub_file(0, 0).expect("exists"), b"main");
        assert_eq!(rebuilt.sub_file(0, 3).expect("exists"), b"changed");
        assert_eq!(rebuilt.sub_file(1, 1).expect("exists"), b"only one");
    }

    #[test]
    fn rejects_broken_archives() {
        assert!(Garc::parse(Vec::new(), &Decompression::None).is_err());
        assert!(Garc::parse(b"not a garc at all".to_vec(), &Decompression::None).is_err());
        let mut img = crate::ctr::testing::garc(&[b"zero"]);
        let n = img.len();
        img.truncate(n - 8);
        // The sub-file now ends past the data.
        assert!(Garc::parse(img, &Decompression::None).is_err());
    }
}
