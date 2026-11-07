use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Which species can learn the moves of TMs, HMs or tutors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum CompatMode {
    /// Keep the game's compatibility.
    #[default]
    Unchanged,
    /// Random, likelier for moves of the species' own types.
    RandomPreferSameType,
    /// Random, half of the moves for every species.
    RandomCompletely,
    /// Every species learns every move.
    Full,
}

/// TM/HM compatibility settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TmHmCompatSettings {
    /// How compatibility is changed.
    pub mode: CompatMode,
    /// Species can learn the TMs of the moves they learn by level.
    pub level_up_sanity: bool,
    /// Evolutions can learn what their pre-evolutions can, and random compatibility mostly carries over.
    pub follow_evolutions: bool,
    /// Every species can learn every HM.
    pub full_hm: bool,
}

/// Move tutor compatibility settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TutorCompatSettings {
    /// How compatibility is changed.
    pub mode: CompatMode,
    /// Species can learn the tutor moves they learn by level.
    pub level_up_sanity: bool,
    /// Evolutions can learn what their pre-evolutions can, and random compatibility mostly carries over.
    pub follow_evolutions: bool,
}
