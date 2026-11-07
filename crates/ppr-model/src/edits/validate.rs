//! Checking an edit list against the loaded game and its handler's rules, for the editor.

use std::collections::BTreeSet;

use super::rules::{EditRules, ShopSize, TrainerLimits, WildLevels};
use super::{Edit, EditError, EditIssue, EditList, ObjectRef, ShopEdit, TradeEdit, TrainerEdit, WildSetEdit, apply};
use crate::{GameData, PriceError};

pub(super) fn validate(list: &EditList, data: &GameData, rules: &EditRules) -> Result<Vec<EditIssue>, EditError> {
    list.check(&data.identity)?;
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    for edit in &list.edits {
        let target = edit.target();
        if !seen.insert(target) {
            issues.push(EditIssue::from_error(&EditError::Duplicate { target }, data));
            continue;
        }
        let errors = apply::check(data, edit);
        if !errors.is_empty() {
            // The object or a value is unknown: the rules below would only repeat it.
            issues.extend(errors.iter().map(|e| EditIssue::from_error(e, data)));
            continue;
        }
        let mut v = Validation {
            data,
            rules,
            list,
            target,
            issues: &mut issues,
        };
        match edit {
            Edit::Shop(e) => v.shop(e),
            Edit::ItemPrice(e) => v.price(e.item, e.price),
            Edit::Trainer(e) => v.trainer(e),
            Edit::WildSet(e) => v.wild_set(e),
            Edit::Trade(e) => v.trade(e),
            Edit::FieldItem(e) => v.field_item(e.index, e.item),
            Edit::Starter(_) | Edit::StarterHeldItem(_) | Edit::Species(_) | Edit::Static(_) => {}
        }
    }
    shop_space(list, data, rules, &mut issues);
    Ok(issues)
}

/// Collects the issues of one edit.
struct Validation<'a> {
    data: &'a GameData,
    rules: &'a EditRules,
    list: &'a EditList,
    target: ObjectRef,
    issues: &'a mut Vec<EditIssue>,
}

impl Validation<'_> {
    fn error(&mut self, field: Option<&str>, text: &str) {
        self.issues.push(EditIssue::error(self.data, self.target, field, text));
    }

    fn warning(&mut self, field: Option<&str>, text: &str) {
        self.issues
            .push(EditIssue::warning(self.data, self.target, field, text));
    }

    fn shop(&mut self, e: &ShopEdit) {
        let Some(shop) = self.data.shops.get(e.index) else {
            return;
        };
        let (have, n) = (shop.items.len(), e.items.len());
        match &self.rules.shops.size {
            ShopSize::Fixed if n != have => {
                let fix = if n > have {
                    format!("remove {}", n - have)
                } else {
                    format!("add {}", have - n)
                };
                self.error(
                    Some("items"),
                    &format!("{n} items, but this game keeps every shop at its size, {have}: {fix}"),
                );
            }
            ShopSize::Shrink if n > have => {
                self.error(
                    Some("items"),
                    &format!("{n} items, but this game has no room to move a shop to, so it holds at most its {have}"),
                );
            }
            ShopSize::Grow { max: Some(max) } | ShopSize::Shared { max, .. } if n > *max => {
                self.error(
                    Some("items"),
                    &format!("{n} items, but this game's shops hold at most {max}"),
                );
            }
            _ => {}
        }
    }

    fn price(&mut self, item: crate::ItemId, price: u32) {
        match self.rules.prices.of_item(self.data, item).check(price) {
            Ok(()) => {}
            Err(PriceError::TooHigh { max }) => {
                self.error(Some("price"), &format!("{price}, but this game stores at most {max}"));
            }
            Err(PriceError::Step { step }) => {
                self.error(
                    Some("price"),
                    &format!("{price}, but this game stores prices in steps of {step}"),
                );
            }
        }
    }

    fn trainer(&mut self, e: &TrainerEdit) {
        let Some(limits) = self.rules.trainer(e.index).copied() else {
            return;
        };
        self.team_size(e, &limits);
        let why = |game_has: bool| {
            if game_has {
                "its team has no room for them"
            } else {
                "this game doesn't store them"
            }
        };
        if !limits.held_items && e.pokemon.iter().any(|p| p.held_item.is_some()) {
            let why = why(self.rules.trainers.held_items);
            self.error(
                Some("held items"),
                &format!("this trainer's Pokémon can't hold items: {why}"),
            );
        }
        if !limits.custom_moves && e.pokemon.iter().any(|p| p.moves.is_some()) {
            let why = why(self.rules.trainers.custom_moves);
            self.error(
                Some("moves"),
                &format!("this trainer's Pokémon can't have moves of their own: {why}"),
            );
        }
        if !limits.levels {
            let loaded = self.data.trainers.iter().find(|t| t.index == e.index);
            let changed = loaded.is_some_and(|t| e.pokemon.iter().zip(&t.pokemon).any(|(p, l)| p.level != l.level));
            if changed {
                self.error(
                    Some("level"),
                    "the game sets this trainer's levels, so they can't change",
                );
            }
        }
        self.starter_links(e);
    }

    /// How many Pokémon trainer `index` has in the loaded game.
    fn loaded_team(&self, index: u16) -> usize {
        self.data
            .trainers
            .iter()
            .find(|t| t.index == index)
            .map_or(0, |t| t.pokemon.len())
    }

    /// The team's size against the trainer's limits; a team bigger than 3 for a trainer that may battle together
    /// with another is a warning only where the edit makes it so.
    fn team_size(&mut self, e: &TrainerEdit, limits: &TrainerLimits) {
        let n = e.pokemon.len();
        if n == 0 {
            return; // `apply` reports an empty team.
        }
        if n > limits.max_pokemon {
            let why = if limits.max_pokemon < 6 {
                "this trainer's team can't grow"
            } else {
                "a team holds at most 6"
            };
            self.error(
                Some("team size"),
                &format!("{n} Pokémon, but {why} beyond {}", limits.max_pokemon),
            );
        } else if limits.shares_battle && n > 3 && n > self.loaded_team(e.index) {
            self.warning(
                Some("team size"),
                &format!("{n} Pokémon, but when it battles together with another trainer it only uses its first 3"),
            );
        }
        if n < limits.min_pokemon {
            self.error(
                Some("team size"),
                &format!(
                    "{n} Pokémon, but its battle style needs at least {}",
                    limits.min_pokemon
                ),
            );
        }
    }

    fn starter_links(&mut self, e: &TrainerEdit) {
        for link in self.rules.starter_links.iter().filter(|l| l.trainer == e.index) {
            let starter = self
                .list
                .edits
                .iter()
                .find_map(|x| match x {
                    Edit::Starter(s) if s.slot == link.starter => Some(s.species),
                    _ => None,
                })
                .or_else(|| self.data.starters.get(link.starter).copied());
            let Some(starter) = starter else { continue };
            let name = self
                .data
                .species(starter)
                .map_or_else(|| format!("#{starter}"), |s| s.name.clone());
            match e.pokemon.get(link.slot) {
                None => self.error(
                    Some("team size"),
                    &format!(
                        "slot {} is the rival's starter ({name}) and can't be removed",
                        link.slot + 1
                    ),
                ),
                Some(p) if p.species != starter => self.error(
                    Some("species"),
                    &format!(
                        "slot {} follows the rival's starter ({name}); change it under starter {}",
                        link.slot + 1,
                        link.starter + 1
                    ),
                ),
                Some(_) => {}
            }
        }
    }

    fn wild_set(&mut self, e: &WildSetEdit) {
        let wild = &self.rules.wild;
        let loaded = self.data.encounters.get(e.index).map_or(&[][..], |s| &s.encounters[..]);
        match wild.levels.get(e.index).copied().unwrap_or(WildLevels::Range) {
            WildLevels::Range => {}
            WildLevels::Single => {
                if e.slots.iter().any(|s| s.min_level != s.max_level) {
                    self.error(
                        Some("level"),
                        "each slot here has one level, so its lowest and highest level must be the same",
                    );
                }
            }
            WildLevels::Shared => {
                if let Some(first) = e.slots.first()
                    && e.slots
                        .iter()
                        .any(|s| (s.min_level, s.max_level) != (first.min_level, first.max_level))
                {
                    self.error(
                        Some("level"),
                        "the slots here share one level range, so every slot needs the same lowest and highest level",
                    );
                }
            }
            WildLevels::Fixed => {
                let changed = e
                    .slots
                    .iter()
                    .zip(loaded)
                    .any(|(s, l)| (s.min_level, s.max_level) != (l.min_level, l.max_level));
                if changed {
                    self.error(
                        Some("level"),
                        "the levels here can't change: the game takes them from another encounter",
                    );
                }
            }
        }
        let set_bans = wild
            .banned_in_sets
            .iter()
            .filter(|b| b.set == e.index)
            .flat_map(|b| &b.species);
        let banned: BTreeSet<_> = wild.banned.iter().chain(set_bans).copied().collect();
        let mut named = BTreeSet::new();
        // Where the game has a banned species itself (Unown in the Tanoby Ruins), it can stay.
        let own: BTreeSet<_> = loaded.iter().map(|l| l.species).collect();
        for slot in &e.slots {
            if banned.contains(&slot.species) && !own.contains(&slot.species) && named.insert(slot.species) {
                let name = self
                    .data
                    .species(slot.species)
                    .map_or_else(|| format!("#{}", slot.species), |s| s.name.clone());
                self.error(Some("species"), &format!("{name} can't appear here in this game"));
            }
        }
    }

    fn trade(&mut self, e: &TradeEdit) {
        let t = self.rules.trades;
        let loaded = self.data.trades.get(e.index);
        if e.requested.is_none() && loaded.is_some_and(|l| l.requested.is_some()) {
            self.error(
                Some("requested"),
                "this trade needs a species to ask for: the game can't take any Pokémon",
            );
        }
        // Only a text that changes must fit: the game's own always does.
        let nickname_changed = loaded.is_none_or(|l| l.nickname != e.nickname);
        let ot_changed = t.original_trainer && loaded.is_none_or(|l| l.ot_name != e.ot_name);
        if !t.texts {
            if nickname_changed {
                self.error(Some("nickname"), "this game's trade nicknames can't change");
            }
            if ot_changed {
                self.error(
                    Some("original trainer"),
                    "this game's trade original trainer names can't change",
                );
            }
            return;
        }
        let nickname = self.data.text_length(&e.nickname);
        if nickname_changed && nickname > t.max_nickname_length {
            self.error(
                Some("nickname"),
                &format!(
                    "the nickname takes {nickname} characters, but this game has room for {}",
                    t.max_nickname_length
                ),
            );
        }
        let ot = self.data.text_length(&e.ot_name);
        if ot_changed && ot > t.max_ot_name_length {
            self.error(
                Some("original trainer"),
                &format!(
                    "the original trainer's name takes {ot} characters, but this game has room for {}",
                    t.max_ot_name_length
                ),
            );
        }
    }

    fn field_item(&mut self, index: usize, item: crate::ItemId) {
        let Some(&before) = self.data.field_items.get(index) else {
            return;
        };
        if before != item && self.rules.field_items.required.contains(&before) {
            let name = self
                .data
                .items
                .iter()
                .find(|i| i.id == before)
                .map_or_else(|| format!("item {before}"), |i| i.name.clone());
            self.warning(
                Some("item"),
                &format!("{name} is only found here, so replacing it leaves the player without it"),
            );
        }
    }
}

/// The shared shop area of Gen 4: all shops, edited or not, must fit in it.
fn shop_space(list: &EditList, data: &GameData, rules: &EditRules, issues: &mut Vec<EditIssue>) {
    let ShopSize::Shared {
        bytes,
        costs,
        only_changed,
        ..
    } = &rules.shops.size
    else {
        return;
    };
    let used: usize = data
        .shops
        .iter()
        .enumerate()
        .filter_map(|(i, shop)| {
            let edited = list.edits.iter().find_map(|e| match e {
                Edit::Shop(s) if s.index == i && s.items != shop.items => Some(s.items.len()),
                _ => None,
            });
            let items = match edited {
                Some(n) => n,
                None if *only_changed => return None,
                None => shop.items.len(),
            };
            Some(costs.get(i).map_or(0, |c| c.of(items)))
        })
        .sum();
    if used > *bytes {
        issues.push(EditIssue {
            target: None,
            field: Some("shop space".into()),
            severity: super::EditSeverity::Error,
            message: format!(
                "The shop lists need {} bytes more than the ROM has room for. Remove items from any shop.",
                used - bytes
            ),
        });
    }
}
