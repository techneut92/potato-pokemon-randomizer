use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How species' movesets are randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum MovesetMode {
    /// Keep the game's movesets.
    #[default]
    Unchanged,
    /// Random moves, often of the species' own types.
    RandomPreferSameType,
    /// Random moves of any type.
    RandomCompletely,
    /// Every species learns only Metronome, and every TM and tutor teaches it.
    MetronomeOnly,
}

/// Moveset settings: the moves species learn by level and as eggs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MovesetSettings {
    /// How movesets are randomized.
    pub mode: MovesetMode,
    /// Every species starts with this many moves at level 1 (2 to 4), where the game supports it.
    pub guaranteed_level_one_moves: Option<u8>,
    /// Damaging moves are learnt from weakest to strongest.
    pub reorder_damaging: bool,
    /// No moves that break the game early, like Sonic Boom and Dragon Rage.
    pub no_game_breaking: bool,
    /// At least this share of each moveset, in percent, are good damaging moves.
    pub good_damaging_percent: Option<u8>,
    /// Every species gets a move on evolving (Gen 7).
    pub evolution_moves_for_all: bool,
}
