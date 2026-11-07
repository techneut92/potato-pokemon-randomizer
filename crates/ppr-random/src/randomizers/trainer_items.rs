//! Trainer held items: trainer Pokémon hold random items, from any with an effect in battle, from those used up
//! in battle, or from those that suit the Pokémon and its moves ([`sensible`]).
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - Z-Crystals and teams that need unique items come with the generations that have them.
//!
//! While trainers' Mega Evolvables are swapped, Pokémon that held a Mega Stone as loaded keep what they hold, as in
//! the original: the swap chose their stone.

use ppr_model::{GameData, HeldItemPools, ItemId, MoveId, TrainerPokemon};
use ppr_settings::TrainerSettings;

use super::trainers::{held_mega_stone, mega_stones};
use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

/// Gives trainer Pokémon held items by [`TrainerSettings::held_items`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerHeldItemRandomizer;

impl Step<RunData> for TrainerHeldItemRandomizer {
    fn id(&self) -> u64 {
        step_ids::TRAINER_HELD_ITEMS
    }

    fn label(&self) -> &'static str {
        "Trainer Held Items"
    }

    fn applies(&self, data: &RunData) -> bool {
        (0..3).any(|i| data.settings.trainers.held_items[i] && data.game.trainer_held_item_additions[i])
            && !data.game.trainers.is_empty()
            && !data.game.held_item_pools.all.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        let s = data.settings.trainers.clone();
        randomize(&mut data.game, &s, &mut rng);
        Ok(())
    }
}

/// Gives held items to the Pokémon of the boss, important and regular trainers the settings choose: all of them,
/// or only the first of the highest level.
fn randomize(game: &mut GameData, s: &TrainerSettings, rng: &mut StepRng) {
    // Only the tiers the game has room to give items to.
    let [boss, important, regular] = std::array::from_fn(|i| s.held_items[i] && game.trainer_held_item_additions[i]);
    // While Mega Evolvables are swapped, those that held a Mega Stone keep what the swap gave them.
    let stones = mega_stones(game);
    let keeps_item = |tp: &TrainerPokemon| s.swap_mega_evos && held_mega_stone(&stones, tp);
    let mut trainers = std::mem::take(&mut game.trainers);
    for t in &mut trainers {
        if t.should_not_get_buffs()
            || (t.is_boss() && !boss)
            || (t.is_important() && !important)
            || (t.is_regular() && !regular)
        {
            continue;
        }
        if s.highest_level_gets_item {
            // The first of the highest level.
            let highest = t.pokemon.iter().map(|p| p.level).max();
            if let Some(tp) = t.pokemon.iter_mut().find(|p| Some(p.level) == highest)
                && !keeps_item(tp)
            {
                give_item(game, s, tp, rng);
            }
        } else {
            for tp in t.pokemon.iter_mut().filter(|tp| !keeps_item(tp)) {
                give_item(game, s, tp, rng);
            }
        }
    }
    game.trainers = trainers;
}

/// Gives `tp` a random item: a sensible one, a consumable one or any, as the settings ask.
fn give_item(game: &GameData, s: &TrainerSettings, tp: &mut TrainerPokemon, rng: &mut StepRng) {
    let pools = &game.held_item_pools;
    let choices = if s.sensible_items_only {
        let moves = if tp.reset_moves {
            game.moves_at_level(tp.species, tp.level)
        } else {
            tp.moves
        };
        sensible(game, tp, moves, s.consumable_items_only)
    } else if s.consumable_items_only {
        pools.consumable.clone()
    } else {
        pools.all.clone()
    };
    if !choices.is_empty() {
        tp.held_item = Some(choices[below(rng, choices.len())]);
    }
}

/// The items that suit `tp`: general items, items for the Pokémon's physical and special attacks and
/// their types, and items for its species (six times, to make them likelier). An item may be listed more than
/// once, each time making it likelier.
fn sensible(game: &GameData, tp: &TrainerPokemon, moves: [MoveId; 4], consumable_only: bool) -> Vec<ItemId> {
    let pools: &HeldItemPools = &game.held_item_pools;
    let mut ids = pools.general_consumable.clone();
    if !consumable_only {
        ids.extend(&pools.general);
    }
    for m in moves {
        let Some(mv) = usize::from(m).checked_sub(1).and_then(|i| game.moves.get(i)) else {
            continue;
        };
        if mv.power == 0 {
            continue;
        }
        let physical = game.is_physical(mv);
        ids.extend(if physical {
            &pools.physical_consumable
        } else {
            &pools.special_consumable
        });
        if !consumable_only {
            if let Some(boosting) = mv.move_type.and_then(|t| pools.type_boosting.get(&t)) {
                ids.extend(boosting);
            }
            if physical {
                ids.extend(&pools.physical);
            }
        }
    }
    if !consumable_only && let Some(species_items) = pools.species_boosting.get(&tp.species) {
        for _ in 0..6 {
            ids.extend(species_items);
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use ppr_model::{LevelUpMove, Move, MoveCategory, Trainer, Type};
    use ppr_settings::Settings;

    use super::*;
    use crate::randomizers::testing::{emerald, species};
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    const TACKLE: MoveId = 1;
    const EMBER: MoveId = 2;
    const GROWL: MoveId = 3;

    fn pools() -> HeldItemPools {
        HeldItemPools {
            all: vec![1, 2, 3, 4, 10, 11, 20, 21, 30, 31, 40],
            consumable: vec![1, 2, 3, 4],
            general_consumable: vec![1, 2],
            general: vec![10, 11],
            physical_consumable: vec![3],
            special_consumable: vec![4],
            physical: vec![20],
            type_boosting: [(Type::Normal, vec![21]), (Type::Fire, vec![30])].into(),
            species_boosting: [(2, vec![40])].into(),
        }
    }

    fn game() -> GameData {
        let mut g = emerald((1..=3).map(|n| species(n, Type::Normal, None, 50)).collect());
        g.moves = vec![
            Move::basic(TACKLE, "TACKLE", 35, Type::Normal, MoveCategory::Physical),
            // Gen 3 makes Fire special, whatever the category says.
            Move::basic(EMBER, "EMBER", 40, Type::Fire, MoveCategory::Physical),
            Move::basic(GROWL, "GROWL", 0, Type::Normal, MoveCategory::Status),
        ];
        g.learnsets
            .insert(1, vec![LevelUpMove::new(1, TACKLE), LevelUpMove::new(5, GROWL)]);
        g.held_item_pools = pools();
        let team = |level: u8| vec![TrainerPokemon::new(1, level), TrainerPokemon::new(1, level + 2)];
        g.trainers = vec![
            Trainer::new(1, "Leader", 0, team(20)),
            Trainer::new(2, "Rival", 0, team(10)),
            Trainer::new(3, "Youngster", 0, team(5)),
            Trainer::new(4, "First rival", 0, team(5)),
        ];
        g.trainers[0].tag = Some("GYM1-LEADER".into());
        g.trainers[1].tag = Some("RIVAL2-0".into());
        g.trainers[3].tag = Some("RIVAL1-0".into());
        g
    }

    fn run(game: GameData, trainers: TrainerSettings, seed: u64) -> GameData {
        let mut data = RunData {
            game,
            settings: Settings {
                trainers,
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game
    }

    fn items(g: &GameData, trainer: usize) -> Vec<Option<ItemId>> {
        g.trainers[trainer].pokemon.iter().map(|p| p.held_item).collect()
    }

    #[test]
    fn tiers_choose_who_gets_items() {
        let g = run(
            game(),
            TrainerSettings {
                held_items: [true, false, true],
                ..TrainerSettings::default()
            },
            1,
        );
        assert!(items(&g, 0).iter().all(Option::is_some));
        assert_eq!(items(&g, 1), [None, None]);
        assert!(items(&g, 2).iter().all(Option::is_some));
        // The first rival battle gets no buffs.
        assert_eq!(items(&g, 3), [None, None]);
        let all = pools().all;
        assert!(
            g.trainers
                .iter()
                .flat_map(|t| &t.pokemon)
                .filter_map(|p| p.held_item)
                .all(|i| all.contains(&i))
        );
    }

    #[test]
    fn tiers_without_room_for_items_get_none() {
        let mut g = game();
        g.trainer_held_item_additions = [true, true, false];
        let g = run(
            g,
            TrainerSettings {
                held_items: [true, false, true],
                ..TrainerSettings::default()
            },
            1,
        );
        assert!(items(&g, 0).iter().all(Option::is_some));
        assert_eq!(items(&g, 2), [None, None], "regular trainers have no room for items");
    }

    #[test]
    fn highest_level_only_and_consumables() {
        let g = run(
            game(),
            TrainerSettings {
                held_items: [true; 3],
                consumable_items_only: true,
                highest_level_gets_item: true,
                ..TrainerSettings::default()
            },
            2,
        );
        for t in [0, 1, 2] {
            let held = items(&g, t);
            assert_eq!(held[0], None);
            assert!(held[1].is_some_and(|i| pools().consumable.contains(&i)), "{held:?}");
        }
        assert!(
            run(game(), TrainerSettings::default(), 2)
                .trainers
                .iter()
                .all(|t| !t.pokemon_have_items())
        );
    }

    #[test]
    fn sensible_items_suit_the_moves_and_species() {
        let g = game();
        let mut tp = TrainerPokemon::new(1, 10);
        tp.moves = [TACKLE, EMBER, GROWL, 0];
        assert_eq!(sensible(&g, &tp, tp.moves, false), [1, 2, 10, 11, 3, 21, 20, 4, 30]);
        assert_eq!(sensible(&g, &tp, tp.moves, true), [1, 2, 3, 4]);
        tp.species = 2;
        let with_species = sensible(&g, &tp, [0; 4], false);
        assert_eq!(with_species.iter().filter(|&&i| i == 40).count(), 6);
        assert_eq!(sensible(&g, &tp, [0; 4], true), [1, 2]);
    }

    #[test]
    fn sensible_items_use_level_up_moves_when_reset() {
        let mut g = game();
        for t in &mut g.trainers {
            for tp in &mut t.pokemon {
                tp.reset_moves = true;
            }
        }
        let g = run(
            g,
            TrainerSettings {
                held_items: [true; 3],
                sensible_items_only: true,
                ..TrainerSettings::default()
            },
            3,
        );
        // Tackle and Growl: no special or Fire items.
        let held: Vec<ItemId> = g
            .trainers
            .iter()
            .flat_map(|t| &t.pokemon)
            .filter_map(|p| p.held_item)
            .collect();
        assert_eq!(held.len(), 6);
        assert!(held.iter().all(|i| [1, 2, 10, 11, 3, 21, 20].contains(i)), "{held:?}");
    }
}
