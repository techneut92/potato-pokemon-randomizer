//! Basic support for ROM hacks of `FireRed` (U) 1.0: they move tables and change how many species, moves and
//! trainers there are. A ROM with that game code that is not the vanilla game gets those tables from the pointers
//! the game code reads them through, and the counts detected from the data.
//!
//! Deliberate choices:
//! - Data that runs off the end of the ROM ends a count.
//! - A ROM whose Pokédex numbers go past its internal species count is an error.

use ppr_formats::crc32;

use super::{learnsets, need, species_names_offset};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;

/// The size (16 MiB) and CRC-32 of vanilla `FireRed` (U) 1.0.
const VANILLA_SIZE: usize = 0x100_0000;
const VANILLA_CRC32: u32 = 3_716_707_868;
/// Pointers the game code reads its tables through.
const TM_HM_COMPAT_POINTER: usize = 0x4_3C68;
const EVOLUTIONS_POINTER: usize = 0x4_2F6C;
const TUTOR_COMPAT_POINTER: usize = 0x12_0C30;
const MOVE_DESCRIPTIONS_POINTER: usize = 0xE_5440;
const TRAINER_DATA_POINTER: usize = 0xFC00;
/// Pokédex numbers stay below this even in hacks that extend the Pokédex.
const MAX_DEX_NUMBER: u16 = 1023;
/// A move description is shorter than this.
const MAX_DESCRIPTION: usize = 100;
/// The byte that ends a string.
const TERMINATOR: u8 = 0xFF;

/// Whether a `FireRed` (U) 1.0 ROM is a hack: anything but the vanilla game.
pub(super) fn is_hack(rom: &GbaRom) -> bool {
    rom.bytes().len() != VANILLA_SIZE || crc32(rom.bytes()) != VANILLA_CRC32
}

/// The pointer at `offset`; `None` for anything but a pointer into the ROM (its end included).
fn pointer(rom: &GbaRom, offset: usize) -> Option<usize> {
    let target = i64::from(rom.u32(offset).ok()?.cast_signed().wrapping_sub(0x800_0000));
    usize::try_from(target).ok().filter(|&t| t <= rom.bytes().len())
}

/// The length of the string at `at`: the bytes up to and including the terminator; `None` without one.
fn string_length(rom: &GbaRom, at: usize) -> Option<usize> {
    rom.bytes()
        .get(at..)?
        .iter()
        .position(|&b| b == TERMINATOR)
        .map(|i| i + 1)
}

/// The species count: names until one does not fit, less an unused egg slot at the end, less slots
/// without a learnset, cut at the first Pokédex number past 1023.
fn species_count(rom: &GbaRom, entry: &RomEntry, text: &TextTable, movesets: usize) -> Result<usize, RomError> {
    let names = species_names_offset(rom, entry)?;
    let name_len = need(entry, "PokemonNameLength")?;
    let mut count = 0;
    loop {
        let at = names + (count + 1) * name_len;
        let fits = string_length(rom, at).is_some_and(|len| len <= name_len) && rom.bytes()[at] != 0;
        if !fits {
            break;
        }
        count += 1;
    }
    let mut count = isize::try_from(count).map_err(|_| RomError::Malformed("species count".into()))?;
    let last_at = names + count.unsigned_abs() * name_len;
    let last = text.decode(rom.bytes().get(last_at..).unwrap_or_default(), usize::MAX);
    if last == "?" || last == "-" {
        count -= 1;
    }
    while count >= 0 && pointer(rom, movesets + count.unsigned_abs() * 4).is_none() {
        count -= 1;
    }
    let mut count = usize::try_from(count).unwrap_or(0);
    let order = need(entry, "PokedexOrder")?;
    for i in 1..=count {
        if rom.u16(order + (i - 1) * 2)? > MAX_DEX_NUMBER {
            count = i - 1;
            break;
        }
    }
    Ok(count)
}

/// The move count: descriptions until one is not a short string.
fn move_count(rom: &GbaRom, descriptions: usize) -> usize {
    let mut count = 0;
    while pointer(rom, descriptions + count * 4)
        .and_then(|at| string_length(rom, at))
        .is_some_and(|len| len < MAX_DESCRIPTION)
    {
        count += 1;
    }
    count
}

/// The trainer count: entries until one has an invalid battle type, party size, party pointer or
/// name.
fn trainer_count(rom: &GbaRom, entry: &RomEntry, trainers: usize) -> Result<usize, RomError> {
    let size = need(entry, "TrainerEntrySize")?;
    let name_len = need(entry, "TrainerNameLength")?;
    let byte = |at: usize| rom.bytes().get(at).copied();
    let mut count = 1;
    loop {
        let at = trainers + size * count;
        let valid = byte(at).is_some_and(|kind| kind < 4)
            && byte(at + size - 8).is_some_and(|party| (1..=6).contains(&party))
            && pointer(rom, at + size - 4).is_some()
            && string_length(rom, at + 4).is_some_and(|len| len - 1 <= name_len);
        if !valid {
            return Ok(count);
        }
        count += 1;
    }
}

/// When the ROM is a hack of `FireRed` (U) 1.0, the entry's tables and counts as the ROM has them.
/// Returns whether it is a hack.
pub(super) fn adjust(rom: &GbaRom, entry: &mut RomEntry, text: &TextTable) -> Result<bool, RomError> {
    if entry.code != "BPRE" || entry.version != 0 || !is_hack(rom) {
        return Ok(false);
    }
    let movesets = learnsets::Format::of(rom, entry)?.table();
    entry.set_int("PokemonMovesets", movesets);
    let species = species_count(rom, entry, text, movesets)?;
    entry.set_int("PokemonCount", species);
    entry.set_int("PokemonTMHMCompat", rom.pointer(TM_HM_COMPAT_POINTER)?);
    entry.set_int("PokemonEvolutions", rom.pointer(EVOLUTIONS_POINTER)?);
    entry.set_int("MoveTutorCompatibility", rom.pointer(TUTOR_COMPAT_POINTER)?);
    let descriptions = rom.pointer(MOVE_DESCRIPTIONS_POINTER)?;
    entry.set_int("MoveDescriptions", descriptions);
    let trainers = rom.pointer(TRAINER_DATA_POINTER)?;
    entry.set_int("TrainerData", trainers);
    entry.set_int("MoveCount", move_count(rom, descriptions));
    entry.set_int("TrainerCount", trainer_count(rom, entry, trainers)?);
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_end_at_a_missing_learnset_or_a_placeholder_name() {
        let f = crate::gen3::fixtures::fire_red();
        let entry = f.entry.clone();
        let text = TextTable::named(entry.value("TableFile").unwrap_or_default()).expect("table");
        let movesets = f.int("PokemonMovesets");
        let count = |rom: &[u8]| species_count(&GbaRom::new(rom.to_vec()), &entry, &text, movesets).expect("counts");
        assert_eq!(count(&f.rom), 411);
        let mut rom = f.rom.clone();
        rom[movesets + 411 * 4..movesets + 412 * 4].fill(0);
        assert_eq!(count(&rom), 410);
        // A last name of "?" is an unused egg slot.
        let names = species_names_offset(&GbaRom::new(f.rom.clone()), &entry).expect("names");
        let name_len = f.int("PokemonNameLength");
        let mut rom = f.rom.clone();
        rom[names + 411 * name_len..names + 411 * name_len + 2].copy_from_slice(&[0xAC, 0xFF]);
        assert_eq!(count(&rom), 410);
    }
}
