//! Evolutions: eight entries of (method, parameter, species) per species and forme, the hardcoded Shedinja and the
//! baby table.
//!
//! Deliberate choices:
//! - Feebas' Beauty evolution in Omega Ruby and Alpha Sapphire is not read, and is written again next to its first
//!   evolution, as the game data has one way for a species to evolve into another.
//! - Espurr's evolution into the female Meowstic has a method of its own that also sets the forme. It is read as a
//!   female-only level evolution into the female Meowstic forme, and a female-only level evolution of Espurr is
//!   written back with that method.
//! - Nincada's Shedinja is not in the evolution data but made by the evolution code; it is read as an extra
//!   evolution at level 20. When Nincada keeps two evolutions, the code is patched to check for the first and to
//!   make the second, and only the first is written to the data. The extra evolution is read from the code as it
//!   is, patched or not, and only when Nincada evolves into the species the code checks for, so a game that was
//!   written reads back as written. A game patched before is patched again in place.
//! - The alternate formes of Floette and Pumpkaboo keep copies of their base forme's evolutions; they are written
//!   again with the base forme's. Other evolutions of alternate formes are not read.
//!
//! Burmy also evolves into Wormadam's Sandy and Trash Cloaks, with no method, so that they count as its split
//! evolutions; such evolutions are not written.

use ppr_model::{DexNumber, Evolution, EvolutionType};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom, find_after};
use super::species::Formes;
use crate::RomError;

const SLOTS: usize = 8;
const SLOT_SIZE: usize = 6;

/// What reading the evolutions found besides them.
#[derive(Debug, Clone, Default)]
pub(crate) struct EvolutionFiles {
    /// Forme files that copy their base forme's evolutions, by base forme.
    pub(crate) copies: Vec<(DexNumber, usize)>,
}

/// Whether `target` has forme `forme`: an alternate forme with data or a cosmetic forme.
fn has_forme(rom: &mut Rom, stats: &str, formes: &Formes, target: DexNumber, forme: u8) -> Result<bool, RomError> {
    if formes.values().any(|f| f.base == target && f.number == forme) {
        return Ok(true);
    }
    let data = rom.garc_file(stats, false, usize::from(target))?;
    let count = data.u8_at(c::BS_FORME_COUNT)?;
    Ok(count > 1 && data.u16_at(c::BS_FORME_OFFSET)? == 0 && forme <= count)
}

/// Reads the evolutions of the 721 species, with Nincada's Shedinja.
pub(crate) fn read(
    rom: &mut Rom,
    path: &str,
    stats: &str,
    evolution_cro: &str,
    family: Family,
    formes: &Formes,
) -> Result<(Vec<Evolution>, EvolutionFiles), RomError> {
    let mut evolutions = Vec::new();
    let mut files = EvolutionFiles::default();
    for i in 1..=super::species::entry_count(family) {
        let data = rom.garc_file(path, false, i)?;
        let forme = formes.get(&i).copied();
        for slot in 0..SLOTS {
            let at = slot * SLOT_SIZE;
            let method = data.u16_at(at)?;
            let mut to = data.u16_at(at + 4)?;
            if !(1..=c::EVOLUTION_METHOD_COUNT).contains(&method) || to < 1 {
                continue;
            }
            let Some(&kind) = c::EVOLUTION_METHODS
                .get(usize::from(method) - 1)
                .or(if method == c::MEOWSTIC_F_METHOD {
                    Some(&EvolutionType::LevelFemaleOnly)
                } else {
                    None
                })
            else {
                continue;
            };
            if kind == EvolutionType::HighBeauty {
                continue;
            }
            if let Some(f) = forme {
                if i > c::SPECIES_COUNT
                    && has_forme(rom, stats, formes, to, f.number)?
                    && !files.copies.contains(&(f.base, i))
                {
                    files.copies.push((f.base, i));
                }
                continue;
            }
            if i > c::SPECIES_COUNT {
                continue;
            }
            let from = DexNumber::try_from(i).unwrap_or_default();
            if from == c::ESPURR && method == c::MEOWSTIC_F_METHOD {
                to = c::MEOWSTIC_F;
            }
            evolutions.push(Evolution::new(from, to, kind, data.u16_at(at + 2)?));
        }
        if i == usize::from(c::BURMY) {
            evolutions.extend(c::WORMADAM_FORMES.map(|to| Evolution::new(c::BURMY, to, EvolutionType::None, 0)));
        }
        if i == usize::from(c::NINCADA) {
            let (primary, extra) = shedinja_species(rom, evolution_cro)?;
            if evolutions.iter().any(|e| e.from == c::NINCADA && e.to == primary) {
                evolutions.push(Evolution::new(c::NINCADA, extra, EvolutionType::LevelIsExtra, 20));
            }
        }
    }
    Ok((evolutions, files))
}

/// The game's method number of an evolution type.
fn method(kind: EvolutionType) -> Option<u16> {
    c::EVOLUTION_METHODS
        .iter()
        .position(|&k| k == kind)
        .and_then(|i| u16::try_from(i + 1).ok())
}

/// Writes every species' evolutions; patches the code for Nincada's extra evolution.
pub(crate) fn write(
    rom: &mut Rom,
    path: &str,
    evolution_cro: &str,
    family: Family,
    files: &EvolutionFiles,
    evolutions: &[Evolution],
) -> Result<(), RomError> {
    for n in 1..=DexNumber::try_from(c::SPECIES_COUNT).unwrap_or_default() {
        let mut evos: Vec<(EvolutionType, u16, u16)> = Vec::new();
        let mut night = Vec::new();
        for e in evolutions.iter().filter(|e| e.from == n) {
            if e.kind == EvolutionType::Item {
                evos.push((EvolutionType::ItemDay, e.parameter, e.to));
                night.push((EvolutionType::ItemNight, e.parameter, e.to));
            } else {
                evos.push((e.kind, e.parameter, e.to));
            }
        }
        evos.extend(night);
        if n == c::NINCADA && evos.len() >= 2 {
            patch_shedinja(rom, evolution_cro, evos[0].2, evos[1].2)?;
            evos.remove(1);
        } else if n == c::FEEBAS
            && family == Family::Oras
            && let Some(&(_, _, to)) = evos.first()
        {
            evos.push((EvolutionType::HighBeauty, c::FEEBAS_BEAUTY, to));
        }
        let index = usize::from(n);
        let mut data = rom.garc_file(path, false, index)?;
        let mut written = 0;
        for &(kind, parameter, to) in &evos {
            let Some(mut m) = method(kind) else { continue };
            if n == c::ESPURR && kind == EvolutionType::LevelFemaleOnly {
                m = c::MEOWSTIC_F_METHOD;
            }
            let at = written * SLOT_SIZE;
            data.set_u16(at, m)?;
            data.set_u16(at + 2, parameter)?;
            data.set_u16(at + 4, to)?;
            written += 1;
            if written == SLOTS {
                break;
            }
        }
        for slot in written..SLOTS {
            let at = slot * SLOT_SIZE;
            data.get_mut(at..at + SLOT_SIZE)
                .ok_or_else(|| RomError::Malformed("short evolution file".into()))?
                .fill(0);
        }
        for &(_, file) in files.copies.iter().filter(|&&(base, _)| base == n) {
            let mut copy = rom.garc_file(path, false, file)?;
            let len = SLOTS * SLOT_SIZE;
            if copy.len() >= len && data.len() >= len {
                copy[..len].copy_from_slice(&data[..len]);
                rom.set_garc_file(path, false, file, copy)?;
            }
        }
        rom.set_garc_file(path, false, index, data)?;
    }
    Ok(())
}

/// The pc-relative load the patch puts in place of the `mov r1, #0x124` that makes Shedinja.
const SHEDINJA_LOAD: [u8; 4] = [0xB4, 0x10, 0x9F, 0xE5];
const NINJASK: DexNumber = 291;

/// The species whose evolution creates the extra species, and the extra species: Ninjask and Shedinja unless the
/// code was patched.
fn shedinja_species(rom: &mut Rom, evolution_cro: &str) -> Result<(DexNumber, DexNumber), RomError> {
    let cro = rom.file(evolution_cro)?;
    let primary = match find_after(cro, c::NINJASK_SPECIES_PREFIX) {
        Some(at) => u16::from(cro.u8_at(at)?) << 8 | u16::from(cro.u8_at(at + 4)?),
        None => NINJASK,
    };
    let code = rom.code();
    let extra = match find_after(code, c::SHEDINJA_SPECIES_PREFIX) {
        Some(at) if code.get(at..at + 4) == Some(&SHEDINJA_LOAD[..]) => {
            u16::try_from(code.u32_at(at + 188)?).unwrap_or(c::SHEDINJA)
        }
        _ => c::SHEDINJA,
    };
    Ok((primary, extra))
}

/// Makes evolving into `primary` create `extra`, as evolving Nincada into Ninjask creates Shedinja.
fn patch_shedinja(rom: &mut Rom, evolution_cro: &str, primary: DexNumber, extra: DexNumber) -> Result<(), RomError> {
    // The evolution scene checks for Ninjask with `sub r0, r1, #0x100` and `subs r0, r0, #0x23`.
    if let Some(at) = find_after(rom.file(evolution_cro)?, c::NINJASK_SPECIES_PREFIX) {
        let [lower, upper] = primary.to_le_bytes();
        let cro = rom.file_mut(evolution_cro)?;
        cro.set_u8(at, upper)?;
        cro.set_u8(at + 4, lower)?;
    }
    // The executable makes Shedinja with `mov r1, #0x124`, which cannot hold every species. The code after it moves
    // up a word to make room for a constant that a pc-relative load reads instead.
    if let Some(at) = find_after(rom.code(), c::SHEDINJA_SPECIES_PREFIX) {
        let code = rom.code_mut();
        if code.len() < at + 192 {
            return Err(RomError::Malformed("the Shedinja code is cut off".into()));
        }
        if code[at..at + 4] == SHEDINJA_LOAD {
            // Patched before: only the constant changes.
            return code.set_u32(at + 188, u32::from(extra));
        }
        code.copy_within(at + 84..at + 192, at + 80);
        // The branches that moved up a word point a word further.
        for bl in [80, 92, 104, 116, 128, 140, 152, 164, 176] {
            code[at + bl] = code[at + bl].wrapping_add(1);
        }
        code.set_u32(at + 188, u32::from(extra))?;
        code[at..at + 4].copy_from_slice(&[0xB4, 0x10, 0x9F, 0xE5]);
    }
    Ok(())
}

/// Points every species' baby table entry at the first species of its evolution line.
pub(crate) fn write_baby_table(rom: &mut Rom, path: &str, evolutions: &[Evolution]) -> Result<(), RomError> {
    let mut master = rom.garc_file(path, false, c::SPECIES_COUNT + 1)?;
    for i in 1..=c::SPECIES_COUNT {
        let mut baby = DexNumber::try_from(i).unwrap_or_default();
        // Evolution lines are short; the bound guards against loops.
        for _ in 0..16 {
            match evolutions.iter().find(|e| e.to == baby) {
                Some(e) => baby = e.from,
                None => break,
            }
        }
        let mut file = rom.garc_file(path, false, i)?;
        file.set_u16(0, baby)?;
        master.set_u16(i * 2, baby)?;
        rom.set_garc_file(path, false, i, file)?;
    }
    rom.set_garc_file(path, false, c::SPECIES_COUNT + 1, master)
}
