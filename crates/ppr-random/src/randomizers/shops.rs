//! Special shops sell shuffled or random items, prices can be balanced, and every shop can sell cheap Rare
//! Candies.
//!
//! Deliberate choices:
//! - Gen 6 and 7 shops keep their size, so the Rare Candies only get cheap there; the handlers fail on a shop with
//!   another number of items.
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - When the shops cannot take the items (more guaranteed items than main-game slots, or nothing to choose
//!   from), the run stops with a message instead of an internal error.

use std::collections::BTreeSet;

use ppr_model::{GameData, ItemId, RARE_CANDY, REGULAR_SHOP_ITEMS, Shop, X_ITEMS};
use ppr_settings::{ShopItemMode, ShopItemSettings};

use crate::choice::shuffle;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Shop Items";

/// The price of the Rare Candies "Add Cheap Rare Candies" adds.
const CHEAP_RARE_CANDY: u32 = 10;

/// Changes shops according to [`ppr_settings::ShopItemSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ShopItemRandomizer;

impl Step<RunData> for ShopItemRandomizer {
    fn id(&self) -> u64 {
        step_ids::SHOPS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.shops;
        (s.mode != ShopItemMode::Unchanged || s.balance_prices || s.add_cheap_rare_candies)
            && !data.game.shops.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.shops.clone();
        let game = &mut data.game;
        match s.mode {
            ShopItemMode::Unchanged => {}
            ShopItemMode::Shuffle => shuffle_items(&mut game.shops, &mut rng),
            ShopItemMode::Random => game.shops = random_items(game, &s, &mut rng)?,
        }
        if s.balance_prices {
            // Balanced prices for the items the game has them for.
            for (id, &price) in &game.balanced_item_prices {
                if let Some(p) = game.item_prices.get_mut(id) {
                    *p = price;
                }
            }
        }
        if s.add_cheap_rare_candies {
            // Every shop, special or not, where shops can grow. Gen 6 and 7 shops keep their size, so there the
            // candies only get cheap, as in the original, whose writes leave out the item it adds.
            if game.identity.generation.number() < 6 {
                for shop in &mut game.shops {
                    shop.items.push(RARE_CANDY);
                }
            }
            if let Some(p) = game.item_prices.get_mut(&RARE_CANDY) {
                *p = CHEAP_RARE_CANDY;
            }
        }
        Ok(())
    }
}

fn impossible(message: &str) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// The special shops' items, shuffled among them.
fn shuffle_items(shops: &mut [Shop], rng: &mut StepRng) {
    let mut all: Vec<ItemId> = shops
        .iter()
        .filter(|s| s.special)
        .flat_map(|s| s.items.clone())
        .collect();
    shuffle(rng, &mut all);
    let mut next = all.into_iter();
    for shop in shops.iter_mut().filter(|s| s.special) {
        for item in &mut shop.items {
            *item = next.next().unwrap_or(*item);
        }
    }
}

/// The items random shops may sell: allowed (or non-bad) items other than TMs, without the banned groups.
fn possible(game: &GameData, s: &ShopItemSettings) -> BTreeSet<ItemId> {
    let base = if s.ban_bad {
        game.non_bad_items()
    } else {
        game.allowed_items()
    };
    base.into_iter()
        .filter(|&id| !game.items.iter().any(|i| i.id == id && i.tm))
        .filter(|id| !s.ban_regular || !REGULAR_SHOP_ITEMS.contains(id))
        .filter(|id| !s.ban_overpowered || !game.op_shop_items.contains(id))
        .collect()
}

/// The items random shops must sell.
fn guaranteed(game: &GameData, s: &ShopItemSettings) -> BTreeSet<ItemId> {
    let mut items = BTreeSet::new();
    if s.guarantee_evolution_items {
        items.extend(&game.evolution_items);
    }
    if s.guarantee_x_items {
        items.extend(X_ITEMS);
    }
    items
}

/// Random shop items: the guaranteed items, then the possible ones evenly spread; non-main-game shops
/// get none of the guaranteed ones.
fn random_items(game: &GameData, s: &ShopItemSettings, rng: &mut StepRng) -> Result<Vec<Shop>, RandomizeError> {
    let guaranteed = guaranteed(game, s);
    let possible: Vec<ItemId> = possible(game, s).difference(&guaranteed).copied().collect();
    let mut shops = game.shops.clone();
    // The new items, one per special shop slot.
    let slots: usize = shops.iter().filter(|s| s.special).map(|s| s.items.len()).sum();
    let mut new: Vec<ItemId> = guaranteed.iter().copied().collect();
    let mut remaining: Vec<ItemId> = Vec::new();
    for _ in 0..slots.saturating_sub(guaranteed.len()) {
        if remaining.is_empty() {
            remaining.clone_from(&possible);
            shuffle(rng, &mut remaining);
        }
        new.push(remaining.pop().ok_or_else(|| impossible("no items to choose from"))?);
    }
    // Place them: shops outside the main game take non-guaranteed items first, main-game shops the rest.
    shuffle(rng, &mut new);
    for shop in shops.iter_mut().filter(|s| s.special && !s.main_game) {
        for item in &mut shop.items {
            let k = new
                .iter()
                .position(|i| !guaranteed.contains(i))
                .ok_or_else(|| impossible("not enough items for the shops outside the main game"))?;
            *item = new.remove(k);
        }
    }
    shuffle(rng, &mut new);
    let mut rest = new.into_iter();
    for shop in shops.iter_mut().filter(|s| s.special && s.main_game) {
        for item in &mut shop.items {
            *item = rest
                .next()
                .ok_or_else(|| impossible("more guaranteed items than the main-game shops sell"))?;
        }
    }
    if rest.next().is_some() {
        return Err(impossible("more guaranteed items than the main-game shops sell"));
    }
    Ok(shops)
}

#[cfg(test)]
mod tests;
