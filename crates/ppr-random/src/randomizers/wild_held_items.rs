//! The items wild Pokémon hold. Species that hold nothing keep holding nothing; the others get new items, in a new
//! arrangement of guaranteed, common and rare.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).

use ppr_model::{ItemId, WildHeldItems};
use rand_core::Rng;

use crate::choice::{below, unit};
use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Wild Held Items";

/// Randomizes wild held items according to [`ppr_settings::WildSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct WildHeldItemRandomizer;

impl Step<RunData> for WildHeldItemRandomizer {
    fn id(&self) -> u64 {
        step_ids::WILD_HELD_ITEMS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.wild.random_held_items && !data.game.wild_held_items.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let game = &mut data.game;
        let possible: Vec<ItemId> = if data.settings.wild.ban_bad_held_items {
            game.non_bad_items()
        } else {
            game.allowed_items()
        };
        if possible.is_empty() {
            return Err(RandomizeError::Impossible {
                step: LABEL,
                message: "no items to choose from".into(),
            });
        }
        let guaranteed_allowed = game.guaranteed_wild_held_items;
        let dark_grass = game.dark_grass_held_items;
        for held in game.wild_held_items.values_mut() {
            if held.is_empty() {
                continue;
            }
            *held = new_items(&mut rng, &possible, *held, guaranteed_allowed);
            if dark_grass && held.guaranteed.is_none() {
                held.dark_grass = (unit(&mut rng) < 0.5).then(|| pick(&mut rng, &possible));
            }
        }
        Ok(())
    }
}

fn pick(rng: &mut impl Rng, possible: &[ItemId]) -> ItemId {
    possible[below(rng, possible.len())]
}

/// The new items of a species that holds something: mostly a new guaranteed item for one that had one; for the
/// others no item at all half the time, else a rare or a common one, both (different, unless only one item is
/// possible), or seldom a guaranteed one. Only "no item" and a guaranteed item drop the dark grass item.
fn new_items<R: Rng>(rng: &mut R, possible: &[ItemId], old: WildHeldItems, guaranteed_allowed: bool) -> WildHeldItems {
    let decision = unit(rng);
    let kept = WildHeldItems {
        dark_grass: old.dark_grass,
        ..WildHeldItems::default()
    };
    let guaranteed = |rng: &mut R| WildHeldItems {
        guaranteed: Some(pick(rng, possible)),
        ..WildHeldItems::default()
    };
    let both = |rng: &mut R| {
        let common = pick(rng, possible);
        let mut rare = pick(rng, possible);
        while rare == common && possible.len() > 1 {
            rare = pick(rng, possible);
        }
        WildHeldItems {
            common: Some(common),
            rare: Some(rare),
            ..kept
        }
    };
    if old.guaranteed.is_some() {
        if decision < 0.9 { guaranteed(rng) } else { both(rng) }
    } else if decision < 0.5 {
        WildHeldItems::default()
    } else if decision < 0.65 {
        WildHeldItems {
            rare: Some(pick(rng, possible)),
            ..kept
        }
    } else if decision < 0.8 {
        WildHeldItems {
            common: Some(pick(rng, possible)),
            ..kept
        }
    } else if decision < 0.95 || !guaranteed_allowed {
        both(rng)
    } else {
        guaranteed(rng)
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{Item, Type};
    use ppr_settings::{Settings, WildSettings};

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn held(guaranteed: Option<ItemId>, common: Option<ItemId>, rare: Option<ItemId>) -> WildHeldItems {
        WildHeldItems {
            guaranteed,
            common,
            rare,
            dark_grass: None,
        }
    }

    /// 200 species: odd ones hold item 1 for sure, even ones nothing; species 2 holds item 1 or 2.
    fn run(ban_bad: bool, seed: u64) -> ppr_model::GameData {
        let mut g = emerald((1..=200).map(|n| species(n, Type::Normal, None, 50)).collect());
        g.items = (1..=10)
            .map(|id| Item {
                id,
                name: format!("I{id}"),
                allowed: id != 10,
                bad: id == 9,
                tm: false,
            })
            .collect();
        g.wild_held_items = (1..=200)
            .map(|n| {
                (
                    n,
                    if n % 2 == 1 {
                        held(Some(1), None, None)
                    } else {
                        held(None, None, None)
                    },
                )
            })
            .collect();
        g.wild_held_items.insert(2, held(None, Some(1), Some(2)));
        let mut data = RunData {
            game: g,
            settings: Settings {
                wild: WildSettings {
                    random_held_items: true,
                    ban_bad_held_items: ban_bad,
                    ..WildSettings::default()
                },
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game
    }

    #[test]
    fn species_holding_something_get_new_items_and_others_nothing() {
        let g = run(true, 3);
        let mut guaranteed = 0;
        for (&n, h) in &g.wild_held_items {
            if n % 2 == 0 && n != 2 {
                assert!(h.is_empty(), "#{n} held nothing: {h:?}");
                continue;
            }
            for item in [h.guaranteed, h.common, h.rare].into_iter().flatten() {
                assert!(item != 9 && item != 10, "#{n}: {h:?}");
            }
            assert!(h.guaranteed.is_none() || (h.common.is_none() && h.rare.is_none()));
            assert!(h.common.is_none() || h.common != h.rare, "#{n}: {h:?}");
            guaranteed += usize::from(h.guaranteed.is_some());
        }
        // Nine in ten of the 100 guaranteed holders keep a guaranteed item.
        assert!((75..=100).contains(&guaranteed), "{guaranteed}");
        assert!(g.wild_held_items.values().any(|h| h.guaranteed.is_some_and(|i| i != 1)));
    }

    #[test]
    fn bad_items_may_appear_without_the_ban() {
        let seen: Vec<ItemId> = (0..5)
            .flat_map(|seed| {
                run(false, seed)
                    .wild_held_items
                    .into_values()
                    .flat_map(|h| [h.guaranteed, h.common, h.rare])
                    .flatten()
                    .collect::<Vec<_>>()
            })
            .collect();
        assert!(seen.contains(&9));
        assert!(!seen.contains(&10));
    }

    #[test]
    fn without_guaranteed_items_species_get_common_and_rare_ones() {
        let mut rng = <rand_chacha::ChaCha8Rng as rand_core::SeedableRng>::seed_from_u64(7);
        for _ in 0..200 {
            let h = new_items(&mut rng, &[1, 2, 3], held(None, Some(1), None), false);
            assert!(h.guaranteed.is_none());
        }
        // One possible item: common and rare may be the same rather than loop forever.
        let h = new_items(&mut rng, &[4], held(Some(1), None, None), true);
        assert!(h.guaranteed == Some(4) || (h.common == Some(4) && h.rare == Some(4)));
    }
}
