//! Pickup items: every item Pickup finds becomes a random one, with the chance of the item it replaces.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).

use ppr_model::ItemId;

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Pickup Items";

/// Randomizes Pickup items according to [`ppr_settings::PickupItemSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct PickupItemRandomizer;

impl Step<RunData> for PickupItemRandomizer {
    fn id(&self) -> u64 {
        step_ids::PICKUP
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        data.settings.pickup.random && !data.game.pickup_items.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let game = &data.game;
        let mut possible: Vec<ItemId> = if data.settings.pickup.ban_bad {
            game.non_bad_items()
        } else {
            game.allowed_items()
        };
        if !game.pickup_tms {
            possible.retain(|&id| !game.items.iter().any(|i| i.id == id && i.tm));
        }
        if possible.is_empty() {
            return Err(RandomizeError::Impossible {
                step: LABEL,
                message: "no items to choose from".into(),
            });
        }
        let new = game
            .pickup_items
            .iter()
            .map(|_| possible[below(&mut rng, possible.len())])
            .collect();
        data.game.pickup_items = new;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::Item;
    use ppr_settings::{PickupItemSettings, Settings};

    use super::*;
    use crate::randomizers::testing::emerald;
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn run(pickup_tms: bool, ban_bad: bool, seed: u64) -> Vec<ItemId> {
        let mut g = emerald(Vec::new());
        g.items = (1..=10)
            .map(|id| Item {
                id,
                name: format!("I{id}"),
                allowed: id != 10,
                bad: id == 9,
                tm: id == 8,
            })
            .collect();
        g.pickup_items = vec![1; 40];
        g.pickup_tms = pickup_tms;
        let mut data = RunData {
            game: g,
            settings: Settings {
                pickup: PickupItemSettings { random: true, ban_bad },
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game.pickup_items
    }

    #[test]
    fn pickup_finds_allowed_items_and_tms_only_where_they_are_used_up() {
        let items = run(true, true, 1);
        assert_eq!(items.len(), 40);
        assert!(items.iter().all(|&i| i != 9 && i != 10));
        assert!(items.iter().any(|&i| i != 1));
        let no_tms = run(false, false, 2);
        assert!(no_tms.iter().all(|&i| i != 8 && i != 10));
        assert!(no_tms.contains(&9), "bad items are allowed without the ban");
    }
}
