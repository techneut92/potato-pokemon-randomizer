//! What the editor shows the user about an edit list: errors that block saving and warnings that don't, each
//! pointing at its object.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{EditError, ObjectRef};
use crate::{GameData, ItemId};

/// How serious an [`EditIssue`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EditSeverity {
    /// The edits can't be saved or run like this.
    Error,
    /// Allowed, but the game may not play as expected.
    Warning,
}

/// One thing wrong with, or risky about, an edit list, for the user.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditIssue {
    /// The object it is about; `None` for the list as a whole (another game, another format) or a limit several
    /// objects share (the shop space of Gen 4).
    pub target: Option<ObjectRef>,
    /// The field of the object it is about, in words (`"team size"`, `"nickname"`), when it is about one.
    pub field: Option<String>,
    /// Whether it blocks saving.
    pub severity: EditSeverity,
    /// A sentence for the user, naming the object.
    pub message: String,
}

impl EditIssue {
    /// An error about `target`, its message starting with the object's name in `data`.
    pub(super) fn error(data: &GameData, target: ObjectRef, field: Option<&str>, text: &str) -> Self {
        Self::about(data, EditSeverity::Error, target, field, text)
    }

    /// A warning about `target`, its message starting with the object's name in `data`.
    pub(super) fn warning(data: &GameData, target: ObjectRef, field: Option<&str>, text: &str) -> Self {
        Self::about(data, EditSeverity::Warning, target, field, text)
    }

    fn about(data: &GameData, severity: EditSeverity, target: ObjectRef, field: Option<&str>, text: &str) -> Self {
        Self {
            target: Some(target),
            field: field.map(str::to_owned),
            severity,
            message: format!("{}: {text}", object_name(data, target)),
        }
    }

    /// The issue for an error [`EditList::apply`](super::EditList::apply) or
    /// [`EditList::check`](super::EditList::check) returned, naming its object as `data` (the loaded game) does.
    pub fn from_error(error: &EditError, data: &GameData) -> Self {
        match error.target() {
            Some(target) => Self::error(data, target, error.field(), &error.detail()),
            None => Self {
                target: None,
                field: None,
                severity: EditSeverity::Error,
                message: capitalized(&error.to_string()),
            },
        }
    }
}

fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

fn item_name(data: &GameData, item: ItemId) -> String {
    data.items
        .iter()
        .find(|i| i.id == item)
        .map_or_else(|| format!("item {item}"), |i| i.name.clone())
}

fn species_name(data: &GameData, number: crate::DexNumber) -> String {
    data.species(number)
        .map_or_else(|| format!("#{number}"), |s| s.name.clone())
}

/// How the editor names `target` in `data`: its place, counted from 1, and what it is, e.g. `Shop 3 (Cherrygrove
/// City)` or `Trainer 7 (Youngster Joey)`.
pub fn object_name(data: &GameData, target: ObjectRef) -> String {
    let with = |what: String, detail: Option<String>| match detail.filter(|d| !d.trim().is_empty()) {
        Some(d) => format!("{what} ({})", d.trim()),
        None => what,
    };
    match target {
        ObjectRef::Starter { slot } => with(
            format!("Starter {}", slot + 1),
            data.starters.get(slot).map(|&s| species_name(data, s)),
        ),
        ObjectRef::StarterHeldItem { slot } => format!("Starter held item {}", slot + 1),
        ObjectRef::Species { number } => with(
            format!("Species #{number}"),
            data.species(number).map(|s| s.name.clone()),
        ),
        ObjectRef::Trainer { index } => with(
            format!("Trainer {index}"),
            data.trainers.iter().find(|t| t.index == index).map(|t| {
                let class = data
                    .trainer_class_names
                    .get(usize::from(t.class))
                    .map_or("", String::as_str);
                format!("{class} {}", t.name)
            }),
        ),
        ObjectRef::WildSet { index } => with(
            format!("Wild set {}", index + 1),
            data.encounters.get(index).map(|s| s.name.clone()),
        ),
        ObjectRef::Static { index } => with(
            format!("Static encounter {}", index + 1),
            data.statics.get(index).map(|s| species_name(data, s.species)),
        ),
        ObjectRef::Trade { index } => with(
            format!("Trade {}", index + 1),
            data.trades.get(index).map(|t| t.nickname.clone()),
        ),
        ObjectRef::Shop { index } => with(
            format!("Shop {}", index + 1),
            data.shops.get(index).map(|s| s.name.clone()),
        ),
        ObjectRef::ItemPrice { item } => format!("Price of {}", item_name(data, item)),
        ObjectRef::FieldItem { index } => with(
            format!("Field item {}", index + 1),
            data.field_items.get(index).map(|&i| item_name(data, i)),
        ),
    }
}
