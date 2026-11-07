//! New starters and starter held items.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - With custom starters and the Unique rule, only the slots without a custom species are picked.
//! - With type triangles, a triangle without valid starters is not drawn again, so the draw always ends, and only
//!   the triangle the starters come from is recorded.
//! - Custom starters are base species: the custom list of the settings names species by national number.
//!
//! Where starters can be alternate formes, "Allow Alternate Formes" adds the alternate formes that are not only
//! cosmetic, but those that need their species' ability while abilities stay and, with "No Irregular Alt Formes",
//! the irregular ones. A starter keeps no random cosmetic forme, as in the original.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, GameData, ItemId, Type};
use ppr_settings::{StarterMode, StarterSettings, StarterTypes};

use super::common::{banned_formes, pool_with_formes};
use crate::choice::{below, shuffle, take};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Starter Pokémon";
/// The "no maximum" BST.
const MAX_BST: u16 = 1530;

/// Chooses new starters according to [`StarterSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct StarterRandomizer;

impl Step<RunData> for StarterRandomizer {
    fn id(&self) -> u64 {
        step_ids::STARTERS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.starters;
        s.mode != StarterMode::Unchanged || s.random_held_items
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = &data.settings.starters;
        if s.mode != StarterMode::Unchanged {
            let banned = banned_formes(
                &data.game,
                false,
                data.settings.abilities.mode != ppr_settings::AbilityMode::Unchanged,
                data.settings.general.no_irregular_alt_formes,
            );
            let (starters, triangle) = choose(&data.game, s, &banned, &mut rng)?;
            data.game.starters = starters;
            if triangle.is_some() {
                data.game.starter_type_triangle = triangle;
            }
        }
        if s.random_held_items {
            data.game.starter_held_items = held_items(&data.game, s.ban_bad_items, &mut rng)?;
        }
        Ok(())
    }
}

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

type SpeciesSet = BTreeSet<DexNumber>;

/// The new starters and, with the type triangle rule, the triangle they were chosen from.
type Choice = (Vec<DexNumber>, Option<[Type; 3]>);

/// The new starters; `banned` are the alternate formes "Allow Alternate Formes" leaves out.
fn choose(
    game: &GameData,
    s: &StarterSettings,
    banned: &SpeciesSet,
    rng: &mut StepRng,
) -> Result<Choice, RandomizeError> {
    let count = game.starters.len();
    let mut picked: Vec<DexNumber> = Vec::with_capacity(count);
    if s.mode == StarterMode::Custom {
        // The custom species come first and random ones are appended.
        for n in s.custom.iter().flatten() {
            if !game.species(*n).is_some_and(ppr_model::Species::is_base_forme) {
                return Err(impossible(format!("custom starter #{n} is not a Pokémon in this game")));
            }
            picked.push(*n);
        }
        if picked.len() > count {
            return Err(impossible(format!(
                "{} custom starters, but this game has {count}",
                picked.len()
            )));
        }
        if picked.len() == count {
            return Ok((picked, None));
        }
    }

    let mut choosable = available(game, s, banned, &picked);
    let mut triangle = None;
    if choosable.len() < count - picked.len() {
        return Err(impossible("not enough Pokémon match the starter restrictions"));
    }

    match s.types {
        StarterTypes::None => {
            let needed = count - picked.len();
            picked.extend(choose_basic(rng, needed, &mut choosable)?);
        }
        StarterTypes::Unique => {
            while picked.len() < count {
                let start_of_trio = picked.len() / 3 * 3;
                let end_of_trio = (start_of_trio + 3).min(count);
                let in_trio: Vec<DexNumber> = picked[start_of_trio..].to_vec();
                let needed = end_of_trio - picked.len();
                let new = choose_unique_type(game, rng, &in_trio, needed, &choosable)?;
                picked.extend(new);
                choosable.retain(|n| !picked.contains(n));
            }
        }
        StarterTypes::FireWaterGrass => {
            let standard = game.standard_type_triangle();
            picked = choose_of_types(game, rng, &by_type(game, &choosable), &standard)?;
            while picked.len() < count {
                choosable.retain(|n| !picked.contains(n));
                picked.extend(choose_of_types(game, rng, &by_type(game, &choosable), &standard)?);
            }
        }
        StarterTypes::SingleType(chosen) => {
            let needed = count - picked.len();
            let mut by_type = by_type(game, &choosable);
            let t = match chosen {
                Some(t) => t,
                None => choose_type(game, rng, needed, &by_type)?,
            };
            let mut pool = by_type.remove(&t).unwrap_or_default();
            picked.extend(choose_basic(rng, needed, &mut pool)?);
        }
        StarterTypes::TypeTriangle => {
            let (first, chosen) = choose_type_triangle(game, rng, &by_type(game, &choosable))?;
            picked = first;
            triangle = Some(chosen);
            while picked.len() < count {
                choosable.retain(|n| !picked.contains(n));
                picked.extend(choose_type_triangle(game, rng, &by_type(game, &choosable))?.0);
            }
        }
    }
    picked.truncate(count);
    Ok((picked, triangle))
}

/// Every `[c, b, a]` where `a` beats `b`, `b` beats `c` and `c` beats `a`, so each
/// type is beaten by the next one, like Fire, Water, Grass. Each triangle appears once per starting type.
fn find_type_triangles(game: &GameData) -> BTreeSet<[Type; 3]> {
    let chart = &game.type_chart;
    let mut triangles = BTreeSet::new();
    for &one in chart.types() {
        for two in chart
            .super_effective_when_attacking(one)
            .into_iter()
            .filter(|&t| t != one)
        {
            for three in chart
                .super_effective_when_attacking(two)
                .into_iter()
                .filter(|&t| t != one && t != two)
            {
                if chart.super_effective_when_attacking(three).contains(&one) {
                    triangles.insert([three, two, one]);
                }
            }
        }
    }
    triangles
}

/// One starter per type of a random triangle that has valid starters.
fn choose_type_triangle(
    game: &GameData,
    rng: &mut StepRng,
    by_type: &BTreeMap<Type, SpeciesSet>,
) -> Result<(Vec<DexNumber>, [Type; 3]), RandomizeError> {
    let mut triangles: Vec<[Type; 3]> = find_type_triangles(game).into_iter().collect();
    if triangles.is_empty() {
        return Err(impossible("the type chart has no type triangles"));
    }
    shuffle(rng, &mut triangles);
    while !triangles.is_empty() {
        let triangle = triangles[below(rng, triangles.len())];
        match choose_of_types(game, rng, by_type, &triangle) {
            Ok(picks) => return Ok((picks, triangle)),
            Err(_) => triangles.retain(|&t| t != triangle),
        }
    }
    Err(impossible("no type triangle has valid starters"))
}

/// A random allowed (or, banning bad items, non-bad) item for each held-item
/// slot.
fn held_items(game: &GameData, ban_bad: bool, rng: &mut StepRng) -> Result<Vec<Option<ItemId>>, RandomizeError> {
    let possible = if ban_bad {
        game.non_bad_items()
    } else {
        game.allowed_items()
    };
    if possible.is_empty() && !game.starter_held_items.is_empty() {
        return Err(impossible("no items to hold"));
    }
    Ok(game
        .starter_held_items
        .iter()
        .map(|_| Some(possible[below(rng, possible.len())]))
        .collect())
}

/// The species a starter may be: with "Allow Alternate Formes", where starters can be alternate formes, also the
/// alternate formes that are not only cosmetic, but `banned`.
fn available(game: &GameData, s: &StarterSettings, banned: &SpeciesSet, already: &[DexNumber]) -> SpeciesSet {
    let basic_only = matches!(s.mode, StarterMode::RandomBasic | StarterMode::RandomWithTwoEvolutions);
    let bst_max = if s.bst_max == 0 { MAX_BST } else { s.bst_max };
    let alt_formes = s.allow_alt_formes && game.formes.starters;
    let mut pool = pool_with_formes(game, alt_formes);
    if alt_formes {
        pool.retain(|n| !banned.contains(n));
    }
    game.species
        .iter()
        .filter(|sp| pool.contains(&sp.number))
        .filter(|sp| !(s.no_legendaries && sp.is_legendary()))
        .filter(|sp| !already.contains(&sp.number))
        .filter(|sp| !(s.no_dual_types && sp.secondary_type.is_some()))
        .filter(|sp| !basic_only || game.pre_evolved_species(sp.number).is_empty())
        .filter(|sp| s.mode != StarterMode::RandomWithTwoEvolutions || game.stages_after(sp.number) >= 2)
        .filter(|sp| (s.bst_min == 0 && bst_max == MAX_BST) || (sp.bst() >= s.bst_min && sp.bst() <= bst_max))
        .map(|sp| sp.number)
        .collect()
}

fn choose_basic(
    rng: &mut StepRng,
    needed: usize,
    available: &mut SpeciesSet,
) -> Result<Vec<DexNumber>, RandomizeError> {
    if available.len() < needed {
        return Err(impossible("not enough starters to choose from"));
    }
    Ok((0..needed).filter_map(|_| take(rng, available)).collect())
}

fn types_of(game: &GameData, n: DexNumber) -> [Option<Type>; 2] {
    game.species(n)
        .map_or([None, None], |s| [Some(s.primary_type), s.secondary_type])
}

fn shares_type(game: &GameData, a: DexNumber, b: DexNumber) -> bool {
    let tb = types_of(game, b);
    types_of(game, a).iter().flatten().any(|t| tb.contains(&Some(*t)))
}

fn choose_unique_type(
    game: &GameData,
    rng: &mut StepRng,
    already: &[DexNumber],
    needed: usize,
    available: &SpeciesSet,
) -> Result<Vec<DexNumber>, RandomizeError> {
    let mut pool: SpeciesSet = available
        .iter()
        .copied()
        .filter(|&n| !already.iter().any(|&p| shares_type(game, n, p)))
        .collect();
    let mut picks = Vec::with_capacity(needed);
    while picks.len() < needed {
        let chosen = take(rng, &mut pool).ok_or_else(|| impossible("no starters with unique types are left"))?;
        pool.retain(|&n| !shares_type(game, n, chosen));
        picks.push(chosen);
    }
    shuffle(rng, &mut picks);
    Ok(picks)
}

fn by_type(game: &GameData, set: &SpeciesSet) -> BTreeMap<Type, SpeciesSet> {
    let mut map: BTreeMap<Type, SpeciesSet> = BTreeMap::new();
    for &n in set {
        for t in types_of(game, n).into_iter().flatten() {
            map.entry(t).or_default().insert(n);
        }
    }
    map
}

/// Per type, a species whose other type is not one of `types`.
fn choose_of_types(
    game: &GameData,
    rng: &mut StepRng,
    by_type: &BTreeMap<Type, SpeciesSet>,
    types: &[Type],
) -> Result<Vec<DexNumber>, RandomizeError> {
    let mut chosen = Vec::with_capacity(types.len());
    for &t in types {
        let mut pool = by_type
            .get(&t)
            .cloned()
            .ok_or_else(|| impossible(format!("no valid starters of type {t:?}")))?;
        let found = std::iter::from_fn(|| take(rng, &mut pool)).find(|&n| {
            let [primary, secondary] = types_of(game, n);
            let other = if primary == Some(t) { secondary } else { primary };
            other.is_none_or(|o| !types.contains(&o))
        });
        chosen.push(found.ok_or_else(|| impossible(format!("no valid starter of type {t:?}")))?);
    }
    Ok(chosen)
}

/// The first type, in random order, with more than `needed` species.
fn choose_type(
    game: &GameData,
    rng: &mut StepRng,
    needed: usize,
    by_type: &BTreeMap<Type, SpeciesSet>,
) -> Result<Type, RandomizeError> {
    let mut types = game.types.clone();
    shuffle(rng, &mut types);
    types
        .into_iter()
        .find(|t| by_type.get(t).is_some_and(|s| s.len() > needed))
        .ok_or_else(|| impossible(format!("no type has more than {needed} possible starters")))
}

#[cfg(test)]
mod tests;
