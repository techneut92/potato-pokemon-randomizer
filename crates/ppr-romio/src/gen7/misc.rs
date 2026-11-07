//! Items, TMs, move tutors, shops, prices, Pickup, the type chart and the code patches.

use std::collections::BTreeMap;

use ppr_model::{Effectiveness, Item, ItemId, Move, MoveId, PriceFormat, Shop, Type, TypeChart};

use super::constants::{self as c, Family};
use super::personal::type_byte;
use super::{find, put_u16, put_u32, word};
use crate::RomError;

/// The items by number from 1, named by the item name texts.
pub(super) fn items(names: &[String], family: Family) -> Vec<Item> {
    let banned: &[u16] = match family {
        Family::SunMoon => &c::BANNED_ITEMS_SM,
        Family::UltraSunMoon => &c::BANNED_ITEMS_USUM,
    };
    let tms: Vec<ItemId> = c::TM_BLOCKS
        .iter()
        .flat_map(|&(first, n)| (0..n).map(move |i| first + ItemId::try_from(i).unwrap_or_default()))
        .collect();
    names
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, name)| {
            let id = ItemId::try_from(i).unwrap_or_default();
            Item {
                id,
                name: name.clone(),
                allowed: !banned.contains(&id),
                bad: c::BAD_ITEMS.contains(&id),
                tm: tms.contains(&id),
            }
        })
        .collect()
}

/// The item numbers of the TMs, TM01 first.
pub(super) fn tm_items() -> Vec<ItemId> {
    c::TM_BLOCKS
        .iter()
        .flat_map(|&(first, n)| (0..n).map(move |i| first + ItemId::try_from(i).unwrap_or_default()))
        .collect()
}

pub(super) fn read_tms(code: &[u8], family: Family) -> Result<Vec<MoveId>, RomError> {
    let Some(at) = find(code, family.tm_data_prefix())? else {
        return Ok(Vec::new());
    };
    (0..c::TM_COUNT).map(|i| word(code, at + i * 2)).collect()
}

/// Writes the TM moves, the TMs' descriptions (their moves') and their palettes (their moves' types).
pub(super) fn write_tms(
    code: &mut [u8],
    family: Family,
    tms: &[MoveId],
    moves: &[Move],
    item_descriptions: &mut [String],
    move_descriptions: &[String],
) -> Result<(), RomError> {
    let Some(at) = find(code, family.tm_data_prefix())? else {
        return Ok(());
    };
    for (i, &m) in tms.iter().take(c::TM_COUNT).enumerate() {
        put_u16(code, at + i * 2, m);
    }
    let items = tm_items();
    for (&item, &m) in items.iter().zip(tms) {
        if let (Some(d), Some(text)) = (
            item_descriptions.get_mut(usize::from(item)),
            move_descriptions.get(usize::from(m)),
        ) {
            d.clone_from(text);
        }
    }
    if let Some(pals) = find(code, c::ITEM_PALETTES_PREFIX)? {
        for (&item, &m) in items.iter().zip(tms) {
            let t = moves.iter().find(|mv| mv.number == m).and_then(|mv| mv.move_type);
            put_u16(code, pals + usize::from(item) * 4, c::tm_palette(t));
        }
    }
    Ok(())
}

/// The move tutor moves, up to `FFFF`.
pub(super) fn read_tutors(code: &[u8]) -> Result<Vec<MoveId>, RomError> {
    let Some(mut at) = find(code, c::TUTORS_PREFIX)? else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    loop {
        let m = word(code, at)?;
        at += 2;
        if m == 0xFFFF {
            return Ok(out);
        }
        out.push(m);
    }
}

/// Writes the move tutor moves into the executable and the shop module.
pub(super) fn write_tutors(code: &mut [u8], shop: &mut [u8], moves: &[MoveId]) -> Result<(), RomError> {
    if let Some(at) = find(code, c::TUTORS_PREFIX)? {
        for (i, &m) in moves.iter().enumerate() {
            put_u16(code, at + i * 2, m);
        }
    }
    for (i, &m) in moves.iter().enumerate() {
        let at = c::TUTORS_OFFSET + i * 4;
        if shop.len() >= at + 2 {
            put_u16(shop, at, m);
        }
    }
    Ok(())
}

/// The shops in the shop module; the TM shops and the regular marts are not special.
pub(super) fn read_shops(
    shop: &[u8],
    family: Family,
    sizes: &[usize],
    tm_shops: &[usize],
    regular_shops: &[usize],
) -> Result<Vec<Shop>, RomError> {
    let names = c::shop_names(family);
    let main_game: &[usize] = match family {
        Family::SunMoon => &c::MAIN_GAME_SHOPS_SM,
        Family::UltraSunMoon => &c::MAIN_GAME_SHOPS_USUM,
    };
    let mut at = family.shop_items_offset();
    let mut shops = Vec::new();
    for (i, &size) in sizes.iter().enumerate() {
        let items = (0..size)
            .map(|j| word(shop, at + j * 2))
            .collect::<Result<Vec<_>, _>>()?;
        at += size * 2;
        shops.push(Shop {
            name: names.get(i).map_or_else(String::new, |&n| n.to_owned()),
            items,
            main_game: main_game.contains(&i),
            special: !tm_shops.contains(&i) && !regular_shops.contains(&i),
        });
    }
    Ok(shops)
}

/// Writes the shops' items; each shop keeps its size, so a shop with another number of items fails.
pub(super) fn write_shops(shop: &mut [u8], family: Family, sizes: &[usize], shops: &[Shop]) -> Result<(), RomError> {
    if shops.len() != sizes.len() {
        return Err(RomError::Malformed(format!(
            "{} shops, where the game has {}",
            shops.len(),
            sizes.len()
        )));
    }
    if let Some((s, size)) = shops.iter().zip(sizes).find(|(s, size)| s.items.len() != **size) {
        return Err(RomError::Malformed(format!(
            "shop {} has {} items, but room for {size}",
            s.name,
            s.items.len()
        )));
    }
    let mut at = family.shop_items_offset();
    for (s, &size) in shops.iter().zip(sizes) {
        for (j, &item) in s.items.iter().enumerate() {
            put_u16(shop, at + j * 2, item);
        }
        at += size * 2;
    }
    Ok(())
}

/// The type chart in the executable: 18 × 18 bytes, 8 super effective, 4 neutral, 2 not very effective, 0 none.
pub(super) fn read_type_chart(code: &[u8], at: usize) -> Result<TypeChart, RomError> {
    let mut chart = TypeChart::neutral(&Type::ALL);
    for &a in &Type::ALL {
        for &d in &Type::ALL {
            let i = at + usize::from(type_byte(a)) * Type::ALL.len() + usize::from(type_byte(d));
            let e = match code.get(i) {
                Some(8) => Effectiveness::Double,
                Some(4) => Effectiveness::Neutral,
                Some(2) => Effectiveness::Half,
                Some(0) => Effectiveness::Zero,
                _ => return Err(RomError::Malformed("unexpected type chart value".into())),
            };
            chart.set(a, d, e);
        }
    }
    Ok(chart)
}

pub(super) fn write_type_chart(code: &mut [u8], at: usize, chart: &TypeChart) {
    for &a in &Type::ALL {
        for &d in &Type::ALL {
            let i = at + usize::from(type_byte(a)) * Type::ALL.len() + usize::from(type_byte(d));
            code[i] = match chart.get(a, d).unwrap_or_default() {
                Effectiveness::Double => 8,
                Effectiveness::Neutral => 4,
                Effectiveness::Half => 2,
                Effectiveness::Zero => 0,
            };
        }
    }
}

/// The prices of the items with one, from the item data (stored in tens).
pub(super) fn read_prices(garc: &ppr_formats::ctr::Garc) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let mut out = BTreeMap::new();
    for i in 1..garc.len() {
        let price = u32::from(word(&garc.file(i)?, 0)?) * PriceFormat::TENS.step;
        if price != 0 {
            out.insert(ItemId::try_from(i).unwrap_or_default(), price);
        }
    }
    Ok(out)
}

/// The Pickup items, from the Pickup table: a count (one more than there are), then 0xC bytes per item.
pub(super) fn read_pickup(data: &[u8]) -> Result<Vec<ItemId>, RomError> {
    let count = data
        .get(..4)
        .map_or(0, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize)
        .saturating_sub(1);
    (0..count).map(|i| word(data, 4 + i * 0xC)).collect()
}

pub(super) fn write_pickup(data: &mut [u8], items: &[ItemId]) {
    for (i, &id) in items.iter().enumerate() {
        let at = 4 + i * 0xC;
        if data.len() >= at + 2 {
            put_u16(data, at, id);
        }
    }
}

/// Text at full speed: two instructions of the text speed setup load the fastest speed.
pub(super) fn fastest_text(code: &mut [u8]) -> Result<(), RomError> {
    let [first, second] = c::FASTEST_TEXT_PREFIXES;
    if let Some(at) = find(code, first)? {
        code[at..at + 4].copy_from_slice(&[0x03, 0x40, 0xA0, 0xE3]);
    }
    if let Some(at) = find(code, second)? {
        code[at..at + 4].copy_from_slice(&[0x03, 0x50, 0xA0, 0xE3]);
    }
    Ok(())
}

/// Formes stay after loading a save (Mega Evolutions, Primal Reversions, Ash-Greninja, Zygarde Complete and Ultra
/// Necrozma) and after a battle (Primal Kyogre and Groudon, Schooling Wishiwashi, Minior's core).
pub(super) fn retain_alt_formes(code: &mut [u8], battle: &mut [u8], family: Family) -> Result<(), RomError> {
    if let Some(at) = find(code, family.save_load_forme_reversion_prefix())? {
        // The 8 bytes after the prefix differ between updates.
        let at = at + 8;
        put_u32(code, at, 0);
        if family == Family::UltraSunMoon {
            put_u32(code, at + 8, 0);
        }
    }
    if let Some(at) = find(battle, c::AFTER_BATTLE_FORME_REVERSION_PREFIX)? {
        for o in [0, 60, 92, 148] {
            put_u32(battle, at + o, 0);
        }
    }
    Ok(())
}

/// Every ball catches every Pokémon: the branch for odds below the highest is removed.
pub(super) fn guaranteed_catching(battle: &mut [u8]) -> Result<(), RomError> {
    if let Some(at) = super::find_at(battle, c::PERFECT_ODDS_BRANCH_LOCATOR)? {
        put_u32(battle, at, 0);
    }
    Ok(())
}

/// Happiness evolutions need 160 instead of 220.
pub(super) fn easier_happiness(code: &mut [u8]) -> Result<(), RomError> {
    if let Some(at) = super::find_at(code, c::FRIENDSHIP_FOR_EVOLUTION_LOCATOR)? {
        for o in [0, 12, 36] {
            if code.get(at + o) == Some(&c::VANILLA_HAPPINESS_TO_EVOLVE) {
                code[at + o] = c::EASIER_HAPPINESS_TO_EVOLVE;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shop(items: Vec<ItemId>) -> Shop {
        Shop {
            name: "Mart".into(),
            items,
            main_game: true,
            special: false,
        }
    }

    #[test]
    fn shops_keep_their_size() {
        let family = Family::UltraSunMoon;
        let mut file = vec![0; family.shop_items_offset() + 10];
        let sizes = [2, 3];
        write_shops(&mut file, family, &sizes, &[shop(vec![1, 2]), shop(vec![3, 4, 5])]).expect("fits");
        let at = family.shop_items_offset();
        let items: Vec<u16> = (0..5).map(|i| word(&file, at + i * 2).expect("in the file")).collect();
        assert_eq!(items, [1, 2, 3, 4, 5]);

        let before = file.clone();
        for shops in [
            vec![shop(vec![1, 2]), shop(vec![3, 4, 5, 50])],
            vec![shop(vec![1]), shop(vec![3, 4, 5])],
            vec![shop(vec![1, 2])],
        ] {
            assert!(write_shops(&mut file, family, &sizes, &shops).is_err(), "{shops:?}");
            assert_eq!(file, before, "nothing is written");
        }
    }
}
