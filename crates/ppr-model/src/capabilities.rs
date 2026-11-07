//! What a game supports: which features its handler can read and change (intro Pokémon, move tutors, …). The UI shows
//! or hides options with it.

use serde::Serialize;
use ts_rs::TS;

/// What a game supports, one flag per feature. Everything is off by default.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per capability")]
pub struct Capabilities {
    /// The intro shows a Pokémon that can be randomized.
    pub intro_pokemon: bool,
    /// The starters can be given held items.
    pub starter_held_items: bool,
    /// The game has move tutors.
    pub move_tutors: bool,
    /// Wild zones can be per map instead of per encounter set.
    pub map_indices: bool,
    /// Wild zones can be per named location.
    pub encounter_locations: bool,
    /// Shops can get extra items, like cheap Rare Candies.
    pub shop_sizes: bool,
    /// Wild encounters depend on the time of day.
    pub time_based_encounters: bool,
    /// The game has Mega Evolutions.
    pub mega_evolutions: bool,
    /// Moves have their own category.
    pub physical_special_split: bool,
    /// Wild Pokémon can appear in alternate formes.
    pub wild_alt_formes: bool,
    /// Static encounters can be alternate formes.
    pub static_alt_formes: bool,
    /// Starters can be alternate formes.
    pub starter_alt_formes: bool,
    /// The game has formes that differ in stats or types.
    pub functional_formes: bool,
    /// The game has Totem Pokémon.
    pub totem_pokemon: bool,
    /// The game is in English, so move names can be randomized.
    pub english: bool,
}
