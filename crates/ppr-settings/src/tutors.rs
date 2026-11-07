use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Move tutor move settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TutorSettings {
    /// Move tutors teach random moves, none of them a TM's.
    pub random: bool,
    /// Tutors of moves used outside battle keep them.
    pub keep_field_moves: bool,
    /// No moves that break the game early, like Sonic Boom and Dragon Rage.
    pub no_game_breaking: bool,
    /// At least this share of the tutors, in percent, teach good damaging moves.
    pub good_damaging_percent: Option<u8>,
}
