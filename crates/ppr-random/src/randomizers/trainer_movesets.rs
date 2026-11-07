//! Trainer movesets: trainer Pokémon get movesets chosen from what they could know
//! by their level (level-up moves, some of their pre-evolutions', TM, tutor and egg moves), weighted towards
//! same-type attacks, moves that suit their ability and stats, and moves that work well together.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - A Pokémon with no moves to choose from keeps the moves it had rather than getting none.
//! - When the last pick has nothing left to choose from (every move left needs a move not picked), the Pokémon
//!   gets three moves rather than failing the run.
//! - The ability is the species' first one in Gen 3, for the game's own trainer Pokémon as well as changed ones.
//! - A special attack of 0 counts as 1, so the attack ratio never divides by zero.

use std::collections::BTreeSet;

use ppr_model::{BattleStyle, GameData, Move, MoveCategory, MoveId, Species, TrainerPokemon, Type};
use ppr_settings::{BattleStyleMode, EvolutionMode, Settings};

use super::common::{is_good_damaging, perfect_accuracy};
use crate::choice::{below, shuffle};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

mod pool;
mod synergy;

use pool::{distinct, fill, selection_pool, trim};
use synergy::{
    GORILLA_TACTICS, HUGE_POWER, HUSTLE, MOXIE, PURE_POWER, REQUIRES_OTHER_MOVE, SOUL_HEART, hard_ability_synergy,
    hard_move_anti_synergy, move_synergy, requires_other_move, soft_ability_anti_synergy, soft_ability_synergy,
    soft_move_anti_synergy, soft_move_synergy, stat_anti_synergy, stat_synergy, synergy_ability,
};

/// Gives trainer Pokémon better movesets by [`ppr_settings::TrainerSettings::better_movesets`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerMovesetRandomizer;

impl Step<RunData> for TrainerMovesetRandomizer {
    fn id(&self) -> u64 {
        step_ids::TRAINER_MOVESETS
    }

    fn label(&self) -> &'static str {
        "Trainer Movesets"
    }

    fn applies(&self, data: &RunData) -> bool {
        (0..3).any(|i| data.settings.trainers.better_movesets[i] && data.game.trainer_custom_movesets[i])
            && !data.game.trainers.is_empty()
            && !data.game.moves.is_empty()
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        randomize(&mut data.game, &data.settings, &mut rng);
        Ok(())
    }
}

/// What every trainer Pokémon's moveset depends on.
struct Context<'a> {
    game: &'a GameData,
    cyclic_evolutions: bool,
    only_multi_battles: bool,
    perfect_accuracy: u8,
    generation: u8,
    has_abilities: bool,
}

/// Gives the Pokémon of the boss, important and regular trainers the settings choose new movesets.
fn randomize(game: &mut GameData, settings: &Settings, rng: &mut StepRng) {
    let [boss, important, regular] =
        std::array::from_fn(|i| settings.trainers.better_movesets[i] && game.trainer_custom_movesets[i]);
    let mut trainers = std::mem::take(&mut game.trainers);
    let ctx = Context {
        game,
        cyclic_evolutions: settings.evolutions.mode == EvolutionMode::RandomEveryLevel,
        only_multi_battles: settings.battle_style.mode == BattleStyleMode::SingleStyle
            && settings.battle_style.style != BattleStyle::Single,
        perfect_accuracy: perfect_accuracy(game),
        generation: game.identity.generation.number(),
        has_abilities: game.identity.generation.number() >= 3,
    };
    for t in &mut trainers {
        let chosen = (t.is_boss() && boss) || (t.is_important() && important) || (t.is_regular() && regular);
        if !chosen || t.should_not_get_buffs() {
            continue;
        }
        let modifier = if t.is_important() {
            1.5
        } else if t.is_boss() {
            2.0
        } else {
            1.0
        };
        for tp in &mut t.pokemon {
            if let Some(moves) = moveset(&ctx, tp, modifier, rng) {
                tp.reset_moves = false;
                tp.moves = moves;
            }
        }
    }
    game.trainers = trainers;
}

/// How many copies a bias adds: `bias` times `len`, rounded up.
fn copies(bias: f64, len: usize) -> usize {
    #[expect(clippy::cast_precision_loss, reason = "a move list's length")]
    let target = bias * len as f64;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small, positive count"
    )]
    let copies = target.ceil().max(0.0) as usize;
    copies
}

/// Adds `list`, shuffled, to `pool` until `bias` times its length is reached, repeating it as needed.
fn add_biased<'a>(pool: &mut Vec<&'a Move>, mut list: Vec<&'a Move>, bias: f64, rng: &mut StepRng) {
    shuffle(rng, &mut list);
    for i in 0..copies(bias, list.len()) {
        pool.push(list[i % list.len()]);
    }
}

/// The trainer Pokémon's ability: its species' first one (Gen 3 trainer Pokémon always have it).
fn ability(ctx: &Context<'_>, species: &Species) -> Option<u16> {
    ctx.has_abilities
        .then(|| synergy_ability(species.abilities.first().copied().unwrap_or_default(), ctx.generation))
}

/// The biases of one Pokémon, from its trainer's importance and how many moves it has to choose from.
struct Biases {
    stab: f64,
    hard_ability: f64,
    soft_ability: f64,
    stat: f64,
    soft_move: f64,
    hard_move: f64,
}

impl Biases {
    fn new(trainer_modifier: f64, pool_size: usize) -> Self {
        #[expect(clippy::cast_precision_loss, reason = "a move list's length")]
        let bonus = trainer_modifier * (pool_size as f64 / 10.0);
        Self {
            stab: 0.25 * bonus,
            hard_ability: bonus,
            soft_ability: 0.5 * bonus,
            stat: 0.5 * bonus,
            soft_move: 0.25 * bonus,
            hard_move: bonus,
        }
    }
}

/// How many copies of moves somewhat at odds with a pick are removed, relative to their number.
const SOFT_MOVE_ANTI_BIAS: f64 = 0.5;
/// How strongly the attack / special attack ratio favours a category.
const ATK_SPATK_RATIO_MODIFIER: f64 = 0.75;

/// One trainer Pokémon's new moves, or `None` when it has none to choose from.
fn moveset(ctx: &Context<'_>, tp: &TrainerPokemon, modifier: f64, rng: &mut StepRng) -> Option<[MoveId; 4]> {
    let species = ctx.game.species(tp.species)?;
    let ability = ability(ctx, species);
    let pool = selection_pool(ctx.game, tp, ctx.cyclic_evolutions, rng);
    if pool.is_empty() {
        return None;
    }
    let mut pool = match trim(pool, ability, ctx.only_multi_battles) {
        Ok(pool) => pool,
        Err(moves) => return Some(moves),
    };
    let biases = Biases::new(modifier, pool.len());
    for t in [Some(species.primary_type), species.secondary_type]
        .into_iter()
        .flatten()
    {
        let stab = stab_moves(&pool, t);
        add_biased(&mut pool, stab, biases.stab, rng);
    }
    if let Some(ability) = ability {
        pool = with_ability_synergies(pool, ability, species, &biases, ctx, rng);
    }
    let left = distinct(&pool);
    if left.len() <= 4 {
        return Some(fill(&left));
    }
    let stat = stat_synergy(&species.stats, &pool);
    add_biased(&mut pool, stat, biases.stat, rng);
    pool = without_all(&pool, &stat_anti_synergy(&species.stats, &pool));
    let left = distinct(&pool);
    if left.len() <= 4 {
        return Some(fill(&left));
    }
    add_category_bias(&mut pool, atk_spatk_ratio(species, ability), rng);
    Some(pick(ctx, pool, &biases, rng))
}

/// The damaging moves of type `t`.
fn stab_moves<'a>(pool: &[&'a Move], t: Type) -> Vec<&'a Move> {
    pool.iter()
        .copied()
        .filter(|m| m.move_type == Some(t) && m.category != MoveCategory::Status)
        .collect()
}

/// `pool` without one copy of each of `remove`, unless that leaves nothing.
fn without_all<'a>(pool: &[&'a Move], remove: &[&Move]) -> Vec<&'a Move> {
    let mut without = pool.to_vec();
    for r in remove {
        if let Some(at) = without.iter().position(|m| m.number == r.number) {
            without.remove(at);
        }
    }
    if without.is_empty() { pool.to_vec() } else { without }
}

/// `pool` with more of the moves the ability makes better and without those it makes worse.
fn with_ability_synergies<'a>(
    mut pool: Vec<&'a Move>,
    ability: u16,
    species: &Species,
    biases: &Biases,
    ctx: &Context<'_>,
    rng: &mut StepRng,
) -> Vec<&'a Move> {
    let (t1, t2) = (species.primary_type, species.secondary_type);
    let hard = hard_ability_synergy(ability, t1, t2, &pool, ctx.generation, ctx.perfect_accuracy);
    add_biased(&mut pool, hard, biases.hard_ability, rng);
    let soft = soft_ability_synergy(ability, &pool, t1, t2);
    add_biased(&mut pool, soft, biases.soft_ability, rng);
    without_all(&pool, &soft_ability_anti_synergy(ability, &pool))
}

/// Attack over special attack (at least 1), scaled by abilities that boost either.
fn atk_spatk_ratio(species: &Species, ability: Option<u16>) -> f64 {
    let ratio = f64::from(species.stats.attack) / f64::from(species.stats.sp_attack.max(1));
    ratio
        * match ability {
            Some(HUGE_POWER | PURE_POWER) => 2.0,
            Some(HUSTLE | GORILLA_TACTICS) => 1.5,
            Some(MOXIE) => 1.1,
            Some(SOUL_HEART) => 0.9,
            _ => 1.0,
        }
}

/// Adds more of the category the Pokémon is better at.
fn add_category_bias(pool: &mut Vec<&Move>, ratio: f64, rng: &mut StepRng) {
    let of = |c: MoveCategory| pool.iter().copied().filter(|m| m.category == c).collect::<Vec<_>>();
    let (physical, special) = (of(MoveCategory::Physical), of(MoveCategory::Special));
    let (favoured, other, ratio) = if ratio < 1.0 && !special.is_empty() {
        (special, physical, 1.0 / ratio)
    } else if !physical.is_empty() {
        (physical, special, ratio)
    } else {
        return;
    };
    #[expect(clippy::cast_precision_loss, reason = "a move list's length")]
    let wanted = other.len() as f64 * (ATK_SPATK_RATIO_MODIFIER * ratio);
    #[expect(clippy::cast_possible_truncation, reason = "truncates towards zero, saturating")]
    let additional = wanted as i64 - i64::try_from(favoured.len()).unwrap_or(i64::MAX);
    for _ in 0..additional {
        pool.push(favoured[below(rng, favoured.len())]);
    }
}

/// The numbers of `moves`.
fn numbers(moves: &[&Move]) -> BTreeSet<MoveId> {
    moves.iter().map(|m| m.number).collect()
}

/// Picks four moves: a good damaging move first, then moves that go well with each pick.
fn pick(ctx: &Context<'_>, mut pool: Vec<&Move>, biases: &Biases, rng: &mut StepRng) -> [MoveId; 4] {
    let mut picked: Vec<&Move> = Vec::with_capacity(4);
    for i in 1..=4 {
        if i == 4 {
            drop_unsupported(&mut pool, &picked);
        }
        let good: Vec<&Move> = if i == 1 {
            pool.iter()
                .copied()
                .filter(|m| is_good_damaging(m, ctx.perfect_accuracy))
                .collect()
        } else {
            Vec::new()
        };
        let from = if good.is_empty() { &pool } else { &good };
        if from.is_empty() {
            break;
        }
        let mv = from[below(rng, from.len())];
        picked.push(mv);
        if i == 4 {
            break;
        }
        pool.retain(|m| m.number != mv.number);
        let anti = numbers(&hard_move_anti_synergy(mv, &pool));
        pool.retain(|m| !anti.contains(&m.number));
        let left = distinct(&pool);
        if left.len() <= 4 - i {
            picked.extend(left);
            break;
        }
        let hard = move_synergy(mv, &pool, ctx.generation);
        add_biased(&mut pool, hard, biases.hard_move, rng);
        let soft = soft_move_synergy(mv, &pool, &ctx.game.type_chart);
        add_biased(&mut pool, soft, biases.soft_move, rng);
        drop_soft_anti_synergy(&mut pool, mv, 4 - i, rng);
        let left = distinct(&pool);
        if left.len() <= 4 - i {
            picked.extend(left);
            break;
        }
    }
    fill(&picked)
}

/// Before the last pick, moves that need a move not picked leave the pool.
fn drop_unsupported(pool: &mut Vec<&Move>, picked: &[&Move]) {
    let picked = numbers(picked);
    let dependent: Vec<&Move> = distinct(pool)
        .into_iter()
        .filter(|m| REQUIRES_OTHER_MOVE.contains(&m.number))
        .collect();
    for dependent in dependent {
        let supported = requires_other_move(dependent, pool)
            .iter()
            .any(|r| picked.contains(&r.number));
        if !supported {
            pool.retain(|m| m.number != dependent.number);
        }
    }
}

/// Removes copies of moves at odds with `mv`, while more than `keep` different moves are left.
fn drop_soft_anti_synergy(pool: &mut Vec<&Move>, mv: &Move, keep: usize, rng: &mut StepRng) {
    let mut anti = soft_move_anti_synergy(mv, pool);
    shuffle(rng, &mut anti);
    for j in 0..copies(SOFT_MOVE_ANTI_BIAS, anti.len()) {
        if distinct(pool).len() <= keep {
            break;
        }
        let number = anti[j % anti.len()].number;
        if let Some(at) = pool.iter().position(|m| m.number == number) {
            pool.remove(at);
        }
    }
}

#[cfg(test)]
mod tests;
