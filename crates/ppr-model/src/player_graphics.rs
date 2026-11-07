//! Custom player graphics: the graphics packs and which player character gets one.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which player character gets the new graphics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PlayerCharacter {
    /// The first: Red in Gen 1; Chris in Gen 2; Brendan in Ruby, Sapphire and Emerald; Red in `FireRed` and
    /// `LeafGreen`.
    #[default]
    Pc1,
    /// The second: Kris in Crystal; May; Leaf.
    Pc2,
}

/// Where a graphics pack's character comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum GraphicsPackCategory {
    /// From a Pokémon game.
    Pokemon,
    /// From another game.
    Games,
    /// Anything else.
    Other,
}

/// A custom player graphics pack the game can use.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PlayerGraphicsPack {
    /// Identifies the pack: the name of its folder.
    pub id: String,
    /// The pack's name, e.g. `May (Emerald)`.
    pub name: String,
    /// What the pack is.
    pub description: String,
    /// Where the graphics come from.
    pub from: String,
    /// Who made the graphics.
    pub creator: String,
    /// Who adapted them for this game, when someone did.
    pub adapter: Option<String>,
    /// The pack's category.
    pub category: Option<GraphicsPackCategory>,
    /// The games the pack is for, as its `info.ini` names them: `Gen1`, `Gen2`, or `RSE` or `FRLG` in Gen 3.
    pub rom_type: String,
}

/// The graphics pack to put in the game, and the player character it replaces.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerGraphicsChoice {
    /// The [`PlayerGraphicsPack::id`].
    pub pack: String,
    /// The player character that gets the graphics.
    pub replace: PlayerCharacter,
}
