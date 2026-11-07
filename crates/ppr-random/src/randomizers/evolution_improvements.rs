//! Evolution improvements: impossible evolutions changed, evolutions made easier (levels condensed, Gen 3's
//! happiness patch) and time-based evolutions removed.
//!
//! Deliberate choices:
//! - The Gen 3 handler patches `FireRed` and `LeafGreen` to evolve species outside the Kanto Pokédex whenever
//!   evolutions change, not only when impossible evolutions are changed.
//! - Gen 1 has only plain trade evolutions to change, and no happiness evolutions to make easier.
//! - Gen 2 follows the Gen 3 rules: trade evolutions by level or stone, and the happiness patch for easier evolutions.
//! - Gen 7 has no "no evolution" entries (Sun and Moon's Alolan Raichu, Exeggutor and Marowak) and no alternate
//!   formes in the game data, so those evolutions and Rockruff's Own Tempo forme are left as they are.
//! - Gen 6 changes the same impossible evolutions as Gen 5 (Pancham, Sliggoo and Sylveon stay, as in the
//!   original); easier evolutions also turn Inkay's upside-down evolution into one at its level.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, Evolution, EvolutionType as E, GameData, ItemId};
use ppr_settings::{EvolutionSettings, MovesetMode, WildMode};

use crate::{RandomizeError, RunData, Step, StepContext, step_ids};

const LABEL: &str = "Evolution Improvements";

/// Item numbers used here.
const SUN_STONE: ItemId = 80;
const MOON_STONE: ItemId = 81;
const WATER_STONE: ItemId = 84;
const DUSK_STONE: ItemId = 108;
const DEEP_SEA_SCALE: ItemId = 227;
const LEAF_STONE: ItemId = 85;
const DAWN_STONE: ItemId = 109;

/// Species numbers used here.
const POLIWHIRL: DexNumber = 61;
const SLOWPOKE: DexNumber = 79;
const SEADRA: DexNumber = 117;
const CLAMPERL: DexNumber = 366;
const KARRABLAST: DexNumber = 588;
const SHELMET: DexNumber = 616;

/// Applies the evolution improvements of [`EvolutionSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct EvolutionImprover;

impl Step<RunData> for EvolutionImprover {
    fn id(&self) -> u64 {
        step_ids::EVOLUTION_IMPROVEMENTS
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.evolutions;
        s.change_impossible || s.easier_max_level.is_some() || s.remove_time_based
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let settings = &ctx.data.settings;
        let s = settings.evolutions.clone();
        let others = Others {
            // Random movesets may no longer teach the move an evolution needs, and random wild Pokémon may no
            // longer include the species another evolution needs in the party.
            move_evolutions: settings.movesets.mode != MovesetMode::Unchanged,
            party_evolutions: settings.wild.mode != WildMode::Unchanged,
        };
        improve(&mut ctx.data.game, &s, others);
        Ok(())
    }
}

/// Which evolutions that other randomizers make impossible are to change too.
#[derive(Debug, Clone, Copy, Default)]
struct Others {
    /// Evolutions that need a move.
    move_evolutions: bool,
    /// Evolutions that need another species in the party.
    party_evolutions: bool,
}

/// Applies the chosen evolution improvements.
fn improve(game: &mut GameData, s: &EvolutionSettings, others: Others) {
    let generation = game.identity.generation.number();
    // The highest evolution level and the evolutions before any improvement: as loaded, as no earlier step
    // changes evolutions yet.
    let highest = game.highest_evolution_level();
    let before = game.evolutions.clone();
    if s.change_impossible {
        match generation {
            4 => remove_impossible_gen4(game, s.use_estimated_levels, others.move_evolutions),
            // Gen 6 changes the same evolutions as Gen 5.
            5 | 6 => remove_impossible_gen5(game, s.use_estimated_levels, others.move_evolutions),
            7 => {
                // The trade and move evolutions change as in Gen 5.
                remove_impossible_gen5(game, s.use_estimated_levels, others.move_evolutions);
                remove_game_specific(game, s.use_estimated_levels);
            }
            _ => remove_impossible(game, s.use_estimated_levels),
        }
    }
    if let Some(max) = s.easier_max_level {
        condense_level_evolutions(game, u16::from(max), highest);
        // With random wild Pokémon, evolutions next to another species happen by level.
        if (4..=7).contains(&generation) && others.party_evolutions {
            for e in game.evolutions.iter_mut().filter(|e| e.kind == E::WithOther) {
                e.update_method_estimated(E::Level, 35, s.use_estimated_levels);
            }
        }
        // Inkay evolves at its level without the 3DS held upside down.
        if generation == 6 {
            for e in game.evolutions.iter_mut().filter(|e| e.kind == E::LevelUpsideDown) {
                let level = e.parameter;
                e.update_method(E::Level, level);
            }
        } else if generation == 7 {
            easier_gen7(game, s.use_estimated_levels);
        }
        // Easier evolutions in Gen 3: only the happiness patch. Gen 1 has no happiness evolutions.
        if generation != 1 {
            game.easier_happiness_evolutions = true;
        }
    }
    if s.remove_time_based {
        remove_time_based(game, &before);
    }
}

/// True for `FireRed` and `LeafGreen`, which have no clock and no contests.
fn is_fire_red_leaf_green(game: &GameData) -> bool {
    game.identity.code.starts_with("BPR") || game.identity.code.starts_with("BPG")
}

/// Gen 3's impossible evolutions changed: trade evolutions, and in `FireRed` and `LeafGreen` the ones needing a
/// clock or contests, happen by level or stone.
fn remove_impossible(game: &mut GameData, estimated: bool) {
    let frlg = is_fire_red_leaf_green(game);
    for e in &mut game.evolutions {
        if frlg {
            match e.kind {
                E::HappinessDay => e.update_method_estimated(E::Stone, SUN_STONE, estimated),
                E::HappinessNight => e.update_method_estimated(E::Stone, MOON_STONE, estimated),
                E::HighBeauty => e.update_method_estimated(E::Level, 35, estimated),
                _ => {}
            }
        }
        match e.kind {
            // Haunter, Machoke, Kadabra, Graveler.
            E::Trade => e.update_method_estimated(E::Level, 37, estimated),
            E::TradeItem => {
                let (kind, parameter) = match (e.from, e.parameter) {
                    (POLIWHIRL, _) => (E::Level, 37),
                    (SLOWPOKE, _) | (CLAMPERL, DEEP_SEA_SCALE) => (E::Stone, WATER_STONE),
                    (SEADRA, _) => (E::Level, 40),
                    // Clamperl with a Deep Sea Tooth; Onix, Scyther, Porygon.
                    _ => (E::Level, 30),
                };
                e.update_method_estimated(kind, parameter, estimated);
            }
            _ => {}
        }
    }
}

/// True for `HeartGold` and `SoulSilver`, which have no contests and none of the places some evolutions need.
fn is_heart_gold_soul_silver(game: &GameData) -> bool {
    game.identity.family.eq_ignore_ascii_case("hgss")
}

/// Gen 4's impossible evolutions changed: trade evolutions happen by level, or by level holding the item; in
/// `HeartGold` and `SoulSilver` the ones needing beauty or a place by level or stone; with random movesets the ones
/// needing a move at the level the species learns it (45 if it doesn't).
fn remove_impossible_gen4(game: &mut GameData, estimated: bool, move_evolutions: bool) {
    let hgss = is_heart_gold_soul_silver(game);
    for i in 0..game.evolutions.len() {
        let e = game.evolutions[i];
        let (kind, parameter, use_estimated) = match e.kind {
            E::HighBeauty if hgss => (E::Level, 35, estimated),
            // Magnezone and Probopass.
            E::MagneticField if hgss => (E::Level, 40, estimated),
            // Leafeon and Glaceon.
            E::MossRock if hgss => (E::Stone, LEAF_STONE, estimated),
            E::IceRock if hgss => (E::Stone, DAWN_STONE, estimated),
            E::WithMove if move_evolutions => {
                let learnt = game
                    .learnsets
                    .get(&e.from)
                    .and_then(|moves| moves.iter().find(|m| m.move_id == e.parameter))
                    .map_or(1, |m| u16::from(m.level));
                match learnt {
                    // Piloswine learns Ancient Power from no level.
                    1 => (E::Level, 45, estimated),
                    level => (E::Level, level, false),
                }
            }
            E::Trade => (E::Level, 37, estimated),
            // Slowpoke already evolves by level, so it can't by level holding an item.
            E::TradeItem if e.from == SLOWPOKE => (E::Stone, WATER_STONE, estimated),
            E::TradeItem => (E::Item, e.parameter, estimated),
            _ => continue,
        };
        game.evolutions[i].update_method_estimated(kind, parameter, use_estimated);
    }
}

/// Gen 5's impossible evolutions changed: trade evolutions happen by level, by holding their item (Slowpoke by
/// Water Stone, as it already evolves by level), Karrablast and Shelmet next to each other, and, with
/// `move_evolutions`, evolutions by a known move at the level the move is learnt (45 when at level 1).
fn remove_impossible_gen5(game: &mut GameData, estimated: bool, move_evolutions: bool) {
    let learnt_at = |game: &GameData, species: DexNumber, move_id: u16| {
        game.learnsets
            .get(&species)
            .and_then(|moves| moves.iter().find(|m| m.move_id == move_id))
            .map_or(1, |m| u16::from(m.level))
    };
    for i in 0..game.evolutions.len() {
        let e = game.evolutions[i];
        let level = (e.kind == E::WithMove && move_evolutions).then(|| learnt_at(game, e.from, e.parameter));
        let e = &mut game.evolutions[i];
        match e.kind {
            E::WithMove if move_evolutions => {
                let level = match level {
                    Some(l) if l != 1 => l,
                    // Piloswine learns Ancient Power at no level.
                    _ if estimated => e.estimated_level,
                    _ => 45,
                };
                e.update_method(E::Level, level);
            }
            E::Trade => e.update_method_estimated(E::Level, 37, estimated),
            E::TradeItem if e.from == SLOWPOKE => e.update_method_estimated(E::Stone, WATER_STONE, estimated),
            E::TradeItem => e.update_method_estimated(E::Item, e.parameter, estimated),
            E::TradeSpecial => {
                let other = if e.from == KARRABLAST { SHELMET } else { KARRABLAST };
                e.update_method_estimated(E::WithOther, other, estimated);
            }
            _ => {}
        }
    }
}

/// Gen 7's evolutions that depend on the game changed: Cosmoem's evolve by day and night (the one of this game by
/// day in Sun and Ultra Sun, by night in Moon and Ultra Moon), and Rockruff's by the time of day alone.
fn remove_game_specific(game: &mut GameData, estimated: bool) {
    let sunny = game.identity.name.ends_with("Sun");
    let (this, other) = if sunny {
        (E::LevelDay, E::LevelNight)
    } else {
        (E::LevelNight, E::LevelDay)
    };
    for e in &mut game.evolutions {
        let kind = match e.kind {
            E::LevelGameThis => this,
            E::LevelGameOther => other,
            E::LevelGameThisDay | E::LevelGameOtherDay => E::LevelDay,
            E::LevelGameThisNight | E::LevelGameOtherNight => E::LevelNight,
            _ => continue,
        };
        e.update_method_estimated(kind, e.parameter, estimated);
    }
}

/// Gen 7's evolutions made easier: Inkay evolves without turning the console upside down, and in Sun and Moon the
/// evolutions in a magnetic field (Magnezone, Probopass, Vikavolt) and Crabrawler's in the snow also happen at
/// level 35 (or the estimated level).
fn easier_gen7(game: &mut GameData, estimated: bool) {
    let sun_moon = game.identity.family.eq_ignore_ascii_case("SM");
    let mut extra = Vec::new();
    let mut last_species = None;
    for e in &mut game.evolutions {
        if e.kind == E::LevelUpsideDown {
            e.update_method(E::Level, e.parameter);
        } else if sun_moon && matches!(e.kind, E::Snowy | E::MagneticField) {
            let level = if estimated { e.estimated_level } else { 35 };
            let mut added = Evolution::new(e.from, e.to, E::Level, level);
            added.estimated_level = e.estimated_level;
            // One added evolution per species: the last.
            if last_species == Some(e.from) {
                extra.pop();
            }
            last_species = Some(e.from);
            extra.push(added);
        }
    }
    game.evolutions.extend(extra);
}

/// Condensed level evolutions: when `max` is below the highest evolution level, evolutions happen at `max`
/// at the latest, and those into species that evolve again at three quarters of it.
fn condense_level_evolutions(game: &mut GameData, max: u16, highest: u16) {
    if max >= highest {
        return;
    }
    let max_intermediate = (f64::from(max) * 0.75).ceil();
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "at most a level")]
    let max_intermediate = max_intermediate as u16;
    for i in 0..game.evolutions.len() {
        let e = game.evolutions[i];
        let evolves_again = game.evolutions.iter().any(|n| n.from == e.to);
        let level = if e.kind.uses_level_threshold() {
            e.parameter
        } else {
            e.estimated_level
        };
        let new = if level > max_intermediate && evolves_again {
            max_intermediate
        } else if level > max {
            max
        } else {
            continue;
        };
        let e = &mut game.evolutions[i];
        if e.kind.uses_level_threshold() {
            e.update_method(e.kind, new);
        } else {
            e.estimated_level = new;
        }
    }
}

/// Time-based evolutions removed: dusk evolutions by Dusk Stone; day and night twins by Sun and Moon Stone;
/// other evolutions by time at any time. `before` are the evolutions before any improvement.
fn remove_time_based(game: &mut GameData, before: &[Evolution]) {
    let mut had: BTreeMap<DexNumber, Vec<E>> = BTreeMap::new();
    for e in before {
        had.entry(e.from).or_default().push(e.kind);
    }
    for e in &mut game.evolutions {
        if e.kind == E::LevelDusk {
            e.update_method(E::Stone, DUSK_STONE);
        } else if let Some(timeless) = e.kind.timeless() {
            let paired = e
                .kind
                .opposite_time()
                .is_some_and(|o| had.get(&e.from).is_some_and(|kinds| kinds.contains(&o)));
            if paired {
                let stone = if e.kind.is_day_type() { SUN_STONE } else { MOON_STONE };
                e.update_method(E::Stone, stone);
            } else {
                e.update_method(timeless, e.parameter);
            }
        }
    }
}

#[cfg(test)]
mod tests;
