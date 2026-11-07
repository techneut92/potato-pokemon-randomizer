use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Move data settings.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches of the UI; not a state machine"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MoveDataSettings {
    /// Random power for damaging moves.
    pub power: bool,
    /// Random accuracy, keeping very inaccurate moves inaccurate.
    pub accuracy: bool,
    /// Random PP.
    pub pp: bool,
    /// Random types.
    pub types: bool,
    /// Physical and special moves swap at random (games with the split only).
    pub category: bool,
    /// Random names made of a word for the type and one for what the move does (English games only).
    pub names: bool,
    /// Change the moves to how this later generation has them; `None` keeps them.
    pub update_to_generation: Option<u8>,
}
