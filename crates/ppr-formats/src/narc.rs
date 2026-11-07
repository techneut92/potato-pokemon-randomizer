//! NARC archives: the Nitro archives that hold most game data of DS games.
//!
//! A NARC is a 16-byte header and three frames: `BTAF` (the file table: start and end of each file within the
//! image), `BTNF` (the name table, usually empty) and `GMIF` (the file image). [`Narc::to_bytes`] lays them out
//! again: files 4-byte aligned and padded with 0xFF, frames in that order.
//!
//! Two deviations from the classic layout code, both for archives with file names, which the games' data archives
//! don't have: names are read one after another (rather than all from the first entry), and a rebuilt name table
//! ends with a 0 byte and is padded to a multiple of 4, so the image after it stays aligned.

use std::collections::BTreeMap;

use crate::FormatError;
use crate::io::{slice_at, u16_at, u32_at, usize_at};

const FORMAT: &str = "NARC";

fn bad(reason: &'static str) -> FormatError {
    FormatError::WrongFormat { format: FORMAT, reason }
}

/// A NARC archive: its files, by index, and their names if it has any.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Narc {
    /// The files, by index.
    pub files: Vec<Vec<u8>>,
    /// One name per file, when the archive has names.
    names: Option<Vec<String>>,
}

impl Narc {
    /// An empty archive without file names.
    pub fn new() -> Self {
        Self::default()
    }

    /// An archive of `files`, without file names.
    pub fn from_files(files: Vec<Vec<u8>>) -> Self {
        Self { files, names: None }
    }

    /// The file names, one per file, when the archive has them.
    pub fn names(&self) -> Option<&[String]> {
        self.names.as_deref()
    }

    /// Reads a NARC.
    ///
    /// The last frame may claim to be shorter than the rest of the data (expanded archives in some ROM hacks do);
    /// it then runs to the end.
    ///
    /// # Errors
    /// When a frame is missing or a table points outside the data.
    pub fn parse(data: &[u8]) -> Result<Self, FormatError> {
        let frames = frames(data)?;
        let (Some(file_table), Some(name_table), Some(image)) =
            (frames.get(b"FATB"), frames.get(b"FNTB"), frames.get(b"FIMG"))
        else {
            return Err(bad("a frame is missing"));
        };
        let count = usize_at(file_table, 0)?;
        if count > file_table.len() / 8 {
            return Err(FormatError::Truncated { offset: 0 });
        }
        let mut files = Vec::with_capacity(count);
        for i in 0..count {
            let start = usize_at(file_table, 4 + i * 8)?;
            let end = usize_at(file_table, 8 + i * 8)?;
            let len = end.checked_sub(start).ok_or(bad("a file ends before it starts"))?;
            files.push(slice_at(image, start, len)?.to_vec());
        }
        let names = if u32_at(name_table, 0)? == 8 {
            let mut at = 8;
            let mut names = Vec::with_capacity(count);
            for _ in 0..count {
                let len = usize::from(*name_table.get(at).ok_or(FormatError::Truncated { offset: at as u64 })?);
                at += 1;
                names.push(ascii(slice_at(name_table, at, len)?));
                at += len;
            }
            Some(names)
        } else {
            None
        };
        Ok(Self { files, names })
    }

    /// The archive as bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        let image_len: usize = self.files.iter().map(|f| f.len().next_multiple_of(4)).sum();

        let mut file_table = frame_header(*b"BTAF", 12 + self.files.len() * 8);
        push32(&mut file_table, self.files.len());
        let mut image_frame = frame_header(*b"GMIF", image_len + 8);
        let mut offset = 0;
        for file in &self.files {
            push32(&mut file_table, offset);
            push32(&mut file_table, offset + file.len());
            image_frame.extend_from_slice(file);
            image_frame.resize(image_frame.len().next_multiple_of(4), 0xFF);
            offset += file.len().next_multiple_of(4);
        }

        let name_table = if let Some(names) = &self.names {
            let names_len: usize = names.iter().map(|n| n.len() + 1).sum();
            let len = (16 + names_len + 1).next_multiple_of(4);
            let mut name_table = frame_header(*b"BTNF", len);
            push32(&mut name_table, 8);
            push32(&mut name_table, 0x10000);
            for name in names {
                name_table.push(u8::try_from(name.len()).unwrap_or(u8::MAX));
                name_table.extend(name.bytes().take(usize::from(u8::MAX)));
            }
            name_table.push(0);
            name_table.resize(len, 0xFF);
            name_table
        } else {
            let mut name_table = frame_header(*b"BTNF", 16);
            push32(&mut name_table, 4);
            push32(&mut name_table, 0x10000);
            name_table
        };

        let total = 16 + file_table.len() + name_table.len() + image_frame.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(b"NARC");
        out.extend_from_slice(&0xFFFE_u16.to_le_bytes());
        out.extend_from_slice(&0x0100_u16.to_le_bytes());
        push32(&mut out, total);
        out.extend_from_slice(&0x10_u16.to_le_bytes());
        out.extend_from_slice(&3_u16.to_le_bytes());
        out.extend(file_table);
        out.extend(name_table);
        out.extend(image_frame);
        out
    }
}

/// The frames of a Nitro file by their (byte-reversed) magic, without their 8-byte headers. A later frame with the
/// same magic replaces an earlier one.
fn frames(data: &[u8]) -> Result<BTreeMap<[u8; 4], &[u8]>, FormatError> {
    let count = usize::from(u16_at(data, 0x0E)?);
    let mut frames = BTreeMap::new();
    let mut at = 0x10;
    for i in 0..count {
        let magic = slice_at(data, at, 4)?;
        let mut size = usize_at(data, at + 4)?;
        if i == count - 1 && at + size < data.len() {
            size = data.len() - at;
        }
        if size < 8 {
            return Err(bad("a frame shorter than its header"));
        }
        frames.insert(
            [magic[3], magic[2], magic[1], magic[0]],
            slice_at(data, at + 8, size - 8)?,
        );
        at += size;
    }
    Ok(frames)
}

fn frame_header(magic: [u8; 4], len: usize) -> Vec<u8> {
    let mut frame = Vec::with_capacity(len);
    frame.extend_from_slice(&magic);
    push32(&mut frame, len);
    frame
}

fn push32(out: &mut Vec<u8>, value: usize) {
    // Archives are far below 4 GiB.
    out.extend_from_slice(&u32::try_from(value).unwrap_or(u32::MAX).to_le_bytes());
}

/// ASCII text; other bytes become U+FFFD.
pub(crate) fn ascii(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| {
            if b.is_ascii() {
                char::from(b)
            } else {
                char::REPLACEMENT_CHARACTER
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Narc {
        Narc {
            files: vec![vec![1, 2, 3], vec![], vec![4, 5, 6, 7, 8]],
            names: None,
        }
    }

    #[test]
    fn lays_out_files_aligned_and_padded() {
        let bytes = sample().to_bytes();
        assert_eq!(&bytes[..4], b"NARC");
        assert_eq!(u32_at(&bytes, 8).expect("size") as usize, bytes.len());
        // FATB: 3 files at 0..3, 4..4, 4..9.
        assert_eq!(&bytes[0x10..0x14], b"BTAF");
        let file_table: Vec<u32> = (0..7).map(|i| u32_at(&bytes, 0x18 + i * 4).expect("entry")).collect();
        assert_eq!(file_table, [3, 0, 3, 4, 4, 4, 9]);
        // FNTB without names, then the image.
        assert_eq!(&bytes[0x34..0x38], b"BTNF");
        assert_eq!(&bytes[0x44..0x48], b"GMIF");
        assert_eq!(&bytes[0x4C..], [1, 2, 3, 0xFF, 4, 5, 6, 7, 8, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn round_trips_and_rebuilds_identically() {
        let narc = sample();
        let bytes = narc.to_bytes();
        let back = Narc::parse(&bytes).expect("valid");
        assert_eq!(back, narc);
        assert_eq!(back.to_bytes(), bytes);
    }

    #[test]
    fn replacing_a_file_moves_the_ones_after_it() {
        let mut narc = Narc::parse(&sample().to_bytes()).expect("valid");
        narc.files[0] = vec![9; 10];
        let back = Narc::parse(&narc.to_bytes()).expect("valid");
        assert_eq!(back.files, [vec![9; 10], vec![], vec![4, 5, 6, 7, 8]]);
    }

    #[test]
    fn reads_and_writes_names() {
        let narc = Narc {
            files: vec![vec![1], vec![2]],
            names: Some(vec!["a.bin".into(), "bc".into()]),
        };
        let bytes = narc.to_bytes();
        assert_eq!(bytes.len() % 4, 0);
        let back = Narc::parse(&bytes).expect("valid");
        assert_eq!(back.names(), Some(&["a.bin".to_owned(), "bc".to_owned()][..]));
        assert_eq!(back.files, narc.files);
    }

    #[test]
    fn the_last_frame_runs_to_the_end() {
        let mut bytes = sample().to_bytes();
        // Grow the last file's slot without updating the image frame's size.
        let image_size_at = 0x44 + 4;
        let size = u32_at(&bytes, image_size_at).expect("size");
        bytes.extend_from_slice(&[0xAA; 4]);
        let end_of_last = 0x18 + 6 * 4;
        bytes[end_of_last..end_of_last + 4].copy_from_slice(&13u32.to_le_bytes());
        assert_eq!(u32_at(&bytes, image_size_at).expect("size"), size);
        let back = Narc::parse(&bytes).expect("valid");
        assert_eq!(back.files[2], [4, 5, 6, 7, 8, 0xFF, 0xFF, 0xFF, 0xAA]);
    }

    #[test]
    fn rejects_broken_archives() {
        assert!(Narc::parse(&[]).is_err());
        let bytes = sample().to_bytes();
        assert!(Narc::parse(&bytes[..0x30]).is_err());
        let mut no_image = bytes.clone();
        no_image[0x0E] = 2;
        assert!(Narc::parse(&no_image).is_err());
        let mut backwards = bytes;
        backwards[0x1C..0x20].copy_from_slice(&9u32.to_le_bytes());
        assert!(Narc::parse(&backwards).is_err());
    }
}
