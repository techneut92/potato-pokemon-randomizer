//! Text archive tests: synthetic files, and the real ones of Platinum and Black 2 (`PPR_TEST_ROMS`).

use ppr_formats::nds::NdsRom;

use super::{gen4, gen5};

#[test]
fn gen4_round_trips() {
    let strings = [
        "Bulbasaur",
        "\\v0103\\z0000: Then, I choose you!\\nI’m picking this one!\\p",
        "\\vFF00\\x0000Poké Ball\\l…",
        "\\x010B and \\and",
        "",
    ];
    let file = gen4::encode(&strings, false).expect("encodes");
    assert_eq!(file[..4], [5, 0, 0x0E, 0xD0]);
    let text = gen4::decode(&file).expect("decodes");
    assert_eq!(text.strings, strings);
    assert!(!text.compressed);
}

#[test]
fn gen4_packed_strings_round_trip_when_every_code_fits_nine_bits() {
    let strings = ["RED", "Lucas", "Ab1", "", "Twelve chars"];
    let file = gen4::encode(&strings, true).expect("encodes");
    let text = gen4::decode(&file).expect("decodes");
    assert_eq!(text.strings, strings);
    assert!(text.compressed);
    // Codes above 0x1FF lose their high bits.
    let file = gen4::encode(&["a\\nb"], true).expect("encodes");
    assert_ne!(gen4::decode(&file).expect("decodes").strings, ["a\\nb"]);
}

#[test]
fn gen4_packs_nine_bits_per_code() {
    // One code and the end code 0x1FF: 18 bits in two words after the marker, two codes more than unpacked.
    let file = gen4::encode(&["A"], true).expect("encodes");
    let plain = gen4::encode(&["A"], false).expect("encodes");
    assert_eq!(file.len(), plain.len() + 4);
    assert_eq!(gen4::decode(&file).expect("decodes").strings, ["A"]);
}

#[test]
fn gen4_drops_unknown_characters_and_rejects_bad_escapes() {
    let file = gen4::encode(&["a😀b\\qc"], false).expect("encodes");
    assert_eq!(gen4::decode(&file).expect("decodes").strings, ["abc"]);
    assert!(gen4::encode(&["\\x12"], false).is_err());
    assert!(gen4::encode(&["\\vZZZZ"], false).is_err());
}

#[test]
fn gen4_rejects_broken_files() {
    assert!(gen4::decode(&[1]).is_err());
    let mut file = gen4::encode(&["abc"], false).expect("encodes");
    assert!(gen4::decode(&file[..10]).is_err());
    file.truncate(file.len() - 2);
    assert!(gen4::decode(&file).is_err());
}

/// A Gen 5 file with one section: each string is (codes without terminator, key of the first code, extra word).
fn gen5_file(strings: &[(&[u16], u16, u16)]) -> Vec<u8> {
    let mut encrypted: Vec<Vec<u16>> = strings
        .iter()
        .map(|&(codes, key, _)| {
            let mut key = key;
            codes
                .iter()
                .chain([0xFFFF].iter())
                .map(|&c| {
                    let out = c ^ key;
                    key = key.rotate_left(3);
                    out
                })
                .collect()
        })
        .collect();
    let data_len: usize = encrypted.iter().map(|c| c.len() * 2).sum();
    if data_len % 4 == 2 {
        let last = encrypted.len() - 1;
        let key = strings[last]
            .1
            .rotate_left(u32::try_from(encrypted[last].len() * 3 % 16).expect("small"));
        encrypted[last].push(0xFFFF ^ key);
    }
    let mut section = Vec::new();
    let mut offset = 4 + 8 * strings.len();
    for (codes, &(_, _, extra)) in encrypted.iter().zip(strings) {
        section.extend(u32::try_from(offset).expect("small").to_le_bytes());
        section.extend(u16::try_from(codes.len()).expect("small").to_le_bytes());
        section.extend(extra.to_le_bytes());
        offset += codes.len() * 2;
    }
    for codes in &encrypted {
        section.extend(codes.iter().flat_map(|c| c.to_le_bytes()));
    }
    let size = u32::try_from(section.len() + 4).expect("small");
    let mut file = vec![1, 0];
    file.extend(u16::try_from(strings.len()).expect("small").to_le_bytes());
    file.extend(size.to_le_bytes());
    file.extend([0; 4]);
    file.extend(16u32.to_le_bytes());
    file.extend(size.to_le_bytes());
    file.extend(section);
    file
}

fn utf16(s: &str) -> Vec<u16> {
    s.encode_utf16().collect()
}

#[test]
fn gen5_decodes_symbols_escapes_and_plain_text() {
    let hello = utf16("Hello");
    let symbols = [0x2467, 0x21D2, 0x0041, 0xFFFE, 0x0005, 0xD800, 0x2FE5, 0xF000];
    let file = gen5_file(&[(&hello, 0x1234, 7), (&symbols, 0xBEEF, 0)]);
    let strings = gen5::decode(&file).expect("decodes");
    assert_eq!(strings, ["Hello", "×\\[angry]A\\xFFFE\\x0005\\xD800\\x2FE5\u{F000}"]);
    // Encoding the same strings gives back the same file.
    assert_eq!(gen5::encode(&file, &strings).expect("encodes"), file);
}

#[test]
fn gen5_keeps_keys_and_extra_words_when_strings_change() {
    let file = gen5_file(&[(&utf16("Bulbasaur"), 0x0101, 3), (&utf16("Ivysaur"), 0x7777, 9)]);
    let changed = gen5::encode(&file, &["Missingno.", "\\x0041×"]).expect("encodes");
    assert_eq!(gen5::decode(&changed).expect("decodes"), ["Missingno.", "A×"]);
    let expected = gen5_file(&[(&utf16("Missingno."), 0x0101, 3), (&[0x41, 0x2467], 0x7777, 9)]);
    assert_eq!(changed, expected);
    // Too few strings leave the file alone; extra ones are ignored.
    assert_eq!(gen5::encode(&file, &["x"]).expect("encodes"), file);
    assert_eq!(
        gen5::encode(&file, &["Bulbasaur", "Ivysaur", "extra"]).expect("encodes"),
        file
    );
    assert!(gen5::encode(&file, &["\\x12", "b"]).is_err());
}

#[test]
fn gen5_rejects_broken_files() {
    assert!(gen5::decode(&[1, 0]).is_err());
    let file = gen5_file(&[(&utf16("abc"), 1, 0)]);
    assert!(gen5::decode(&file[..file.len() - 4]).is_err());
    let mut many = file;
    many[0] = 4;
    assert!(gen5::decode(&many).is_err());
}

#[test]
fn gen5_table_texts_are_prefix_free() {
    // Encoding replaces table texts left to right; that only matches the games' tooling when no text starts
    // another.
    let texts: Vec<&str> = include_str!("../../data/texttables/gen5.tbl")
        .lines()
        .filter_map(|l| l.split_once('=').map(|(_, t)| t))
        .collect();
    for a in &texts {
        for b in &texts {
            assert!(a == b || !b.starts_with(a), "{a} starts {b}");
        }
    }
}

/// A real ROM from `PPR_TEST_ROMS` whose file name contains `name`.
fn real_rom(name: &str) -> NdsRom {
    let dir = std::env::var_os("PPR_TEST_ROMS").expect("set PPR_TEST_ROMS to a folder of ROM dumps");
    let path = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains(name))
        .unwrap_or_else(|| panic!("no ROM named like {name}"));
    NdsRom::from_bytes(std::fs::read(path).expect("readable")).expect("opens")
}

#[test]
#[ignore = "needs a real ROM"]
fn platinum_species_names() {
    let mut rom = real_rom("Platinum Version (Europe)");
    let mut msg = rom.narc("/msgdata/pl_msg.narc").expect("text archive");
    // Species names are file 412 (`PokemonNamesTextOffset`).
    let names = gen4::decode(&msg.files[412]).expect("decodes");
    assert!(!names.compressed);
    assert_eq!(names.strings.len(), 496);
    assert_eq!(names.strings[1].to_lowercase(), "bulbasaur");
    assert_eq!(names.strings[493].to_lowercase(), "arceus");
    // Every text file decodes, and re-encodes to the same strings.
    for (i, file) in msg.files.iter().enumerate() {
        let text = gen4::decode(file).unwrap_or_else(|e| panic!("file {i}: {e}"));
        let again = gen4::encode(&text.strings, text.compressed).expect("encodes");
        assert_eq!(gen4::decode(&again).expect("decodes"), text, "file {i}");
    }
    // A renamed species survives a rebuild of the ROM.
    let mut strings = names.strings.clone();
    strings[1] = "TESTASAUR".into();
    msg.files[412] = gen4::encode(&strings, false).expect("encodes");
    rom.replace_narc("msgdata/pl_msg.narc", &msg).expect("exists");
    let back = NdsRom::from_bytes(rom.save().expect("saves").into_owned()).expect("reopens");
    let names = gen4::decode(&back.narc("msgdata/pl_msg.narc").expect("narc").files[412]).expect("decodes");
    assert_eq!(names.strings[1], "TESTASAUR");
}

#[test]
#[ignore = "needs a real ROM"]
fn black_2_species_names() {
    let rom = real_rom("Black Version 2");
    let strings = rom.narc("a/0/0/2").expect("text archive");
    // Species names are file 90 (`PokemonNamesTextOffset`).
    let names = gen5::decode(&strings.files[90]).expect("decodes");
    assert_eq!(names[1], "Bulbasaur");
    assert_eq!(names[649], "Genesect");
    // Every text file decodes, and encoding its own strings gives back the same strings (the padding code counts
    // towards the last string, so the bytes may differ).
    for path in ["a/0/0/2", "a/0/0/3"] {
        for (i, file) in rom.narc(path).expect("text archive").files.iter().enumerate() {
            let text = gen5::decode(file).unwrap_or_else(|e| panic!("{path} file {i}: {e}"));
            let again = gen5::encode(file, &text).expect("encodes");
            assert_eq!(gen5::decode(&again).expect("decodes"), text, "{path} file {i}");
        }
    }
}
