use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How abilities are randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AbilityMode {
    /// Keep the game's abilities.
    #[default]
    Unchanged,
    /// Random abilities.
    Random,
}

/// Ability settings.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches of the UI; not a state machine"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AbilitySettings {
    /// How the abilities are randomized.
    pub mode: AbilityMode,
    /// Wonder Guard may be given out.
    pub allow_wonder_guard: bool,
    /// Evolutions keep their pre-evolution's abilities.
    pub follow_evolutions: bool,
    /// With [`Self::follow_evolutions`]: Mega Evolutions keep their base species' abilities like evolutions.
    #[serde(default)]
    pub follow_megas: bool,
    /// Abilities that do the same count as one when picking ("Combine Duplicate Abilities").
    pub combine_duplicates: bool,
    /// Every species gets a second ability.
    pub ensure_two: bool,
    /// Leave out abilities that keep the foe from fleeing or switching.
    pub ban_trapping: bool,
    /// Leave out abilities that hurt their holder, like Truant.
    pub ban_negative: bool,
    /// Leave out abilities that hardly do anything, like Plus and Minus.
    pub ban_bad: bool,
}
