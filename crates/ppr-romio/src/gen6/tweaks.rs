//! The misc. tweaks of Gen 6, guaranteed catching, easier happiness evolutions, the type chart and the intro Pokémon.
//!
//! Deliberate choice: Fast Egg Hatching and No EV Yields change the species files, alternate formes included, when
//! they are applied, as they are not part of the species data.

use ppr_model::{DexNumber, Effectiveness, MiscTweak, Type, TypeChart};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom, find, find_after};
use crate::RomError;

const VANILLA_HAPPINESS: u8 = 220;
const EASIER_HAPPINESS: u8 = 160;
/// The happiness bytes of the happiness evolutions (any time, day and night), after the locator's start.
const HAPPINESS_BYTES: [usize; 3] = [0, 12, 36];
/// `cpy r2, r0; ldr r1, =CRY; str r0, [sp]; bl` to the function that plays a cry by its value.
const PLAY_CRY_BY_VALUE: [u32; 4] = [0xE1A0_2000, 0xE59F_100C, 0xE58D_0000, 0xEBFF_FDE9];
/// The save-load forme reversion call is this far past its prefix.
const FORME_REVERSION_CALL: usize = 0x10;

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// The tweaks the game offers, in order. Fast Egg Hatching is left out of Omega Ruby and Alpha Sapphire, whose Wynaut
/// and Togepi eggs hatch slowly whatever the species data says.
pub(crate) fn available(family: Family) -> Vec<MiscTweak> {
    let mut out = vec![
        MiscTweak::FastestText,
        MiscTweak::BanLuckyEgg,
        MiscTweak::RetainAltFormes,
        MiscTweak::NationalDexAtStart,
    ];
    if family == Family::Xy {
        out.push(MiscTweak::FastEggHatching);
    }
    out.push(MiscTweak::NoEvYields);
    out
}

/// The files and family a tweak needs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Files<'a> {
    pub(crate) family: Family,
    pub(crate) stats: &'a str,
    pub(crate) battle: &'a str,
}

/// Applies a tweak. Ban Lucky Egg is done on the game data.
pub(crate) fn apply(rom: &mut Rom, files: Files<'_>, tweak: MiscTweak) -> Result<(), RomError> {
    match tweak {
        MiscTweak::FastestText => fastest_text(rom),
        MiscTweak::RetainAltFormes => retain_alt_formes(rom, files),
        MiscTweak::NationalDexAtStart => national_dex(rom, files.family),
        MiscTweak::FastEggHatching => for_every_species(rom, files, |data| data.set_u8(c::BS_EGG_CYCLES, 1)),
        MiscTweak::NoEvYields => for_every_species(rom, files, |data| data.set_u16(c::BS_EV_YIELD, 0)),
        _ => Ok(()),
    }
}

/// Changes the personal data of every species and alternate forme.
fn for_every_species(
    rom: &mut Rom,
    files: Files<'_>,
    change: impl Fn(&mut [u8]) -> Result<(), RomError>,
) -> Result<(), RomError> {
    for file in 1..=c::SPECIES_COUNT + files.family.forme_count() {
        let mut data = rom.garc_file(files.stats, false, file)?;
        change(&mut data)?;
        rom.set_garc_file(files.stats, false, file, data)?;
    }
    Ok(())
}

/// Text speed is fixed at its fastest: two loads of the speed setting become `mov r4, #3` and `mov r5, #3`.
fn fastest_text(rom: &mut Rom) -> Result<(), RomError> {
    for (prefix, instruction) in c::FASTEST_TEXT_PREFIXES
        .iter()
        .zip([[0x03, 0x40, 0xA0, 0xE3], [0x03, 0x50, 0xA0, 0xE3]])
    {
        if let Some(at) = find_after(rom.code(), prefix) {
            write_bytes(rom.code_mut(), at, &instruction)?;
        }
    }
    Ok(())
}

fn write_bytes(data: &mut [u8], at: usize, bytes: &[u8]) -> Result<(), RomError> {
    let len = data.len();
    data.get_mut(at..at + bytes.len())
        .ok_or_else(|| malformed(format!("offset {at:#x} is past the end of {len:#x} bytes")))?
        .copy_from_slice(bytes);
    Ok(())
}

/// Mega Evolutions and Primal Reversions in the party stay when a save is loaded; in Omega Ruby and Alpha Sapphire,
/// Primal Kyogre and Groudon also stay after being caught.
fn retain_alt_formes(rom: &mut Rom, files: Files<'_>) -> Result<(), RomError> {
    if let Some(at) = find_after(rom.code(), files.family.forme_reversion_prefix()) {
        // The call to the function that checks for Mega Evolutions and Primal Reversions goes.
        write_bytes(rom.code_mut(), at + FORME_REVERSION_CALL, &[0; 4])?;
    }
    if files.family == Family::Oras
        && let Some(at) = find_after(rom.file(files.battle)?, c::AFTER_BATTLE_FORME_REVERSION_PREFIX)
    {
        // The species the battle checks for (Kyogre, 0x17E) becomes one that doesn't exist.
        write_bytes(rom.file_mut(files.battle)?, at, &[0xFF, 0xFF])?;
    }
    Ok(())
}

/// The player has the National Pokédex from the start.
fn national_dex(rom: &mut Rom, family: Family) -> Result<(), RomError> {
    if let Some(at) = find(rom.code(), c::NATIONAL_DEX_FUNCTION_LOCATOR) {
        // The function that reads the National Pokédex flag returns 1 (`mov r0, #1`).
        write_bytes(rom.code_mut(), at, &[0x01, 0x00, 0xA0, 0xE3])?;
    }
    match family {
        Family::Xy => {
            if let Some(at) = find(rom.code(), c::XY_GET_DEX_FLAG_FUNCTION_LOCATOR) {
                // The function that tells which Pokédexes the player has answers "yes" for all of them: the
                // comparison goes and the move and branch after it become unconditional.
                let code = rom.code_mut();
                code.set_u32(at, 0)?;
                code.set_u8(at + 7, 0xE3)?;
                code.set_u8(at + 11, 0xEA)?;
            }
        }
        Family::Oras => {
            if let Some(at) = find_after(rom.code(), c::ORAS_GET_HOENN_DEX_CAUGHT_FUNCTION_PREFIX) {
                // The Pokédexes can't be opened with no Hoenn Pokémon caught, so the count of caught Hoenn Pokémon is
                // at least 1: the checksum check always branches to the body, the space of its failure path gets
                // `mov r0, r7; cmp r0, #0; moveq r0, #1; ldmia sp!, {r4-r12, pc}`, and the function's end branches
                // there.
                let code = rom.code_mut();
                code.set_u8(at + 31, 0xEA)?;
                for (i, word) in [0x0700_A0E1_u32, 0x0000_50E3, 0x0100_A003, 0xF09F_BDE8]
                    .into_iter()
                    .enumerate()
                {
                    write_bytes(code, at + 32 + i * 4, &word.to_be_bytes())?;
                }
                write_bytes(code, at + 208, &0xD2FF_FFEA_u32.to_be_bytes())?;
            }
        }
    }
    Ok(())
}

/// Every ball catches: the branch for odds below 255 goes.
pub(crate) fn guaranteed_catching(rom: &mut Rom, battle: &str) -> Result<(), RomError> {
    if let Some(at) = find(rom.file(battle)?, c::PERFECT_ODDS_LOCATOR) {
        write_bytes(rom.file_mut(battle)?, at, &[0; 4])?;
    }
    Ok(())
}

/// Happiness evolutions need 160 instead of 220.
pub(crate) fn ease_happiness(rom: &mut Rom) -> Result<(), RomError> {
    let Some(at) = find(rom.code(), c::FRIENDSHIP_FOR_EVO_LOCATOR) else {
        return Ok(());
    };
    for offset in HAPPINESS_BYTES {
        if rom.code().u8_at(at + offset)? == VANILLA_HAPPINESS {
            rom.code_mut().set_u8(at + offset, EASIER_HAPPINESS)?;
        }
    }
    Ok(())
}

fn effectiveness_byte(e: Effectiveness) -> u8 {
    match e {
        Effectiveness::Double => 8,
        Effectiveness::Neutral => 4,
        Effectiveness::Half => 2,
        Effectiveness::Zero => 0,
    }
}

/// The type chart: a byte for each attacking and defending type, in the game's type order, in the battle module.
pub(crate) fn read_type_chart(rom: &mut Rom, battle: &str, at: usize) -> Result<TypeChart, RomError> {
    let data = rom.file(battle)?;
    let width = c::TYPES.len();
    let mut chart = TypeChart::neutral(&c::TYPES);
    for attacker in c::TYPES {
        for defender in c::TYPES {
            let b =
                data.u8_at(at + usize::from(c::type_byte(attacker)) * width + usize::from(c::type_byte(defender)))?;
            let e = match b {
                8 => Effectiveness::Double,
                4 => Effectiveness::Neutral,
                2 => Effectiveness::Half,
                0 => Effectiveness::Zero,
                _ => return Err(malformed(format!("type effectiveness {b}"))),
            };
            chart.set(attacker, defender, e);
        }
    }
    Ok(chart)
}

/// Writes the type chart.
pub(crate) fn write_type_chart(rom: &mut Rom, battle: &str, at: usize, chart: &TypeChart) -> Result<(), RomError> {
    let width = c::TYPES.len();
    let cell = |a: Type, d: Type| at + usize::from(c::type_byte(a)) * width + usize::from(c::type_byte(d));
    let mut changes = Vec::new();
    {
        let data = rom.file(battle)?;
        for attacker in c::TYPES {
            for defender in c::TYPES {
                let Some(e) = chart.get(attacker, defender) else {
                    continue;
                };
                let offset = cell(attacker, defender);
                if data.u8_at(offset)? != effectiveness_byte(e) {
                    changes.push((offset, effectiveness_byte(e)));
                }
            }
        }
    }
    if !changes.is_empty() {
        let data = rom.file_mut(battle)?;
        for (offset, b) in changes {
            data.set_u8(offset, b)?;
        }
    }
    Ok(())
}

/// Shows forme `forme` of `species` (0 for the species itself) in the intro of X and Y instead of Fennekin, with its
/// own cries.
pub(crate) fn write_intro(rom: &mut Rom, intro: &str, species: DexNumber, forme: u8) -> Result<(), RomError> {
    let cries = find_after(rom.code(), c::CRIES_TABLE_PREFIX_XY).ok_or_else(|| malformed("no cry table"))?;
    // An alternate forme's cries follow the other species: its base species' entry starts with where its formes'
    // entries start.
    let mut index = usize::from(species);
    if forme != 0 {
        let formes = rom.code().u32_at(cries + index * 0x14)?;
        index = usize::try_from(formes).map_err(|_| malformed("cry table"))? + usize::from(forme) - 1;
    }
    let entry = cries + index * 0x14;
    let initial_cry = rom.code().u32_at(entry + 4)?;
    let repeated_cry = rom.code().u32_at(entry + 0x10)?;

    let data = rom.file(intro)?;
    let model = find_after(data, c::INTRO_MODEL_PREFIX_XY).ok_or_else(|| malformed("no intro model"))?;
    let cry1 = find_after(data, c::INTRO_CRY_PREFIXES_XY[0]).ok_or_else(|| malformed("no intro cry 1"))?;
    let cry2 = find_after(data, c::INTRO_CRY_PREFIXES_XY[1]).ok_or_else(|| malformed("no intro cry 2"))?;
    let cry3 = find_after(data, c::INTRO_CRY_PREFIXES_XY[2]).ok_or_else(|| malformed("no intro cry 3"))?;
    let repeated =
        find_after(data, c::INTRO_REPEATED_CRY_PREFIX_XY).ok_or_else(|| malformed("no intro repeated cry"))?;

    let data = rom.file_mut(intro)?;
    // The model loaded, and its forme.
    data.set_u16(model, species)?;
    data.set_u8(model + 2, forme)?;
    // The cry when it leaves the ball: two branches re-pointed, then the call changed to the function that plays a
    // cry by its value, the two instructions after it moved up and the cry in the literal pool.
    // A game patched before only gets its cry.
    data.set_u8(cry1, 0x5E)?;
    data.set_u8(cry2, 0x2F)?;
    if data.u32_at(cry3)? != PLAY_CRY_BY_VALUE[0] {
        let after = (data.u32_at(cry3 + 0x14)?, data.u32_at(cry3 + 0x18)?);
        for (i, word) in PLAY_CRY_BY_VALUE.into_iter().chain([after.0, after.1]).enumerate() {
            data.set_u32(cry3 + i * 4, word)?;
        }
    }
    data.set_u32(cry3 + 0x18, initial_cry)?;
    // The cry while it stands around: a literal pool value.
    data.set_u32(repeated, repeated_cry)
}
