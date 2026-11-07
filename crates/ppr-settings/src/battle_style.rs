use ppr_model::BattleStyle;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How trainers' battle styles change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum BattleStyleMode {
    /// Trainers battle as in the game.
    #[default]
    Unchanged,
    /// Each trainer gets a random style the game has.
    Random,
    /// Every trainer battles in [`BattleStyleSettings::style`].
    SingleStyle,
}

/// Trainer battle style settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BattleStyleSettings {
    /// How the styles change.
    pub mode: BattleStyleMode,
    /// The style of `SingleStyle`.
    pub style: BattleStyle,
}
