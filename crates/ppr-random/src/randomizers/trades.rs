//! In-game trade randomization.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005). A species or name is drawn once from the unused
//!   ones rather than redrawn until an unused one comes up, which is the same distribution.
//! - Gen 1 trades have no original trainer, individual values or held item, so those are left alone.
//! - Not implemented yet: cosmetic formes (none in Gen 3).

use ppr_model::{DexNumber, GameData};
use ppr_settings::{CustomNames, TradeMode, TradeSettings};

use super::common::SpeciesSet;
use crate::choice::{below, pick};
use crate::names::{NameList, names};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "In-Game Trades";

/// Individual values run below this from Gen 3 on; Gen 1 and 2 have DVs, below 16.
const IV_LIMIT: u8 = 32;
const DV_LIMIT: u8 = 16;

/// Randomizes in-game trades according to [`TradeSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TradeRandomizer;

impl Step<RunData> for TradeRandomizer {
    fn id(&self) -> u64 {
        step_ids::TRADES
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.trades.mode != TradeMode::Unchanged && !data.game.trades.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.trades.clone();
        let custom = data.settings.custom_names.clone();
        randomize(&mut data.game, &s, custom.as_ref(), &mut rng)
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// The names of `list` that take at most `max` bytes in the game, each once, in list order.
fn fitting<'a>(game: &GameData, list: &[&'a str], max: usize) -> Vec<&'a str> {
    let mut names: Vec<&'a str> = Vec::new();
    for &name in list {
        if game.text_length(name) <= max && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// A name from `names` not in `used`, now used; `None` once all are.
fn unused_name<'a>(rng: &mut StepRng, names: &[&'a str], used: &mut Vec<&'a str>) -> Option<&'a str> {
    let left: Vec<&'a str> = names.iter().copied().filter(|n| !used.contains(n)).collect();
    if left.is_empty() {
        return None;
    }
    let name = left[below(rng, left.len())];
    used.push(name);
    Some(name)
}

/// Randomizes the trades' species, nicknames, original trainers, individual values and items as chosen.
fn randomize(
    game: &mut GameData,
    s: &TradeSettings,
    custom: Option<&CustomNames>,
    rng: &mut StepRng,
) -> Result<(), RandomizeError> {
    let all: SpeciesSet = game.species_pool();
    let nicknames = if s.random_nicknames {
        fitting(
            game,
            &names(custom, NameList::PokemonNicknames),
            game.max_trade_nickname_length,
        )
    } else {
        Vec::new()
    };
    // Gen 1 trades have no original trainer, individual values or held item.
    let full_trades = game.identity.generation.number() != 1;
    let ot_names = if s.random_ots && full_trades {
        fitting(
            game,
            &names(custom, NameList::TrainerNames),
            game.max_trade_ot_name_length,
        )
    } else {
        Vec::new()
    };
    let items = game.allowed_items();
    let iv_limit = if game.identity.generation.number() <= 2 {
        DV_LIMIT
    } else {
        IV_LIMIT
    };
    let name_of = |game: &GameData, n: DexNumber| game.species(n).map(|sp| sp.name.clone()).unwrap_or_default();

    let mut used_given = SpeciesSet::new();
    let mut used_requested = SpeciesSet::new();
    let mut used_nicknames = Vec::new();
    let mut used_ots = Vec::new();
    let mut trades = game.trades.clone();
    for trade in &mut trades {
        let old_given = trade.given;
        let left: SpeciesSet = all.difference(&used_given).copied().collect();
        let given = pick(rng, &left).ok_or_else(|| impossible("no species left to give"))?;
        used_given.insert(given);
        trade.given = given;

        if trade.requested == Some(old_given) {
            // A trade of a species for itself stays one.
            trade.requested = Some(given);
        } else if s.mode == TradeMode::GivenAndRequested && trade.requested.is_some() {
            let left: SpeciesSet = all
                .iter()
                .copied()
                .filter(|n| *n != given && !used_requested.contains(n))
                .collect();
            let requested = pick(rng, &left).ok_or_else(|| impossible("no species left to ask for"))?;
            used_requested.insert(requested);
            trade.requested = Some(requested);
        }

        if let Some(nickname) = unused_name(rng, &nicknames, &mut used_nicknames) {
            nickname.clone_into(&mut trade.nickname);
        } else if trade.nickname.to_lowercase() == name_of(game, old_given).to_lowercase() {
            // Named after its species: keep it that way.
            trade.nickname = name_of(game, given);
        }

        if let Some(ot) = unused_name(rng, &ot_names, &mut used_ots) {
            ot.clone_into(&mut trade.ot_name);
            trade.ot_id = u32::try_from(below(rng, 1 << 16)).unwrap_or_default();
        }

        if s.random_ivs && full_trades {
            for iv in &mut trade.ivs {
                *iv = u8::try_from(below(rng, usize::from(iv_limit))).unwrap_or_default();
            }
        }

        if s.random_items && full_trades && !items.is_empty() {
            trade.held_item = Some(items[below(rng, items.len())]);
        }
    }
    game.trades = trades;
    Ok(())
}

#[cfg(test)]
mod tests;
