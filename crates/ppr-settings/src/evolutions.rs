use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How evolutions are randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EvolutionMode {
    /// Keep the game's evolutions.
    #[default]
    Unchanged,
    /// Each evolution leads to a random species, the way it happened before.
    Random,
    /// Every species evolves into a random one at every level up.
    RandomEveryLevel,
}

/// Evolution settings.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches of the UI; not a state machine"
)]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EvolutionSettings {
    /// How evolutions are randomized.
    pub mode: EvolutionMode,
    /// Random evolutions lead to species about as strong as the old ones (not with every level).
    pub similar_strength: bool,
    /// Random evolutions share a type with the species that evolves.
    pub same_typing: bool,
    /// Random evolution lines have at most three stages (not with every level).
    pub limit_three_stages: bool,
    /// Random evolutions never lead to an old one.
    pub force_change: bool,
    /// Random evolutions have a higher base stat total (not with every level).
    pub force_growth: bool,
    /// No two species evolve into the same one.
    pub no_convergence: bool,
    /// "Allow Alternate Formes" (Gen 7). As in the original, random evolutions take in alternate formes where they
    /// have evolutions of their own whatever this says; it only decides whether Follow Mega Evolutions can be used
    /// with Random Every Level.
    #[serde(default)]
    pub allow_alt_formes: bool,
    /// Evolutions at a level happen at a level fitting the new base stat totals.
    pub adjust_levels: bool,
    /// Evolutions that can't happen in the game alone, like trade evolutions, happen another way.
    pub change_impossible: bool,
    /// Make Evolutions Easier: evolutions happen at this level at the latest, and need less happiness; `None`
    /// leaves them.
    pub easier_max_level: Option<u8>,
    /// Evolutions changed into ones at a level use the estimated level instead of fixed ones.
    pub use_estimated_levels: bool,
    /// Evolutions by time of day happen at any time.
    pub remove_time_based: bool,
}
