//! Use cases shared by every front end (Facade pattern, ADR 0001).
//!
//! The desktop front ends and the CLI call these functions and never reach into the lower crates themselves, so
//! they all behave the same.

mod batch;
mod check_value;
mod edit_files;
mod edits;
mod error;
mod log;
mod names;
mod premade;
mod randomize;
mod roms;
mod settings_files;
mod support;

pub use batch::{BatchProgress, BatchReport, BatchRequest, BatchRom, batch_output, randomize_batch};
pub use check_value::check_value;
pub use edit_files::{
    EDITS_EXTENSION, EditFileError, edits_file_path, edits_file_text, load_edits_file, read_edits_file, save_edits_file,
};
pub use edits::{
    EditIssue, EditSeverity, EditableGame, EditableShop, EditableSpecies, EditableStatic, EditableTrainer,
    EditableWildSet, ItemName, RomSession, applying_edits_step, editable_game, validate_edits,
};
pub use error::{ApiError, ServiceError};
pub use names::{default_custom_names, load_custom_names, save_custom_names};
pub use ppr_model::{BattleStyle, EditGame, EditList, ExpCurve, MiscTweak, PlayerCharacter, PlayerGraphicsPack, Type};
pub use ppr_random::{CancelToken, Progress, ProgressSink, Seed};
pub use ppr_settings::{
    AbilityMode, AbilitySettings, AuraMode, BaseStatMode, BaseStatSettings, BattleStyleMode, BattleStyleSettings,
    BstMode, BstSettings, CompatMode, CustomNames, EvolutionMode, EvolutionSettings, ExpCurveScope, ExpCurveSettings,
    FieldItemMode, FieldItemSettings, GeneralSettings, GenerationLimit, MoveDataSettings, MovesetMode, MovesetSettings,
    PaletteSettings, PickupItemSettings, PlayerGraphicsSettings, Settings, SettingsExtras, ShopItemMode,
    ShopItemSettings, StarterMode, StarterSettings, StarterTypes, StaticMode, StaticSettings, TmHmCompatSettings,
    TmSettings, TotemMode, TotemSettings, TradeMode, TradeSettings, TrainerMode, TrainerSettings, TutorCompatSettings,
    TutorSettings, TypeEffectivenessMode, TypeEffectivenessSettings, TypeMode, TypeSettings, WildEvolution, WildMode,
    WildSettings, WildTypes, WildZones,
};
pub use premade::{
    PREMADE_SEED_EXTENSION, PremadeSeed, load_premade_seed, premade_seed_bytes, premade_seed_path, read_premade_seed,
    save_premade_seed,
};
pub use randomize::{
    RandomizeRequest, RunReport, StarterChange, WRITING, log_path, randomization_steps, randomize_file, save_log,
};
pub use roms::{
    LoadedRom, PlayerGraphicsOption, RomReport, SupportedGame, identify_file, load_rom, load_rom_with_update,
    supported_games,
};
pub use settings_files::{
    SETTINGS_EXTENSION, SharedSettings, load_settings_file, read_settings_string, save_settings_file,
    settings_file_path, settings_string,
};
pub use support::{SUPPORT_URL, open_support_page};
