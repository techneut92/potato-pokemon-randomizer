//! Randomization pipeline (ADR 0002, 0005, 0006).
//!
//! A run is a fixed sequence of [`Step`]s over [`RunData`] (the game data plus settings). Each step gets its own
//! deterministic RNG stream derived from the run's [`Seed`], reports progress and can be cancelled.

mod choice;
mod control;
mod names;
mod pipeline;
mod randomizers;
mod seed;

use ppr_model::GameData;
use ppr_settings::Settings;

pub use control::{CancelToken, Progress, ProgressSink};
pub use names::default_custom_names;
pub use pipeline::{DONE, Pipeline, RandomizeError, Step, StepContext};
pub use randomizers::{
    AbilityRandomizer, BaseStatRandomizer, BaseStatUpdater, BattleStyleRandomizer, BstRandomizer, EvolutionImprover,
    EvolutionLevelAdjuster, EvolutionRandomizer, ExpCurveStandardizer, FieldItemRandomizer, IntroPokemonRandomizer,
    MetronomeOnlyMode, MiscTweaks, MoveDataRandomizer, MoveNameRandomizer, MoveUpdater, MovesetRandomizer,
    PaletteRandomizer, PickupItemRandomizer, PlayerGraphicsChooser, ShopItemRandomizer, SpeciesRestrictions,
    StarterRandomizer, StaticRandomizer, TmHmCompatibilityRandomizer, TmMoveRandomizer, TotemRandomizer,
    TradeRandomizer, TrainerHeldItemRandomizer, TrainerLevelModifier, TrainerMovesetRandomizer, TrainerNameRandomizer,
    TrainerPokemonAdder, TrainerPokemonRandomizer, TutorCompatibilityRandomizer, TutorMoveRandomizer,
    TypeEffectivenessRandomizer, TypeEffectivenessUpdater, TypeRandomizer, WildHeldItemRandomizer, WildRandomizer,
    standard_pipeline,
};
pub use seed::{Seed, SeedParseError, StepRng};

/// What a run transforms.
#[derive(Debug, Clone)]
pub struct RunData {
    /// The game, as loaded and then changed by the steps so far.
    pub game: GameData,
    /// The settings of the run.
    pub settings: Settings,
}

/// Stable step IDs. They select RNG streams (ADR 0005), so released values must never change.
pub mod step_ids {
    /// "Limit Pokémon", before everything else.
    pub const SPECIES_RESTRICTIONS: u64 = 1;
    /// Type effectiveness updates.
    pub const TYPE_EFFECTIVENESS_UPDATES: u64 = 2;
    /// Move updates to a later generation.
    pub const MOVE_UPDATES: u64 = 5;
    /// Base stat updates to a later generation.
    pub const BASE_STAT_UPDATES: u64 = 10;
    /// Type effectiveness.
    pub const TYPE_EFFECTIVENESS: u64 = 12;
    /// Move data.
    pub const MOVE_DATA: u64 = 15;
    /// Move names.
    pub const MOVE_NAMES: u64 = 16;
    /// Misc tweaks.
    pub const MISC_TWEAKS: u64 = 18;
    /// EXP curves.
    pub const EXP_CURVES: u64 = 20;
    /// Random evolutions.
    pub const EVOLUTIONS: u64 = 30;
    /// Base stat totals.
    pub const BSTS: u64 = 40;
    /// Evolution levels adjusted to the base stat totals.
    pub const EVOLUTION_LEVELS: u64 = 42;
    /// Species types.
    pub const TYPES: u64 = 45;
    /// Wild held items.
    pub const WILD_HELD_ITEMS: u64 = 47;
    /// Base statistics.
    pub const BASE_STATS: u64 = 50;
    /// Abilities.
    pub const ABILITIES: u64 = 60;
    /// Evolution improvements.
    pub const EVOLUTION_IMPROVEMENTS: u64 = 70;
    /// Starter Pokémon.
    pub const STARTERS: u64 = 100;
    /// Species' movesets.
    pub const MOVESETS: u64 = 110;
    /// TM moves.
    pub const TM_MOVES: u64 = 120;
    /// TM/HM compatibility.
    pub const TM_HM_COMPAT: u64 = 125;
    /// Move tutor moves.
    pub const TUTOR_MOVES: u64 = 130;
    /// Move tutor compatibility.
    pub const TUTOR_COMPAT: u64 = 135;
    /// Wild Pokémon.
    pub const WILD: u64 = 200;
    /// Trainer Pokémon levels.
    pub const TRAINER_LEVELS: u64 = 210;
    /// Pokémon added to trainers' teams.
    pub const TRAINER_ADDITIONS: u64 = 220;
    /// Trainers' battle styles.
    pub const BATTLE_STYLE: u64 = 230;
    /// Trainer Pokémon species.
    pub const TRAINER_POKEMON: u64 = 240;
    /// Trainer Pokémon movesets.
    pub const TRAINER_MOVESETS: u64 = 250;
    /// Trainer Pokémon held items.
    pub const TRAINER_HELD_ITEMS: u64 = 260;
    /// Trainer and trainer class names.
    pub const TRAINER_NAMES: u64 = 270;
    /// Metronome Only Mode, after the trainers.
    pub const METRONOME_ONLY: u64 = 290;
    /// Static Pokémon.
    pub const STATICS: u64 = 300;
    /// Totem Pokémon, their allies and auras.
    pub const TOTEMS: u64 = 310;
    /// In-game trades.
    pub const TRADES: u64 = 400;
    /// Field items.
    pub const FIELD_ITEMS: u64 = 500;
    /// Shop items and prices.
    pub const SHOPS: u64 = 510;
    /// The intro Pokémon.
    pub const INTRO_POKEMON: u64 = 600;
    /// Pokémon palettes.
    pub const PALETTES: u64 = 550;
    /// Pickup items.
    pub const PICKUP: u64 = 520;
    /// Custom player graphics, last of all.
    pub const PLAYER_GRAPHICS: u64 = 700;
}
