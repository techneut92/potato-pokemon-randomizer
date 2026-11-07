//! Game data model (ADR 0002).
//!
//! Plain, owned, serializable data. ROM handlers produce and consume it, randomizers transform it, and the
//! types that reach the frontend are exported to TypeScript (ADR 0004).

mod capabilities;
mod edits;
mod encounters;
mod evolution;
mod exp;
mod game;
mod items;
mod moves;
mod palettes;
mod player_graphics;
mod prices;
mod rom;
mod species;
mod statics;
mod totems;
mod trades;
mod trainers;
mod tweaks;
mod type_chart;
mod types;

pub use capabilities::Capabilities;
pub use edits::{
    EDIT_LIST_VERSION, Edit, EditError, EditGame, EditIssue, EditList, EditRules, EditSeverity, FieldItemEdit,
    FieldItemRules, ItemPriceEdit, ObjectRef, PriceRules, ShopCost, ShopEdit, ShopRules, ShopSize, SpeciesEdit,
    StarterEdit, StarterHeldItemEdit, StarterLink, StaticEdit, StatsEdit, TradeEdit, TradeRules, TrainerEdit,
    TrainerLimits, TrainerPokemonEdit, TrainerRules, WildLevels, WildRules, WildSetBan, WildSetEdit, WildSlotEdit,
    object_name,
};
pub use encounters::{EncounterKind, EncounterSet, WildEncounter};
pub use evolution::{Evolution, EvolutionType};
pub use exp::ExpCurve;
pub use game::{FormeRules, GameData};
pub use items::{HeldItemPools, Item, ItemId, RARE_CANDY, REGULAR_SHOP_ITEMS, Shop, WildHeldItems, X_ITEMS};
pub use moves::{
    CriticalChance, LevelUpMove, Move, MoveCategory, MoveId, STRUGGLE, SWIFT, StatChange, StatChangeMoveType,
    StatChangeType, StatusMoveType, StatusType,
};
pub use palettes::{Color, SpeciesPalettes};
pub use player_graphics::{GraphicsPackCategory, PlayerCharacter, PlayerGraphicsChoice, PlayerGraphicsPack};
pub use prices::{PriceError, PriceFormat};
pub use rom::{Generation, Platform, RomIdentity};
pub use species::{
    BaseStats, DexNumber, Forme, Species, generation_of, is_legendary, is_strong_legendary, is_ultra_beast,
};
pub use statics::StaticEncounter;
pub use totems::{Aura, AuraStat, TotemAlly, TotemPokemon};
pub use trades::InGameTrade;
pub use trainers::{BattleStyle, MultiBattleStatus, Trainer, TrainerPokemon};
pub use tweaks::{CatchingTutorial, LUCKY_EGG, MiscTweak};
pub use type_chart::{Effectiveness, TypeChart};
pub use types::{PHYSICAL_TYPES, Type, UnknownType};
