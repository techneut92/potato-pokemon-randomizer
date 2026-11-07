//! Checking edits against a game and applying them to its data.

use std::collections::BTreeSet;

use super::{
    Edit, EditError, EditList, FieldItemEdit, ItemPriceEdit, ObjectRef, ShopEdit, SpeciesEdit, StarterEdit,
    StarterHeldItemEdit, StaticEdit, TradeEdit, TrainerEdit, TrainerPokemonEdit, WildSetEdit,
};
use crate::{BaseStats, DexNumber, GameData, ItemId, MoveId, TrainerPokemon, Type, WildEncounter};

/// The highest level a Pokémon can have.
const MAX_LEVEL: u8 = 100;
/// The most Pokémon a team can have.
const MAX_TEAM: usize = 6;

pub(super) fn apply(list: &EditList, data: &mut GameData) -> Result<(), Vec<EditError>> {
    list.check(&data.identity).map_err(|e| vec![e])?;
    let mut out = data.clone();
    let mut errors = Vec::new();
    let mut seen = BTreeSet::new();
    for edit in &list.edits {
        let target = edit.target();
        if !seen.insert(target) {
            errors.push(EditError::Duplicate { target });
            continue;
        }
        let found = check(&out, edit);
        if found.is_empty() {
            write(&mut out, edit);
        } else {
            errors.extend(found);
        }
    }
    if errors.is_empty() {
        *data = out;
        Ok(())
    } else {
        Err(errors)
    }
}

/// Collects what doesn't fit while one edit is checked.
struct Check<'a> {
    data: &'a GameData,
    target: ObjectRef,
    errors: Vec<EditError>,
}

impl Check<'_> {
    fn generation(&self) -> u8 {
        self.data.identity.generation.number()
    }

    fn invalid(&mut self, reason: &'static str) {
        self.errors.push(EditError::Invalid {
            target: self.target,
            reason,
        });
    }

    fn unsupported(&mut self, field: &'static str) {
        self.errors.push(EditError::Unsupported {
            target: self.target,
            field,
        });
    }

    fn range(&mut self, field: &'static str, value: u32, min: u32, max: u32) {
        if !(min..=max).contains(&value) {
            self.errors.push(EditError::OutOfRange {
                target: self.target,
                field,
                value,
                min,
                max,
            });
        }
    }

    fn level(&mut self, field: &'static str, level: u8) {
        self.range(field, level.into(), 1, MAX_LEVEL.into());
    }

    fn count(&mut self, field: &'static str, expected: usize, found: usize) -> bool {
        if expected != found {
            self.errors.push(EditError::WrongCount {
                target: self.target,
                field,
                expected,
                found,
            });
        }
        expected == found
    }

    fn species(&mut self, species: DexNumber) {
        if self.data.species(species).is_none() {
            self.errors.push(EditError::UnknownSpecies {
                target: self.target,
                species,
            });
        }
    }

    fn item(&mut self, item: ItemId) {
        if !self.data.items.iter().any(|i| i.id == item) {
            self.errors.push(EditError::UnknownItem {
                target: self.target,
                item,
            });
        }
    }

    /// A held item; Gen 1 has none.
    fn held_item(&mut self, item: Option<ItemId>) {
        if let Some(item) = item {
            if self.generation() == 1 {
                self.unsupported("held items");
            } else {
                self.item(item);
            }
        }
    }

    fn ty(&mut self, ty: Type) {
        if !self.data.types.contains(&ty) {
            self.errors.push(EditError::UnknownType {
                target: self.target,
                ty,
            });
        }
    }

    fn moves(&mut self, moves: [MoveId; 4]) {
        if self.data.moves.is_empty() {
            self.unsupported("moves");
            return;
        }
        if moves.iter().all(|&m| m == 0) {
            self.invalid("a Pokémon with its own moves needs at least one");
        }
        for mv in moves {
            if mv != 0 && usize::from(mv) > self.data.moves.len() {
                self.errors.push(EditError::UnknownMove {
                    target: self.target,
                    mv,
                });
            }
        }
    }

    fn unknown(&mut self) {
        self.errors.push(EditError::UnknownObject { target: self.target });
    }
}

/// What doesn't fit the game in `edit`; empty when it can be applied.
pub(super) fn check(data: &GameData, edit: &Edit) -> Vec<EditError> {
    let mut c = Check {
        data,
        target: edit.target(),
        errors: Vec::new(),
    };
    match edit {
        Edit::Starter(e) => check_starter(&mut c, e),
        Edit::StarterHeldItem(e) => check_starter_held_item(&mut c, e),
        Edit::Species(e) => check_species(&mut c, e),
        Edit::Trainer(e) => check_trainer(&mut c, e),
        Edit::WildSet(e) => check_wild_set(&mut c, e),
        Edit::Static(e) => check_static(&mut c, e),
        Edit::Trade(e) => check_trade(&mut c, e),
        Edit::Shop(e) => check_shop(&mut c, e),
        Edit::ItemPrice(e) => check_item_price(&mut c, e),
        Edit::FieldItem(e) => check_field_item(&mut c, e),
    }
    c.errors
}

fn check_starter(c: &mut Check<'_>, e: &StarterEdit) {
    if e.slot >= c.data.starters.len() {
        return c.unknown();
    }
    c.species(e.species);
}

fn check_starter_held_item(c: &mut Check<'_>, e: &StarterHeldItemEdit) {
    if e.slot >= c.data.starter_held_items.len() {
        return c.unknown();
    }
    c.held_item(e.item);
}

fn check_species(c: &mut Check<'_>, e: &SpeciesEdit) {
    let Some(species) = c.data.species(e.number) else {
        return c.unknown();
    };
    c.ty(e.primary_type);
    if let Some(secondary) = e.secondary_type {
        c.ty(secondary);
        if secondary == e.primary_type {
            c.invalid("the second type is the same as the first; leave it empty for a single type");
        }
    }
    let s = e.stats;
    // Gen 1 has one Special stat, kept as the Special Attack; its Special Defense is 0.
    let gen1 = c.generation() == 1;
    for (field, value) in [
        ("HP", s.hp),
        ("Attack", s.attack),
        ("Defense", s.defense),
        ("Speed", s.speed),
        (if gen1 { "Special" } else { "Special Attack" }, s.sp_attack),
    ] {
        c.range(field, value.into(), 1, u8::MAX.into());
    }
    if !gen1 {
        c.range("Special Defense", s.sp_defense.into(), 1, u8::MAX.into());
    } else if s.sp_defense != 0 {
        c.unsupported("Special Defense");
    }
    let slots = species.abilities.len();
    if slots == 0 && !e.abilities.is_empty() {
        return c.unsupported("abilities");
    }
    if !c.count("abilities", slots, e.abilities.len()) {
        return;
    }
    if e.abilities.first() == Some(&0) {
        c.invalid("the first ability slot can't be empty");
    }
    for &ability in &e.abilities {
        if ability != 0 && usize::from(ability) >= c.data.ability_names.len() {
            c.errors.push(EditError::UnknownAbility {
                target: c.target,
                ability,
            });
        }
    }
}

fn check_trainer(c: &mut Check<'_>, e: &TrainerEdit) {
    if !c.data.trainers.iter().any(|t| t.index == e.index) {
        return c.unknown();
    }
    let size = u32::try_from(e.pokemon.len()).unwrap_or(u32::MAX);
    c.range("team size", size, 1, u32::try_from(MAX_TEAM).unwrap_or(u32::MAX));
    for p in &e.pokemon {
        c.species(p.species);
        c.level("level", p.level);
        c.held_item(p.held_item);
        if let Some(moves) = p.moves {
            c.moves(moves);
        }
    }
}

fn check_wild_set(c: &mut Check<'_>, e: &WildSetEdit) {
    let Some(set) = c.data.encounters.get(e.index) else {
        return c.unknown();
    };
    if !c.count("slots", set.encounters.len(), e.slots.len()) {
        return;
    }
    for slot in &e.slots {
        c.species(slot.species);
        c.level("lowest level", slot.min_level);
        c.level("highest level", slot.max_level);
        if slot.min_level > slot.max_level {
            c.invalid("the lowest level is above the highest");
        }
    }
}

fn check_static(c: &mut Check<'_>, e: &StaticEdit) {
    let Some(encounter) = c.data.statics.get(e.index) else {
        return c.unknown();
    };
    c.species(e.species);
    if encounter.egg {
        if e.level != encounter.level {
            c.unsupported("level for an egg");
        }
    } else {
        c.level("level", e.level);
    }
}

fn check_trade(c: &mut Check<'_>, e: &TradeEdit) {
    let Some(trade) = c.data.trades.get(e.index) else {
        return c.unknown();
    };
    if let Some(requested) = e.requested {
        c.species(requested);
    }
    c.species(e.given);
    if e.nickname.is_empty() {
        c.invalid("the nickname is empty");
    }
    if c.generation() == 1 {
        if !e.ot_name.is_empty() || e.ot_id != 0 {
            c.unsupported("original trainer");
        }
    } else {
        if e.ot_name.is_empty() {
            c.invalid("the original trainer's name is empty");
        }
        c.range(
            "original trainer ID",
            e.ot_id,
            0,
            super::rules::max_trade_ot_id(c.generation()),
        );
    }
    if trade.ivs.is_empty() && !e.ivs.is_empty() {
        c.unsupported("IVs");
    } else if c.count("IVs", trade.ivs.len(), e.ivs.len()) {
        let max = if c.generation() <= 2 { 15 } else { 31 };
        let random = super::rules::random_trade_iv(c.generation());
        for &iv in e.ivs.iter().filter(|&&iv| Some(iv) != random) {
            c.range("IV", iv.into(), 0, max);
        }
    }
    c.held_item(e.held_item);
}

fn check_shop(c: &mut Check<'_>, e: &ShopEdit) {
    if e.index >= c.data.shops.len() {
        return c.unknown();
    }
    if e.items.is_empty() {
        c.invalid("a shop must sell at least one item");
    }
    for &item in &e.items {
        c.item(item);
    }
}

fn check_item_price(c: &mut Check<'_>, e: &ItemPriceEdit) {
    if !c.data.item_prices.contains_key(&e.item) {
        c.unknown();
    }
}

fn check_field_item(c: &mut Check<'_>, e: &FieldItemEdit) {
    if e.index >= c.data.field_items.len() {
        return c.unknown();
    }
    c.item(e.item);
}

/// Applies an edit [`check`] found nothing wrong with.
fn write(data: &mut GameData, edit: &Edit) {
    match edit {
        Edit::Starter(e) => {
            if let Some(s) = data.starters.get_mut(e.slot) {
                *s = e.species;
            }
        }
        Edit::StarterHeldItem(e) => {
            if let Some(i) = data.starter_held_items.get_mut(e.slot) {
                *i = e.item;
            }
        }
        Edit::Species(e) => write_species(data, e),
        Edit::Trainer(e) => write_trainer(data, e),
        Edit::WildSet(e) => write_wild_set(data, e),
        Edit::Static(e) => {
            let forme = forme_number(data, e.species);
            if let Some(s) = data.statics.get_mut(e.index) {
                if s.species != e.species {
                    s.forme = forme;
                }
                s.species = e.species;
                if !s.egg {
                    s.level = e.level;
                }
            }
        }
        Edit::Trade(e) => {
            if let Some(t) = data.trades.get_mut(e.index) {
                t.requested = e.requested;
                t.given = e.given;
                t.nickname.clone_from(&e.nickname);
                t.ot_name.clone_from(&e.ot_name);
                t.ot_id = e.ot_id;
                t.ivs.clone_from(&e.ivs);
                t.held_item = e.held_item;
            }
        }
        Edit::Shop(e) => {
            if let Some(s) = data.shops.get_mut(e.index) {
                s.items.clone_from(&e.items);
            }
        }
        Edit::ItemPrice(e) => {
            data.item_prices.insert(e.item, e.price);
        }
        Edit::FieldItem(e) => {
            if let Some(i) = data.field_items.get_mut(e.index) {
                *i = e.item;
            }
        }
    }
}

fn write_species(data: &mut GameData, e: &SpeciesEdit) {
    let changes_types = data
        .species(e.number)
        .is_some_and(|s| (s.primary_type, s.secondary_type) != (e.primary_type, e.secondary_type));
    if changes_types {
        data.keep_original_types();
    }
    let Ok(i) = data.species.binary_search_by_key(&e.number, |s| s.number) else {
        return;
    };
    let species = &mut data.species[i];
    species.primary_type = e.primary_type;
    species.secondary_type = e.secondary_type;
    let s = e.stats;
    species.stats = BaseStats {
        hp: s.hp,
        attack: s.attack,
        defense: s.defense,
        speed: s.speed,
        sp_attack: s.sp_attack,
        sp_defense: s.sp_defense,
    };
    species.abilities.clone_from(&e.abilities);
}

fn write_trainer(data: &mut GameData, e: &TrainerEdit) {
    let formes: Vec<u8> = e.pokemon.iter().map(|p| forme_number(data, p.species)).collect();
    let Some(trainer) = data.trainers.iter_mut().find(|t| t.index == e.index) else {
        return;
    };
    let ivs = trainer.pokemon.last().map_or(0, |p| p.ivs);
    let old = std::mem::take(&mut trainer.pokemon);
    trainer.pokemon = e
        .pokemon
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let mut tp = old.get(i).cloned().unwrap_or_else(|| TrainerPokemon {
                ivs,
                added: true,
                ..TrainerPokemon::new(p.species, p.level)
            });
            if tp.species != p.species || tp.added {
                tp.forme = formes[i];
            }
            set_trainer_pokemon(&mut tp, p);
            tp
        })
        .collect();
}

fn set_trainer_pokemon(tp: &mut TrainerPokemon, p: &TrainerPokemonEdit) {
    tp.species = p.species;
    tp.level = p.level;
    tp.held_item = p.held_item;
    tp.moves = p.moves.unwrap_or([0; 4]);
    tp.reset_moves = p.moves.is_none();
}

fn write_wild_set(data: &mut GameData, e: &WildSetEdit) {
    let formes: Vec<u8> = e.slots.iter().map(|s| forme_number(data, s.species)).collect();
    if let Some(set) = data.encounters.get_mut(e.index) {
        let old = std::mem::take(&mut set.encounters);
        set.encounters = e
            .slots
            .iter()
            .enumerate()
            .map(|(i, s)| WildEncounter {
                species: s.species,
                forme: old
                    .get(i)
                    .filter(|o| o.species == s.species)
                    .map_or(formes[i], |o| o.forme),
                min_level: s.min_level,
                max_level: s.max_level,
            })
            .collect();
    }
}

/// The forme a Pokémon of species number `n` is in: an alternate forme's number, 0 for a base species. A Pokémon
/// whose species stays keeps its forme (a cosmetic one, say).
fn forme_number(data: &GameData, n: DexNumber) -> u8 {
    data.species(n).map_or(0, |sp| sp.forme.number)
}
