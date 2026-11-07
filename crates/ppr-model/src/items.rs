//! Items, by "standard" numbers shared by every generation (items unique to one generation from 1000 on).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{DexNumber, Type};

/// Standard item number, the same in every generation.
pub type ItemId = u16;

/// One item of the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    /// Standard number.
    pub id: ItemId,
    /// Name as the game spells it.
    pub name: String,
    /// May appear in most places, like shops, field items and held items; key items and unused items may not.
    pub allowed: bool,
    /// Useless or nearly so as a held item (mail, most berries, items for one species …).
    pub bad: bool,
    /// A TM.
    pub tm: bool,
}

/// What wild Pokémon of a species may hold. A guaranteed item excludes the others.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WildHeldItems {
    /// Always held.
    pub guaranteed: Option<ItemId>,
    /// Held often.
    pub common: Option<ItemId>,
    /// Held rarely.
    pub rare: Option<ItemId>,
    /// Held in dark grass (Gen 5).
    pub dark_grass: Option<ItemId>,
}

impl WildHeldItems {
    /// True when the species holds nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Rare Candy.
pub const RARE_CANDY: ItemId = 50;

/// Guard Spec., Dire Hit and the X items.
pub const X_ITEMS: [ItemId; 8] = [55, 56, 57, 58, 59, 60, 61, 62];

/// The regular Poké Mart goods: balls, potions, status healers, Revive, repels and Escape Rope.
pub const REGULAR_SHOP_ITEMS: [ItemId; 19] = [4, 3, 2, 17, 26, 25, 24, 18, 19, 20, 21, 22, 27, 23, 28, 79, 76, 77, 78];

/// A shop and what it sells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Shop {
    /// Where it is, as a readable name.
    pub name: String,
    /// What it sells, in order.
    pub items: Vec<ItemId>,
    /// Reached in the main game, before the Hall of Fame.
    pub main_game: bool,
    /// Sells something other than the usual Poké Mart goods, and may be randomized.
    pub special: bool,
}

/// The items trainer Pokémon may be given to hold: all of them, the consumable ones, and the lists sensible held items
/// for a species are picked from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeldItemPools {
    /// Every item with an effect in battle for any species.
    pub all: Vec<ItemId>,
    /// Those used up in battle.
    pub consumable: Vec<ItemId>,
    /// Consumable items useful to any Pokémon.
    pub general_consumable: Vec<ItemId>,
    /// Lasting items useful to any Pokémon.
    pub general: Vec<ItemId>,
    /// Consumable items for Pokémon with a physical attack (Liechi Berry).
    pub physical_consumable: Vec<ItemId>,
    /// Consumable items for Pokémon with a special attack (Petaya Berry).
    pub special_consumable: Vec<ItemId>,
    /// Lasting items for Pokémon with a physical attack (Choice Band).
    pub physical: Vec<ItemId>,
    /// Items that power up attacks of a type.
    pub type_boosting: BTreeMap<Type, Vec<ItemId>>,
    /// Items with an effect for one species only, by species.
    pub species_boosting: BTreeMap<DexNumber, Vec<ItemId>>,
}
