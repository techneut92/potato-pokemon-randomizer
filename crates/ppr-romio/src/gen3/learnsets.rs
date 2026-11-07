//! Moves learnt by level and egg moves, read and written.
//!
//! Deliberate choices:
//! - Egg moves are only written when there are as many as the game had, so the list keeps its end mark.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, LevelUpMove, MoveId, Species};

use super::need;
use super::space::Space;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;

/// What ends a learnset in the games and with Jambo's learnset hack.
const VANILLA_END: [u8; 2] = [0xFF, 0xFF];
const JAMBO_END: [u8; 3] = [0x00, 0x00, 0xFF];
/// Where `FireRed` 1.0 points to its learnset table, and the signature and pointer of Jambo's learnset hack, which
/// stores each move in three bytes.
const FIRE_RED_TABLE_POINTER: usize = 0x3_EA7C;
const JAMBO_SIGNATURE_AT: usize = 0x3_EB20;
const JAMBO_SIGNATURE: u32 = 0x4708_4918;
const JAMBO_ROUTINE_POINTER: usize = 0x3_EB84;
const JAMBO_TABLE_IN_ROUTINE: usize = 75;
/// An egg move list entry above this starts a species: its internal index plus this.
const EGG_SPECIES_BASE: u16 = 20_000;
const EGG_END: u16 = 0xFFFF;

/// Where the learnsets are, and how they are stored.
#[derive(Debug, Clone, Copy)]
pub(super) struct Format {
    /// The table of pointers to each internal species' learnset.
    table: usize,
    /// Jambo's learnset hack: three bytes a move.
    jambo: bool,
}

impl Format {
    /// The learnset table as the game code points to it for `FireRed` 1.0, which hacks build on; the ROM entry's
    /// table otherwise.
    pub(super) fn of(rom: &GbaRom, entry: &RomEntry) -> Result<Self, RomError> {
        if entry.code != "BPRE" || entry.version != 0 {
            return Ok(Self {
                table: need(entry, "PokemonMovesets")?,
                jambo: false,
            });
        }
        if rom.u32(JAMBO_SIGNATURE_AT)? == JAMBO_SIGNATURE {
            let routine = rom.pointer(JAMBO_ROUTINE_POINTER)?;
            Ok(Self {
                table: rom.pointer(routine + JAMBO_TABLE_IN_ROUTINE)?,
                jambo: true,
            })
        } else {
            Ok(Self {
                table: rom.pointer(FIRE_RED_TABLE_POINTER)?,
                jambo: false,
            })
        }
    }

    /// The table of pointers to each internal species' learnset.
    pub(super) fn table(self) -> usize {
        self.table
    }

    fn end(self) -> &'static [u8] {
        if self.jambo { &JAMBO_END } else { &VANILLA_END }
    }

    /// The learnset at `at`, up to its end mark.
    fn read_at(self, rom: &GbaRom, mut at: usize) -> Result<Vec<LevelUpMove>, RomError> {
        let mut moves = Vec::new();
        loop {
            if self.jambo {
                if rom.slice(at, 3)? == JAMBO_END {
                    return Ok(moves);
                }
                moves.push(LevelUpMove::new(rom.slice(at + 2, 1)?[0], rom.u16(at)?));
                at += 3;
            } else {
                let [low, high] = [rom.slice(at, 1)?[0], rom.slice(at + 1, 1)?[0]];
                if [low, high] == VANILLA_END {
                    return Ok(moves);
                }
                let move_id = u16::from(low) + if high & 1 == 1 { 0x100 } else { 0 };
                moves.push(LevelUpMove::new(high >> 1, move_id));
                at += 2;
            }
        }
    }

    /// A learnset as bytes, end mark included.
    fn bytes(self, moves: &[LevelUpMove]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for m in moves {
            if self.jambo {
                bytes.extend(m.move_id.to_le_bytes());
                bytes.push(m.level);
            } else {
                let [low, high] = m.move_id.to_le_bytes();
                bytes.push(low);
                bytes.push((m.level << 1) | u8::from(high > 0));
            }
        }
        bytes.extend(self.end());
        bytes
    }

    fn pointer_offset(self, internal: u16) -> usize {
        self.table + usize::from(internal) * 4
    }
}

/// The moves each species learns by level.
pub(super) fn read(
    rom: &GbaRom,
    format: Format,
    species: &[Species],
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let mut learnsets = BTreeMap::new();
    for sp in species {
        let internal =
            internal_of(sp.number).ok_or_else(|| RomError::Malformed(format!("no species {}", sp.number)))?;
        let at = rom.pointer(format.pointer_offset(internal))?;
        learnsets.insert(sp.number, format.read_at(rom, at)?);
    }
    Ok(learnsets)
}

/// Writes the learnsets: each species' learnset moves to free space.
pub(super) fn write(
    space: &mut Space<'_>,
    format: Format,
    species: &[Species],
    learnsets: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    for sp in species {
        let Some(moves) = learnsets.get(&sp.number) else {
            continue;
        };
        let internal =
            internal_of(sp.number).ok_or_else(|| RomError::Malformed(format!("no species {}", sp.number)))?;
        let pointer_offset = format.pointer_offset(internal);
        let old = space.rom.pointer(pointer_offset)?;
        let old_length = format.bytes(&format.read_at(space.rom, old)?).len();
        space.rewrite(pointer_offset, old_length, &format.bytes(moves), &[])?;
    }
    Ok(())
}

/// The egg moves.
pub(super) fn read_egg_moves(
    rom: &GbaRom,
    entry: &RomEntry,
    internal_to_dex: &[DexNumber],
) -> Result<BTreeMap<DexNumber, Vec<MoveId>>, RomError> {
    let mut at = need(entry, "EggMoves")?;
    let mut egg_moves = BTreeMap::new();
    let mut current: (usize, Vec<MoveId>) = (0, Vec::new());
    let mut finish = |(internal, moves): (usize, Vec<MoveId>)| {
        if !moves.is_empty() {
            egg_moves.insert(internal_to_dex.get(internal).copied().unwrap_or_default(), moves);
        }
    };
    loop {
        let value = rom.u16(at)?;
        if value == EGG_END {
            break;
        }
        if value > EGG_SPECIES_BASE {
            finish(std::mem::replace(
                &mut current,
                (usize::from(value - EGG_SPECIES_BASE), Vec::new()),
            ));
        } else {
            current.1.push(value);
        }
        at += 2;
    }
    finish(current);
    Ok(egg_moves)
}

/// Writes the egg moves: the species in national order, each with its moves.
pub(super) fn write_egg_moves(
    rom: &mut GbaRom,
    entry: &RomEntry,
    old: &BTreeMap<DexNumber, Vec<MoveId>>,
    egg_moves: &BTreeMap<DexNumber, Vec<MoveId>>,
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    let words = |m: &BTreeMap<DexNumber, Vec<MoveId>>| m.values().map(|v| v.len() + 1).sum::<usize>();
    if words(egg_moves) != words(old) {
        return Err(RomError::Malformed("egg moves must keep their number".into()));
    }
    let mut at = need(entry, "EggMoves")?;
    for (&species, moves) in egg_moves {
        let internal = internal_of(species).ok_or_else(|| RomError::Malformed(format!("no species {species}")))?;
        rom.write_u16(at, internal + EGG_SPECIES_BASE)?;
        at += 2;
        for &m in moves {
            rom.write_u16(at, m)?;
            at += 2;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learnsets_round_trip_in_both_formats() {
        let moves = [
            LevelUpMove::new(1, 33),
            LevelUpMove::new(50, 300),
            LevelUpMove::new(100, 354),
        ];
        for jambo in [false, true] {
            let format = Format { table: 0, jambo };
            let bytes = format.bytes(&moves);
            let mut rom = GbaRom::new(vec![0; 64]);
            rom.write(8, &bytes).expect("fits");
            assert_eq!(format.read_at(&rom, 8).expect("reads"), moves, "jambo {jambo}");
        }
        assert_eq!(
            Format { table: 0, jambo: false }.bytes(&moves[1..2]),
            [44, 101, 0xFF, 0xFF]
        );
    }
}
