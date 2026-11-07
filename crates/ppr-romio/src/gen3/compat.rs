//! Which species can learn which TMs, HMs and move tutor moves, read and written.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, Species};

use super::tms::TM_COUNT;
use super::{Family, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;

/// The number of HMs.
pub(super) const HM_COUNT: usize = 8;
/// Bytes of TM/HM flags each internal species has.
const TM_HM_BYTES: usize = 8;

/// Reads `count` flags in `bytes` bytes, lowest bit first.
fn read_flags(rom: &GbaRom, at: usize, bytes: usize, count: usize) -> Result<Vec<bool>, RomError> {
    let data = rom.slice(at, bytes)?;
    Ok((0..count).map(|i| data[i / 8] >> (i % 8) & 1 == 1).collect())
}

/// The flags as bytes, lowest bit first; bits past the flags are cleared.
fn flag_bytes(flags: &[bool], bytes: usize) -> Vec<u8> {
    let mut data = vec![0; bytes];
    for (i, _) in flags.iter().enumerate().filter(|&(_, &on)| on) {
        data[i / 8] |= 1 << (i % 8);
    }
    data
}

/// Reads the TM/HM compatibility: TM01 to TM50, then HM01 to HM08, for each species.
pub(super) fn read_tm_hm(
    rom: &GbaRom,
    entry: &RomEntry,
    species: &[Species],
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    let table = need(entry, "PokemonTMHMCompat")?;
    species
        .iter()
        .map(|sp| {
            let internal =
                internal_of(sp.number).ok_or_else(|| RomError::Malformed(format!("no species {}", sp.number)))?;
            let at = table + usize::from(internal) * TM_HM_BYTES;
            Ok((sp.number, read_flags(rom, at, TM_HM_BYTES, TM_COUNT + HM_COUNT)?))
        })
        .collect()
}

/// Writes the TM/HM compatibility.
pub(super) fn write_tm_hm(
    rom: &mut GbaRom,
    entry: &RomEntry,
    compat: &BTreeMap<DexNumber, Vec<bool>>,
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    let table = need(entry, "PokemonTMHMCompat")?;
    for (&n, flags) in compat {
        if flags.len() != TM_COUNT + HM_COUNT {
            return Err(RomError::Malformed(format!(
                "species {n} needs {} TM/HM flags",
                TM_COUNT + HM_COUNT
            )));
        }
        let internal = internal_of(n).ok_or_else(|| RomError::Malformed(format!("no species {n}")))?;
        rom.write(
            table + usize::from(internal) * TM_HM_BYTES,
            &flag_bytes(flags, TM_HM_BYTES),
        )?;
    }
    Ok(())
}

/// Where the tutor flags are, how many bytes each species has and how many tutors there are: the flags follow the tutor moves, in whole bytes. `None` for Ruby and
/// Sapphire, which have no tutors.
fn tutor_table(entry: &RomEntry, family: Family) -> Result<Option<(usize, usize, usize)>, RomError> {
    if family == Family::RubySapphire {
        return Ok(None);
    }
    let count = need(entry, "MoveTutorMoves")?;
    // Right after the tutor moves, unless FireRed hack support found the table.
    let table = match entry.int("MoveTutorCompatibility") {
        Some(_) => need(entry, "MoveTutorCompatibility")?,
        None => need(entry, "MoveTutorData")? + count * 2,
    };
    Ok(Some((table, count.div_ceil(8), count)))
}

/// Reads the move tutor compatibility: a flag a tutor for each species; empty without tutors.
pub(super) fn read_tutors(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    species: &[Species],
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    let Some((table, bytes, count)) = tutor_table(entry, family)? else {
        return Ok(BTreeMap::new());
    };
    species
        .iter()
        .map(|sp| {
            let internal =
                internal_of(sp.number).ok_or_else(|| RomError::Malformed(format!("no species {}", sp.number)))?;
            Ok((
                sp.number,
                read_flags(rom, table + usize::from(internal) * bytes, bytes, count)?,
            ))
        })
        .collect()
}

/// Writes the move tutor compatibility.
pub(super) fn write_tutors(
    rom: &mut GbaRom,
    entry: &RomEntry,
    family: Family,
    compat: &BTreeMap<DexNumber, Vec<bool>>,
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    let Some((table, bytes, count)) = tutor_table(entry, family)? else {
        return Ok(());
    };
    for (&n, flags) in compat {
        if flags.len() != count {
            return Err(RomError::Malformed(format!(
                "species {n} needs {count} move tutor flags"
            )));
        }
        let internal = internal_of(n).ok_or_else(|| RomError::Malformed(format!("no species {n}")))?;
        rom.write(table + usize::from(internal) * bytes, &flag_bytes(flags, bytes))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_round_trip_lowest_bit_first() {
        let mut flags = vec![false; 58];
        flags[0] = true;
        flags[9] = true;
        flags[57] = true;
        let bytes = flag_bytes(&flags, 8);
        assert_eq!(bytes, [1, 2, 0, 0, 0, 0, 0, 2]);
        let rom = GbaRom::new(bytes);
        assert_eq!(read_flags(&rom, 0, 8, 58).expect("reads"), flags);
    }
}
