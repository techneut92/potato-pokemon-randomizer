//! Small synthetic 3DS images for tests: RomFS, GARC, CXI and CCI. They contain no copyrighted data.

use std::collections::BTreeMap;

use sha2::{Digest, Sha256};

use super::romfs::{HEADER_LEN, LEVEL3_HEADER_LEN, MAGIC, UNUSED, hash_blocks};

/// A RomFS with the given files (path, contents). Hash blocks are 2^`block_log` bytes and files are 16-byte
/// aligned. The lookup hash tables are empty, as the reader does not need them.
///
/// # Panics
/// When the image would not fit in 32-bit offsets.
#[must_use]
pub fn romfs(files: &[(&str, &[u8])], block_log: u32) -> Vec<u8> {
    let mut root = Dir::default();
    for (path, data) in files {
        let mut parts: Vec<&str> = path.split('/').collect();
        let name = parts.pop().expect("a name");
        let mut dir = &mut root;
        for p in parts {
            dir = dir.dirs.entry(p.to_owned()).or_default();
        }
        dir.files.push((name.to_owned(), data.to_vec()));
    }
    let mut tables = Tables::default();
    tables.emit(&root, "", 0);
    let Tables {
        dir_meta,
        file_meta,
        data,
    } = tables;

    // Level 3: header, directory hash table, directory table, file hash table, file table, file data.
    let mut l3 = vec![0u8; LEVEL3_HEADER_LEN];
    let dir_hash = l3.len();
    l3.extend_from_slice(&UNUSED.to_le_bytes());
    let dir_meta_at = l3.len();
    l3.extend_from_slice(&dir_meta);
    let file_hash = l3.len();
    l3.extend_from_slice(&UNUSED.to_le_bytes());
    let file_meta_at = l3.len();
    l3.extend_from_slice(&file_meta);
    l3.resize(l3.len().next_multiple_of(16), 0);
    let data_at = l3.len();
    l3.extend_from_slice(&data);
    let words = [
        LEVEL3_HEADER_LEN,
        dir_hash,
        4,
        dir_meta_at,
        dir_meta.len(),
        file_hash,
        4,
        file_meta_at,
        file_meta.len(),
        data_at,
    ];
    for (i, w) in words.into_iter().enumerate() {
        l3[i * 4..i * 4 + 4].copy_from_slice(&le32(w));
    }

    let block = 1usize << block_log;
    let level2 = hash_blocks(&l3, block);
    let level1 = hash_blocks(&level2, block);
    let master = hash_blocks(&level1, block);
    let mut header = [0u8; HEADER_LEN];
    header[..8].copy_from_slice(&MAGIC);
    header[0x08..0x0C].copy_from_slice(&le32(master.len()));
    let level2_logical = level1.len().next_multiple_of(block);
    let level3_logical = (level2_logical + level2.len()).next_multiple_of(block);
    let level_table = [
        (0, level1.len()),
        (level2_logical, level2.len()),
        (level3_logical, l3.len()),
    ];
    for (level, (logical, size)) in level_table.into_iter().enumerate() {
        let at = 0x0C + level * 0x18;
        header[at..at + 8].copy_from_slice(&(logical as u64).to_le_bytes());
        header[at + 8..at + 16].copy_from_slice(&(size as u64).to_le_bytes());
        header[at + 16..at + 20].copy_from_slice(&block_log.to_le_bytes());
    }
    header[0x58..0x5C].copy_from_slice(&le32(HEADER_LEN));
    let mut out = header.to_vec();
    out.resize(0x60, 0);
    out.extend_from_slice(&master);
    for part in [&l3, &level1, &level2] {
        out.resize(out.len().next_multiple_of(block), 0);
        out.extend_from_slice(part);
    }
    out.resize(out.len().next_multiple_of(block), 0);
    out.resize(out.len().next_multiple_of(MEDIA), 0);
    out
}

fn le32(v: usize) -> [u8; 4] {
    u32::try_from(v).expect("fits in 32 bits").to_le_bytes()
}

#[derive(Default)]
struct Dir {
    dirs: BTreeMap<String, Dir>,
    files: Vec<(String, Vec<u8>)>,
}

#[derive(Default)]
struct Tables {
    dir_meta: Vec<u8>,
    file_meta: Vec<u8>,
    data: Vec<u8>,
}

fn name_bytes(name: &str) -> (u32, Vec<u8>) {
    let mut b: Vec<u8> = name.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let len = u32::try_from(b.len()).expect("short name");
    b.resize(b.len().next_multiple_of(4), 0);
    (len, b)
}

impl Tables {
    /// Appends the entry of `dir` and everything below it; returns the entry's offset.
    fn emit(&mut self, dir: &Dir, name: &str, parent: u32) -> u32 {
        let me = u32::try_from(self.dir_meta.len()).expect("small table");
        let at = self.dir_meta.len();
        let (name_len, name) = name_bytes(name);
        for word in [parent, UNUSED, UNUSED, UNUSED, UNUSED, name_len] {
            self.dir_meta.extend_from_slice(&word.to_le_bytes());
        }
        self.dir_meta.extend_from_slice(&name);
        let mut prev_file: Option<usize> = None;
        for (file_name, contents) in &dir.files {
            self.data.resize(self.data.len().next_multiple_of(16), 0);
            let f = self.file_meta.len();
            let (name_len, name) = name_bytes(file_name);
            self.file_meta.extend_from_slice(&me.to_le_bytes());
            self.file_meta.extend_from_slice(&UNUSED.to_le_bytes());
            self.file_meta
                .extend_from_slice(&(self.data.len() as u64).to_le_bytes());
            self.file_meta.extend_from_slice(&(contents.len() as u64).to_le_bytes());
            self.file_meta.extend_from_slice(&UNUSED.to_le_bytes());
            self.file_meta.extend_from_slice(&name_len.to_le_bytes());
            self.file_meta.extend_from_slice(&name);
            self.data.extend_from_slice(contents);
            let link = le32(f);
            match prev_file {
                None => self.dir_meta[at + 0x0C..at + 0x10].copy_from_slice(&link),
                Some(p) => self.file_meta[p + 4..p + 8].copy_from_slice(&link),
            }
            prev_file = Some(f);
        }
        let mut prev_dir: Option<usize> = None;
        for (dir_name, sub) in &dir.dirs {
            let d = self.emit(sub, dir_name, me);
            match prev_dir {
                None => self.dir_meta[at + 0x08..at + 0x0C].copy_from_slice(&d.to_le_bytes()),
                Some(p) => self.dir_meta[p + 4..p + 8].copy_from_slice(&d.to_le_bytes()),
            }
            prev_dir = Some(d as usize);
        }
        me
    }
}

/// A version 6 GARC with 4-byte padding (0xFF) and one sub-file (number 0) per file, stored as given.
#[must_use]
pub fn garc(files: &[&[u8]]) -> Vec<u8> {
    garc_with(
        &files.iter().map(|f| vec![(0u8, f.to_vec())]).collect::<Vec<_>>(),
        0x0600,
    )
}

/// A GARC of `version` (0x0400 or 0x0600) whose files have the given sub-files (number, contents).
///
/// # Panics
/// When the archive would not fit in 32-bit offsets.
#[must_use]
pub fn garc_with(files: &[Vec<(u8, Vec<u8>)>], version: u16) -> Vec<u8> {
    let mut offsets = b"OTAF".to_vec();
    offsets.extend_from_slice(&le32(12 + files.len() * 4));
    offsets.extend_from_slice(&u16::try_from(files.len()).expect("few files").to_le_bytes());
    offsets.extend_from_slice(&0xFFFFu16.to_le_bytes());
    let mut fatb_entries = Vec::new();
    let mut fimb = Vec::new();
    let (mut largest, mut largest_padded) = (0, 0);
    for subs in files {
        offsets.extend_from_slice(&le32(fatb_entries.len()));
        let vector = subs.iter().fold(0u32, |v, (bit, _)| v | 1 << bit);
        fatb_entries.extend_from_slice(&vector.to_le_bytes());
        for (_, data) in subs {
            let start = fimb.len();
            fimb.extend_from_slice(data);
            fimb.resize(fimb.len().next_multiple_of(4), 0xFF);
            largest = largest.max(data.len());
            largest_padded = largest_padded.max(data.len().next_multiple_of(4));
            fatb_entries.extend_from_slice(&le32(start));
            fatb_entries.extend_from_slice(&le32(fimb.len()));
            fatb_entries.extend_from_slice(&le32(data.len()));
        }
    }
    let mut fatb = b"BTAF".to_vec();
    fatb.extend_from_slice(&le32(12 + fatb_entries.len()));
    fatb.extend_from_slice(&le32(files.len()));
    fatb.extend_from_slice(&fatb_entries);
    let header_len = if version == 0x0400 { 0x1C } else { 0x24 };
    let data_offset = header_len + offsets.len() + fatb.len() + 12;
    let mut out = b"CRAG".to_vec();
    out.extend_from_slice(&le32(header_len));
    out.extend_from_slice(&0xFEFFu16.to_le_bytes());
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&le32(4));
    out.extend_from_slice(&le32(data_offset));
    out.extend_from_slice(&le32(data_offset + fimb.len()));
    if version == 0x0400 {
        out.extend_from_slice(&le32(largest));
    } else {
        out.extend_from_slice(&le32(largest_padded));
        out.extend_from_slice(&le32(largest));
        out.extend_from_slice(&le32(4));
    }
    out.extend_from_slice(&offsets);
    out.extend_from_slice(&fatb);
    out.extend_from_slice(b"BMIF");
    out.extend_from_slice(&le32(12));
    out.extend_from_slice(&le32(fimb.len()));
    out.extend_from_slice(&fimb);
    out
}

/// What goes into a synthetic CXI.
#[derive(Debug, Clone)]
pub struct Cxi<'a> {
    /// Product code, e.g. `CTR-P-EKJA`.
    pub product_code: &'a str,
    /// Title ID.
    pub title_id: u64,
    /// The decompressed executable.
    pub code: &'a [u8],
    /// Whether `.code` is stored compressed.
    pub compress_code: bool,
    /// RomFS files (path, contents).
    pub files: &'a [(&'a str, &'a [u8])],
    /// Whether the "no crypto" flag is set.
    pub decrypted: bool,
}

const MEDIA: usize = 0x200;

fn put_mu(h: &mut [u8], at: usize, bytes: usize) {
    h[at..at + 4].copy_from_slice(&le32(bytes / MEDIA));
}

/// A CXI laid out like a real one: header, extended header (0x800 bytes), a plain region, ExeFS (`.code`, `icon`)
/// and a RomFS with 4 KiB hash blocks, with every hash filled in.
///
/// # Panics
/// When the image would not fit in 32-bit offsets.
#[must_use]
pub fn cxi(spec: &Cxi<'_>) -> Vec<u8> {
    let code = if spec.compress_code {
        crate::blz::compress(spec.code, 0, crate::blz::Mode::Best)
    } else {
        spec.code.to_vec()
    };
    let mut icon = vec![0u8; 0x36C0];
    icon[..4].copy_from_slice(b"SMDH");
    let exefs_files: [(&[u8; 8], &[u8]); 2] = [(b".code\0\0\0", &code), (b"icon\0\0\0\0", &icon)];
    let mut exefs = vec![0u8; 0x200];
    let mut offset = 0;
    for (i, (name, data)) in exefs_files.iter().enumerate() {
        exefs[i * 0x10..i * 0x10 + 8].copy_from_slice(*name);
        exefs[i * 0x10 + 8..i * 0x10 + 12].copy_from_slice(&le32(offset));
        exefs[i * 0x10 + 12..i * 0x10 + 16].copy_from_slice(&le32(data.len()));
        let h = 0x200 - (i + 1) * 0x20;
        exefs[h..h + 0x20].copy_from_slice(&Sha256::digest(data));
        exefs.resize(0x200 + offset, 0);
        exefs.extend_from_slice(data);
        offset = (offset + data.len()).next_multiple_of(MEDIA);
    }
    exefs.resize(exefs.len().next_multiple_of(MEDIA), 0);
    let romfs = romfs(spec.files, 12);

    let plain_at = 0xA00;
    let exefs_at = plain_at + MEDIA;
    let romfs_at = (exefs_at + exefs.len()).next_multiple_of(0x1000);
    let total = romfs_at + romfs.len();
    let mut img = vec![0u8; total];
    let h = &mut img[..0x200];
    h[..0x100].fill(0xA5); // a stand-in signature
    h[0x100..0x104].copy_from_slice(b"NCCH");
    put_mu(h, 0x104, total);
    h[0x118..0x120].copy_from_slice(&spec.title_id.to_le_bytes());
    h[0x150..0x150 + spec.product_code.len()].copy_from_slice(spec.product_code.as_bytes());
    h[0x180..0x184].copy_from_slice(&le32(0x400));
    h[0x18F] = if spec.decrypted { 0x04 } else { 0x00 };
    put_mu(h, 0x190, plain_at);
    put_mu(h, 0x194, MEDIA);
    put_mu(h, 0x1A0, exefs_at);
    put_mu(h, 0x1A4, exefs.len());
    put_mu(h, 0x1A8, MEDIA);
    put_mu(h, 0x1B0, romfs_at);
    put_mu(h, 0x1B4, romfs.len());
    put_mu(h, 0x1B8, MEDIA);
    h[0x1C0..0x1E0].copy_from_slice(&Sha256::digest(&exefs[..MEDIA]));
    h[0x1E0..0x200].copy_from_slice(&Sha256::digest(&romfs[..MEDIA]));
    img[0x200 + 0x0D] = u8::from(spec.compress_code);
    img[plain_at..plain_at + 12].copy_from_slice(b"plain region");
    img[exefs_at..exefs_at + exefs.len()].copy_from_slice(&exefs);
    img[romfs_at..].copy_from_slice(&romfs);
    img
}

/// A cartridge image (CCI) with `ncch` as its first partition.
///
/// # Panics
/// When the image would not fit in 32-bit offsets.
#[must_use]
pub fn cci(ncch: &[u8]) -> Vec<u8> {
    let mut img = vec![0u8; 0x4000];
    img[0x100..0x104].copy_from_slice(b"NCSD");
    put_mu(&mut img, 0x104, 0x4000 + ncch.len());
    put_mu(&mut img, 0x120, 0x4000);
    put_mu(&mut img, 0x124, ncch.len());
    img.extend_from_slice(ncch);
    img
}

/// An installable archive (CIA) with `ncch` as its only content and `version` in its title metadata, which is
/// signed (with a stand-in signature) by RSA-2048 and SHA-256. Certificates and ticket are zeros.
///
/// # Panics
/// When the archive would not fit in 32-bit sizes.
#[must_use]
pub fn cia(ncch: &[u8], version: u16) -> Vec<u8> {
    const HEADER: usize = 0x2020;
    const CERTS: usize = 0xA00;
    const TICKET: usize = 0x350;
    // Signature type and signature with padding, header, 64 content info records and one content chunk record.
    const TMD: usize = 4 + 0x100 + 0x3C + 0xC4 + 0x24 * 64 + 0x30;
    let align = |n: usize| n.next_multiple_of(64);
    let mut img = vec![0u8; HEADER];
    img[..4].copy_from_slice(&le32(HEADER));
    img[0x08..0x0C].copy_from_slice(&le32(CERTS));
    img[0x0C..0x10].copy_from_slice(&le32(TICKET));
    img[0x10..0x14].copy_from_slice(&le32(TMD));
    img[0x18..0x20].copy_from_slice(&(ncch.len() as u64).to_le_bytes());
    img[0x20] = 0x80; // content 0 is present
    let tmd_at = align(align(align(HEADER) + CERTS) + TICKET);
    let content_at = align(tmd_at + TMD);
    img.resize(content_at, 0);
    img[tmd_at..tmd_at + 4].copy_from_slice(&0x0001_0004_u32.to_be_bytes());
    img[tmd_at + 4..tmd_at + 4 + 0x100].fill(0xA5);
    let header = tmd_at + 4 + 0x100 + 0x3C;
    img[header + 0x9C..header + 0x9E].copy_from_slice(&version.to_be_bytes());
    img.extend_from_slice(ncch);
    img
}
