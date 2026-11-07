//! Totem Pokémon (Gen 7): new species for the totems and the allies they call, new auras, new held items and new
//! levels.
//!
//! Each totem in turn gets its new species, then its allies theirs, then its aura, its held item and the levels,
//! as in the original. Totems and allies draw from one pool of species, each taken once until the pool runs out
//! and fills again. "Allow Alternate Formes" adds the alternate formes that are not only cosmetic to the pool; the
//! pool leaves out the alternate formes that need their species' ability while abilities stay and, with "No
//! Irregular Alt Formes", the irregular ones. A new species that is a base species gets one of its cosmetic formes
//! (or the plain one) at random.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005); species are taken in national order.
//! - "Randomize Held Items" fails the run when the game has no consumable held items, instead of crashing.

use std::collections::BTreeSet;

use ppr_model::{Aura, AuraStat, DexNumber, GameData, ItemId, TotemPokemon};
use ppr_settings::{AuraMode, TotemMode, TotemSettings};

use super::common::{Pool, banned_formes, forme_for, modified_level, pool_with_formes, power_level_replacement};
use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Totem Pokémon";

/// Aura bytes of a random aura: one stat or every stat, raised by 1 to 3 stages.
const RANDOM_AURAS: u8 = 18;

/// Changes Totem Pokémon, their allies and auras according to [`TotemSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TotemRandomizer;

impl Step<RunData> for TotemRandomizer {
    fn id(&self) -> u64 {
        step_ids::TOTEMS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        !data.game.totems.is_empty() && data.settings.totems.changes_anything()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.totems.clone();
        let banned = banned_formes(
            &data.game,
            false,
            data.settings.abilities.mode != ppr_settings::AbilityMode::Unchanged,
            data.settings.general.no_irregular_alt_formes,
        );
        data.game.totems = randomize(&data.game, &s, &banned, &mut rng)?;
        Ok(())
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// A new species and forme for a totem or ally of `old`'s species and forme, the species taken from the pool.
fn pick(
    rng: &mut StepRng,
    game: &GameData,
    pool: &mut Pool,
    old: (DexNumber, u8),
    mode: TotemMode,
) -> Result<(DexNumber, u8), RandomizeError> {
    let new = match mode {
        TotemMode::Unchanged => return Ok(old),
        TotemMode::Random => pool.take(rng),
        TotemMode::SimilarStrength => power_level_replacement(rng, game, &pool.left, old.0, false, true),
    }
    .ok_or_else(|| impossible("no species to choose from"))?;
    pool.remove(new);
    Ok((new, forme_for(rng, game, new, true)))
}

/// A random aura: any stat or every stat, raised by 1 to 3 stages.
fn random_aura(rng: &mut StepRng) -> Aura {
    let b = u8::try_from(below(rng, usize::from(RANDOM_AURAS))).unwrap_or_default() + 1;
    Aura::from_byte(b)
}

/// An aura of the same strength: another single stat (or the same), raised by as many stages. No aura, and auras
/// that raise every stat, stay.
fn same_strength_aura(rng: &mut StepRng, old: Aura) -> Aura {
    if matches!(old.stat, AuraStat::None | AuraStat::All) {
        return old;
    }
    Aura {
        stat: AuraStat::SINGLE[below(rng, AuraStat::SINGLE.len())],
        stages: old.stages,
    }
}

/// The totems after the settings; `banned` are the alternate formes the pool leaves out.
fn randomize(
    game: &GameData,
    s: &TotemSettings,
    banned: &BTreeSet<DexNumber>,
    rng: &mut StepRng,
) -> Result<Vec<TotemPokemon>, RandomizeError> {
    let mut all = pool_with_formes(game, s.allow_alt_formes);
    all.retain(|n| !banned.contains(n));
    let mut pool = Pool::new(all);
    let consumables: Vec<ItemId> = game
        .held_item_pools
        .consumable
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut totems = game.totems.clone();
    for t in &mut totems {
        (t.species, t.forme) = pick(rng, game, &mut pool, (t.species, t.forme), s.mode)?;
        for a in &mut t.allies {
            (a.species, a.forme) = pick(rng, game, &mut pool, (a.species, a.forme), s.ally_mode)?;
        }
        t.aura = match s.aura_mode {
            AuraMode::Unchanged => t.aura,
            AuraMode::Random => random_aura(rng),
            AuraMode::SameStrength => same_strength_aura(rng, t.aura),
        };
        // Only totems that hold an item get a new one.
        if s.randomize_held_items && t.held_item.is_some() {
            if consumables.is_empty() {
                return Err(impossible("the game has no consumable held items"));
            }
            t.held_item = Some(consumables[below(rng, consumables.len())]);
        }
        if s.level_modifier != 0 {
            t.level = modified_level(t.level, s.level_modifier);
            for a in &mut t.allies {
                a.level = modified_level(a.level, s.level_modifier);
            }
        }
    }
    Ok(totems)
}

#[cfg(test)]
mod tests;
