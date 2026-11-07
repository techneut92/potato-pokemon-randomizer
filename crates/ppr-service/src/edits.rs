//! Manual edits (ADR 0008) for front ends: the editable view of a loaded game, checking an edit list against it, and
//! the ROM open in the app with the edits made to it.

use std::path::{Path, PathBuf};

use ppr_model::{
    EditError, EditGame, EditList, EditRules, EncounterKind, EncounterSet, FieldItemEdit, GameData, InGameTrade,
    ItemId, ItemPriceEdit, ShopEdit, Species, SpeciesEdit, StarterEdit, StarterHeldItemEdit, StaticEdit, StatsEdit,
    TradeEdit, Trainer, TrainerEdit, TrainerPokemonEdit, Type, WildSetEdit, WildSlotEdit,
};
pub use ppr_model::{EditIssue, EditSeverity};
use serde::Serialize;
use ts_rs::TS;

use crate::{LoadedRom, ServiceError};

/// What the manual edits editor shows of a game: every object an edit can change, as the game has it when loaded
/// (before any randomizer), with the names it needs to show them.
///
/// Each object's values are in the shape of its edit ([`TrainerEdit`], [`ShopEdit`], …), so an unchanged object
/// and its edit compare equal, and the values are the "before" of the changes list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableGame {
    /// The game, for [`EditList::game`].
    pub game: EditGame,
    /// The generation, 1–7.
    pub generation: u8,
    /// The types the game has, in its order.
    pub types: Vec<Type>,
    /// Every species, by national number.
    pub species: Vec<EditableSpecies>,
    /// Ability names by number, 0 being no ability; empty when the game has no abilities.
    pub abilities: Vec<String>,
    /// Move names by number, `moves[0]` being the empty name of "no move"; just that one entry when the game's moves
    /// are not read.
    pub moves: Vec<String>,
    /// The game's items, by number.
    pub items: Vec<ItemName>,
    /// The starters, in the game's order.
    pub starters: Vec<StarterEdit>,
    /// The starters' held items, in the game's order; one for all three in Gen 3, none before Gen 2.
    pub starter_held_items: Vec<StarterHeldItemEdit>,
    /// Trainer class names by number.
    pub trainer_classes: Vec<String>,
    /// The trainer battles, in the game's order.
    pub trainers: Vec<EditableTrainer>,
    /// The wild encounter sets, in the handler's order.
    pub wild_sets: Vec<EditableWildSet>,
    /// The static encounters, in the handler's order.
    pub statics: Vec<EditableStatic>,
    /// The in-game trades, in the game's order.
    pub trades: Vec<TradeEdit>,
    /// The shops, in the handler's order.
    pub shops: Vec<EditableShop>,
    /// What items cost, for the items that have a price, by item number.
    pub item_prices: Vec<ItemPriceEdit>,
    /// The field items, in the game's map order.
    pub field_items: Vec<FieldItemEdit>,
    /// What the game allows edits to do, as its ROM handler reports it.
    pub rules: EditRules,
}

/// A species with its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableSpecies {
    /// The species' name.
    pub name: String,
    /// Its types, base stats and abilities.
    pub value: SpeciesEdit,
}

/// An item's number and name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct ItemName {
    /// The standard item number.
    pub id: ItemId,
    /// The name.
    pub name: String,
    /// A TM, whose price follows the TM price rules.
    pub tm: bool,
}

/// A trainer battle with what tells it apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableTrainer {
    /// The trainer's name; may be empty, as for some classes in Gen 1.
    pub name: String,
    /// The trainer class, by number.
    pub class: u16,
    /// The class name.
    pub class_name: String,
    /// What the battle is in the story: `GYM1`, `GYM1-LEADER`, `ELITE1`, `CHAMPION`, `RIVAL1-0`, … `None` for a
    /// regular trainer.
    pub tag: Option<String>,
    /// The team.
    pub value: TrainerEdit,
}

/// A wild encounter set with where and how it is met.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableWildSet {
    /// Its name, e.g. "Route 101 Grass/Cave"; several sets can share one.
    pub name: String,
    /// How it is met.
    pub kind: EncounterKind,
    /// The encounter rate, as the game stores it.
    pub rate: u8,
    /// Only reachable after the main story.
    pub post_game: bool,
    /// The slots.
    pub value: WildSetEdit,
}

/// A static encounter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableStatic {
    /// An egg, whose level can't be changed.
    pub egg: bool,
    /// The species and level.
    pub value: StaticEdit,
}

/// A shop with its name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditableShop {
    /// Where it is.
    pub name: String,
    /// Reached in the main game, before the Hall of Fame.
    pub main_game: bool,
    /// Sells something other than the usual Poké Mart goods.
    pub special: bool,
    /// What it sells.
    pub value: ShopEdit,
}

/// The editable view of `game`, with its handler's edit `rules`.
#[must_use]
pub fn editable_game(game: &GameData, rules: &EditRules) -> EditableGame {
    let mut moves = Vec::with_capacity(game.moves.len() + 1);
    moves.push(String::new());
    moves.extend(game.moves.iter().map(|m| m.name.clone()));
    EditableGame {
        game: EditGame::of(&game.identity),
        generation: game.identity.generation.number(),
        types: game.types.clone(),
        species: game.species.iter().map(species_view).collect(),
        abilities: game.ability_names.clone(),
        moves,
        items: game
            .items
            .iter()
            .map(|i| ItemName {
                id: i.id,
                name: i.name.clone(),
                tm: i.tm,
            })
            .collect(),
        starters: game
            .starters
            .iter()
            .enumerate()
            .map(|(slot, &species)| StarterEdit { slot, species })
            .collect(),
        starter_held_items: game
            .starter_held_items
            .iter()
            .enumerate()
            .map(|(slot, &item)| StarterHeldItemEdit { slot, item })
            .collect(),
        trainer_classes: game.trainer_class_names.clone(),
        trainers: game.trainers.iter().map(|t| trainer_view(game, t)).collect(),
        wild_sets: game
            .encounters
            .iter()
            .enumerate()
            .map(|(index, set)| wild_set_view(index, set))
            .collect(),
        statics: game
            .statics
            .iter()
            .enumerate()
            .map(|(index, s)| EditableStatic {
                egg: s.egg,
                value: StaticEdit {
                    index,
                    species: s.species,
                    level: s.level,
                },
            })
            .collect(),
        trades: game
            .trades
            .iter()
            .enumerate()
            .map(|(index, t)| trade_edit(index, t))
            .collect(),
        shops: game
            .shops
            .iter()
            .enumerate()
            .map(|(index, s)| EditableShop {
                name: s.name.clone(),
                main_game: s.main_game,
                special: s.special,
                value: ShopEdit {
                    index,
                    items: s.items.clone(),
                },
            })
            .collect(),
        item_prices: game
            .item_prices
            .iter()
            .map(|(&item, &price)| ItemPriceEdit { item, price })
            .collect(),
        field_items: game
            .field_items
            .iter()
            .enumerate()
            .map(|(index, &item)| FieldItemEdit { index, item })
            .collect(),
        rules: rules.clone(),
    }
}

fn species_view(s: &Species) -> EditableSpecies {
    EditableSpecies {
        name: s.name.clone(),
        value: SpeciesEdit {
            number: s.number,
            primary_type: s.primary_type,
            secondary_type: s.secondary_type,
            stats: StatsEdit {
                hp: s.stats.hp,
                attack: s.stats.attack,
                defense: s.stats.defense,
                speed: s.stats.speed,
                sp_attack: s.stats.sp_attack,
                sp_defense: s.stats.sp_defense,
            },
            abilities: s.abilities.clone(),
        },
    }
}

fn trainer_view(game: &GameData, t: &Trainer) -> EditableTrainer {
    EditableTrainer {
        name: t.name.clone(),
        class: t.class,
        class_name: game
            .trainer_class_names
            .get(usize::from(t.class))
            .cloned()
            .unwrap_or_default(),
        tag: t.tag.clone(),
        value: TrainerEdit {
            index: t.index,
            pokemon: t
                .pokemon
                .iter()
                .map(|p| TrainerPokemonEdit {
                    species: p.species,
                    level: p.level,
                    held_item: p.held_item,
                    // All 0 when it knows the moves it learns by its level.
                    moves: (!p.reset_moves && p.moves.iter().any(|&m| m != 0)).then_some(p.moves),
                })
                .collect(),
        },
    }
}

fn wild_set_view(index: usize, set: &EncounterSet) -> EditableWildSet {
    EditableWildSet {
        name: set.name.clone(),
        kind: set.kind,
        rate: set.rate,
        post_game: set.post_game,
        value: WildSetEdit {
            index,
            slots: set
                .encounters
                .iter()
                .map(|e| WildSlotEdit {
                    species: e.species,
                    min_level: e.min_level,
                    max_level: e.max_level,
                })
                .collect(),
        },
    }
}

fn trade_edit(index: usize, t: &InGameTrade) -> TradeEdit {
    TradeEdit {
        index,
        requested: t.requested,
        given: t.given,
        nickname: t.nickname.clone(),
        ot_name: t.ot_name.clone(),
        ot_id: t.ot_id,
        ivs: t.ivs.clone(),
        held_item: t.held_item,
    }
}

/// Checks `edits` against `game`, the game as loaded, with its handler's `rules` ([`EditList::validate`]). An edit
/// list in another format or for another game fails as a whole ([`ServiceError::EditList`]); otherwise every issue
/// of every edit is returned, errors and warnings, empty when there is none.
pub fn validate_edits(game: &GameData, rules: &EditRules, edits: &EditList) -> Result<Vec<EditIssue>, ServiceError> {
    Ok(edits.validate(game, rules)?)
}

/// The issues [`EditList::apply`]'s errors are, naming objects as `game` does, or the error about the whole list
/// among them.
pub(crate) fn issues(errors: Vec<EditError>, game: &GameData) -> Result<Vec<EditIssue>, ServiceError> {
    let mut found = Vec::with_capacity(errors.len());
    for error in errors {
        if error.target().is_none() {
            return Err(error.into());
        }
        found.push(EditIssue::from_error(&error, game));
    }
    Ok(found)
}

/// The [`ppr_random::Progress::step`] of a run that applies `count` manual edits.
#[must_use]
pub fn applying_edits_step(count: usize) -> String {
    if count == 1 {
        "Applying 1 manual edit".to_owned()
    } else {
        format!("Applying {count} manual edits")
    }
}

/// A ROM open in the app: its data as loaded, kept for the editor, and the manual edits made to it.
#[derive(Debug, Clone)]
pub struct RomSession {
    path: PathBuf,
    game: GameData,
    rules: EditRules,
    edits: EditList,
}

impl RomSession {
    /// Opens the ROM at `path` like [`crate::load_rom_with_update`], with the game update at `update` laid over it,
    /// and keeps its data when its generation is ported: the editor and validation see the game as a run does
    /// before randomizing. The wild sets are always those without time of day; a run with time of day applies an
    /// edit of a wild set to all the sets of its table.
    pub fn open(path: &Path, update: Option<&Path>) -> Result<(LoadedRom, Option<Self>), ServiceError> {
        let (rom, game) = crate::roms::load(path, update)?;
        let session = game.map(|(game, rules)| Self {
            path: path.to_owned(),
            edits: EditList::new(&game.identity),
            game,
            rules,
        });
        Ok((rom, session))
    }

    /// Takes over the edits of `previous` when it is the same file and game, as when the user opens the ROM again.
    /// Another ROM starts without edits.
    pub fn keep_edits_of(&mut self, previous: Option<Self>) {
        if let Some(previous) = previous
            && previous.path == self.path
            && previous.edits.game.matches(&self.game.identity)
        {
            self.edits = previous.edits;
        }
    }

    /// The ROM file.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The game as loaded.
    #[must_use]
    pub fn game(&self) -> &GameData {
        &self.game
    }

    /// What the game allows edits to do, as its handler reports it.
    #[must_use]
    pub fn rules(&self) -> &EditRules {
        &self.rules
    }

    /// The editable view of the game.
    #[must_use]
    pub fn editable(&self) -> EditableGame {
        editable_game(&self.game, &self.rules)
    }

    /// The manual edits made to this ROM; an empty list for its game until [`RomSession::set_edits`].
    #[must_use]
    pub fn edits(&self) -> &EditList {
        &self.edits
    }

    /// Checks `edits` against this ROM (see [`validate_edits`]).
    pub fn validate(&self, edits: &EditList) -> Result<Vec<EditIssue>, ServiceError> {
        validate_edits(&self.game, &self.rules, edits)
    }

    /// Keeps `edits` for this ROM and returns their issues. A list in another format or for another game is
    /// rejected and the edits stay as they were; a list with issues is kept, as the user is still editing it.
    pub fn set_edits(&mut self, edits: EditList) -> Result<Vec<EditIssue>, ServiceError> {
        let issues = self.validate(&edits)?;
        self.edits = edits;
        Ok(issues)
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{EDIT_LIST_VERSION, Edit};

    use super::*;

    fn emerald() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("emerald.gba");
        std::fs::write(&path, ppr_romio::testing::emerald()).expect("write");
        (dir, path)
    }

    fn session() -> (tempfile::TempDir, RomSession) {
        let (dir, path) = emerald();
        let (_, session) = RomSession::open(&path, None).expect("opens");
        (dir, session.expect("ported"))
    }

    #[test]
    fn the_view_is_the_loaded_game_in_the_shape_of_its_edits() {
        let (_dir, session) = session();
        let game = session.game();
        let view = session.editable();
        assert_eq!(view.game, EditGame::of(&game.identity));
        assert_eq!(view.generation, 3);
        assert_eq!(view.species.len(), game.species.len());
        let treecko = view.species.iter().find(|s| s.value.number == 252).expect("species");
        assert_eq!(treecko.name, game.species(252).expect("species").name);
        assert_eq!(view.moves.len(), game.moves.len() + 1);
        assert_eq!(view.moves[0], "");
        assert_eq!(
            view.starters.iter().map(|s| s.species).collect::<Vec<_>>(),
            game.starters
        );
        assert_eq!(view.starter_held_items.len(), 1, "Gen 3 has one for all");
        assert_eq!(view.trainers.len(), game.trainers.len());
        assert_eq!(view.wild_sets.len(), game.encounters.len());
        assert_eq!(view.statics.len(), game.statics.len());
        assert_eq!(view.trades.len(), game.trades.len());
        assert_eq!(view.shops.len(), game.shops.len());
        assert_eq!(view.item_prices.len(), game.item_prices.len());
        assert_eq!(view.field_items.len(), game.field_items.len());
        let t = &view.trainers[0];
        assert_eq!(t.class_name, game.trainer_class_names[usize::from(t.class)]);

        // Every object's values, sent back as edits, are valid and change nothing.
        let mut list = EditList::new(&game.identity);
        list.edits.extend(view.starters.iter().cloned().map(Edit::Starter));
        list.edits
            .extend(view.starter_held_items.iter().cloned().map(Edit::StarterHeldItem));
        // The synthetic ROM's species have no first ability, which no real game has (the real-ROM tests check those).
        list.edits.extend(
            view.species
                .iter()
                .filter(|s| s.value.abilities.first() != Some(&0))
                .map(|s| Edit::Species(s.value.clone())),
        );
        list.edits
            .extend(view.trainers.iter().map(|t| Edit::Trainer(t.value.clone())));
        list.edits
            .extend(view.wild_sets.iter().map(|w| Edit::WildSet(w.value.clone())));
        list.edits
            .extend(view.statics.iter().map(|s| Edit::Static(s.value.clone())));
        list.edits.extend(view.trades.iter().cloned().map(Edit::Trade));
        list.edits
            .extend(view.shops.iter().map(|s| Edit::Shop(s.value.clone())));
        list.edits.extend(view.item_prices.iter().cloned().map(Edit::ItemPrice));
        list.edits.extend(view.field_items.iter().cloned().map(Edit::FieldItem));
        assert_eq!(session.validate(&list).expect("this game"), []);
        let mut copy = game.clone();
        list.apply(&mut copy).expect("applies");
        assert_eq!(editable_game(&copy, session.rules()), view);
    }

    #[test]
    fn issues_point_at_the_edit_and_its_field() {
        let (_dir, session) = session();
        let mut list = EditList::new(&session.game().identity);
        list.set(Edit::Starter(StarterEdit { slot: 0, species: 9999 }));
        let mut trainer = session.editable().trainers[0].value.clone();
        trainer.pokemon[0].level = 0;
        let index = trainer.index;
        list.set(Edit::Trainer(trainer));
        let issues = session.validate(&list).expect("this game");
        assert_eq!(issues.len(), 2, "{issues:?}");
        assert_eq!(issues[0].target, Some(ppr_model::ObjectRef::Starter { slot: 0 }));
        assert_eq!(issues[0].field, None);
        assert!(issues[0].message.contains("#9999"), "{}", issues[0].message);
        assert_eq!(issues[1].target, Some(ppr_model::ObjectRef::Trainer { index }));
        assert_eq!(issues[1].field.as_deref(), Some("level"));
        assert!(issues.iter().all(|i| i.severity == EditSeverity::Error));
    }

    #[test]
    fn lists_for_another_game_or_format_fail_as_a_whole() {
        let (_dir, mut session) = session();
        let mut list = EditList::new(&session.game().identity);
        list.set(Edit::Starter(StarterEdit { slot: 0, species: 1 }));
        assert_eq!(session.set_edits(list.clone()).expect("this game"), []);

        let mut other = list.clone();
        other.game.name = "Ruby (U)".into();
        let err = session.set_edits(other).expect_err("another game");
        assert_eq!(err.code(), "edits_wrong_game");
        let mut newer = list.clone();
        newer.version = EDIT_LIST_VERSION + 1;
        assert_eq!(
            session.validate(&newer).expect_err("newer").code(),
            "edits_unsupported_version"
        );
        assert_eq!(session.edits(), &list, "a rejected list leaves the edits as they were");
    }

    #[test]
    fn edits_are_kept_per_rom() {
        let (_dir, mut session) = session();
        assert_eq!(session.edits().edits.len(), 0);
        let mut list = EditList::new(&session.game().identity);
        list.set(Edit::Starter(StarterEdit { slot: 0, species: 9999 }));
        assert_eq!(
            session.set_edits(list.clone()).expect("this game").len(),
            1,
            "kept with its issue"
        );
        assert_eq!(session.edits(), &list);

        let (_, again) = RomSession::open(session.path(), None).expect("opens");
        let mut again = again.expect("ported");
        again.keep_edits_of(Some(session.clone()));
        assert_eq!(again.edits(), &list, "the same ROM keeps its edits");

        let (_other_dir, other_path) = emerald();
        let (_, other) = RomSession::open(&other_path, None).expect("opens");
        let mut other = other.expect("ported");
        other.keep_edits_of(Some(session));
        assert!(other.edits().edits.is_empty(), "another ROM starts without edits");
    }

    #[test]
    fn the_step_counts_the_edits() {
        assert_eq!(applying_edits_step(1), "Applying 1 manual edit");
        assert_eq!(applying_edits_step(3), "Applying 3 manual edits");
    }
}
