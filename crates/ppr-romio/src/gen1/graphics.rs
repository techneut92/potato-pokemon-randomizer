//! Pokémon palettes and custom player graphics.
//!
//! A species' palette is the number of one of the game's Super Game Boy palettes, one byte per species in Pokédex
//! order. Custom player graphics replace the player's front and back pictures (compressed) and the walking,
//! cycling and fishing sprites; only in the games that are not Japanese, as those have no room to spare.
//!
//! The player's back picture is followed by the old man's of the catching tutorial, and in Yellow by Professor
//! Oak's, so a new back picture moves them along and their pointers follow.
//!
//! Deliberate choices:
//! - The graphics are written after everything else, so they take free space last.
//! - A missing entry key is an error rather than a write to offset 0.
//! - There is one player character; a choice of the second one gets the first's graphics.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, SpeciesPalettes};

use super::constants::Family;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::{GbRom, need, offsets, optional, pic};
use crate::graphics::{GbPlayerGraphics, GbcImage};

/// The extra pointer to the front picture, after the second pointer the entry lists.
const FRONT_POINTER_5: usize = 54;
/// Where the bank byte of each of the front picture's pointers is, after the pointer (from the second on).
const FRONT_BANK_OFFSETS: [usize; 5] = [4, 3, 4, 3, 4];
/// Where the bank byte of the back picture's first pointer is, after it, in Red, Green and Blue, and in Yellow.
const BACK_BANK_OFFSET_RGB: usize = 8;
const BACK_BANK_OFFSET_YELLOW: usize = 3;
/// Where the bank byte of the back picture's second pointer is, after it.
const BACK_BANK_OFFSET_1: usize = 3;
/// Where the pointer to the old man's back picture is, relative to the first pointer to the player's.
const OLD_MAN_POINTER_RGB: isize = 5;
const OLD_MAN_POINTER_YELLOW: isize = -14;
/// Where the pointer to Professor Oak's back picture is in Yellow, relative to the first pointer to the player's.
const OAK_POINTER: isize = -7;

/// Each species' palette number, when the entry knows where they are.
pub(super) fn read_palettes(
    rom: &GbRom,
    entry: &RomEntry,
    numbers: impl Iterator<Item = DexNumber>,
) -> Result<BTreeMap<DexNumber, SpeciesPalettes>, RomError> {
    let at = optional(entry, "MonPaletteIndicesOffset");
    if at == 0 {
        return Ok(BTreeMap::new());
    }
    numbers
        .map(|n| {
            let id = rom.byte(at + usize::from(n))?;
            Ok((
                n,
                SpeciesPalettes {
                    id: Some(id),
                    ..SpeciesPalettes::default()
                },
            ))
        })
        .collect()
}

/// The palette numbers that changed.
pub(super) fn write_palettes(
    rom: &mut GbRom,
    entry: &RomEntry,
    new: &BTreeMap<DexNumber, SpeciesPalettes>,
    old: &BTreeMap<DexNumber, SpeciesPalettes>,
) -> Result<(), RomError> {
    let at = need(entry, "MonPaletteIndicesOffset")?;
    for (&n, p) in new {
        if old.get(&n).map(|o| o.id) != Some(p.id)
            && let Some(id) = p.id
        {
            rom.write_byte(at + usize::from(n), id)?;
        }
    }
    Ok(())
}

fn add(base: usize, offset: isize, key: &str) -> Result<usize, RomError> {
    base.checked_add_signed(offset)
        .ok_or_else(|| RomError::Malformed(format!("{key} is out of range")))
}

/// The pointers to the front picture and the bytes with its bank. The entry lists five pointers and the first bank
/// byte; the sixth pointer and the other bank bytes are at known distances from them.
fn front_pointers(entry: &RomEntry) -> (Vec<usize>, Vec<usize>) {
    let mut pointers = offsets(entry, "PlayerFrontImagePointers");
    let mut banks = offsets(entry, "PlayerFrontImageBankOffsets");
    if pointers.len() == 5 && banks.len() == 1 {
        pointers.push(pointers[1] + FRONT_POINTER_5);
        banks.extend(pointers[1..].iter().zip(FRONT_BANK_OFFSETS).map(|(p, o)| p + o));
    }
    (pointers, banks)
}

/// The pointers to the back picture and the bytes with its bank.
fn back_pointers(entry: &RomEntry, family: Family) -> (Vec<usize>, Vec<usize>) {
    let pointers = offsets(entry, "PlayerBackImagePointers");
    let banks = if pointers.len() == 2 {
        let first = if family == Family::Yellow {
            BACK_BANK_OFFSET_YELLOW
        } else {
            BACK_BANK_OFFSET_RGB
        };
        vec![pointers[0] + first, pointers[1] + BACK_BANK_OFFSET_1]
    } else {
        offsets(entry, "PlayerBackImageBankOffsets")
    };
    (pointers, banks)
}

/// The entry's value for `key`, or the first back picture pointer plus `relative`.
fn back_relative(entry: &RomEntry, key: &str, back: &[usize], relative: isize) -> Result<usize, RomError> {
    match (optional(entry, key), back.first()) {
        (0, Some(&first)) => add(first, relative, key),
        _ => need(entry, key),
    }
}

fn compressed(image: &GbcImage) -> Result<Vec<u8>, RomError> {
    pic::compress(image.width(), image.height(), image.indices())
}

/// The length of the compressed pictures one after the other from `at`.
fn length_of_pictures(rom: &GbRom, at: usize, count: usize) -> Result<usize, RomError> {
    let mut length = 0;
    for _ in 0..count {
        length += pic::compressed_length(rom.bytes(), at + length)?;
    }
    Ok(length)
}

fn split(pointers: &[usize], what: &str) -> Result<(usize, Vec<usize>), RomError> {
    let (&primary, secondary) = pointers
        .split_first()
        .ok_or_else(|| RomError::Malformed(format!("the ROM entry has no pointers to the player's {what}")))?;
    Ok((primary, secondary.to_vec()))
}

fn rewrite_front(rom: &mut GbRom, entry: &RomEntry, front: &GbcImage) -> Result<(), RomError> {
    let (pointers, banks) = front_pointers(entry);
    let (primary, secondary) = split(&pointers, "front picture")?;
    let data = compressed(front)?;
    let first_bank = *banks
        .first()
        .ok_or_else(|| RomError::Malformed("the ROM entry has no bank of the player's front picture".into()))?;
    let bank = usize::from(rom.byte(first_bank)?);
    let old_length = pic::compressed_length(rom.bytes(), rom.pointer_in(primary, bank)?)?;
    rom.rewrite_indirect_bank_shared(primary, &secondary, &banks, &data, old_length)?;
    Ok(())
}

fn rewrite_back(rom: &mut GbRom, entry: &RomEntry, family: Family, picture: &GbcImage) -> Result<(), RomError> {
    let (pointers, banks) = back_pointers(entry, family);
    let (primary, secondary) = split(&pointers, "back picture")?;
    let yellow = family == Family::Yellow;
    let old_man_pointer = back_relative(
        entry,
        "OldManBackImagePointer",
        &pointers,
        if yellow {
            OLD_MAN_POINTER_YELLOW
        } else {
            OLD_MAN_POINTER_RGB
        },
    )?;
    let oak_pointer = if yellow {
        Some(back_relative(entry, "OakBackImagePointer", &pointers, OAK_POINTER)?)
    } else {
        None
    };
    let first_bank = *banks
        .first()
        .ok_or_else(|| RomError::Malformed("the ROM entry has no bank of the player's back picture".into()))?;
    let bank = usize::from(rom.byte(first_bank)?);
    // The new picture, then the others as they are.
    let mut data = compressed(picture)?;
    for p in std::iter::once(old_man_pointer).chain(oak_pointer) {
        let at = rom.pointer_in(p, bank)?;
        data.extend_from_slice(rom.slice(at, pic::compressed_length(rom.bytes(), at)?)?);
    }
    let old = rom.pointer_in(primary, bank)?;
    let old_length = length_of_pictures(rom, old, 2 + usize::from(yellow))?;
    rom.rewrite_indirect_bank_shared(primary, &secondary, &banks, &data, old_length)?;

    let bank = usize::from(rom.byte(first_bank)?);
    let new = rom.pointer_in(primary, bank)?;
    let old_man = new + pic::compressed_length(rom.bytes(), new)?;
    if let Some(p) = oak_pointer {
        let oak = old_man + pic::compressed_length(rom.bytes(), old_man)?;
        rom.write_pointer(p, oak)?;
    }
    rom.write_pointer(old_man_pointer, old_man)
}

/// Writes a pack's graphics over the player's.
pub(super) fn write_player_graphics(
    rom: &mut GbRom,
    entry: &RomEntry,
    family: Family,
    graphics: &GbPlayerGraphics,
) -> Result<(), RomError> {
    if let Some(front) = &graphics.front {
        rewrite_front(rom, entry, front)?;
    }
    if let Some(back) = &graphics.back {
        rewrite_back(rom, entry, family, back)?;
    }
    for (image, key) in [
        (&graphics.walk, "PlayerWalkSprite"),
        (&graphics.bike, "PlayerBikeSprite"),
        (&graphics.fish, "PlayerFishSprite"),
    ] {
        if let Some(image) = image {
            rom.write(need(entry, key)?, &image.to_bytes())?;
        }
    }
    Ok(())
}
