use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How the type chart changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TypeEffectivenessMode {
    /// Keep the game's chart.
    #[default]
    Unchanged,
    /// As many immunities, resistances and weaknesses as before, on random pairs.
    Random,
    /// Like `Random`, but no type gets more of each than any type had.
    RandomBalanced,
    /// Swaps between types that keep how strong and weak each type is overall.
    KeepIdentities,
    /// Weaknesses become resistances and the other way round; immunities become weaknesses.
    Inverse,
}

/// Type effectiveness settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TypeEffectivenessSettings {
    /// How the chart changes.
    pub mode: TypeEffectivenessMode,
    /// With `Inverse`, as many former weaknesses as there were immunities become immunities.
    pub random_immunities: bool,
    /// The chart first gets the changes Gen 6 made.
    pub update: bool,
}
