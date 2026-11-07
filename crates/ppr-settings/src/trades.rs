use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which side of in-game trades is randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TradeMode {
    /// Keep the game's trades.
    #[default]
    Unchanged,
    /// Only the Pokémon the NPCs give.
    Given,
    /// The Pokémon the NPCs give and the ones they ask for.
    GivenAndRequested,
}

/// In-game trade settings.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches of the UI; not a state machine"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TradeSettings {
    /// Which side is randomized.
    pub mode: TradeMode,
    /// Give the traded Pokémon nicknames from the custom names.
    pub random_nicknames: bool,
    /// Give them original trainers from the custom trainer names, with random IDs.
    pub random_ots: bool,
    /// Give them random individual values.
    pub random_ivs: bool,
    /// Give them random held items.
    pub random_items: bool,
}
