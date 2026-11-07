//! Randomizer settings, one section at a time, and the settings-string codec.

mod abilities;
mod base_stats;
mod battle_style;
mod bsts;
mod compat;
mod evolutions;
mod exp_curves;
mod field_items;
mod general;
mod move_data;
mod movesets;
mod names;
mod palettes;
mod pickup;
mod player_graphics;
mod settings_string;
mod shops;
mod starters;
mod statics;
mod tms;
mod totems;
mod trades;
mod trainers;
mod tutors;
mod type_effectiveness;
mod types;
mod wild;

pub use abilities::{AbilityMode, AbilitySettings};
pub use base_stats::{BaseStatMode, BaseStatSettings};
pub use battle_style::{BattleStyleMode, BattleStyleSettings};
pub use bsts::{BstMode, BstSettings};
pub use compat::{CompatMode, TmHmCompatSettings, TutorCompatSettings};
pub use evolutions::{EvolutionMode, EvolutionSettings};
pub use exp_curves::{ExpCurveScope, ExpCurveSettings};
pub use field_items::{FieldItemMode, FieldItemSettings};
pub use general::{GeneralSettings, GenerationLimit};
pub use move_data::MoveDataSettings;
pub use movesets::{MovesetMode, MovesetSettings};
pub use names::CustomNames;
pub use palettes::PaletteSettings;
pub use pickup::PickupItemSettings;
pub use player_graphics::PlayerGraphicsSettings;
pub use settings_string::{
    SETTINGS_VERSION, SettingsExtras, SettingsStringError, from_settings_file, from_settings_string, to_settings_file,
    to_settings_string,
};
pub use shops::{ShopItemMode, ShopItemSettings};
pub use starters::{StarterMode, StarterSettings, StarterTypes};
pub use statics::{StaticMode, StaticSettings};
pub use tms::TmSettings;
pub use totems::{AuraMode, TotemMode, TotemSettings};
pub use trades::{TradeMode, TradeSettings};
pub use trainers::{TrainerMode, TrainerSettings};
pub use tutors::TutorSettings;
pub use type_effectiveness::{TypeEffectivenessMode, TypeEffectivenessSettings};
pub use types::{TypeMode, TypeSettings};
pub use wild::{WildEvolution, WildMode, WildSettings, WildTypes, WildZones};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// All settings of a run. Sections are added as their randomizers are ported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    /// Abilities.
    pub abilities: AbilitySettings,
    /// Base statistics.
    pub base_stats: BaseStatSettings,
    /// Base stat totals.
    pub bsts: BstSettings,
    /// General options.
    pub general: GeneralSettings,
    /// Evolutions.
    pub evolutions: EvolutionSettings,
    /// EXP curves.
    pub exp_curves: ExpCurveSettings,
    /// Move data.
    pub move_data: MoveDataSettings,
    /// Species' movesets.
    pub movesets: MovesetSettings,
    /// TM moves.
    pub tms: TmSettings,
    /// Which species can learn the TMs and HMs.
    pub tm_hm_compat: TmHmCompatSettings,
    /// Move tutor moves.
    pub tutors: TutorSettings,
    /// Which species can learn the move tutor moves.
    pub tutor_compat: TutorCompatSettings,
    /// Items on the ground and hidden items.
    pub field_items: FieldItemSettings,
    /// Shop items and prices.
    pub shops: ShopItemSettings,
    /// What Pickup finds.
    pub pickup: PickupItemSettings,
    /// The type chart.
    pub type_effectiveness: TypeEffectivenessSettings,
    /// Trainer Pokémon.
    pub trainers: TrainerSettings,
    /// Trainers' battle styles.
    pub battle_style: BattleStyleSettings,
    /// Starter Pokémon.
    pub starters: StarterSettings,
    /// Static Pokémon.
    pub statics: StaticSettings,
    /// Totem Pokémon (Gen 7).
    #[serde(default)]
    pub totems: TotemSettings,
    /// In-game trades.
    pub trades: TradeSettings,
    /// Species types.
    pub types: TypeSettings,
    /// Wild Pokémon.
    pub wild: WildSettings,
    /// Pokémon palettes.
    #[serde(default)]
    pub palettes: PaletteSettings,
    /// Custom player graphics.
    #[serde(default)]
    pub player_graphics: PlayerGraphicsSettings,
    /// The misc tweaks to apply where the game supports them.
    #[serde(default)]
    pub misc_tweaks: Vec<ppr_model::MiscTweak>,
    /// The Custom Names Editor's lists; the built-in default names when `None`. Not part of settings strings.
    #[serde(default)]
    pub custom_names: Option<CustomNames>,
}

/// A combination of settings that the UI would not allow.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid settings: {0}")]
pub struct InvalidSettings(pub &'static str);

impl Settings {
    /// Checks combinations that the option rules of the frontend also forbid.
    pub fn validate(&self) -> Result<(), InvalidSettings> {
        self.general.validate()?;
        self.base_stats.validate()?;
        self.bsts.validate()?;
        self.starters.validate()?;
        self.statics.validate()?;
        self.totems.validate()?;
        self.trainers.validate()?;
        self.wild.validate()
    }
}
