//! Shops and item prices, read and written.

use std::collections::BTreeMap;

use ppr_model::{ItemId, PriceFormat, Shop};

use super::constants as c;
use super::space::Space;
use super::{Family, items, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;

/// Where an item's price is in its `ItemData` entry.
const ITEM_PRICE: usize = 16;

fn names(family: Family) -> &'static [&'static str] {
    match family {
        Family::RubySapphire => &c::RS_SHOP_NAMES,
        Family::Emerald => &c::E_SHOP_NAMES,
        Family::FireRedLeafGreen => &c::FRLG_SHOP_NAMES,
    }
}

fn pointer_offsets(entry: &RomEntry) -> Vec<usize> {
    entry
        .array("ShopPointerOffsets")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|o| usize::try_from(o).ok())
        .collect()
}

fn contains(entry: &RomEntry, key: &str, i: usize) -> bool {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .any(|o| usize::try_from(o) == Ok(i))
}

/// Each shop's items, up to a 0.
pub(super) fn read(rom: &GbaRom, entry: &RomEntry, family: Family) -> Result<Vec<Shop>, RomError> {
    let names = names(family);
    pointer_offsets(entry)
        .into_iter()
        .enumerate()
        .map(|(i, pointer)| {
            let mut at = rom.pointer(pointer)?;
            let mut shop_items = Vec::new();
            loop {
                let value = rom.u16(at)?;
                if value == 0 {
                    break;
                }
                shop_items.push(items::standard(value));
                at += 2;
            }
            Ok(Shop {
                name: names.get(i).copied().unwrap_or("Shop").to_owned(),
                items: shop_items,
                main_game: contains(entry, "MainGameShops", i),
                special: !contains(entry, "SkipShops", i),
            })
        })
        .collect()
}

/// The length of the data at `at` up to the first terminator, which counts.
fn length_with_terminator(rom: &GbaRom, at: usize) -> Result<usize, RomError> {
    rom.bytes()
        .get(at..)
        .and_then(|rest| {
            rest.windows(c::SHOP_TERMINATOR.len())
                .position(|w| w == c::SHOP_TERMINATOR)
        })
        .map(|start| start + c::SHOP_TERMINATOR.len())
        .ok_or_else(|| RomError::Malformed(format!("no shop end after {at:#x}")))
}

/// Writes every shop: its items move to free space, so shops can grow.
pub(super) fn write(space: &mut Space<'_>, entry: &RomEntry, shops: &[Shop]) -> Result<(), RomError> {
    let pointers = pointer_offsets(entry);
    if shops.len() != pointers.len() {
        return Err(RomError::Malformed(format!(
            "wrong number of shops: should be {}, is {}",
            pointers.len(),
            shops.len()
        )));
    }
    for (shop, &pointer) in shops.iter().zip(&pointers) {
        let mut data = Vec::with_capacity(shop.items.len() * 2 + c::SHOP_TERMINATOR.len());
        for &id in &shop.items {
            let internal = items::internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {id}")))?;
            data.extend(internal.to_le_bytes());
        }
        data.extend(c::SHOP_TERMINATOR);
        let old_length = length_with_terminator(space.rom, space.rom.pointer(pointer)?)?;
        space.rewrite(pointer, old_length, &data, &[])?;
    }
    Ok(())
}

/// The item prices, by item. The last internal item is left out.
pub(super) fn read_prices(rom: &GbaRom, entry: &RomEntry) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let (data, size) = (
        super::entry_offset(rom, entry, "ItemData")?,
        need(entry, "ItemEntrySize")?,
    );
    (1..need(entry, "ItemCount")?)
        .map(|internal| {
            let id = items::standard(u16::try_from(internal).unwrap_or_default());
            Ok((id, u32::from(rom.u16(data + internal * size + ITEM_PRICE)?)))
        })
        .collect()
}

/// Writes the item prices; prices that don't fit are written as [`PriceFormat::fit`] makes them.
pub(super) fn write_prices(rom: &mut GbaRom, entry: &RomEntry, prices: &BTreeMap<ItemId, u32>) -> Result<(), RomError> {
    let (data, size) = (
        super::entry_offset(rom, entry, "ItemData")?,
        need(entry, "ItemEntrySize")?,
    );
    for internal in 1..need(entry, "ItemCount")? {
        let id = items::standard(u16::try_from(internal).unwrap_or_default());
        if let Some(&price) = prices.get(&id) {
            let stored = PriceFormat::WORD.stored_u16(price);
            rom.write_u16(data + internal * size + ITEM_PRICE, stored)?;
        }
    }
    Ok(())
}

/// Item prices balanced against what the items are worth.
pub(super) fn balanced_prices() -> BTreeMap<ItemId, u32> {
    c::BALANCED_ITEM_PRICES.into_iter().collect()
}
