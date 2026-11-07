//! The 3DS container layer against real decrypted Gen 6 and Gen 7 images from `PPR_TEST_ROMS`: reading the
//! executable, RomFS files, GARC archives and text, and saving with and without changes.
//!
//! Saved images are not written to disk: they go into an [`Overlay`] that keeps only the 64 KiB chunks that differ
//! from the source, so even a 4 GiB image needs no temporary file. Run in release mode:
//! `PPR_TEST_ROMS=… cargo test --release -p ppr-romio --test ctr_real_roms -- --ignored`.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use ppr_formats::ctr::{Decompression, Garc, Ncch};
use ppr_formats::{blz, crc32};
use ppr_model::Generation;
use ppr_romio::entries::{RomEntry, catalog};
use ppr_romio::{ctr, ctr_text};

/// A real image from `PPR_TEST_ROMS` whose file name contains `name`.
fn real_rom(name: &str) -> Option<PathBuf> {
    let dir = std::env::var_os("PPR_TEST_ROMS")?;
    std::fs::read_dir(dir)
        .expect("PPR_TEST_ROMS is readable")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| {
            let n = p.to_string_lossy();
            n.contains(name) && !n.contains("randomized")
        })
}

fn entry(generation: u8, name: &str) -> &'static RomEntry {
    let generation = Generation::new(generation).expect("valid generation");
    catalog(generation)
        .entries
        .iter()
        .find(|e| e.name == name)
        .expect("ROM entry")
}

/// The path and base-game CRC of a `File<…>` entry, e.g. `<a/2/1/8, [7834E6AE, 7834E6AE]>`.
fn file_entry(entry: &RomEntry, key: &str) -> (String, u32) {
    let raw = entry.value(&format!("File<{key}>")).expect("file entry");
    let inner = raw.trim_matches(['<', '>']);
    let (path, crcs) = inner.split_once(',').expect("path and CRCs");
    let crc = crcs
        .trim()
        .trim_matches(['[', ']'])
        .split(',')
        .next()
        .expect("a CRC")
        .trim();
    (path.trim().to_owned(), u32::from_str_radix(crc, 16).expect("hex CRC"))
}

type Rom = Ncch<BufReader<File>>;

fn open(path: &PathBuf) -> Rom {
    ctr::open(BufReader::with_capacity(1 << 16, File::open(path).expect("readable"))).expect("opens")
}

/// A saved image: the source's game partition with the chunks that differ laid over it.
struct Overlay {
    src: File,
    /// Where the game partition starts in the source.
    base: u64,
    /// Length of the game partition.
    src_len: u64,
    chunks: BTreeMap<u64, Vec<u8>>,
    len: u64,
    pos: u64,
}

const CHUNK: u64 = 1 << 16;

impl Overlay {
    fn new(path: &PathBuf, rom: &Rom) -> Self {
        let mut src = File::open(path).expect("readable");
        let base = rom.identity().ncch_offset;
        let mut size = [0u8; 4];
        src.seek(SeekFrom::Start(base + 0x104)).expect("seek");
        src.read_exact(&mut size).expect("header");
        Self {
            src,
            base,
            src_len: u64::from(u32::from_le_bytes(size)) * 0x200,
            chunks: BTreeMap::new(),
            len: 0,
            pos: 0,
        }
    }

    /// Reads source bytes at `at` of the partition; past its end they are zeros.
    fn read_source(&mut self, at: u64, buf: &mut [u8]) -> io::Result<()> {
        buf.fill(0);
        if at < self.src_len {
            let n = usize::try_from((self.src_len - at).min(buf.len() as u64)).expect("small");
            self.src.seek(SeekFrom::Start(self.base + at))?;
            self.src.read_exact(&mut buf[..n])?;
        }
        Ok(())
    }

    fn changed_bytes(&self) -> u64 {
        self.chunks.len() as u64 * CHUNK
    }
}

impl Write for Overlay {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut done = 0;
        let mut source = vec![0u8; usize::try_from(CHUNK).expect("small")];
        while done < buf.len() {
            let idx = self.pos / CHUNK;
            let off = usize::try_from(self.pos % CHUNK).expect("small");
            let n = (source.len() - off).min(buf.len() - done);
            let piece = &buf[done..done + n];
            if let Some(chunk) = self.chunks.get_mut(&idx) {
                chunk[off..off + n].copy_from_slice(piece);
            } else {
                self.read_source(idx * CHUNK, &mut source)?;
                if &source[off..off + n] != piece {
                    let mut chunk = source.clone();
                    chunk[off..off + n].copy_from_slice(piece);
                    self.chunks.insert(idx, chunk);
                }
            }
            self.pos += n as u64;
            self.len = self.len.max(self.pos);
            done += n;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Read for Overlay {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.pos >= self.len {
            return Ok(0);
        }
        let idx = self.pos / CHUNK;
        let off = usize::try_from(self.pos % CHUNK).expect("small");
        let n = (usize::try_from(CHUNK).expect("small") - off)
            .min(buf.len())
            .min(usize::try_from(self.len - self.pos).unwrap_or(usize::MAX));
        if let Some(chunk) = self.chunks.get(&idx) {
            buf[..n].copy_from_slice(&chunk[off..off + n]);
        } else {
            let at = self.pos;
            self.read_source(at, &mut buf[..n])?;
        }
        self.pos += n as u64;
        Ok(n)
    }
}

impl Seek for Overlay {
    fn seek(&mut self, to: SeekFrom) -> io::Result<u64> {
        self.pos = match to {
            SeekFrom::Start(p) => p,
            SeekFrom::End(d) => self.len.saturating_add_signed(d),
            SeekFrom::Current(d) => self.pos.saturating_add_signed(d),
        };
        Ok(self.pos)
    }
}

/// Everything the Gen 6 and Gen 7 handlers need from the container, on one real image.
fn check(rom_name: &str, generation: u8, entry_name: &str, bulbasaur_stats: [u8; 6]) {
    let Some(path) = real_rom(rom_name) else {
        eprintln!("PPR_TEST_ROMS has no {rom_name}; skipped");
        return;
    };
    let entry = entry(generation, entry_name);
    let mut rom = open(&path);
    assert_eq!(Some(&rom.identity().title_id), entry.title_id.as_ref());
    assert!(rom.is_marked_decrypted());

    // The executable.
    let code = rom.code().expect("code.bin").to_vec();
    assert!(code.len() > 1 << 20, "code.bin is {} bytes", code.len());
    if rom.is_code_compressed() {
        let packed = blz::compress(&code, 0, blz::Mode::Best);
        assert_eq!(
            blz::decompress(&packed).expect("decodes"),
            code,
            "code.bin survives recompression"
        );
    }

    // Every file the ROM entry names has the CRC the entry gives.
    let names: Vec<String> = rom.file_names().map(str::to_owned).collect();
    assert!(names.len() > 100, "{} RomFS files", names.len());
    for (key, value) in entry.keys_with_prefix("File<") {
        let key = &key["File<".len()..key.len() - 1];
        let (file, crc) = file_entry(entry, key);
        let data = rom.read_file(&file).unwrap_or_else(|e| panic!("{file} ({value}): {e}"));
        assert_eq!(crc32(&data), crc, "CRC of {file} ({key})");
    }

    // The personal GARC.
    let (stats_path, _) = file_entry(entry, "PokemonStats");
    let stats_bytes = rom.read_file(&stats_path).expect("stats GARC");
    let stats = Garc::parse(stats_bytes.clone(), &Decompression::All).expect("parses");
    assert!(stats.len() > 721, "{} personal entries", stats.len());
    let bulbasaur = stats.file(1).expect("Bulbasaur");
    assert_eq!(bulbasaur[..6], bulbasaur_stats);
    assert_eq!(
        stats.to_bytes().expect("builds"),
        stats_bytes,
        "unchanged GARC rebuilds identically"
    );

    // Species names.
    let (text_path, _) = file_entry(entry, "TextStrings");
    let texts = Garc::parse(rom.read_file(&text_path).expect("text GARC"), &Decompression::None).expect("parses");
    let names_index = usize::try_from(entry.int("PokemonNamesTextOffset").expect("offset")).expect("index");
    let names_file = texts.file(names_index).expect("names file");
    let species = ctr_text::read_texts(&names_file, true).expect("readable");
    assert_eq!(species[1], "Bulbasaur");
    assert_eq!(species[25], "Pikachu");
    assert_eq!(species[32], "Nidoran\u{2642}");
    let rewritten = ctr_text::write_texts(Some(&names_file), &species, true).expect("writable");
    assert_eq!(ctr_text::read_texts(&rewritten, true).expect("readable"), species);
    let first_difference = rewritten.iter().zip(&names_file).position(|(a, b)| a != b);
    assert_eq!(
        (first_difference, rewritten.len()),
        (None, names_file.len()),
        "unchanged text file is rewritten identically"
    );

    // An unchanged save is the game partition, byte for byte.
    let mut overlay = Overlay::new(&path, &rom);
    let written = rom.write_cxi(&mut overlay).expect("saves");
    assert_eq!(written, overlay.src_len);
    assert_eq!(overlay.changed_bytes(), 0, "unchanged save is identical");

    // Replace a GARC with one that grew, save, and read it back.
    let mut changed = stats.clone();
    let mut new_bulbasaur = bulbasaur.clone();
    new_bulbasaur[0] = 99;
    new_bulbasaur.extend_from_slice(&[0xAB; 16]);
    changed.set_file(1, new_bulbasaur.clone()).expect("exists");
    let changed_bytes = changed.to_bytes().expect("builds");
    rom.replace_file(&stats_path, changed_bytes.clone()).expect("replaces");
    let mut overlay = Overlay::new(&path, &rom);
    let written = rom.write_cxi(&mut overlay).expect("saves");
    assert_eq!(written, overlay.len);
    assert!(
        overlay.changed_bytes() < 64 << 20,
        "only {} bytes differ",
        overlay.changed_bytes()
    );
    overlay.seek(SeekFrom::Start(0)).expect("seek");
    let mut saved = Ncch::open(overlay).expect("saved image opens");
    assert!(
        saved.verify().expect("readable"),
        "every hash of the saved image is valid"
    );
    assert_eq!(saved.read_file(&stats_path).expect("stats GARC"), changed_bytes);
    let reread = Garc::parse(saved.read_file(&stats_path).expect("stats GARC"), &Decompression::All).expect("parses");
    assert_eq!(reread.file(1).expect("Bulbasaur"), new_bulbasaur);
    assert_eq!(reread.file(2).expect("Ivysaur"), stats.file(2).expect("Ivysaur"));
    assert_eq!(
        saved.read_file(&text_path).expect("text GARC"),
        rom.read_file(&text_path).expect("text GARC")
    );
    assert_eq!(saved.code().expect("code.bin"), code);

    // The same change as a mod folder.
    let dir = tempfile::tempdir().expect("temp dir");
    rom.set_code(code.iter().map(|b| b ^ 1).collect()).expect("code");
    let root = rom.write_layered_fs(dir.path()).expect("writes");
    assert_eq!(
        std::fs::read(root.join("romfs").join(&stats_path)).expect("written"),
        changed_bytes
    );
    assert_eq!(std::fs::read(root.join("code.bin")).expect("written").len(), code.len());
}

#[test]
#[ignore = "needs a real ROM"]
fn pokemon_x() {
    check("Pokemon X", 6, "X", [45, 49, 49, 45, 65, 65]);
}

#[test]
#[ignore = "needs a real ROM"]
fn pokemon_ultra_sun() {
    check("Pokemon Ultra Sun", 7, "Ultra Sun", [45, 49, 49, 45, 65, 65]);
}
