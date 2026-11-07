//! In-game trades: a table of 0x24-byte entries in the executable, with the nicknames and original trainer names
//! in one text file.
//!
//! An entry holds the given species (word at 0), its IVs (bytes 5 to 10, 0xFF for a random IV), the original
//! trainer's id (word at 0xE), the held item (word at 0x10, 0 for none) and the requested species (word at 0x20, 0
//! for any).

use ppr_model::{DexNumber, InGameTrade, Species};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom, find_after, need};
use crate::RomError;
use crate::entries::RomEntry;

/// The longest nickname and original trainer name a trade can have.
pub(crate) const MAX_NICKNAME_LENGTH: usize = 10;
pub(crate) const MAX_OT_NAME_LENGTH: usize = 7;

const GIVEN: usize = 0;
const IVS: usize = 5;
const OT_ID: usize = 0xE;
const HELD_ITEM: usize = 0x10;
const REQUESTED: usize = 0x20;

/// Where the trades and their names are.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Table {
    /// The first entry in the executable.
    pub(crate) at: usize,
    pub(crate) count: usize,
    /// The text file of the names, and the first nickname in it.
    pub(crate) text_file: usize,
    pub(crate) text_extra: usize,
}

impl Table {
    /// The table, or `None` when the executable doesn't have it where expected.
    pub(crate) fn find(code: &[u8], entry: &RomEntry, family: Family) -> Result<Option<Self>, RomError> {
        let Some(at) = find_after(code, family.trades_prefix()) else {
            return Ok(None);
        };
        Ok(Some(Self {
            at,
            count: need(entry, "IngameTradeCount")?,
            text_file: need(entry, "IngameTradesTextOffset")?,
            text_extra: need(entry, "IngameTradesTextExtraOffset")?,
        }))
    }
}

fn name(texts: &[String], i: usize) -> Result<String, RomError> {
    texts
        .get(i)
        .cloned()
        .ok_or_else(|| RomError::Malformed(format!("no trade text {i}")))
}

/// The trades.
pub(crate) fn read(rom: &mut Rom, text: &str, table: Table) -> Result<Vec<InGameTrade>, RomError> {
    let texts = rom.texts(text, table.text_file)?;
    let code = rom.code();
    (0..table.count)
        .map(|i| {
            let at = table.at + i * c::TRADE_SIZE;
            let requested = code.u16_at(at + REQUESTED)?;
            let item = code.u16_at(at + HELD_ITEM)?;
            Ok(InGameTrade {
                requested: (requested != 0).then_some(requested),
                given: code.u16_at(at + GIVEN)?,
                nickname: name(&texts, table.text_extra + i)?,
                ot_name: name(&texts, table.text_extra + table.count + i)?,
                ot_id: code.u16_at(at + OT_ID)?.into(),
                ivs: (0..6).map(|iv| code.u8_at(at + IVS + iv)).collect::<Result<_, _>>()?,
                held_item: (item != 0).then_some(item),
            })
        })
        .collect()
}

/// Writes the changed trades; in X and Y, two trades name their species in story text, which follows the change.
#[expect(clippy::too_many_arguments, reason = "the files and data a trade is in")]
pub(crate) fn write(
    rom: &mut Rom,
    text: &str,
    story: &str,
    table: Table,
    family: Family,
    species: &[Species],
    old: &[InGameTrade],
    new: &[InGameTrade],
) -> Result<(), RomError> {
    let mut texts = rom.texts(text, table.text_file)?;
    for (i, (before, trade)) in old.iter().zip(new).enumerate().take(table.count) {
        if before == trade {
            continue;
        }
        let at = table.at + i * c::TRADE_SIZE;
        let code = rom.code_mut();
        code.set_u16(at + GIVEN, trade.given)?;
        for (iv, &value) in trade.ivs.iter().take(6).enumerate() {
            code.set_u8(at + IVS + iv, value)?;
        }
        code.set_u16(at + OT_ID, RomError::short_ot_id(trade.ot_id)?)?;
        code.set_u16(at + HELD_ITEM, trade.held_item.unwrap_or(0))?;
        code.set_u16(at + REQUESTED, trade.requested.unwrap_or(0))?;
        let set = |texts: &mut Vec<String>, at: usize, value: &str| -> Result<(), RomError> {
            value.clone_into(
                texts
                    .get_mut(at)
                    .ok_or_else(|| RomError::Malformed(format!("no trade text {at}")))?,
            );
            Ok(())
        };
        set(&mut texts, table.text_extra + i, &trade.nickname)?;
        set(&mut texts, table.text_extra + table.count + i, &trade.ot_name)?;
        if family == Family::Xy
            && let Some(&(_, file)) = c::XY_HARDCODED_TRADES.iter().find(|&&(t, _)| t == i)
        {
            update_story_text(rom, story, file, species, before, trade)?;
        }
    }
    rom.set_texts(text, table.text_file, &texts)
}

/// Replaces the old requested and given species' names with the new ones in a story text file. The text is not
/// reflowed.
fn update_story_text(
    rom: &mut Rom,
    story: &str,
    file: usize,
    species: &[Species],
    old: &InGameTrade,
    new: &InGameTrade,
) -> Result<(), RomError> {
    let name_of = |n: DexNumber| species.iter().find(|s| s.number == n).map(|s| s.name.clone());
    let old_requested = old.requested.and_then(name_of);
    let new_requested = new.requested.and_then(name_of);
    let old_given = name_of(old.given);
    let new_given = name_of(new.given);
    let mut texts = rom.texts(story, file)?;
    for line in &mut texts {
        if let (Some(from), Some(to)) = (&old_requested, &new_requested)
            && line.contains(from.as_str())
        {
            *line = line.replace(from.as_str(), to);
        }
        if let (Some(from), Some(to)) = (&old_given, &new_given)
            && line.contains(from.as_str())
        {
            *line = line.replace(from.as_str(), to);
        }
    }
    rom.set_texts(story, file, &texts)
}
