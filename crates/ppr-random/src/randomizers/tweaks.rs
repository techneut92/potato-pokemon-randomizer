//! Misc. tweaks: applies the chosen misc tweaks the game supports, the ones with priority first. Tweaks that
//! change game data are done here; the handler patches the rest into the ROM ([`GameData::applied_tweaks`]).
//!
//! Deliberate choice: each side of the catching tutorial is drawn once from the species it can take, rather than
//! drawing pairs until the ROM takes one (and giving up after so many tries); the same distribution.

use ppr_model::{CatchingTutorial, GameData, LUCKY_EGG, MiscTweak, Type};

use crate::choice::below;
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Misc. Tweaks";

/// Rotom, and the second types of its formes from Gen 5 on: heat, wash, frost, fan and mow.
const ROTOM: u16 = 479;
const ROTOM_FORME_TYPES: [(u8, Type); 5] = [
    (1, Type::Fire),
    (2, Type::Water),
    (3, Type::Ice),
    (4, Type::Flying),
    (5, Type::Grass),
];
/// Horn Drill, Fissure and Guillotine.
const ONE_HIT_KO_MOVES: [u16; 3] = [32, 90, 12];
/// The level for fossils with "Balance Static Pokémon Levels".
const FOSSIL_LEVEL: u8 = 30;
/// The items only the Big Maniac and the like buy dearly, with Gen 5 numbers.
const BIG_MANIAC_ITEMS: [u16; 15] = [
    580, 581, 582, 583, 587, 588, 589, 590, 206, 207, 208, 209, 210, 211, 212,
];

/// Applies [`ppr_settings::Settings::misc_tweaks`].
#[derive(Debug, Clone, Copy, Default)]
pub struct MiscTweaks;

impl Step<RunData> for MiscTweaks {
    fn id(&self) -> u64 {
        step_ids::MISC_TWEAKS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        !to_apply(data).is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        for tweak in to_apply(data) {
            apply(&mut data.game, tweak, &mut rng)?;
        }
        Ok(())
    }
}

/// The chosen tweaks the game supports, in order: by priority, then in [`MiscTweak`]'s order.
fn to_apply(data: &RunData) -> Vec<MiscTweak> {
    let mut tweaks: Vec<MiscTweak> = data
        .game
        .available_tweaks
        .iter()
        .copied()
        .filter(|t| data.settings.misc_tweaks.contains(t))
        .collect();
    tweaks.sort_by_key(|&t| (!t.has_priority(), t));
    tweaks.dedup();
    tweaks
}

fn impossible(message: &str) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

fn apply(game: &mut GameData, tweak: MiscTweak, rng: &mut StepRng) -> Result<(), RandomizeError> {
    match tweak {
        MiscTweak::BanLuckyEgg => {
            for item in game.items.iter_mut().filter(|i| i.id == LUCKY_EGG) {
                item.allowed = false;
            }
        }
        MiscTweak::RandomizePcPotion => {
            // The PC potion becomes a non-bad item that is no TM.
            let possible: Vec<_> = game
                .non_bad_items()
                .into_iter()
                .filter(|&id| !game.items.iter().any(|i| i.id == id && i.tm))
                .collect();
            if possible.is_empty() {
                return Err(impossible("no item for the PC"));
            }
            game.pc_potion = Some(possible[below(rng, possible.len())]);
        }
        MiscTweak::RandomizeCatchingTutorial => {
            let pool = game.species_pool();
            let draw = |list: &[u16], rng: &mut StepRng| {
                let fits: Vec<u16> = list.iter().copied().filter(|n| pool.contains(n)).collect();
                (!fits.is_empty()).then(|| fits[below(rng, fits.len())])
            };
            let opponent = draw(&game.catching_tutorial_opponents, rng);
            let player = draw(&game.catching_tutorial_players, rng);
            let (Some(opponent), Some(player)) = (opponent, player) else {
                return Err(impossible("no species for the catching tutorial"));
            };
            game.catching_tutorial = Some(CatchingTutorial { opponent, player });
        }
        MiscTweak::LowerCaseNames => {
            for sp in &mut game.species {
                sp.name = camel_case(&sp.name);
            }
        }
        MiscTweak::BalanceStaticLevels => {
            for &i in &game.fossils {
                if let Some(s) = game.statics.get_mut(i) {
                    s.level = FOSSIL_LEVEL;
                }
            }
            // Games whose fossil levels are not static levels (Black and White) write them for the tweak.
            game.applied_tweaks.insert(tweak);
        }
        MiscTweak::BanBigManiacItems => {
            // Balm Mushroom, Big Nugget, Pearl String and Comet Shard; four relics; the rare berries.
            for item in game.items.iter_mut().filter(|i| BIG_MANIAC_ITEMS.contains(&i.id)) {
                item.allowed = false;
            }
        }
        MiscTweak::NerfXAccuracy => {
            // In Gen 1, once X Accuracy no longer makes them always hit, the one-hit
            // KO moves are fine.
            if game.identity.generation.number() == 1 {
                game.game_breaking_moves.retain(|m| !ONE_HIT_KO_MOVES.contains(m));
            }
            game.applied_tweaks.insert(tweak);
        }
        MiscTweak::UpdateRotomFormeTyping => {
            // Rotom's appliance formes get the second types they have from Gen 5 on.
            for (forme, t) in ROTOM_FORME_TYPES {
                let n = game.forme_species(ROTOM, forme);
                if let Some(sp) = game.species.iter_mut().find(|sp| sp.number == n && n != ROTOM) {
                    sp.secondary_type = Some(t);
                }
            }
        }
        MiscTweak::ReusableTms => {
            // With reusable TMs, Pickup finds no TMs, so it doesn't find the same one again and again.
            game.pickup_tms = false;
            game.applied_tweaks.insert(tweak);
        }
        _ => {
            game.applied_tweaks.insert(tweak);
        }
    }
    Ok(())
}

/// Title case: lower case, with a capital at the first letter of each word; apostrophes don't
/// end a word.
fn camel_case(name: &str) -> String {
    let mut capital = true;
    name.to_lowercase()
        .chars()
        .map(|c| {
            if capital && c.is_alphabetic() {
                capital = false;
                c.to_uppercase().next().unwrap_or(c)
            } else {
                if !capital && !c.is_alphabetic() && c != '\'' && c != '’' {
                    capital = true;
                }
                c
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
