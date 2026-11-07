//! Manual edits (ADR 0008): changes the user makes to objects of the loaded game by hand, applied to [`GameData`]
//! after the randomizers so they win over random results.
//!
//! An [`EditList`] is plain, versioned data bound to one game. Each [`Edit`] names its object by a stable identity
//! (a trainer's index, a species' national number, …) and holds the new value of every editable field of that object,
//! so removing the edit reverts the object. The edit types are a file format of their own, apart from the model types
//! they change, so that a change to the model does not break saved edit lists.

mod apply;
mod issue;
mod rules;
#[cfg(test)]
mod tests;
mod validate;

use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DexNumber, GameData, ItemId, MoveId, RomIdentity, Type};

pub use issue::{EditIssue, EditSeverity, object_name};
pub use rules::{
    EditRules, FieldItemRules, PriceRules, ShopCost, ShopRules, ShopSize, StarterLink, TradeRules, TrainerLimits,
    TrainerRules, WildLevels, WildRules, WildSetBan,
};

/// The format version this build writes and reads. Raise it when a change to the edit types would make an older
/// file mean something else.
pub const EDIT_LIST_VERSION: u32 = 1;

/// All manual edits for one game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditList {
    /// The format version, [`EDIT_LIST_VERSION`] when written by this build.
    pub version: u32,
    /// The game the edits are for.
    pub game: EditGame,
    /// The edits, at most one per object, in the order they were made.
    pub edits: Vec<Edit>,
}

impl EditList {
    /// An empty edit list for `identity`'s game.
    pub fn new(identity: &RomIdentity) -> Self {
        Self {
            version: EDIT_LIST_VERSION,
            game: EditGame::of(identity),
            edits: Vec::new(),
        }
    }

    /// Checks that this build reads the list's format and that the list is for `identity`'s game.
    pub fn check(&self, identity: &RomIdentity) -> Result<(), EditError> {
        self.check_game(&EditGame::of(identity))
    }

    /// Checks that this build reads the list's format and that the list is for `game`, e.g. the game the editor
    /// shows when a saved list is loaded.
    pub fn check_game(&self, game: &EditGame) -> Result<(), EditError> {
        if self.version != EDIT_LIST_VERSION {
            return Err(EditError::UnsupportedVersion { found: self.version });
        }
        if self.game != *game {
            return Err(EditError::WrongGame {
                expected: game.name.clone(),
                found: self.game.name.clone(),
            });
        }
        Ok(())
    }

    /// Applies every edit to `data`, or none: on any error `data` is left as it was and every error is returned.
    ///
    /// Checks that each edit fits the game: its object exists, the game has the fields it sets, and its values
    /// are in the ranges every game shares (levels 1–100, teams of 1–6). Limits a ROM handler reports are checked
    /// separately, before saving.
    pub fn apply(&self, data: &mut GameData) -> Result<(), Vec<EditError>> {
        apply::apply(self, data)
    }

    /// Every issue of the list in the loaded game `data` with its handler's `rules`: what [`EditList::apply`] would
    /// reject, what breaks a rule, and warnings about edits that are allowed but risky. Empty when there is nothing
    /// to say; saving needs no [`EditSeverity::Error`] among them.
    ///
    /// # Errors
    /// What [`EditList::check`] finds: the list is in another format or for another game, so no edit is checked.
    pub fn validate(&self, data: &GameData, rules: &EditRules) -> Result<Vec<EditIssue>, EditError> {
        validate::validate(self, data, rules)
    }

    /// Puts `edit` in the list, replacing the edit of the same object.
    pub fn set(&mut self, edit: Edit) {
        let target = edit.target();
        match self.edits.iter_mut().find(|e| e.target() == target) {
            Some(existing) => *existing = edit,
            None => self.edits.push(edit),
        }
    }

    /// Removes the edit of `target`, reverting that object; returns whether there was one.
    pub fn revert(&mut self, target: &ObjectRef) -> bool {
        let before = self.edits.len();
        self.edits.retain(|e| e.target() != *target);
        self.edits.len() != before
    }
}

/// The game an edit list is for. All three fields must match the loaded game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditGame {
    /// The ROM entry name, e.g. `Emerald (U)`; unique among the supported games.
    pub name: String,
    /// The game code from the header, e.g. `BPEE`.
    pub code: String,
    /// The header version byte, when the platform has one.
    pub version: Option<u8>,
}

impl EditGame {
    /// The game `identity` names.
    pub fn of(identity: &RomIdentity) -> Self {
        Self {
            name: identity.name.clone(),
            code: identity.code.clone(),
            version: identity.version,
        }
    }

    /// True when `identity` is this game.
    pub fn matches(&self, identity: &RomIdentity) -> bool {
        self.name == identity.name && self.code == identity.code && self.version == identity.version
    }
}

/// One object of a game an edit can change, by its stable identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum ObjectRef {
    /// A starter, by its place in the game's order (from 0).
    Starter {
        /// The place.
        slot: usize,
    },
    /// A starter's held item, by its place in the game's order (from 0); Gen 3 has one for all starters.
    StarterHeldItem {
        /// The place.
        slot: usize,
    },
    /// A species, by national number.
    Species {
        /// The national number.
        number: DexNumber,
    },
    /// A trainer battle, by the game's trainer number.
    Trainer {
        /// The trainer number, from 1.
        index: u16,
    },
    /// A wild encounter set, by its place in the handler's list (from 0).
    WildSet {
        /// The place.
        index: usize,
    },
    /// A static encounter, by its place in the handler's fixed list (from 0).
    Static {
        /// The place.
        index: usize,
    },
    /// An in-game trade, by its place in the game's order (from 0).
    Trade {
        /// The place.
        index: usize,
    },
    /// A shop, by its place in the handler's fixed list (from 0).
    Shop {
        /// The place.
        index: usize,
    },
    /// The price of an item, by standard item number.
    ItemPrice {
        /// The item.
        item: ItemId,
    },
    /// A field item, by its place in the game's map order (from 0).
    FieldItem {
        /// The place.
        index: usize,
    },
}

/// Names the object for a message: its place counted from 1, or its number.
impl fmt::Display for ObjectRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Starter { slot } => write!(f, "starter {}", slot + 1),
            Self::StarterHeldItem { slot } => write!(f, "starter held item {}", slot + 1),
            Self::Species { number } => write!(f, "species #{number}"),
            Self::Trainer { index } => write!(f, "trainer {index}"),
            Self::WildSet { index } => write!(f, "wild set {}", index + 1),
            Self::Static { index } => write!(f, "static encounter {}", index + 1),
            Self::Trade { index } => write!(f, "trade {}", index + 1),
            Self::Shop { index } => write!(f, "shop {}", index + 1),
            Self::ItemPrice { item } => write!(f, "price of item {item}"),
            Self::FieldItem { index } => write!(f, "field item {}", index + 1),
        }
    }
}

/// A change to one object: its identity and the new value of each of its editable fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum Edit {
    /// A starter's species.
    Starter(StarterEdit),
    /// A starter's held item.
    StarterHeldItem(StarterHeldItemEdit),
    /// A species' types, base stats and abilities.
    Species(SpeciesEdit),
    /// A trainer's team.
    Trainer(TrainerEdit),
    /// A wild encounter set's slots.
    WildSet(WildSetEdit),
    /// A static encounter.
    Static(StaticEdit),
    /// An in-game trade.
    Trade(TradeEdit),
    /// What a shop sells.
    Shop(ShopEdit),
    /// What an item costs.
    ItemPrice(ItemPriceEdit),
    /// A field item.
    FieldItem(FieldItemEdit),
}

impl Edit {
    /// The object this edit changes.
    pub fn target(&self) -> ObjectRef {
        match self {
            Self::Starter(e) => ObjectRef::Starter { slot: e.slot },
            Self::StarterHeldItem(e) => ObjectRef::StarterHeldItem { slot: e.slot },
            Self::Species(e) => ObjectRef::Species { number: e.number },
            Self::Trainer(e) => ObjectRef::Trainer { index: e.index },
            Self::WildSet(e) => ObjectRef::WildSet { index: e.index },
            Self::Static(e) => ObjectRef::Static { index: e.index },
            Self::Trade(e) => ObjectRef::Trade { index: e.index },
            Self::Shop(e) => ObjectRef::Shop { index: e.index },
            Self::ItemPrice(e) => ObjectRef::ItemPrice { item: e.item },
            Self::FieldItem(e) => ObjectRef::FieldItem { index: e.index },
        }
    }
}

/// A starter's species.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StarterEdit {
    /// Which starter, by its place in the game's order (from 0).
    pub slot: usize,
    /// The species.
    pub species: DexNumber,
}

/// A starter's held item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StarterHeldItemEdit {
    /// Which held item, by its place in the game's order (from 0).
    pub slot: usize,
    /// The item; `None` is no item.
    pub item: Option<ItemId>,
}

/// The six base stats of a species.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[allow(missing_docs)]
pub struct StatsEdit {
    pub hp: u8,
    pub attack: u8,
    pub defense: u8,
    pub speed: u8,
    pub sp_attack: u8,
    pub sp_defense: u8,
}

/// A species' types, base stats and abilities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpeciesEdit {
    /// Which species, by national number.
    pub number: DexNumber,
    /// The first type.
    pub primary_type: Type,
    /// The second type; `None` for a single type.
    pub secondary_type: Option<Type>,
    /// The base stats.
    pub stats: StatsEdit,
    /// Ability numbers, one per slot the game has, 0 for an empty slot; empty before Gen 3.
    pub abilities: Vec<u16>,
}

/// One Pokémon of a trainer's team.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrainerPokemonEdit {
    /// The species.
    pub species: DexNumber,
    /// The level, 1–100.
    pub level: u8,
    /// The held item; `None` is no item, and the only choice in Gen 1.
    pub held_item: Option<ItemId>,
    /// Its own moves, 0 for an empty slot; `None` for the moves its species learns by its level.
    pub moves: Option<[MoveId; 4]>,
}

/// A trainer's team. Pokémon are matched to the team as it is by place: what the edit doesn't set (IVs, ability)
/// stays as it was, and a Pokémon added to the team gets the IVs of the last one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrainerEdit {
    /// Which trainer, by the game's trainer number.
    pub index: u16,
    /// The team, 1–6 Pokémon.
    pub pokemon: Vec<TrainerPokemonEdit>,
}

/// One slot of a wild encounter set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WildSlotEdit {
    /// The species.
    pub species: DexNumber,
    /// The lowest level, 1–100.
    pub min_level: u8,
    /// The highest level, from the lowest to 100.
    pub max_level: u8,
}

/// The slots of a wild encounter set. The game fixes how many slots a set has.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WildSetEdit {
    /// Which set, by its place in the handler's list (from 0).
    pub index: usize,
    /// Every slot, in the game's order.
    pub slots: Vec<WildSlotEdit>,
}

/// A static encounter's species and level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StaticEdit {
    /// Which encounter, by its place in the handler's fixed list (from 0).
    pub index: usize,
    /// The species.
    pub species: DexNumber,
    /// The level, 1–100; an egg's stays as it is.
    pub level: u8,
}

/// An in-game trade. Gen 1 trades have no original trainer, IVs or held item: there those must stay empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TradeEdit {
    /// Which trade, by its place in the game's order (from 0).
    pub index: usize,
    /// The species the NPC asks for; `None` when any will do.
    pub requested: Option<DexNumber>,
    /// The species the NPC gives.
    pub given: DexNumber,
    /// The given Pokémon's nickname.
    pub nickname: String,
    /// Its original trainer's name.
    pub ot_name: String,
    /// Its original trainer's ID, up to [`TradeRules::max_ot_id`].
    pub ot_id: u32,
    /// Its IVs, as many as the game stores: four DVs of 0–15 in Gen 2, six IVs of 0–31 from Gen 3.
    pub ivs: Vec<u8>,
    /// The item it holds; `None` is no item.
    pub held_item: Option<ItemId>,
}

/// What a shop sells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShopEdit {
    /// Which shop, by its place in the handler's fixed list (from 0).
    pub index: usize,
    /// The items, in order; at least one.
    pub items: Vec<ItemId>,
}

/// What an item costs, everywhere in the game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ItemPriceEdit {
    /// The item, by standard number.
    pub item: ItemId,
    /// Its price.
    pub price: u32,
}

/// A field item: an item on the ground or hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FieldItemEdit {
    /// Which field item, by its place in the game's map order (from 0).
    pub index: usize,
    /// The item.
    pub item: ItemId,
}

/// Why an edit list, or one of its edits, doesn't fit the game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// The list is in a format this build can't read.
    UnsupportedVersion {
        /// The list's format version.
        found: u32,
    },
    /// The list is for another game.
    WrongGame {
        /// The loaded game.
        expected: String,
        /// The game the list is for.
        found: String,
    },
    /// Two edits change the same object.
    Duplicate {
        /// The object.
        target: ObjectRef,
    },
    /// The object is not in this game.
    UnknownObject {
        /// The object.
        target: ObjectRef,
    },
    /// The game has no such field for this object.
    Unsupported {
        /// The object.
        target: ObjectRef,
        /// The field.
        field: &'static str,
    },
    /// A number outside its range.
    OutOfRange {
        /// The object.
        target: ObjectRef,
        /// The field.
        field: &'static str,
        /// The value.
        value: u32,
        /// The lowest allowed.
        min: u32,
        /// The highest allowed.
        max: u32,
    },
    /// A list with another length than the game fixes.
    WrongCount {
        /// The object.
        target: ObjectRef,
        /// The field.
        field: &'static str,
        /// The game's length.
        expected: usize,
        /// The edit's length.
        found: usize,
    },
    /// A species the game doesn't have.
    UnknownSpecies {
        /// The object.
        target: ObjectRef,
        /// The species.
        species: DexNumber,
    },
    /// An item the game doesn't have.
    UnknownItem {
        /// The object.
        target: ObjectRef,
        /// The item.
        item: ItemId,
    },
    /// A move the game doesn't have.
    UnknownMove {
        /// The object.
        target: ObjectRef,
        /// The move.
        mv: MoveId,
    },
    /// An ability the game doesn't have.
    UnknownAbility {
        /// The object.
        target: ObjectRef,
        /// The ability.
        ability: u16,
    },
    /// A type the game doesn't have.
    UnknownType {
        /// The object.
        target: ObjectRef,
        /// The type.
        ty: Type,
    },
    /// A value that is never valid, for the reason given.
    Invalid {
        /// The object.
        target: ObjectRef,
        /// Why, for the user.
        reason: &'static str,
    },
}

impl EditError {
    /// The object the error is about; `None` for the list as a whole.
    pub fn target(&self) -> Option<ObjectRef> {
        match self {
            Self::UnsupportedVersion { .. } | Self::WrongGame { .. } => None,
            Self::Duplicate { target }
            | Self::UnknownObject { target }
            | Self::Unsupported { target, .. }
            | Self::OutOfRange { target, .. }
            | Self::WrongCount { target, .. }
            | Self::UnknownSpecies { target, .. }
            | Self::UnknownItem { target, .. }
            | Self::UnknownMove { target, .. }
            | Self::UnknownAbility { target, .. }
            | Self::UnknownType { target, .. }
            | Self::Invalid { target, .. } => Some(*target),
        }
    }

    /// The field the error is about, when it names one.
    pub fn field(&self) -> Option<&'static str> {
        match self {
            Self::Unsupported { field, .. } | Self::OutOfRange { field, .. } | Self::WrongCount { field, .. } => {
                Some(field)
            }
            _ => None,
        }
    }

    /// What is wrong, without the object: the message after `"trainer 7: "`.
    pub fn detail(&self) -> String {
        match self {
            Self::UnsupportedVersion { found } => format!(
                "these edits are in format {found}, which this version can't read (it reads format {EDIT_LIST_VERSION})"
            ),
            Self::WrongGame { expected, found } => format!("these edits are for {found}, not for {expected}"),
            Self::Duplicate { .. } => "it is edited twice".into(),
            Self::UnknownObject { .. } => "it is not in this game".into(),
            Self::Unsupported { field, .. } => format!("this game has no {field}"),
            Self::OutOfRange {
                field, value, min, max, ..
            } => format!("{field} is {value}, but must be {min}–{max}"),
            Self::WrongCount {
                field, expected, found, ..
            } => format!("{field} has {found} entries, but the game has {expected}"),
            Self::UnknownSpecies { species, .. } => format!("species #{species} is not in this game"),
            Self::UnknownItem { item, .. } => format!("item {item} is not in this game"),
            Self::UnknownMove { mv, .. } => format!("move {mv} is not in this game"),
            Self::UnknownAbility { ability, .. } => format!("ability {ability} is not in this game"),
            Self::UnknownType { ty, .. } => format!("this game has no {ty} type"),
            Self::Invalid { reason, .. } => (*reason).to_owned(),
        }
    }
}

impl fmt::Display for EditError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.target() {
            Some(target) => write!(f, "{target}: {}", self.detail()),
            None => f.write_str(&self.detail()),
        }
    }
}

impl std::error::Error for EditError {}
