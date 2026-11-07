use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How the items of special shops change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ShopItemMode {
    /// Keep the game's items.
    #[default]
    Unchanged,
    /// The same items in other special shops.
    Shuffle,
    /// Random items.
    Random,
}

/// Shop item settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option of the UI")]
pub struct ShopItemSettings {
    /// How special shops' items change.
    pub mode: ShopItemMode,
    /// Random shops sell no Rare Candies, Lucky Eggs or items that sell for a lot.
    pub ban_overpowered: bool,
    /// Random shops sell no bad items.
    pub ban_bad: bool,
    /// Random shops sell none of the usual Poké Mart goods.
    pub ban_regular: bool,
    /// Random shops sell every evolution item.
    pub guarantee_evolution_items: bool,
    /// Random shops sell every X item, Guard Spec. and Dire Hit.
    pub guarantee_x_items: bool,
    /// Items whose price does not fit their use get a better one.
    pub balance_prices: bool,
    /// Every shop sells Rare Candies, for 10 each.
    pub add_cheap_rare_candies: bool,
}
