use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How items on the ground and hidden items change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FieldItemMode {
    /// Keep the game's items.
    #[default]
    Unchanged,
    /// The same items in other places.
    Shuffle,
    /// Random items.
    Random,
    /// Random items, each about as often as the others.
    RandomEven,
}

/// Field item settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FieldItemSettings {
    /// How field items change.
    pub mode: FieldItemMode,
    /// Random field items are never bad items.
    pub ban_bad: bool,
}
