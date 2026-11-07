//! Field item randomization: items on the ground and hidden items are shuffled or random, TMs staying TMs and other
//! items other items. Random items place each Mega Stone only once, unless shop prices are balanced and they can be
//! bought.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - When nothing but Mega Stones placed already is left to choose from, the step fails with
//!   [`RandomizeError::Impossible`], where the original crashes.

use std::collections::BTreeSet;

use ppr_model::{GameData, ItemId};
use ppr_settings::{FieldItemMode, FieldItemSettings};

use crate::choice::{below, shuffle};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Field Items";

/// Changes field items according to [`ppr_settings::FieldItemSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct FieldItemRandomizer;

impl Step<RunData> for FieldItemRandomizer {
    fn id(&self) -> u64 {
        step_ids::FIELD_ITEMS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.field_items.mode != FieldItemMode::Unchanged && !data.game.field_items.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.field_items.clone();
        let new = randomize(&data.game, &s, data.settings.shops.balance_prices, &mut rng)?;
        data.game.field_items = new;
        Ok(())
    }
}

fn is_tm(game: &GameData, id: ItemId) -> bool {
    game.items.iter().any(|i| i.id == id && i.tm)
}

/// TMs and other items are changed apart, then put back where their kind was.
fn randomize(
    game: &GameData,
    s: &FieldItemSettings,
    balanced_prices: bool,
    rng: &mut StepRng,
) -> Result<Vec<ItemId>, RandomizeError> {
    let current = &game.field_items;
    let (mut tms, mut others): (Vec<ItemId>, Vec<ItemId>) = current.iter().partition(|&&i| is_tm(game, i));
    match s.mode {
        FieldItemMode::Unchanged => {}
        FieldItemMode::Shuffle => {
            shuffle(rng, &mut tms);
            shuffle(rng, &mut others);
        }
        FieldItemMode::Random | FieldItemMode::RandomEven => {
            tms = random_tms(game, tms.len(), rng)?;
            others = random_others(game, s, balanced_prices, others.len(), rng)?;
        }
    }
    // Both stacks are taken from the end.
    Ok(current
        .iter()
        .map(|&i| if is_tm(game, i) { tms.pop() } else { others.pop() })
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default())
}

/// TMs: the required TMs and random others, no TM twice.
fn random_tms(game: &GameData, count: usize, rng: &mut StepRng) -> Result<Vec<ItemId>, RandomizeError> {
    let all: Vec<ItemId> = game.items.iter().filter(|i| i.tm).map(|i| i.id).collect();
    let mut new: BTreeSet<ItemId> = game.required_field_tms.iter().copied().collect();
    while new.len() < count && new.len() < all.len() {
        new.insert(all[below(rng, all.len())]);
    }
    if new.len() != count {
        return Err(RandomizeError::Impossible {
            step: LABEL,
            message: "too many TMs asked for".into(),
        });
    }
    let mut tms: Vec<ItemId> = new.into_iter().collect();
    shuffle(rng, &mut tms);
    Ok(tms)
}

/// Other items: any allowed item but TMs, or without bad items; evenly spread, each item
/// once before any twice, or entirely at random. A Mega Stone that is chosen is not chosen again, unless shop prices
/// are balanced.
fn random_others(
    game: &GameData,
    s: &FieldItemSettings,
    balanced_prices: bool,
    count: usize,
    rng: &mut StepRng,
) -> Result<Vec<ItemId>, RandomizeError> {
    let mut possible = if s.ban_bad {
        game.non_bad_items()
    } else {
        game.allowed_items()
    };
    possible.retain(|&i| !is_tm(game, i));
    let unique: &[ItemId] = if balanced_prices { &[] } else { &game.mega_stones };
    let mut chosen = Vec::with_capacity(count);
    if s.mode == FieldItemMode::RandomEven {
        let mut remaining: Vec<ItemId> = Vec::new();
        for _ in 0..count {
            if remaining.is_empty() {
                remaining.clone_from(&possible);
                shuffle(rng, &mut remaining);
            }
            let item = remaining.pop().ok_or_else(no_items)?;
            chosen.push(item);
            if unique.contains(&item) {
                possible.retain(|&i| i != item);
            }
        }
    } else {
        for _ in 0..count {
            if possible.is_empty() {
                return Err(no_items());
            }
            let item = possible[below(rng, possible.len())];
            chosen.push(item);
            if unique.contains(&item) {
                possible.retain(|&i| i != item);
            }
        }
    }
    Ok(chosen)
}

fn no_items() -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: "no items to choose from".into(),
    }
}

#[cfg(test)]
mod tests;
