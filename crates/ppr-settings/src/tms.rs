use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// TM move settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TmSettings {
    /// TMs teach random moves.
    pub random: bool,
    /// TMs of moves used outside battle, like Cut and Fly, keep them.
    pub keep_field_moves: bool,
    /// No moves that break the game early, like Sonic Boom and Dragon Rage.
    pub no_game_breaking: bool,
    /// At least this share of the TMs, in percent, teach good damaging moves.
    pub good_damaging_percent: Option<u8>,
}
