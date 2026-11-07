//! One randomizer per domain, each a [`Step`](crate::Step) (ADR 0002).

mod abilities;
mod base_stat_updates;
mod base_stats;
mod battle_style;
mod bsts;
mod common;
mod compat;
mod copy_up;
mod evolution_improvements;
mod evolutions;
mod exp_curves;
mod field_items;
mod intro;
mod metronome;
mod move_data;
mod move_names;
mod move_updates;
mod movesets;
mod palettes;
mod pickup;
mod player_graphics;
mod restrictions;
mod shops;
mod starters;
mod stat_points;
mod statics;
#[cfg(test)]
mod testing;
mod tm_moves;
mod totems;
mod trades;
mod trainer_items;
mod trainer_movesets;
mod trainer_names;
mod trainers;
mod tutor_moves;
mod tweaks;
mod type_effectiveness;
mod types;
mod wild;
mod wild_held_items;

pub use abilities::AbilityRandomizer;
pub use base_stat_updates::BaseStatUpdater;
pub use base_stats::BaseStatRandomizer;
pub use battle_style::BattleStyleRandomizer;
pub use bsts::BstRandomizer;
pub use compat::{TmHmCompatibilityRandomizer, TutorCompatibilityRandomizer};
pub use evolution_improvements::EvolutionImprover;
pub use evolutions::{EvolutionLevelAdjuster, EvolutionRandomizer};
pub use exp_curves::ExpCurveStandardizer;
pub use field_items::FieldItemRandomizer;
pub use intro::IntroPokemonRandomizer;
pub use metronome::MetronomeOnlyMode;
pub use move_data::MoveDataRandomizer;
pub use move_names::MoveNameRandomizer;
pub use move_updates::MoveUpdater;
pub use movesets::MovesetRandomizer;
pub use palettes::PaletteRandomizer;
pub use pickup::PickupItemRandomizer;
pub use player_graphics::PlayerGraphicsChooser;
pub use restrictions::SpeciesRestrictions;
pub use shops::ShopItemRandomizer;
pub use starters::StarterRandomizer;
pub use statics::StaticRandomizer;
pub use tm_moves::TmMoveRandomizer;
pub use totems::TotemRandomizer;
pub use trades::TradeRandomizer;
pub use trainer_items::TrainerHeldItemRandomizer;
pub use trainer_movesets::TrainerMovesetRandomizer;
pub use trainer_names::TrainerNameRandomizer;
pub use trainers::{TrainerLevelModifier, TrainerPokemonAdder, TrainerPokemonRandomizer};
pub use tutor_moves::TutorMoveRandomizer;
pub use tweaks::MiscTweaks;
pub use type_effectiveness::{TypeEffectivenessRandomizer, TypeEffectivenessUpdater};
pub use types::TypeRandomizer;
pub use wild::WildRandomizer;
pub use wild_held_items::WildHeldItemRandomizer;

use crate::{Pipeline, RunData};

/// Every randomizer, in the order they run.
pub fn standard_pipeline() -> Pipeline<RunData> {
    Pipeline::default()
        // Species restrictions first, before the updaters.
        .with(SpeciesRestrictions)
        .with(TypeEffectivenessUpdater)
        .with(MoveUpdater)
        .with(BaseStatUpdater)
        .with(TypeEffectivenessRandomizer)
        .with(MoveDataRandomizer)
        // After all move data, so names fit the final types and categories.
        .with(MoveNameRandomizer)
        // The misc tweaks come after the move data, before anything uses species or items.
        .with(MiscTweaks)
        .with(ExpCurveStandardizer)
        // Before anything that follows evolutions, so that uses the new ones.
        .with(EvolutionRandomizer)
        .with(BstRandomizer)
        // After both random evolutions and base stat totals.
        .with(EvolutionLevelAdjuster)
        .with(TypeRandomizer)
        // Wild held items come between the types and the base stats.
        .with(WildHeldItemRandomizer)
        .with(BaseStatRandomizer)
        .with(AbilityRandomizer)
        .with(EvolutionImprover)
        .with(StarterRandomizer)
        .with(MovesetRandomizer)
        .with(TmMoveRandomizer)
        .with(TmHmCompatibilityRandomizer)
        .with(TutorMoveRandomizer)
        .with(TutorCompatibilityRandomizer)
        .with(WildRandomizer)
        // The trainer steps: levels, added Pokémon, battle styles, the species, movesets, held items, then names.
        .with(TrainerLevelModifier)
        .with(TrainerPokemonAdder)
        .with(BattleStyleRandomizer)
        .with(TrainerPokemonRandomizer)
        .with(TrainerMovesetRandomizer)
        .with(TrainerHeldItemRandomizer)
        .with(TrainerNameRandomizer)
        // Metronome Only after the trainers, so they still choose from the real moves.
        .with(MetronomeOnlyMode)
        .with(StaticRandomizer)
        .with(TotemRandomizer)
        .with(TradeRandomizer)
        .with(FieldItemRandomizer)
        .with(ShopItemRandomizer)
        .with(PickupItemRandomizer)
        .with(PaletteRandomizer)
        .with(IntroPokemonRandomizer)
        .with(PlayerGraphicsChooser)
}
