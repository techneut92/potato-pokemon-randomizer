use ppr_model::PlayerCharacter;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Custom player graphics, chosen beside the settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlayerGraphicsSettings {
    /// The id of the graphics pack to put in the game; `None` keeps the game's own graphics.
    pub pack: Option<String>,
    /// The player character the pack replaces.
    pub replace: PlayerCharacter,
}
