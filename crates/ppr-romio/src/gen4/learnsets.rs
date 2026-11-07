//! Moves learnt by level and egg moves, read and written.
//!
//! A learnset is a list of words, each a move's low byte and then its level shifted left by one, with the move's
//! ninth bit as bit 0; `FFFF` ends it. Egg moves are a list of words: a species plus 20000, then its moves; `FFFF`
//! ends the list. `HeartGold` and `SoulSilver` keep it in a file of its own, the others in the field overlay.
//!
//! Deliberate choices:
//! - Only the learnsets that changed are rewritten, each in a file of its own size (a multiple of four bytes),
//!   rather than the whole archive being built anew.
//! - Egg moves end with the end mark when there are fewer than the game had, and more than fit are refused.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, LevelUpMove, MoveId};

use super::Rom;
use super::constants as c;
use super::files::{find_after, put_word, word};
use crate::RomError;

const END: u16 = 0xFFFF;

/// The learnsets of species 1 to 493 and of the alternate formes after them.
pub(super) fn read(rom: &mut Rom) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let path = rom.entry.file("PokemonMovesets")?;
    let numbers = rom.species_numbers();
    let narc = rom.files.narc(&path)?;
    numbers
        .map(|number| {
            let data = narc
                .files
                .get(c::species_file(number))
                .ok_or_else(|| RomError::Malformed(format!("no learnset for species {number}")))?;
            Ok((number, decode(data)?))
        })
        .collect()
}

fn decode(data: &[u8]) -> Result<Vec<LevelUpMove>, RomError> {
    let mut moves = Vec::new();
    for at in (0..).step_by(2) {
        let w = word(data, at)?;
        if w == END {
            return Ok(moves);
        }
        let [low, high] = w.to_le_bytes();
        let move_id = MoveId::from(low) + if high & 1 == 1 { 256 } else { 0 };
        moves.push(LevelUpMove::new(high >> 1, move_id));
    }
    Ok(moves)
}

fn encode(moves: &[LevelUpMove]) -> Result<Vec<u8>, RomError> {
    let mut data = Vec::with_capacity(moves.len() * 2 + 4);
    for m in moves {
        if m.level > 0x7F || m.move_id > 0x1FF {
            return Err(RomError::Malformed(format!(
                "Gen 4 can't store move {} at level {}",
                m.move_id, m.level
            )));
        }
        let [low, high] = m.move_id.to_le_bytes();
        data.extend([low, (m.level << 1) | (high & 1)]);
    }
    data.extend(END.to_le_bytes());
    if !data.len().is_multiple_of(4) {
        data.extend([0, 0]);
    }
    Ok(data)
}

/// Rewrites the learnsets that changed.
pub(super) fn write(
    rom: &mut Rom,
    loaded: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
    learnsets: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
) -> Result<(), RomError> {
    let path = rom.entry.file("PokemonMovesets")?;
    let numbers = rom.species_numbers();
    for (&number, moves) in learnsets {
        if loaded.get(&number) == Some(moves) || !numbers.contains(&number) {
            continue;
        }
        *rom.files.narc_file(&path, c::species_file(number))? = encode(moves)?;
    }
    Ok(())
}

/// Where the egg moves are: a file of the egg move NARC, or the field overlay after its prefix.
enum EggMoves {
    File(String),
    Overlay(usize, usize),
}

fn egg_moves_place(rom: &mut Rom) -> Result<Option<EggMoves>, RomError> {
    if !rom.family().is_dppt() {
        return Ok(Some(EggMoves::File(rom.entry.file("EggMoves")?)));
    }
    let number = rom.entry.need("FieldOvlNumber")?;
    let overlay = rom.files.overlay(number)?;
    Ok(find_after(overlay, c::DPPT_EGG_MOVE_TABLE_PREFIX).map(|at| EggMoves::Overlay(number, at)))
}

fn egg_moves_data<'a>(rom: &'a mut Rom, place: &EggMoves) -> Result<(&'a mut Vec<u8>, usize), RomError> {
    match place {
        EggMoves::File(path) => Ok((rom.files.narc_file(path, 0)?, 0)),
        EggMoves::Overlay(number, at) => Ok((rom.files.overlay(*number)?, *at)),
    }
}

/// The egg moves, by species; species without any are left out.
pub(super) fn read_egg_moves(rom: &mut Rom) -> Result<BTreeMap<DexNumber, Vec<MoveId>>, RomError> {
    let Some(place) = egg_moves_place(rom)? else {
        return Ok(BTreeMap::new());
    };
    let (data, start) = egg_moves_data(rom, &place)?;
    let mut egg_moves = BTreeMap::new();
    let mut species = 0;
    let mut moves = Vec::new();
    for at in (start..).step_by(2) {
        let w = word(data, at)?;
        if w == c::EGG_MOVES_END {
            break;
        }
        if w > c::EGG_MOVES_SPECIES_BASE {
            if !moves.is_empty() {
                egg_moves.insert(species, std::mem::take(&mut moves));
            }
            species = w - c::EGG_MOVES_SPECIES_BASE;
        } else {
            moves.push(w);
        }
    }
    if !moves.is_empty() {
        egg_moves.insert(species, moves);
    }
    Ok(egg_moves)
}

/// Writes the egg moves in species order, with the end mark when there is room for it.
pub(super) fn write_egg_moves(rom: &mut Rom, egg_moves: &BTreeMap<DexNumber, Vec<MoveId>>) -> Result<(), RomError> {
    let Some(place) = egg_moves_place(rom)? else {
        return Ok(());
    };
    let (data, start) = egg_moves_data(rom, &place)?;
    let mut free = 0;
    while word(data, start + free * 2)? != c::EGG_MOVES_END {
        free += 1;
    }
    let mut words = Vec::new();
    for (&species, moves) in egg_moves {
        words.push(species + c::EGG_MOVES_SPECIES_BASE);
        words.extend(moves);
    }
    if words.len() > free {
        return Err(RomError::Malformed(format!(
            "{} egg move words don't fit the {free} the game has",
            words.len()
        )));
    }
    words.push(c::EGG_MOVES_END);
    for (i, w) in words.into_iter().enumerate() {
        put_word(data, start + i * 2, w)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn learnsets_round_trip_with_the_ninth_bit() {
        let moves = vec![LevelUpMove::new(1, 33), LevelUpMove::new(45, 300)];
        let data = encode(&moves).expect("fits");
        assert_eq!(data, [33, 2, 44, 91, 0xFF, 0xFF, 0, 0]);
        assert_eq!(decode(&data).expect("decodes"), moves);
        assert_eq!(encode(&[]).expect("fits"), [0xFF, 0xFF, 0, 0]);
    }
}
