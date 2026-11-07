//! Items: their names and prices (`ItemData`), the items lying in the field and hidden, the shops and the Pickup
//! table.
//!
//! Field items are in two script files: item balls in one, and hidden items in another (X and Y) or in a table in the
//! executable (Omega Ruby and Alpha Sapphire). Omega Ruby and Alpha Sapphire also have sparkling Mega Stones lying in
//! the field, in a third script; a Mega Stone listed twice gets the same new item both times. The shops and the
//! Pickup table are lists of item words in the executable, found by their first items, so the table of a game
//! whose first shop or Pickup items changed is not found again: the game then has no shops or Pickup items.

use std::collections::BTreeMap;

use ppr_model::{Item, ItemId, PriceFormat, Shop};

use super::amx::Amx;
use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom, find, find_after, need};
use crate::RomError;
use crate::entries::RomEntry;

/// The distance between two item words in a field item script.
const FIELD_ITEM_STRIDE: usize = 12;
/// The size of a hidden item entry of Omega Ruby and Alpha Sapphire, and where its item is.
const HIDDEN_ITEM_SIZE_ORAS: usize = 0xE;
const HIDDEN_ITEM_ITEM_ORAS: usize = 2;

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// Every item but the empty one.
pub(crate) fn read(names: &[String], family: Family) -> Vec<Item> {
    let (banned, bad): (&[ItemId], &[ItemId]) = match family {
        Family::Xy => (&c::BANNED_ITEMS_XY, &c::BAD_ITEMS_XY),
        Family::Oras => (&c::BANNED_ITEMS_ORAS, &c::BAD_ITEMS_ORAS),
    };
    names
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, name)| {
            let id = ItemId::try_from(i).ok()?;
            Some(Item {
                id,
                name: name.clone(),
                allowed: !banned.contains(&id),
                bad: bad.contains(&id),
                tm: (c::TM01..=c::TM92).contains(&id)
                    || (c::TM93..=c::TM95).contains(&id)
                    || (c::TM96..=c::TM100).contains(&id),
            })
        })
        .collect()
}

/// Where the field items are.
#[derive(Debug, Clone)]
pub(crate) struct FieldFiles<'a> {
    pub(crate) scripts: &'a str,
    pub(crate) normal: (usize, usize),
    /// The hidden item script and offset of X and Y.
    pub(crate) hidden: Option<(usize, usize)>,
    /// The Mega Stone script of Omega Ruby and Alpha Sapphire.
    pub(crate) mega_stones: Option<usize>,
}

impl<'a> FieldFiles<'a> {
    pub(crate) fn new(scripts: &'a str, entry: &RomEntry, family: Family) -> Result<Self, RomError> {
        Ok(Self {
            scripts,
            normal: (need(entry, "FieldItemsScriptNumber")?, need(entry, "FieldItemsOffset")?),
            hidden: match family {
                Family::Xy => Some((
                    need(entry, "HiddenItemsScriptNumber")?,
                    need(entry, "HiddenItemsOffset")?,
                )),
                Family::Oras => None,
            },
            mega_stones: match family {
                Family::Xy => None,
                Family::Oras => Some(need(entry, "MegaStoneItemScriptNumber")?),
            },
        })
    }
}

fn script_items(script: &Amx, start: usize) -> Result<Vec<ItemId>, RomError> {
    (start..script.data.len())
        .step_by(FIELD_ITEM_STRIDE)
        .map(|at| script.data.u16_at(at))
        .collect()
}

fn mega_stone_offsets() -> impl Iterator<Item = usize> {
    (0..c::MEGA_STONE_TABLE_LENGTH_ORAS)
        .map(|i| c::MEGA_STONE_TABLE_START_ORAS + i * c::MEGA_STONE_TABLE_ENTRY_SIZE_ORAS)
}

/// Every field item, allowed or not, in the order they are written.
pub(crate) fn read_field_ids(rom: &mut Rom, files: &FieldFiles<'_>) -> Result<Vec<ItemId>, RomError> {
    let (file, start) = files.normal;
    let mut out = script_items(&Amx::parse(&rom.garc_file(files.scripts, false, file)?)?, start)?;
    if let Some((file, start)) = files.hidden {
        out.extend(script_items(
            &Amx::parse(&rom.garc_file(files.scripts, false, file)?)?,
            start,
        )?);
    } else if let Some(at) = find_after(rom.code(), c::HIDDEN_ITEMS_PREFIX_ORAS) {
        for i in 0..c::HIDDEN_ITEM_COUNT_ORAS {
            out.push(
                rom.code()
                    .u16_at(at + i * HIDDEN_ITEM_SIZE_ORAS + HIDDEN_ITEM_ITEM_ORAS)?,
            );
        }
    }
    if let Some(file) = files.mega_stones {
        let script = Amx::parse(&rom.garc_file(files.scripts, false, file)?)?;
        for at in mega_stone_offsets() {
            out.push(script.data.u16_at(at)?);
        }
    }
    Ok(out)
}

fn is_allowed(items: &[Item], id: ItemId) -> bool {
    items.iter().any(|i| i.id == id && i.allowed)
}

/// The allowed field items, the ones the game data holds.
pub(crate) fn allowed_field_items(ids: &[ItemId], items: &[Item]) -> Vec<ItemId> {
    ids.iter().copied().filter(|&id| is_allowed(items, id)).collect()
}

fn write_script_items(
    rom: &mut Rom,
    scripts: &str,
    (file, start): (usize, usize),
    next: &mut impl Iterator<Item = ItemId>,
) -> Result<(), RomError> {
    let mut script = Amx::parse(&rom.garc_file(scripts, false, file)?)?;
    for at in (start..script.data.len()).step_by(FIELD_ITEM_STRIDE) {
        let id = next.next().ok_or_else(|| malformed("too few field items"))?;
        script.data.set_u16(at, id)?;
    }
    rom.set_garc_file(scripts, false, file, script.to_bytes()?)
}

/// Writes the field items: each allowed item of `ids` is replaced by the next of `items`, in order.
pub(crate) fn write_field_items(
    rom: &mut Rom,
    files: &FieldFiles<'_>,
    ids: &[ItemId],
    all_items: &[Item],
    items: &[ItemId],
) -> Result<(), RomError> {
    let mut new = items.iter().copied();
    let mut replaced = Vec::with_capacity(ids.len());
    for &id in ids {
        if is_allowed(all_items, id) {
            replaced.push(new.next().ok_or_else(|| malformed("too few field items"))?);
        } else {
            replaced.push(id);
        }
    }
    let mut next = replaced.into_iter();
    write_script_items(rom, files.scripts, files.normal, &mut next)?;
    if let Some(hidden) = files.hidden {
        write_script_items(rom, files.scripts, hidden, &mut next)?;
    } else if let Some(at) = find_after(rom.code(), c::HIDDEN_ITEMS_PREFIX_ORAS) {
        for i in 0..c::HIDDEN_ITEM_COUNT_ORAS {
            let id = next.next().ok_or_else(|| malformed("too few field items"))?;
            let offset = at + i * HIDDEN_ITEM_SIZE_ORAS + HIDDEN_ITEM_ITEM_ORAS;
            if rom.code().u16_at(offset)? != id {
                rom.code_mut().set_u16(offset, id)?;
            }
        }
    }
    if let Some(file) = files.mega_stones {
        let mut script = Amx::parse(&rom.garc_file(files.scripts, false, file)?)?;
        let mut chosen: BTreeMap<ItemId, ItemId> = BTreeMap::new();
        for at in mega_stone_offsets() {
            let old = script.data.u16_at(at)?;
            let id = next.next().ok_or_else(|| malformed("too few field items"))?;
            let id = *chosen.entry(old).or_insert(id);
            script.data.set_u16(at, id)?;
        }
        rom.set_garc_file(files.scripts, false, file, script.to_bytes()?)?;
    }
    Ok(())
}

/// Where the shops are and how many items each has.
#[derive(Debug, Clone)]
pub(crate) struct ShopTable {
    at: usize,
    sizes: Vec<usize>,
}

impl ShopTable {
    /// The table, or `None` when it is not found, as in a game whose first shop items changed.
    pub(crate) fn find(code: &[u8], entry: &RomEntry, family: Family) -> Result<Option<Self>, RomError> {
        let Some(at) = find(code, family.shop_items_locator()) else {
            return Ok(None);
        };
        let count = need(entry, "ShopCount")?;
        let sizes: Vec<usize> = entry
            .array("ShopItemSizes")
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| usize::try_from(v).ok())
            .take(count)
            .collect();
        if sizes.len() != count {
            return Err(malformed("ShopItemSizes is shorter than ShopCount"));
        }
        Ok(Some(Self { at, sizes }))
    }
}

/// The shops. The TM shops and the regular marts are the only ones that aren't special.
pub(crate) fn read_shops(
    code: &[u8],
    table: Option<&ShopTable>,
    entry: &RomEntry,
    family: Family,
) -> Result<Vec<Shop>, RomError> {
    let Some(table) = table else {
        return Ok(Vec::new());
    };
    let (names, main_game): (&[&str], &[usize]) = match family {
        Family::Xy => (&c::SHOP_NAMES_XY, &c::MAIN_GAME_SHOPS_XY),
        Family::Oras => (&c::SHOP_NAMES_ORAS, &c::MAIN_GAME_SHOPS_ORAS),
    };
    let not_special: Vec<i64> = [entry.array("TMShops"), entry.array("RegularShops")]
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    let mut at = table.at;
    let mut shops = Vec::with_capacity(table.sizes.len());
    for (i, &size) in table.sizes.iter().enumerate() {
        let items = (0..size)
            .map(|j| code.u16_at(at + j * 2))
            .collect::<Result<Vec<_>, _>>()?;
        at += size * 2;
        shops.push(Shop {
            name: (*names.get(i).ok_or_else(|| malformed(format!("no name for shop {i}")))?).to_owned(),
            items,
            main_game: main_game.contains(&i),
            special: !not_special.iter().any(|&s| usize::try_from(s).ok() == Some(i)),
        });
    }
    Ok(shops)
}

/// Writes the shops' items; each shop keeps its size, so a shop with another number of items fails.
pub(crate) fn write_shops(rom: &mut Rom, table: &ShopTable, shops: &[Shop]) -> Result<(), RomError> {
    if shops.len() != table.sizes.len() {
        return Err(malformed(format!(
            "{} shops, where the game has {}",
            shops.len(),
            table.sizes.len()
        )));
    }
    if let Some((shop, size)) = shops.iter().zip(&table.sizes).find(|(s, size)| s.items.len() != **size) {
        return Err(malformed(format!(
            "shop {} has {} items, but room for {size}",
            shop.name,
            shop.items.len()
        )));
    }
    let mut at = table.at;
    for (shop, &size) in shops.iter().zip(&table.sizes) {
        for (j, &id) in shop.items.iter().enumerate() {
            if rom.code().u16_at(at + j * 2)? != id {
                rom.code_mut().set_u16(at + j * 2, id)?;
            }
        }
        at += size * 2;
    }
    Ok(())
}

/// Item prices, stored in tens.
pub(crate) fn read_prices(rom: &mut Rom, path: &str) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let count = rom.garc(path, false)?.len();
    (1..count)
        .map(|i| {
            let id = ItemId::try_from(i).map_err(|_| malformed("item number"))?;
            let tens = u32::from(rom.garc_file(path, false, i)?.u16_at(0)?);
            Ok((id, tens * PriceFormat::TENS.step))
        })
        .collect()
}

/// Writes the prices that changed, in tens; prices that don't fit are written as [`PriceFormat::fit`] makes them.
pub(crate) fn write_prices(
    rom: &mut Rom,
    path: &str,
    old: &BTreeMap<ItemId, u32>,
    new: &BTreeMap<ItemId, u32>,
) -> Result<(), RomError> {
    let count = rom.garc(path, false)?.len();
    for (&id, &price) in new {
        let file = usize::from(id);
        if old.get(&id) == Some(&price) || file == 0 || file >= count {
            continue;
        }
        let mut data = rom.garc_file(path, false, file)?;
        data.set_u16(0, PriceFormat::TENS.stored_u16(price))?;
        rom.set_garc_file(path, false, file, data)?;
    }
    Ok(())
}

/// Where the Pickup table is, if it is found.
pub(crate) fn pickup_table(code: &[u8]) -> Option<usize> {
    find(code, c::PICKUP_TABLE_LOCATOR)
}

/// The Pickup items.
pub(crate) fn read_pickup(code: &[u8], table: Option<usize>) -> Result<Vec<ItemId>, RomError> {
    let Some(at) = table else {
        return Ok(Vec::new());
    };
    (0..c::PICKUP_ITEM_COUNT).map(|i| code.u16_at(at + i * 2)).collect()
}

/// Writes the Pickup items.
pub(crate) fn write_pickup(rom: &mut Rom, table: Option<usize>, items: &[ItemId]) -> Result<(), RomError> {
    let Some(at) = table else {
        return Ok(());
    };
    for (i, &id) in items.iter().take(c::PICKUP_ITEM_COUNT).enumerate() {
        if rom.code().u16_at(at + i * 2)? != id {
            rom.code_mut().set_u16(at + i * 2, id)?;
        }
    }
    Ok(())
}
