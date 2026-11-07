//! In-game trades, read and written: a table of
//! 60-byte entries (`struct InGameTrade` in pokeemerald), some of which the ROM entry marks unused.
//!
//! Deliberate choice: names are read only within their fields, not on until a terminator.

use ppr_model::{DexNumber, InGameTrade};

use super::items;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;

const ENTRY_SIZE: usize = 60;
const NICKNAME: usize = 0;
const NICKNAME_SIZE: usize = 12;
const SPECIES: usize = 12;
const IVS: usize = 14;
const IV_COUNT: usize = 6;
const OT_ID: usize = 24;
const HELD_ITEM: usize = 40;
const OT_NAME: usize = 43;
const OT_NAME_SIZE: usize = 11;
const REQUESTED: usize = 56;

/// The longest nickname and original trainer name a trade can have, the same for every Gen 3 game.
pub(super) const MAX_NICKNAME_LENGTH: usize = 10;
pub(super) const MAX_OT_NAME_LENGTH: usize = 7;

/// Byte after a name that is shorter than its field, and the padding after that.
const TERMINATOR: u8 = 0xFF;
const PADDING: u8 = 0x00;

/// Offsets of the used trade entries.
pub(super) fn entries(entry: &RomEntry) -> Vec<usize> {
    let int = |key: &str| entry.int(key).and_then(|v| usize::try_from(v).ok());
    let (Some(table), Some(size)) = (int("TradeTableOffset"), int("TradeTableSize")) else {
        return Vec::new();
    };
    let unused: Vec<usize> = entry
        .array("TradesUnused")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| usize::try_from(i).ok())
        .collect();
    (0..size)
        .filter(|i| !unused.contains(i))
        .map(|i| table + i * ENTRY_SIZE)
        .collect()
}

/// The in-game trades.
pub(super) fn read(
    rom: &GbaRom,
    entries: &[usize],
    text: &TextTable,
    internal_to_dex: &[DexNumber],
) -> Result<Vec<InGameTrade>, RomError> {
    let species = |at: usize| -> Result<DexNumber, RomError> {
        let internal = rom.u16(at)?;
        internal_to_dex
            .get(usize::from(internal))
            .copied()
            .filter(|&n| n != 0)
            .ok_or_else(|| RomError::Malformed(format!("no species {internal} at {at:#x}")))
    };
    entries
        .iter()
        .map(|&at| {
            let held = rom.u16(at + HELD_ITEM)?;
            Ok(InGameTrade {
                requested: Some(species(at + REQUESTED)?),
                given: species(at + SPECIES)?,
                nickname: text.decode(rom.slice(at + NICKNAME, NICKNAME_SIZE)?, NICKNAME_SIZE),
                ot_name: text.decode(rom.slice(at + OT_NAME, OT_NAME_SIZE)?, OT_NAME_SIZE),
                ot_id: rom.u16(at + OT_ID)?.into(),
                ivs: rom.slice(at + IVS, IV_COUNT)?.to_vec(),
                held_item: (held != 0).then(|| items::standard(held)),
            })
        })
        .collect()
}

/// Writes the encoded name, cut to the field, then a terminator and padding if it is
/// shorter.
pub(super) fn write_name(
    rom: &mut GbaRom,
    at: usize,
    name: &str,
    size: usize,
    text: &TextTable,
) -> Result<(), RomError> {
    let mut bytes = text.encode(name);
    bytes.truncate(size);
    if bytes.len() < size {
        bytes.push(TERMINATOR);
    }
    bytes.resize(size, PADDING);
    rom.write(at, &bytes)
}

/// Writes every field of every used entry.
pub(super) fn write(
    rom: &mut GbaRom,
    entries: &[usize],
    trades: &[InGameTrade],
    text: &TextTable,
    internal: impl Fn(DexNumber) -> Result<u16, RomError>,
) -> Result<(), RomError> {
    if trades.len() != entries.len() {
        return Err(RomError::Malformed(format!(
            "the game has {} in-game trades, got {}",
            entries.len(),
            trades.len()
        )));
    }
    for (&at, trade) in entries.iter().zip(trades) {
        write_name(rom, at + NICKNAME, &trade.nickname, NICKNAME_SIZE, text)?;
        rom.write_u16(at + SPECIES, internal(trade.given)?)?;
        let mut ivs = trade.ivs.clone();
        ivs.resize(IV_COUNT, 0);
        rom.write(at + IVS, &ivs)?;
        rom.write_u16(at + OT_ID, RomError::short_ot_id(trade.ot_id)?)?;
        let held = match trade.held_item {
            Some(id) => items::internal(id).ok_or_else(|| RomError::Malformed(format!("no Gen 3 item {id}")))?,
            None => 0,
        };
        rom.write_u16(at + HELD_ITEM, held)?;
        write_name(rom, at + OT_NAME, &trade.ot_name, OT_NAME_SIZE, text)?;
        let requested = trade
            .requested
            .ok_or_else(|| RomError::Malformed("Gen 3 trades always ask for a species".into()))?;
        rom.write_u16(at + REQUESTED, internal(requested)?)?;
    }
    Ok(())
}
