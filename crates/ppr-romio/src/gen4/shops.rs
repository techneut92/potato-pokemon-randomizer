//! Shops and item prices, read and written.
//!
//! The progressive shop (the Poké Mart counter whose stock grows with the badges) is a list of an item and the
//! badges it needs; the other shops are lists of items ending in `FFFF`, found through a table of pointers. All of
//! them are in the ARM9 binary; shops that grow move to the ARM9's free room. Prices are the first word of each
//! item's data.

use std::collections::BTreeMap;

use ppr_model::{ItemId, PriceFormat, Shop};

use super::Rom;
use super::constants::{self as c, Family};
use super::files::{byte, put, put_long, put_word, word};
use crate::RomError;

const SHOP_END: u16 = 0xFFFF;

const DP_SPECIAL_SHOP_NAMES: [&str; 19] = [
    "Jubilife Secondary",
    "Oreburgh Secondary",
    "Floaroma Secondary",
    "Eterna Secondary",
    "Eterna Herbs",
    "Hearthome Secondary",
    "Solaceon Secondary",
    "Pastoria Secondary",
    "Veilstone Department Store 1F (Right)",
    "Veilstone Department Store 1F (Left)",
    "Veilstone Department Store 2F (Top)",
    "Veilstone Department Store 2F (Middle)",
    "Veilstone Department Store 3F (Top)",
    "Veilstone Department Store 3F (Bottom)",
    "Celestic Secondary",
    "Snowpoint Secondary",
    "Canalave Secondary",
    "Sunyshore Secondary",
    "Pokemon League Secondary",
];
/// Platinum has one more shop than Diamond and Pearl.
const PT_EXTRA_SPECIAL_SHOP_NAME: &str = "Veilstone Department Store B1F Berries";
const HGSS_SPECIAL_SHOP_NAMES: [&str; 30] = [
    "Cherrygrove Secondary",
    "Violet Secondary",
    "Azalea Secondary",
    "Goldenrod Department Store 2F (Lower)",
    "Goldenrod Department Store 2F (Upper)",
    "Goldenrod Department Store 3F",
    "Goldenrod Department Store 4F",
    "Goldenrod Department Store 5F",
    "Goldenrod Herb Shop",
    "Ecruteak Secondary",
    "Olivine Secondary",
    "Cianwood Pharmacy",
    "Blackthorn Secondary",
    "Unused Secondary",
    "Safari Zone Gate Southwest",
    "Saffron Secondary",
    "Lavender Secondary",
    "Cerulean Secondary",
    "Celadon Department Store 2F (Left)",
    "Celadon Department Store 2F (Right)",
    "Celadon Department Store 3F",
    "Celadon Department Store 4F",
    "Celadon Department Store 5F (Left)",
    "Celadon Department Store 5F (Right)",
    "Fuchsia Secondary",
    "Pewter Secondary",
    "Viridian Secondary",
    "Mt. Moon Square",
    "Mahogany Before Hideout",
    "Mahogany After Hideout",
];
/// The special shops the main game can always reach; Mahogany's shop before the hideout closes.
const HGSS_MAIN_GAME_SHOPS: [usize; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15, 30];

fn special_shop_name(family: Family, i: usize) -> &'static str {
    match family {
        Family::HeartGoldSoulSilver => HGSS_SPECIAL_SHOP_NAMES.get(i).copied().unwrap_or("Shop"),
        _ => DP_SPECIAL_SHOP_NAMES
            .get(i)
            .copied()
            .or((family == Family::Platinum && i == DP_SPECIAL_SHOP_NAMES.len()).then_some(PT_EXTRA_SPECIAL_SHOP_NAME))
            .unwrap_or("Shop"),
    }
}

fn special_shop_in_main_game(family: Family, i: usize) -> bool {
    match family {
        Family::DiamondPearl => i < DP_SPECIAL_SHOP_NAMES.len(),
        Family::Platinum => i <= DP_SPECIAL_SHOP_NAMES.len(),
        Family::HeartGoldSoulSilver => HGSS_MAIN_GAME_SHOPS.contains(&i),
    }
}

/// The ARM9 offset a pointer at `at` of the ARM9 points to.
fn arm9_pointer(rom: &Rom, at: usize) -> Result<usize, RomError> {
    let arm9 = rom.files.arm9_ref();
    rom.room.read_pointer(arm9, arm9, at)
}

fn set_arm9_pointer(rom: &mut Rom, at: usize, target: usize) -> Result<(), RomError> {
    let value = rom.room.pointer_value(rom.files.arm9_ref(), target)?;
    put_long(rom.files.arm9(), at, value)
}

fn checked_item(id: u16, item_count: usize) -> Result<ItemId, RomError> {
    if id == 0 || usize::from(id) > item_count {
        Err(RomError::Malformed(format!("item {id} in a shop")))
    } else {
        Ok(id)
    }
}

/// The progressive shop, then the special shops; the TM shops count as not special.
pub(super) fn read(rom: &mut Rom, item_count: usize) -> Result<Vec<Shop>, RomError> {
    let family = rom.family();
    let mut shops = Vec::new();
    let (at, size) = progressive(rom)?;
    let arm9 = rom.files.arm9_ref();
    let items = (0..size)
        .map(|i| checked_item(word(arm9, at + i * 4)?, item_count))
        .collect::<Result<Vec<_>, _>>()?;
    shops.push(Shop {
        name: "Progressive".to_owned(),
        items,
        main_game: true,
        special: false,
    });
    for (i, mut at) in special_shops(rom)?.into_iter().enumerate() {
        let arm9 = rom.files.arm9_ref();
        let mut items = Vec::new();
        loop {
            let id = word(arm9, at)?;
            if id == SHOP_END {
                break;
            }
            items.push(checked_item(id, item_count)?);
            at += 2;
        }
        shops.push(Shop {
            name: special_shop_name(family, i).to_owned(),
            items,
            main_game: special_shop_in_main_game(family, i),
            special: true,
        });
    }
    for i in rom.entry.array("TMShops") {
        if let Some(shop) = shops.get_mut(i) {
            shop.special = false;
        }
    }
    Ok(shops)
}

/// Where the progressive shop is and how many items it has.
fn progressive(rom: &Rom) -> Result<(usize, usize), RomError> {
    let at = arm9_pointer(rom, rom.entry.need("ProgressiveShopPointerOffset")?)?;
    let size = byte(rom.files.arm9_ref(), rom.entry.need("ProgressiveShopSizeOffset")?)?;
    Ok((at, usize::from(size)))
}

/// Where each special shop is.
fn special_shops(rom: &Rom) -> Result<Vec<usize>, RomError> {
    let table = arm9_pointer(rom, rom.entry.need("SpecialShopsPointerOffset")?)?;
    (0..rom.entry.need("SpecialShopCount")?)
        .map(|i| arm9_pointer(rom, table + 4 * i))
        .collect()
}

/// Writes the shops: the progressive shop's items keep the badges they needed, new ones need none; the special
/// shops are all freed and written again.
pub(super) fn write(rom: &mut Rom, loaded: &[Shop], shops: &[Shop], item_count: usize) -> Result<(), RomError> {
    let (Some(progressive_shop), Some(old_progressive)) = (shops.first(), loaded.first()) else {
        return Ok(());
    };
    if progressive_shop != old_progressive {
        write_progressive(rom, &progressive_shop.items, item_count)?;
    }
    if shops.get(1..) != loaded.get(1..) {
        write_special(rom, shops.get(1..).unwrap_or_default(), item_count)?;
    }
    Ok(())
}

fn write_progressive(rom: &mut Rom, items: &[ItemId], item_count: usize) -> Result<(), RomError> {
    let pointer = rom.entry.need("ProgressiveShopPointerOffset")?;
    let size_at = rom.entry.need("ProgressiveShopSizeOffset")?;
    let (mut at, size) = progressive(rom)?;
    let arm9 = rom.files.arm9_ref();
    let badges: BTreeMap<u16, u16> = (0..size)
        .map(|i| Ok((word(arm9, at + i * 4)?, word(arm9, at + i * 4 + 2)?)))
        .collect::<Result<_, RomError>>()?;
    if items.len() != size {
        let new_size = u8::try_from(items.len())
            .map_err(|_| RomError::Malformed(format!("{} items don't fit the progressive shop", items.len())))?;
        rom.room.free.free(at, size * 4)?;
        at = rom
            .room
            .free
            .find_and_unfree(items.len() * 4)
            .ok_or_else(|| RomError::Malformed("no room in the ARM9 for the progressive shop".into()))?;
        set_arm9_pointer(rom, pointer, at)?;
        put(rom.files.arm9(), size_at, &[new_size])?;
    }
    for (i, &id) in items.iter().enumerate() {
        let id = checked_item(id, item_count)?;
        let arm9 = rom.files.arm9();
        put_word(arm9, at + i * 4, id)?;
        put_word(arm9, at + i * 4 + 2, badges.get(&id).copied().unwrap_or(1))?;
    }
    Ok(())
}

fn write_special(rom: &mut Rom, shops: &[Shop], item_count: usize) -> Result<(), RomError> {
    let table = arm9_pointer(rom, rom.entry.need("SpecialShopsPointerOffset")?)?;
    let places = special_shops(rom)?;
    if shops.len() != places.len() {
        return Err(RomError::Malformed(format!(
            "{} special shops, where the game has {}",
            shops.len(),
            places.len()
        )));
    }
    for &at in &places {
        let arm9 = rom.files.arm9_ref();
        let mut size = 0;
        while word(arm9, at + size * 2)? != SHOP_END {
            size += 1;
        }
        rom.room.free.free(at, (size + 1) * 2)?;
    }
    for (i, shop) in shops.iter().enumerate() {
        let at = rom
            .room
            .free
            .find_and_unfree((shop.items.len() + 1) * 2)
            .ok_or_else(|| RomError::Malformed(format!("no room in the ARM9 for shop {}", shop.name)))?;
        set_arm9_pointer(rom, table + 4 * i, at)?;
        for (j, &id) in shop.items.iter().enumerate() {
            put_word(rom.files.arm9(), at + j * 2, checked_item(id, item_count)?)?;
        }
        put_word(rom.files.arm9(), at + shop.items.len() * 2, SHOP_END)?;
    }
    Ok(())
}

/// The item data file of each item with a price: the files follow the item numbers, except that a range of unused
/// numbers before the Adamant Orb (from 112 in Diamond and Pearl, from 113 otherwise) has no files.
fn item_of_file(family: Family, file: usize) -> usize {
    let unused_from = if family == Family::DiamondPearl { 112 } else { 113 };
    if file < unused_from {
        file
    } else {
        file + usize::from(c::ADAMANT_ORB) - unused_from
    }
}

/// The item prices, by item.
pub(super) fn read_prices(rom: &mut Rom) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let family = rom.family();
    let path = rom.entry.file("ItemData")?;
    let narc = rom.files.narc(&path)?;
    narc.files
        .iter()
        .enumerate()
        .skip(1)
        .map(|(file, data)| {
            let item = ItemId::try_from(item_of_file(family, file))
                .map_err(|_| RomError::Malformed("too many items".into()))?;
            Ok((item, u32::from(word(data, 0)?)))
        })
        .collect()
}

/// Writes the prices that changed; prices that don't fit are written as [`PriceFormat::fit`] makes them.
pub(super) fn write_prices(
    rom: &mut Rom,
    loaded: &BTreeMap<ItemId, u32>,
    prices: &BTreeMap<ItemId, u32>,
) -> Result<(), RomError> {
    let family = rom.family();
    let path = rom.entry.file("ItemData")?;
    let count = rom.files.narc(&path)?.files.len();
    for file in 1..count {
        let item = ItemId::try_from(item_of_file(family, file)).unwrap_or_default();
        match prices.get(&item) {
            Some(&price) if loaded.get(&item) != Some(&price) => {
                let stored = PriceFormat::WORD.stored_u16(price);
                put_word(rom.files.narc_file(&path, file)?, 0, stored)?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_data_files_skip_the_unused_numbers() {
        assert_eq!(item_of_file(Family::DiamondPearl, 111), 111);
        assert_eq!(item_of_file(Family::DiamondPearl, 112), 135);
        assert_eq!(item_of_file(Family::Platinum, 112), 112);
        assert_eq!(item_of_file(Family::Platinum, 113), 135);
    }

    #[test]
    fn platinum_has_one_more_special_shop() {
        assert_eq!(special_shop_name(Family::Platinum, 19), PT_EXTRA_SPECIAL_SHOP_NAME);
        assert_eq!(special_shop_name(Family::DiamondPearl, 19), "Shop");
        assert!(special_shop_in_main_game(Family::Platinum, 19));
        assert!(!special_shop_in_main_game(Family::HeartGoldSoulSilver, 14));
    }

    /// With `PPR_TEST_ROMS`: Platinum's special shops keep their number; one more or one less fails instead of
    /// being left out.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_special_shops_keep_their_number() {
        use crate::GameHandler;
        use crate::gen4::tests::{load, real_rom};
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes);
        let loaded = h.game_data().clone();
        let mut more = loaded.clone();
        let extra = more.shops[1].clone();
        more.shops.push(extra);
        let err = h.apply(&more).expect_err("one more shop");
        assert!(err.to_string().contains("special shops, where the game has"), "{err}");
        let mut fewer = loaded;
        fewer.shops.pop();
        assert!(h.apply(&fewer).is_err(), "one less shop");
    }
}
