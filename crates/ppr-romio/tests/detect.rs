//! Detection on synthetic headers: no copyrighted ROMs are needed (ADR 0007).

use std::io::Cursor;

use ppr_model::Platform;
use ppr_romio::{Detection, Unsupported, detect};

fn gb(title: &[u8], destination: u8, version: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x10_0000];
    rom[0x134..0x134 + title.len()].copy_from_slice(title);
    rom[0x14A] = destination;
    rom[0x14C] = version;
    rom[0x14D] = rom[0x134..0x14D]
        .iter()
        .fold(0u8, |x, &b| x.wrapping_sub(b).wrapping_sub(1));
    rom
}

fn gba(code: [u8; 4], version: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x100_0000];
    rom[0xAC..0xB0].copy_from_slice(&code);
    rom[0xBC] = version;
    rom[0xBD] = rom[0xA0..0xBD]
        .iter()
        .fold(0u8, |c, &b| c.wrapping_sub(b))
        .wrapping_sub(0x19);
    rom
}

fn nds(code: [u8; 4], version: u8) -> Vec<u8> {
    let mut rom = vec![0u8; 0x4000];
    rom[0x0C..0x10].copy_from_slice(&code);
    rom[0x1E] = version;
    let crc = rom[..0x15E].iter().fold(0xFFFF_u16, |crc, &b| {
        (0..8).fold(
            crc ^ u16::from(b),
            |c, _| if c & 1 == 1 { (c >> 1) ^ 0xA001 } else { c >> 1 },
        )
    });
    rom[0x15E..0x160].copy_from_slice(&crc.to_le_bytes());
    rom
}

fn cci(product: &str, title_id: u64) -> Vec<u8> {
    let mut img = vec![0u8; 0x4200];
    img[0x100..0x104].copy_from_slice(b"NCSD");
    img[0x4100..0x4104].copy_from_slice(b"NCCH");
    img[0x4118..0x4120].copy_from_slice(&title_id.to_le_bytes());
    img[0x4150..0x4150 + product.len()].copy_from_slice(product.as_bytes());
    img
}

fn name_of(rom: Vec<u8>) -> String {
    match detect(&mut Cursor::new(rom)).expect("readable") {
        Detection::Supported(id) => format!("{} | {} | {}", id.name, id.generation, id.family),
        Detection::Unsupported(u) => format!("unsupported: {u:?}"),
    }
}

#[test]
fn detects_one_game_per_generation() {
    assert_eq!(name_of(gb(b"POKEMON RED", 1, 0)), "Red (U) | Gen 1 | RB");
    assert_eq!(
        name_of(gb(b"PM_CRYSTAL\0BYTE\x80", 1, 0)),
        "Crystal (U) | Gen 2 | Crystal"
    );
    assert_eq!(name_of(gba(*b"BPEE", 0)), "Emerald (U) | Gen 3 | Em");
    assert_eq!(name_of(nds(*b"CPUE", 0)), "Platinum (U) | Gen 4 | Plat");
    assert_eq!(name_of(nds(*b"IREO", 0)), "Black 2 (U) | Gen 5 | BW2");
    assert_eq!(name_of(cci("CTR-P-EKJA", 0x0004_0000_0005_5D00)), "X | Gen 6 | XY");
    assert_eq!(
        name_of(cci("CTR-P-A2BA", 0x0004_0000_001B_5100)),
        "Ultra Moon | Gen 7 | USUM"
    );
}

#[test]
fn japanese_and_revisions_are_distinct() {
    assert_eq!(name_of(gb(b"POKEMON RED", 0, 0)), "Red (J) | Gen 1 | RB");
    assert_eq!(name_of(gba(*b"AXVE", 1)), "Ruby (E) | Gen 3 | Ruby");
}

#[test]
fn unofficial_emerald_translation_is_detected() {
    let mut rom = gba(*b"\0\0\0\0", 0);
    rom[0xA0..0xAB].copy_from_slice(b"YJencrypted");
    assert!(name_of(rom).starts_with("Emerald"), "translated Emerald");
}

#[test]
fn unknown_games_name_their_platform() {
    let d = detect(&mut Cursor::new(nds(*b"ABCD", 0))).expect("readable");
    assert_eq!(
        d,
        Detection::Unsupported(Unsupported::UnknownGame {
            platform: Platform::NintendoDs,
            code: "ABCD".into()
        })
    );
    let d = detect(&mut Cursor::new(gba(*b"AGBJ", 0))).expect("readable");
    assert!(matches!(
        d,
        Detection::Unsupported(Unsupported::UnknownGame {
            platform: Platform::GameBoyAdvance,
            ..
        })
    ));
}

#[test]
fn other_files_are_unknown() {
    for junk in [vec![], vec![0u8; 0x200], vec![0xAB; 0x10_0000]] {
        assert_eq!(
            detect(&mut Cursor::new(junk)).expect("readable"),
            Detection::Unsupported(Unsupported::UnknownFile)
        );
    }
}
