use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How species' types are randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TypeMode {
    /// Keep the game's types.
    #[default]
    Unchanged,
    /// Random types that evolutions keep, sometimes adding a second.
    FollowEvolutions,
    /// Random types for every species.
    Random,
}

/// Species type settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TypeSettings {
    /// How the types are randomized.
    pub mode: TypeMode,
    /// Give every species two types.
    pub force_dual: bool,
    /// With [`TypeMode::FollowEvolutions`]: Mega Evolutions follow their base species' types like evolutions.
    #[serde(default)]
    pub follow_megas: bool,
}
