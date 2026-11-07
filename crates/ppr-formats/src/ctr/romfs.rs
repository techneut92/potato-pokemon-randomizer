//! RomFS: the read-only file system of a 3DS title.
//!
//! A RomFS is an IVFC hash tree: level 3 holds the file system (directory and file tables, then the file data),
//! level 2 the SHA-256 of every level 3 block, level 1 those of level 2, and the master hash after the header those
//! of level 1. On disc the order is header, master hash, level 3, level 1, level 2, each level aligned to its
//! block size.
//!
//! A rebuild keeps every unchanged byte of level 3 where it was:
//!
//! * a replaced file that fits in its old slot (up to the next file) is written there; the rest of the slot keeps
//!   its old bytes;
//! * a replaced file that does not fit moves to the end of the file data (16-byte aligned) and its old slot keeps
//!   its old bytes, which nothing refers to any more;
//! * only the file table entries of replaced files change, and only the level 3 blocks that changed are hashed
//!   again; the hashes of the others are taken from the original level 2.
//!
//! So a rebuild without changes gives the original RomFS, and a rebuild of a multi-gigabyte RomFS with a few
//! changed files hashes only those files, not the whole image.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::Write;

use sha2::{Digest, Sha256};

use crate::io::{COPY_CHUNK, align_up, read_at, read_into, read_vec_at};
use crate::{FormatError, RomSource};

const FORMAT: &str = "RomFS";
pub(crate) const HEADER_LEN: usize = 0x5C;
pub(crate) const MAGIC: [u8; 8] = *b"IVFC\0\0\x01\0";
const MASTER_HASH_OFFSET: u64 = 0x60;
pub(crate) const LEVEL3_HEADER_LEN: usize = 0x28;
pub(crate) const UNUSED: u32 = u32::MAX;
const FILE_ALIGN: u64 = 0x10;
const HASH_LEN: usize = 0x20;
const MEDIA_UNIT: u64 = 0x200;
/// Largest directory or file table accepted; real ones are a few hundred KiB.
const MAX_TABLE: u64 = 64 << 20;

fn malformed(reason: &'static str) -> FormatError {
    FormatError::WrongFormat { format: FORMAT, reason }
}

fn le32(b: &[u8], at: usize) -> Result<u32, FormatError> {
    b.get(at..at + 4)
        .map(|w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]))
        .ok_or_else(|| malformed("table entry out of bounds"))
}

fn le64(b: &[u8], at: usize) -> Result<u64, FormatError> {
    b.get(at..at + 8)
        .map(|w| u64::from_le_bytes([w[0], w[1], w[2], w[3], w[4], w[5], w[6], w[7]]))
        .ok_or_else(|| malformed("table entry out of bounds"))
}

fn to_usize(n: u64) -> Result<usize, FormatError> {
    usize::try_from(n).map_err(|_| malformed("size out of range"))
}

/// A name in a directory or file table: UTF-16LE, `len` bytes, without surrounding control characters and spaces.
fn name_at(table: &[u8], at: usize, len: u32) -> Result<String, FormatError> {
    if len == UNUSED {
        return Ok(String::new());
    }
    let bytes = table
        .get(at..at + len as usize)
        .ok_or_else(|| malformed("name out of bounds"))?;
    let units: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    Ok(String::from_utf16_lossy(&units)
        .trim_matches(|c: char| c <= ' ')
        .to_owned())
}

fn sha256(data: &[u8]) -> [u8; HASH_LEN] {
    Sha256::digest(data).into()
}

/// SHA-256 of every `block`-sized piece of `data`, the last one padded with zeros.
pub(crate) fn hash_blocks(data: &[u8], block: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len().div_ceil(block) * HASH_LEN);
    let mut padded = vec![0u8; block];
    for chunk in data.chunks(block) {
        if chunk.len() == block {
            out.extend_from_slice(&sha256(chunk));
        } else {
            padded[..chunk.len()].copy_from_slice(chunk);
            padded[chunk.len()..].fill(0);
            out.extend_from_slice(&sha256(&padded));
        }
    }
    out
}

/// A file in the RomFS.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// Full path without a leading slash, e.g. `a/0/1/6`.
    pub(crate) path: String,
    /// Offset of the file's entry in the file table.
    meta: usize,
    /// Offset of the data from the start of the file data.
    pub(crate) offset: u64,
    /// Length of the data.
    pub(crate) len: u64,
}

/// The parsed structure of a RomFS. Offsets are relative to the start of the RomFS unless noted.
#[derive(Debug, Clone)]
pub(crate) struct RomFs {
    header: [u8; HEADER_LEN],
    /// Block sizes of levels 1, 2 and 3, as powers of two.
    block_log: [u32; 3],
    level3_offset: u64,
    level3_size: u64,
    level2_offset: u64,
    level2_size: u64,
    /// Offset of the file table in level 3.
    file_meta_offset: u64,
    file_meta: Vec<u8>,
    /// Offset of the file data in level 3.
    file_data_offset: u64,
    pub(crate) files: Vec<Entry>,
    pub(crate) index: BTreeMap<String, usize>,
}

impl RomFs {
    /// Parses the RomFS at `base` in `src`, which is `size` bytes long.
    pub(crate) fn parse(src: &mut (impl RomSource + ?Sized), base: u64, size: u64) -> Result<Self, FormatError> {
        let header = read_at::<HEADER_LEN>(src, base)?;
        if header[..8] != MAGIC {
            return Err(malformed("no IVFC header"));
        }
        let master_len = u64::from(le32(&header, 0x08)?);
        let mut block_log = [0u32; 3];
        for (level, log) in block_log.iter_mut().enumerate() {
            *log = le32(&header, 0x1C + level * 0x18)?;
            if !(6..=24).contains(log) {
                return Err(malformed("bad hash block size"));
            }
        }
        let [b1, _, b3] = block_log.map(|l| 1u64 << l);
        let level1_size = le64(&header, 0x14)?;
        let level2_size = le64(&header, 0x2C)?;
        let level3_size = le64(&header, 0x44)?;
        if [level1_size, level2_size, level3_size, master_len]
            .iter()
            .any(|&s| s > size)
        {
            return Err(malformed("level larger than the RomFS"));
        }
        let level3_offset = align_up(MASTER_HASH_OFFSET + master_len, b3);
        let level1_offset = level3_offset + align_up(level3_size, b3);
        let level2_offset = level1_offset + align_up(level1_size, b1);
        if level2_offset + level2_size > size {
            return Err(malformed("hash levels extend past the RomFS"));
        }

        let l3 = read_at::<LEVEL3_HEADER_LEN>(src, base + level3_offset)?;
        if le32(&l3, 0)? as usize != LEVEL3_HEADER_LEN {
            return Err(malformed("bad level 3 header"));
        }
        let table = |at: usize| -> Result<(u64, u64), FormatError> {
            let (offset, len) = (u64::from(le32(&l3, at)?), u64::from(le32(&l3, at + 4)?));
            if len > MAX_TABLE || offset + len > level3_size {
                return Err(malformed("table out of bounds"));
            }
            Ok((offset, len))
        };
        let (dir_meta_offset, dir_meta_len) = table(0x0C)?;
        let (file_meta_offset, file_meta_len) = table(0x1C)?;
        let file_data_offset = u64::from(le32(&l3, 0x24)?);
        if file_data_offset > level3_size {
            return Err(malformed("file data out of bounds"));
        }
        let dir_meta = read_vec_at(src, base + level3_offset + dir_meta_offset, to_usize(dir_meta_len)?)?;
        let file_meta = read_vec_at(src, base + level3_offset + file_meta_offset, to_usize(file_meta_len)?)?;
        let files = walk(&dir_meta, &file_meta, level3_size - file_data_offset)?;
        let index = files.iter().enumerate().map(|(i, f)| (f.path.clone(), i)).collect();
        Ok(Self {
            header,
            block_log,
            level3_offset,
            level3_size,
            level2_offset,
            level2_size,
            file_meta_offset,
            file_meta,
            file_data_offset,
            files,
            index,
        })
    }

    /// Offset of a file's data from the start of the RomFS.
    pub(crate) fn data_offset(&self, entry: &Entry) -> u64 {
        self.level3_offset + self.file_data_offset + entry.offset
    }

    /// Plans a RomFS with `changes` (file index → new contents) applied.
    pub(crate) fn rebuild<'a>(
        &'a self,
        src: &mut (impl RomSource + ?Sized),
        base: u64,
        changes: &'a BTreeMap<usize, Vec<u8>>,
    ) -> Result<Rebuilt<'a>, FormatError> {
        let [b1, b2, b3] = self.block_log.map(|l| 1u64 << l);
        let level3 = self.new_level3(base, changes);
        let level3_size = level3.size;
        let level2 = self.new_level2(src, base, &level3)?;
        let level1 = hash_blocks(&level2, to_usize(b2)?);
        let master = hash_blocks(&level1, to_usize(b1)?);

        let mut header = self.header;
        let level2_logical = align_up(level1.len() as u64, b1);
        let level3_logical = align_up(level2_logical + level2.len() as u64, b2);
        let fields: [(usize, u64); 6] = [
            (0x0C, 0),
            (0x14, level1.len() as u64),
            (0x24, level2_logical),
            (0x2C, level2.len() as u64),
            (0x3C, level3_logical),
            (0x44, level3_size),
        ];
        header[0x08..0x0C].copy_from_slice(&u32::try_from(master.len()).unwrap_or(u32::MAX).to_le_bytes());
        for (at, value) in fields {
            header[at..at + 8].copy_from_slice(&value.to_le_bytes());
        }
        let level3_offset = align_up(MASTER_HASH_OFFSET + master.len() as u64, b3);
        let mut prefix = vec![0u8; to_usize(level3_offset)?];
        prefix[..HEADER_LEN].copy_from_slice(&header);
        prefix[0x60..0x60 + master.len()].copy_from_slice(&master);

        let mut tail = vec![0u8; to_usize(align_up(level3_size, b3) - level3_size)?];
        tail.extend_from_slice(&level1);
        tail.resize(
            tail.len() + to_usize(align_up(level1.len() as u64, b1))? - level1.len(),
            0,
        );
        tail.extend_from_slice(&level2);
        tail.resize(
            tail.len() + to_usize(align_up(level2.len() as u64, b2))? - level2.len(),
            0,
        );
        let unpadded = level3_offset + level3_size + tail.len() as u64;
        tail.resize(tail.len() + to_usize(align_up(unpadded, MEDIA_UNIT) - unpadded)?, 0);
        Ok(Rebuilt {
            prefix,
            master_len: master.len() as u64,
            level3,
            tail,
        })
    }

    /// Level 3 with the changed files: in their old slot when they fit, else after the end of the file data.
    fn new_level3<'a>(&'a self, base: u64, changes: &'a BTreeMap<usize, Vec<u8>>) -> Level3<'a> {
        let data_end = self.level3_size - self.file_data_offset;
        let mut order: Vec<usize> = (0..self.files.len()).collect();
        order.sort_by_key(|&i| (self.files[i].offset, i));
        let mut placed: Vec<(u64, u64)> = self.files.iter().map(|f| (f.offset, f.len)).collect();
        let mut cursor = align_up(data_end, FILE_ALIGN);
        let mut new_data_end = data_end;
        for (k, &i) in order.iter().enumerate() {
            let Some(data) = changes.get(&i) else { continue };
            let old = self.files[i].offset;
            let slot_end = order.get(k + 1).map_or(data_end, |&n| self.files[n].offset).max(old);
            let len = data.len() as u64;
            let offset = if old + len <= slot_end {
                old
            } else {
                let at = cursor;
                cursor = align_up(at + len, FILE_ALIGN);
                new_data_end = new_data_end.max(at + len);
                at
            };
            placed[i] = (offset, len);
        }

        let mut file_meta = self.file_meta.clone();
        for (entry, &(offset, len)) in self.files.iter().zip(&placed) {
            file_meta[entry.meta + 0x08..entry.meta + 0x10].copy_from_slice(&offset.to_le_bytes());
            file_meta[entry.meta + 0x10..entry.meta + 0x18].copy_from_slice(&len.to_le_bytes());
        }
        let mut overlays: Vec<(u64, Cow<'a, [u8]>)> = changes
            .iter()
            .filter(|(_, data)| !data.is_empty())
            .map(|(&i, data)| (self.file_data_offset + placed[i].0, Cow::Borrowed(data.as_slice())))
            .collect();
        if file_meta != self.file_meta {
            overlays.push((self.file_meta_offset, Cow::Owned(file_meta)));
        }
        overlays.sort_by_key(|(at, _)| *at);
        Level3 {
            original_offset: base + self.level3_offset,
            original_size: self.level3_size,
            size: self.file_data_offset + new_data_end,
            overlays,
        }
    }

    /// Level 2 of `level3`: the hashes of unchanged blocks come from the original level 2.
    fn new_level2(
        &self,
        src: &mut (impl RomSource + ?Sized),
        base: u64,
        level3: &Level3<'_>,
    ) -> Result<Vec<u8>, FormatError> {
        let b3 = 1u64 << self.block_log[2];
        let block_len = to_usize(b3)?;
        let block_count = to_usize(level3.size.div_ceil(b3))?;
        let original_count = to_usize(self.level3_size.div_ceil(b3))?;
        let mut dirty = vec![false; block_count];
        for (at, data) in &level3.overlays {
            let first = to_usize(at / b3)?;
            let last = to_usize((at + data.len() as u64 - 1) / b3)?;
            dirty[first..=last].fill(true);
        }
        if level3.size != self.level3_size {
            // The old last block was padded with zeros where data may follow now.
            if let Some(d) = dirty.get_mut(to_usize(self.level3_size / b3)?) {
                *d = true;
            }
        }
        dirty.iter_mut().skip(original_count).for_each(|d| *d = true);
        let original = read_vec_at(
            src,
            base + self.level2_offset,
            to_usize(self.level2_size.min((original_count * HASH_LEN) as u64))?,
        )?;
        let chunk_blocks = (to_usize(COPY_CHUNK)? / block_len).max(1);
        let mut level2 = vec![0u8; block_count * HASH_LEN];
        let mut buf = Vec::new();
        let mut block = 0;
        while block < block_count {
            if !dirty[block] {
                let at = block * HASH_LEN;
                let hash = original
                    .get(at..at + HASH_LEN)
                    .ok_or_else(|| malformed("level 2 shorter than level 3"))?;
                level2[at..at + HASH_LEN].copy_from_slice(hash);
                block += 1;
                continue;
            }
            // A run of changed blocks, read at most a copy chunk at a time.
            let mut end = block + 1;
            while end < block_count && dirty[end] && end - block < chunk_blocks {
                end += 1;
            }
            buf.resize((end - block) * block_len, 0);
            level3.read(src, (block * block_len) as u64, &mut buf)?;
            for (k, chunk) in buf.chunks(block_len).enumerate() {
                let at = (block + k) * HASH_LEN;
                level2[at..at + HASH_LEN].copy_from_slice(&sha256(chunk));
            }
            block = end;
        }
        Ok(level2)
    }

    /// Checks the whole hash tree of the RomFS at `base` against its data.
    pub(crate) fn verify(&self, src: &mut (impl RomSource + ?Sized), base: u64) -> Result<bool, FormatError> {
        let [b1, b2, b3] = self.block_log.map(|l| 1usize << l);
        let master_len = to_usize(u64::from(le32(&self.header, 0x08)?))?;
        let level1_size = le64(&self.header, 0x14)?;
        let level1_offset = self.level3_offset + align_up(self.level3_size, b3 as u64);
        let master = read_vec_at(src, base + MASTER_HASH_OFFSET, master_len)?;
        let level1 = read_vec_at(src, base + level1_offset, to_usize(level1_size)?)?;
        let level2 = read_vec_at(src, base + self.level2_offset, to_usize(self.level2_size)?)?;
        if !hash_blocks(&level1, b1).starts_with(&master) || !hash_blocks(&level2, b2).starts_with(&level1) {
            return Ok(false);
        }
        let level3 = Level3 {
            original_offset: base + self.level3_offset,
            original_size: self.level3_size,
            size: self.level3_size,
            overlays: Vec::new(),
        };
        let chunk_blocks = to_usize(COPY_CHUNK)? / b3;
        let blocks = to_usize(self.level3_size.div_ceil(b3 as u64))?;
        let mut buf = Vec::new();
        for first in (0..blocks).step_by(chunk_blocks) {
            let n = chunk_blocks.min(blocks - first);
            buf.resize(n * b3, 0);
            level3.read(src, (first * b3) as u64, &mut buf)?;
            let hashes = hash_blocks(&buf, b3);
            if level2.get(first * HASH_LEN..(first + n) * HASH_LEN) != Some(hashes.as_slice()) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Level 3 of a rebuilt RomFS: the original bytes, with replaced pieces laid over them and zeros past the original
/// end.
#[derive(Debug)]
struct Level3<'a> {
    /// Absolute offset of the original level 3 in the source.
    original_offset: u64,
    original_size: u64,
    size: u64,
    /// Replaced pieces: offset in level 3 and contents, sorted and not overlapping.
    overlays: Vec<(u64, Cow<'a, [u8]>)>,
}

impl Level3<'_> {
    /// Fills `buf` with the bytes at `start`; bytes past the end read as zeros.
    fn read(&self, src: &mut (impl RomSource + ?Sized), start: u64, buf: &mut [u8]) -> Result<(), FormatError> {
        let end = start + buf.len() as u64;
        buf.fill(0);
        if start < self.original_size {
            let n = to_usize(self.original_size.min(end) - start)?;
            read_into(src, self.original_offset + start, &mut buf[..n])?;
        }
        for (at, data) in &self.overlays {
            let data_end = at + data.len() as u64;
            if data_end <= start || *at >= end {
                continue;
            }
            let from = start.max(*at);
            let to = end.min(data_end);
            let dst = to_usize(from - start)?;
            let src_at = to_usize(from - at)?;
            let n = to_usize(to - from)?;
            buf[dst..dst + n].copy_from_slice(&data[src_at..src_at + n]);
        }
        Ok(())
    }
}

/// A planned RomFS, written by [`Rebuilt::write`].
#[derive(Debug)]
pub(crate) struct Rebuilt<'a> {
    /// Header, master hash and the padding up to level 3.
    prefix: Vec<u8>,
    master_len: u64,
    level3: Level3<'a>,
    /// Padding after level 3, level 1 and level 2.
    tail: Vec<u8>,
}

impl Rebuilt<'_> {
    /// Total length of the RomFS, a multiple of the media unit.
    pub(crate) fn len(&self) -> u64 {
        self.prefix.len() as u64 + self.level3.size + self.tail.len() as u64
    }

    /// Length of the header and master hash, the part the superblock hash must cover.
    pub(crate) fn hashed_len(&self) -> u64 {
        MASTER_HASH_OFFSET + self.master_len
    }

    /// The first `n` bytes of the RomFS.
    pub(crate) fn head(&self, src: &mut (impl RomSource + ?Sized), n: usize) -> Result<Vec<u8>, FormatError> {
        let mut out = self.prefix[..n.min(self.prefix.len())].to_vec();
        if out.len() < n {
            let mut buf = vec![0u8; (n - out.len()).min(to_usize(self.level3.size)?)];
            self.level3.read(src, 0, &mut buf)?;
            out.extend_from_slice(&buf);
        }
        if out.len() < n {
            let k = (n - out.len()).min(self.tail.len());
            out.extend_from_slice(&self.tail[..k]);
        }
        Ok(out)
    }

    /// Writes the RomFS.
    pub(crate) fn write(
        &self,
        src: &mut (impl RomSource + ?Sized),
        out: &mut (impl Write + ?Sized),
    ) -> Result<(), FormatError> {
        out.write_all(&self.prefix)?;
        let mut buf = vec![0u8; to_usize(COPY_CHUNK.min(self.level3.size))?];
        let mut at = 0;
        while at < self.level3.size {
            let n = to_usize((self.level3.size - at).min(COPY_CHUNK))?;
            self.level3.read(src, at, &mut buf[..n])?;
            out.write_all(&buf[..n])?;
            at += n as u64;
        }
        out.write_all(&self.tail)?;
        Ok(())
    }
}

/// Lists the files of the directory tree, depth first.
fn walk(dir_meta: &[u8], file_meta: &[u8], data_len: u64) -> Result<Vec<Entry>, FormatError> {
    // Every directory entry is at least 0x18 bytes and every file entry 0x20, which bounds a loop in a forged tree.
    let max_dirs = dir_meta.len() / 0x18 + 1;
    let max_files = file_meta.len() / 0x20 + 1;
    let mut files = Vec::new();
    let mut stack = vec![(0u32, String::new())];
    let mut dirs = 0;
    while let Some((offset, parent)) = stack.pop() {
        dirs += 1;
        if dirs > max_dirs {
            return Err(malformed("directory tree loops"));
        }
        let at = offset as usize;
        let sibling = le32(dir_meta, at + 0x04)?;
        let child = le32(dir_meta, at + 0x08)?;
        let mut file = le32(dir_meta, at + 0x0C)?;
        let name = name_at(dir_meta, at + 0x18, le32(dir_meta, at + 0x14)?)?;
        let path = if name.is_empty() {
            parent.clone()
        } else {
            format!("{parent}{name}/")
        };
        if sibling != UNUSED {
            stack.push((sibling, parent));
        }
        if child != UNUSED {
            stack.push((child, path.clone()));
        }
        while file != UNUSED {
            if files.len() >= max_files {
                return Err(malformed("file list loops"));
            }
            let at = file as usize;
            let offset = le64(file_meta, at + 0x08)?;
            let len = le64(file_meta, at + 0x10)?;
            if offset.checked_add(len).is_none_or(|end| end > data_len) {
                return Err(malformed("file data out of bounds"));
            }
            let name = name_at(file_meta, at + 0x20, le32(file_meta, at + 0x1C)?)?;
            files.push(Entry {
                path: format!("{path}{name}"),
                meta: at,
                offset,
                len,
            });
            file = le32(file_meta, at + 0x04)?;
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    const FILES: &[(&str, &[u8])] = &[
        ("a/0/0/0", b"first file"),
        ("a/0/0/1", &[7; 300]),
        ("a/0/1/0", b"x"),
        ("DllBattle.cro", b"battle code"),
        ("empty", b""),
    ];

    fn parse(img: &[u8]) -> RomFs {
        RomFs::parse(&mut Cursor::new(img), 0, img.len() as u64).expect("parses")
    }

    fn read(fs: &RomFs, img: &[u8], path: &str) -> Vec<u8> {
        let e = &fs.files[fs.index[path]];
        let at = usize::try_from(fs.data_offset(e)).expect("small");
        img[at..at + usize::try_from(e.len).expect("small")].to_vec()
    }

    fn rebuild(img: &[u8], changes: &BTreeMap<usize, Vec<u8>>) -> Vec<u8> {
        let fs = parse(img);
        let mut src = Cursor::new(img);
        let plan = fs.rebuild(&mut src, 0, changes).expect("plans");
        let mut out = Vec::new();
        plan.write(&mut src, &mut out).expect("writes");
        assert_eq!(out.len() as u64, plan.len());
        out
    }

    #[test]
    fn lists_files_by_path() {
        let img = crate::ctr::testing::romfs(FILES, 7);
        let fs = parse(&img);
        let names: Vec<&str> = fs.index.keys().map(String::as_str).collect();
        assert_eq!(names, ["DllBattle.cro", "a/0/0/0", "a/0/0/1", "a/0/1/0", "empty"]);
        assert_eq!(read(&fs, &img, "a/0/0/1"), [7; 300]);
        assert_eq!(read(&fs, &img, "empty"), b"");
        assert!(fs.verify(&mut Cursor::new(&img), 0).expect("reads"));
    }

    #[test]
    fn rebuild_without_changes_is_identical() {
        let img = crate::ctr::testing::romfs(FILES, 7);
        assert_eq!(rebuild(&img, &BTreeMap::new()), img);
    }

    #[test]
    fn rebuild_keeps_files_in_place_or_moves_them_to_the_end() {
        let img = crate::ctr::testing::romfs(FILES, 7);
        let fs = parse(&img);
        let mut changes = BTreeMap::new();
        changes.insert(fs.index["a/0/0/0"], b"short".to_vec());
        changes.insert(fs.index["a/0/1/0"], vec![9; 1000]);
        let out = rebuild(&img, &changes);
        let new = parse(&out);
        assert!(new.verify(&mut Cursor::new(&out), 0).expect("reads"));
        assert_eq!(read(&new, &out, "a/0/0/0"), b"short");
        assert_eq!(read(&new, &out, "a/0/1/0"), [9; 1000]);
        assert_eq!(read(&new, &out, "a/0/0/1"), [7; 300]);
        assert_eq!(read(&new, &out, "DllBattle.cro"), b"battle code");
        let old = &fs.files[fs.index["a/0/0/0"]];
        assert_eq!(new.files[new.index["a/0/0/0"]].offset, old.offset);
        assert!(new.files[new.index["a/0/1/0"]].offset >= fs.level3_size - fs.file_data_offset);
        // Rebuilding the result with the original contents keeps the new layout.
        let mut undo = BTreeMap::new();
        undo.insert(new.index["a/0/0/0"], b"first file".to_vec());
        let back = rebuild(&out, &undo);
        let back_fs = parse(&back);
        assert_eq!(read(&back_fs, &back, "a/0/0/0"), b"first file");
        assert!(back_fs.verify(&mut Cursor::new(&back), 0).expect("reads"));
    }

    #[test]
    fn grows_the_hash_tree_for_large_files() {
        let img = crate::ctr::testing::romfs(FILES, 6);
        let fs = parse(&img);
        let mut changes = BTreeMap::new();
        changes.insert(fs.index["empty"], (0..200_000u32).map(|i| (i % 251) as u8).collect());
        let out = rebuild(&img, &changes);
        let new = parse(&out);
        assert!(new.verify(&mut Cursor::new(&out), 0).expect("reads"));
        assert_eq!(read(&new, &out, "empty").len(), 200_000);
    }

    #[test]
    fn verify_finds_changed_data() {
        let mut img = crate::ctr::testing::romfs(FILES, 7);
        let fs = parse(&img);
        let e = &fs.files[fs.index["a/0/0/1"]];
        let at = usize::try_from(fs.data_offset(e)).expect("small") + 100;
        img[at] ^= 1;
        assert!(!fs.verify(&mut Cursor::new(&img), 0).expect("reads"));
    }

    #[test]
    fn rejects_garbage() {
        let mut img = crate::ctr::testing::romfs(FILES, 7);
        assert!(RomFs::parse(&mut Cursor::new(vec![0u8; 0x400]), 0, 0x400).is_err());
        // Scramble level 3 as an encrypted image would look.
        for (i, b) in img.iter_mut().enumerate().skip(0x80) {
            *b ^= (i * 37 % 251).to_le_bytes()[0] | 1;
        }
        assert!(RomFs::parse(&mut Cursor::new(&img), 0, img.len() as u64).is_err());
    }
}
