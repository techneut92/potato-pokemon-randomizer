//! The Custom Names Editor's lists.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Names the trainer name and trade randomizers pick from. Blank names are never used.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CustomNames {
    /// Trainer names, also the original trainers of traded Pokémon.
    pub trainer_names: Vec<String>,
    /// Trainer class names.
    pub trainer_classes: Vec<String>,
    /// Names of pairs of trainers battling together, like twins.
    pub doubles_trainer_names: Vec<String>,
    /// Class names of pairs of trainers battling together.
    pub doubles_trainer_classes: Vec<String>,
    /// Pokémon nicknames.
    pub pokemon_nicknames: Vec<String>,
}
