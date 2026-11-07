//! Items: their names and prices (`ItemData`), the items lying in the field and hidden (two script files), the shops
//! (an overlay in Black and White, `ShopItems` and `ShopSizes` in Black 2 and White 2), the Pickup table and the
//! in-game trades (`InGameTrades`).
//!
//! The Pickup table is found by its first items, so the table of a ROM whose first Pickup items changed is not found
//! again.
//!
//! Deliberate choices:
//! - The skip table of hidden items is walked by its own length.
//! - Only the shops and trades that changed are written. A Black and White shop that changes moves into the
//!   extended ARM9, as the overlay has no room to grow.
//! - The names in what trade partners say are replaced as plain text.
//! - Only the prices that changed are written; prices that don't fit are written as [`PriceFormat::fit`] makes them.

use std::collections::BTreeMap;

use ppr_model::{InGameTrade, Item, ItemId, PriceFormat, Shop};

use super::constants as c;
use super::entry::{Family, TradeScript, file_path, indices, need};
use super::store::{Store, Text, find, long, put_long, put_word, relative_pointer, word};
use crate::RomError;
use crate::entries::RomEntry;

/// The longest nickname and original trainer name of a traded Pokémon.
pub(super) const MAX_NICKNAME_LENGTH: usize = 10;
pub(super) const MAX_OT_NAME_LENGTH: usize = 7;

const SCRIPT_LIST_END: u16 = 0xFD13;
const SET_VAR: u16 = 0x28;
const ITEM_BALL_VAR: u16 = 0x800C;
const SET_VAR_HIDDEN: u16 = 0x2A;
const HIDDEN_ITEM_VAR: u16 = 0x8000;

const PICKUP_LOCATOR: &str = "19005C00DD00";
const PICKUP_COUNT: usize = 29;

const TM01: ItemId = 328;
const TM92: ItemId = 419;
const TM93: ItemId = 618;
const TM95: ItemId = 620;

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// Every item but the empty one.
pub(super) fn read(store: &mut Store, entry: &RomEntry, family: Family) -> Result<Vec<Item>, RomError> {
    let names = store
        .strings(Text::Strings, need(entry, "ItemNamesTextOffset")?)?
        .to_vec();
    let bad: &[ItemId] = family.pick(&c::BAD_ITEMS_BW1, &c::BAD_ITEMS_BW2);
    Ok(names
        .iter()
        .enumerate()
        .skip(1)
        .filter_map(|(i, name)| {
            let id = ItemId::try_from(i).ok()?;
            Some(Item {
                id,
                name: name.clone(),
                allowed: !c::BANNED_ITEMS.contains(&id),
                bad: bad.contains(&id),
                tm: (TM01..=TM92).contains(&id) || (TM93..=TM95).contains(&id),
            })
        })
        .collect())
}

/// Where the item of each item ball and hidden item is: the script file and the offset of the item word.
pub(super) fn field_item_places(
    store: &mut Store,
    entry: &RomEntry,
    scripts: &str,
) -> Result<Vec<(usize, usize)>, RomError> {
    let mut out = Vec::new();
    for (file_key, skip_key, command, var) in [
        ("ItemBallsScriptOffset", "ItemBallsSkip", SET_VAR, ITEM_BALL_VAR),
        (
            "HiddenItemsScriptOffset",
            "HiddenItemsSkip",
            SET_VAR_HIDDEN,
            HIDDEN_ITEM_VAR,
        ),
    ] {
        let file = need(entry, file_key)?;
        let skip = indices(entry, skip_key);
        let data = store.narc_file(scripts, file)?;
        let mut offset = 0;
        let mut skipped = 0;
        while word(data, offset)? != SCRIPT_LIST_END {
            let at = relative_pointer(data, offset)?;
            offset += 4;
            if at > data.len() {
                break;
            }
            if skip.get(skipped) == Some(&(offset / 4 - 1)) {
                skipped += 1;
                continue;
            }
            if word(data, at + 2)? == command && word(data, at + 4)? == var {
                out.push((file, at + 6));
            }
        }
    }
    Ok(out)
}

fn is_allowed(items: &[Item], id: ItemId) -> bool {
    items.iter().any(|i| i.id == id && i.allowed)
}

/// The allowed field items.
pub(super) fn read_field_items(
    store: &mut Store,
    scripts: &str,
    places: &[(usize, usize)],
    items: &[Item],
) -> Result<Vec<ItemId>, RomError> {
    let mut out = Vec::new();
    for &(file, at) in places {
        let id = word(store.narc_file(scripts, file)?, at)?;
        if is_allowed(items, id) {
            out.push(id);
        }
    }
    Ok(out)
}

/// Writes the field items over the allowed ones, in order.
pub(super) fn write_field_items(
    store: &mut Store,
    scripts: &str,
    places: &[(usize, usize)],
    items: &[Item],
    old: &[ItemId],
    new: &[ItemId],
) -> Result<(), RomError> {
    if old == new {
        return Ok(());
    }
    let mut next = new.iter();
    for &(file, at) in places {
        let id = word(store.narc_file(scripts, file)?, at)?;
        if !is_allowed(items, id) {
            continue;
        }
        let Some(&replacement) = next.next() else { break };
        if replacement != id {
            put_word(store.narc_file_mut(scripts, file)?, at, replacement)?;
        }
    }
    Ok(())
}

fn shop_names(family: Family) -> &'static [&'static str] {
    family.pick(&c::BW1_SHOP_NAMES, &c::BW2_SHOP_NAMES)
}

fn make_shops(entry: &RomEntry, family: Family, contents: Vec<Vec<ItemId>>) -> Vec<Shop> {
    let main_game: &[usize] = family.pick(&c::BW1_MAIN_GAME_SHOPS, &c::BW2_MAIN_GAME_SHOPS);
    let mut regular = indices(entry, "TMShops");
    regular.extend(indices(entry, "RegularShops"));
    contents
        .into_iter()
        .enumerate()
        .map(|(i, items)| Shop {
            name: shop_names(family).get(i).copied().unwrap_or_default().to_owned(),
            items,
            main_game: main_game.contains(&i),
            special: !regular.contains(&i),
        })
        .collect()
}

/// Where the items of a Black and White shop are: in the shop overlay or, once moved, in the ARM9.
fn bw1_shop_items(store: &mut Store, entry: &RomEntry, i: usize) -> Result<(bool, usize), RomError> {
    let number = need(entry, "ShopItemOvlNumber")?;
    let pointer = long(store.overlay(number)?, need(entry, "ShopPointerTableOffset")? + i * 4)?;
    if store.is_arm9_pointer(pointer)? {
        return Ok((true, store.arm9_offset(pointer)?));
    }
    let address = store.overlay_address(number)?;
    Ok((false, (pointer as usize).wrapping_sub(address)))
}

/// Every shop.
pub(super) fn read_shops(store: &mut Store, entry: &RomEntry, family: Family) -> Result<Vec<Shop>, RomError> {
    let count = need(entry, "ShopCount")?;
    let mut contents = Vec::with_capacity(count);
    match family {
        Family::Bw1 => {
            let number = need(entry, "ShopItemOvlNumber")?;
            let sizes = need(entry, "ShopSizeTableOffset")?;
            for i in 0..count {
                let size = usize::from(super::store::byte(store.overlay(number)?, sizes + i)?);
                let (in_arm9, at) = bw1_shop_items(store, entry, i)?;
                let data = if in_arm9 { store.arm9()? } else { store.overlay(number)? };
                contents.push((0..size).map(|j| word(data, at + j * 2)).collect::<Result<_, _>>()?);
            }
        }
        Family::Bw2 => {
            let path = file_path(entry, "ShopItems")?;
            for i in 0..count {
                let data = store.narc_file(&path, i)?;
                contents.push(data.as_chunks::<2>().0.iter().map(|&w| u16::from_le_bytes(w)).collect());
            }
        }
    }
    Ok(make_shops(entry, family, contents))
}

/// Writes the shops whose items changed. In Black and White the ARM9 must have been extended.
pub(super) fn write_shops(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    old: &[Shop],
    new: &[Shop],
) -> Result<(), RomError> {
    if new.len() != old.len() {
        return Err(malformed("the shops must keep their number"));
    }
    let changed: Vec<usize> = (0..new.len()).filter(|&i| new[i].items != old[i].items).collect();
    if changed.is_empty() {
        return Ok(());
    }
    let size_byte =
        |shop: &Shop| u8::try_from(shop.items.len()).map_err(|_| malformed(format!("too many items in {}", shop.name)));
    match family {
        Family::Bw1 => {
            let number = need(entry, "ShopItemOvlNumber")?;
            let sizes = need(entry, "ShopSizeTableOffset")?;
            let pointers = need(entry, "ShopPointerTableOffset")?;
            for i in changed {
                let old_size = usize::from(super::store::byte(store.overlay(number)?, sizes + i)?);
                let (in_arm9, old_at) = bw1_shop_items(store, entry, i)?;
                if in_arm9 {
                    store.free_arm9_space(old_at, old_size * 2)?;
                }
                let items = &new[i].items;
                let at = store.take_arm9_space(items.len() * 2)?;
                let pointer = store.arm9_pointer(at)?;
                let arm9 = store.arm9_mut()?;
                for (j, &id) in items.iter().enumerate() {
                    put_word(arm9, at + j * 2, id)?;
                }
                let overlay = store.overlay_mut(number)?;
                super::store::put_byte(overlay, sizes + i, size_byte(&new[i])?)?;
                put_long(overlay, pointers + i * 4, pointer)?;
            }
        }
        Family::Bw2 => {
            let sizes_path = file_path(entry, "ShopSizes")?;
            let sizes = need(entry, "ShopSizesOffset")?;
            let items_path = file_path(entry, "ShopItems")?;
            for i in changed {
                let size = size_byte(&new[i])?;
                super::store::put_byte(store.file_mut(&sizes_path)?, sizes + i, size)?;
                *store.narc_file_mut(&items_path, i)? = new[i].items.iter().flat_map(|id| id.to_le_bytes()).collect();
            }
        }
    }
    Ok(())
}

/// The price of every item but the empty one: stored in tens.
pub(super) fn read_prices(store: &mut Store, path: &str) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let narc = store.narc(path)?;
    narc.files
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, data)| {
            let id = ItemId::try_from(i).map_err(|_| malformed("item number"))?;
            Ok((id, u32::from(word(data, 0)?) * PriceFormat::TENS.step))
        })
        .collect()
}

/// Writes the prices that changed, in tens.
pub(super) fn write_prices(
    store: &mut Store,
    path: &str,
    old: &BTreeMap<ItemId, u32>,
    new: &BTreeMap<ItemId, u32>,
) -> Result<(), RomError> {
    for (&id, &price) in new {
        if old.get(&id) == Some(&price) {
            continue;
        }
        let file = usize::from(id);
        if file == 0 || file >= store.narc(path)?.files.len() {
            continue;
        }
        let tens = PriceFormat::TENS.stored_u16(price);
        put_word(store.narc_file_mut(path, file)?, 0, tens)?;
    }
    Ok(())
}

/// Where the Pickup table is in its overlay, if it is found.
pub(super) fn pickup_table(store: &mut Store, entry: &RomEntry) -> Result<Option<usize>, RomError> {
    let Some(number) = entry.int("PickupOvlNumber").and_then(|n| usize::try_from(n).ok()) else {
        return Ok(None);
    };
    find(store.overlay(number)?, PICKUP_LOCATOR)
}

/// The items Pickup finds.
pub(super) fn read_pickup(store: &mut Store, entry: &RomEntry, table: Option<usize>) -> Result<Vec<ItemId>, RomError> {
    let Some(at) = table else { return Ok(Vec::new()) };
    let data = store.overlay(need(entry, "PickupOvlNumber")?)?;
    (0..PICKUP_COUNT).map(|i| word(data, at + i * 2)).collect()
}

/// Writes the Pickup items when they changed.
pub(super) fn write_pickup(
    store: &mut Store,
    entry: &RomEntry,
    table: Option<usize>,
    old: &[ItemId],
    new: &[ItemId],
) -> Result<(), RomError> {
    let Some(at) = table else { return Ok(()) };
    if old == new {
        return Ok(());
    }
    let data = store.overlay_mut(need(entry, "PickupOvlNumber")?)?;
    for (i, &id) in new.iter().take(PICKUP_COUNT).enumerate() {
        put_word(data, at + i * 2, id)?;
    }
    Ok(())
}

/// The files of the used trades.
fn used_trades(store: &mut Store, entry: &RomEntry, path: &str) -> Result<Vec<usize>, RomError> {
    let unused = indices(entry, "TradesUnused");
    let count = store.narc(path)?.files.len();
    Ok((0..count).filter(|i| !unused.contains(i)).collect())
}

/// The in-game trades.
pub(super) fn read_trades(store: &mut Store, entry: &RomEntry, path: &str) -> Result<Vec<InGameTrade>, RomError> {
    let text = store
        .strings(Text::Strings, need(entry, "IngameTradesTextOffset")?)?
        .to_vec();
    let mut out = Vec::new();
    for i in used_trades(store, entry, path)? {
        let data = store.narc_file(path, i)?;
        let species = |at| -> Result<u16, RomError> {
            u16::try_from(long(data, at)?).map_err(|_| malformed(format!("trade {i}: species")))
        };
        let requested = species(0x5C)?;
        let held = long(data, 0x4C)?;
        out.push(InGameTrade {
            requested: (requested != 0).then_some(requested),
            given: species(4)?,
            nickname: text.get(i * 2).cloned().unwrap_or_default(),
            ot_name: text.get(i * 2 + 1).cloned().unwrap_or_default(),
            ot_id: word(data, 0x34)?.into(),
            ivs: (0..6)
                .map(|iv| long(data, 0x10 + iv * 4).map(|v| u8::try_from(v).unwrap_or(u8::MAX)))
                .collect::<Result<_, _>>()?,
            held_item: u16::try_from(held).ok().filter(|&h| h != 0),
        });
    }
    Ok(out)
}

/// Writes the trades that changed, the scripts that name their species, and what the trade partners say.
pub(super) fn write_trades(
    store: &mut Store,
    entry: &RomEntry,
    path: &str,
    scripts: &str,
    species_names: &[String],
    old: &[InGameTrade],
    new: &[InGameTrade],
) -> Result<(), RomError> {
    if old == new {
        return Ok(());
    }
    if new.len() != old.len() {
        return Err(malformed("the trades must keep their number"));
    }
    let text_index = need(entry, "IngameTradesTextOffset")?;
    let trade_scripts: Vec<TradeScript> = entry
        .list("TradeScript[]")
        .iter()
        .map(|s| TradeScript::parse(s))
        .collect();
    let files = used_trades(store, entry, path)?;
    for (k, (&i, (t, before))) in files.iter().zip(new.iter().zip(old)).enumerate() {
        if t == before {
            continue;
        }
        let strings = store.strings_mut(Text::Strings, text_index)?;
        if let Some(s) = strings.get_mut(i * 2) {
            s.clone_from(&t.nickname);
        }
        if let Some(s) = strings.get_mut(i * 2 + 1) {
            s.clone_from(&t.ot_name);
        }
        let data = store.narc_file_mut(path, i)?;
        put_long(data, 4, u32::from(t.given))?;
        // No forme, and a random nature.
        put_long(data, 8, 0)?;
        for (iv, &value) in t.ivs.iter().take(6).enumerate() {
            put_long(data, 0x10 + iv * 4, u32::from(value))?;
        }
        put_long(data, 0x2C, 0xFF)?;
        put_word(data, 0x34, RomError::short_ot_id(t.ot_id)?)?;
        put_long(data, 0x4C, u32::from(t.held_item.unwrap_or(0)))?;
        put_long(data, 0x5C, u32::from(t.requested.unwrap_or(0)))?;
        if let Some(script) = trade_scripts.get(k) {
            let file = store.narc_file_mut(scripts, script.file)?;
            for &at in &script.requested {
                put_word(file, at, t.requested.unwrap_or(0))?;
            }
            for &at in &script.given {
                put_word(file, at, t.given)?;
            }
        }
    }
    // What the trade partners say, as the original indexes it: from the 25th trade on.
    let name = |n: u16| species_names.get(usize::from(n)).cloned().unwrap_or_default();
    let unused = indices(entry, "TradesUnused");
    let mut skipped = 0;
    for (tr, &text) in indices(entry, "IngameTradePersonTextOffsets").iter().enumerate() {
        if unused.get(skipped) == Some(&(tr + 24)) {
            skipped += 1;
            continue;
        }
        if text == 0 {
            continue;
        }
        let (Some(before), Some(t)) = (old.get(tr + 24), new.get(tr + 24)) else {
            break;
        };
        if t == before {
            continue;
        }
        let mut replacements = BTreeMap::new();
        replacements.insert(name(before.given), name(t.given));
        if before.requested != t.requested {
            replacements.insert(
                before.requested.map(name).unwrap_or_default(),
                t.requested.map(name).unwrap_or_default(),
            );
        }
        replacements.retain(|from, to| !from.is_empty() && from != to);
        if replacements.is_empty() {
            continue;
        }
        for s in store.strings_mut(Text::Story, text)? {
            for (from, to) in &replacements {
                *s = s.replace(from.as_str(), to);
            }
        }
    }
    Ok(())
}
