use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// How trainers' Pokémon are chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TrainerMode {
    /// Trainers keep their species.
    #[default]
    Unchanged,
    /// Any species.
    Random,
    /// Any species, each used about as often as the others.
    RandomEvenDistribution,
    /// Like `RandomEvenDistribution`, counting main-game trainers only.
    RandomEvenDistributionMainGame,
    /// Each trainer's team shares a random type; gyms and the Elite Four share one.
    TypeThemed,
    /// Only gyms and the Elite Four get a type.
    TypeThemedEliteFourGyms,
    /// Trainers whose team shares a type keep it.
    KeepThemed,
    /// Like `KeepThemed`; the others keep their main Pokémon's primary type.
    KeepThemesOrPrimary,
}

/// Trainer Pokémon settings.
#[expect(clippy::struct_excessive_bools, reason = "one flag per option of the UI")]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrainerSettings {
    /// How the species are chosen.
    pub mode: TrainerMode,
    /// Changes every trainer Pokémon's level by this percentage, -100 to 155; the first rival battle only gets
    /// lower.
    pub level_modifier: Option<i16>,
    /// Trainers' Pokémon evolve as far as their level allows, at evolution levels changed by this percentage.
    pub evolve: Option<i16>,
    /// Rivals and friends use the starter the player didn't pick, evolving through the game.
    pub rival_carries_starter: bool,
    /// Replacements are about as strong as what they replace.
    pub similar_strength: bool,
    /// A trainer has no species twice while others are left.
    pub avoid_duplicates: bool,
    /// With `TypeThemed`, types are picked by how many species have them.
    pub weight_types: bool,
    /// No legendaries.
    pub no_legendaries: bool,
    /// No Wonder Guard below level 20.
    pub no_early_wonder_guard: bool,
    /// Only species met in the wild in the main game, and their families; the Elite Four's unique ones excepted.
    pub local_pokemon: bool,
    /// Each chosen trainer Pokémon is shiny with a chance of 1 in 256 (Gen 7).
    pub random_shiny: bool,
    /// The Elite Four's and Champion's strongest 1 or 2 Pokémon are unique to them.
    pub elite_four_unique: Option<u8>,
    /// Pokémon added to each team, for boss, important and regular trainers, up to 6 (3 with a partner).
    pub additional_pokemon: [u8; 3],
    /// No two Pokémon of a team share a type, for boss, important and regular trainers.
    pub diverse_types: [bool; 3],
    /// Pokémon of boss, important and regular trainers hold random items.
    pub held_items: [bool; 3],
    /// Only items used up in battle, like berries.
    pub consumable_items_only: bool,
    /// Only items that suit the Pokémon and its moves.
    pub sensible_items_only: bool,
    /// Only each team's highest-level Pokémon gets an item.
    pub highest_level_gets_item: bool,
    /// Pokémon of boss, important and regular trainers get movesets chosen for them.
    pub better_movesets: [bool; 3],
    /// Trainers get random names from the custom names.
    pub random_names: bool,
    /// Trainer classes get random names from the custom names.
    pub random_class_names: bool,
    /// Replacements may be alternate formes, where trainers' Pokémon have a forme.
    #[serde(default)]
    pub allow_alt_formes: bool,
    /// Trainers' Pokémon that hold their Mega Stone become species that can Mega Evolve, holding a Mega Stone of
    /// theirs (Gen 6 and 7).
    #[serde(default)]
    pub swap_mega_evos: bool,
}

impl TrainerSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        let in_range = |m: Option<i16>| m.is_none_or(|m| (-100..=155).contains(&m));
        if !in_range(self.level_modifier) || !in_range(self.evolve) {
            return Err(InvalidSettings("the trainer level modifiers are -100% to 155%"));
        }
        if self.elite_four_unique.is_some_and(|n| !(1..=2).contains(&n)) {
            return Err(InvalidSettings("the Elite Four have 1 or 2 unique Pokémon"));
        }
        if self.additional_pokemon.iter().any(|&n| n > 5) {
            return Err(InvalidSettings("trainers get at most 5 added Pokémon"));
        }
        Ok(())
    }
}
