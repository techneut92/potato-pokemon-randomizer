//! The move selection pool: the moves a trainer Pokémon
//! might know, without those that are useless or outdone by another.

use std::collections::BTreeSet;

use ppr_model::{DexNumber, GameData, Move, MoveId, StatChange, StatChangeMoveType as Scm, TrainerPokemon};

use super::synergy::{
    CANNOT_BE_OBSOLETED_MOVES, CANNOT_OBSOLETE_MOVES, DOUBLE_BATTLE_MOVES, REQUIRES_OTHER_MOVE, USELESS_MOVES,
    hard_ability_anti_synergy, requires_other_move,
};
use crate::StepRng;
use crate::choice::{below, unit};
use crate::randomizers::common::strength;

/// The chances of a pre-evolution's, TM or tutor, and egg move joining the pool.
const PRE_EVO_MOVE_CHANCE: f64 = 0.5;
const TM_MOVE_CHANCE: f64 = 0.6;
const TUTOR_MOVE_CHANCE: f64 = 0.6;
const EGG_MOVE_CHANCE: f64 = 0.1;
/// From this generation on, alternate formes evolve on their own (Alolan Raichu from Pikachu).
const FORME_EVOLUTIONS_GENERATION: u8 = 7;

/// The move with this number.
pub(super) fn move_of(game: &GameData, number: MoveId) -> Option<&Move> {
    usize::from(number)
        .checked_sub(1)
        .and_then(|i| game.moves.get(i))
        .filter(|m| m.number == number)
}

/// The moves in order of first appearance, each once.
pub(super) fn distinct<'a>(moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut seen = BTreeSet::new();
    moves.iter().copied().filter(|m| seen.insert(m.number)).collect()
}

/// The species `number` evolves from, following the first evolution to it, as far back as it goes; never a
/// species twice, so cyclic evolutions end.
fn pre_evolutions(game: &GameData, number: DexNumber) -> Vec<DexNumber> {
    let mut chain = vec![number];
    while let Some(from) = game
        .evolutions
        .iter()
        .find(|e| Some(&e.to) == chain.last())
        .map(|e| e.from)
    {
        if chain.contains(&from) {
            break;
        }
        chain.push(from);
    }
    chain.remove(0);
    chain
}

/// The species whose pre-evolutions and first stage's egg moves a Pokémon of `number` gets: an alternate forme
/// takes its base species' before Gen 7, where alternate formes have no evolutions of their own (Basculin's
/// Blue-Striped Form gets Basculin's egg moves); from Gen 7 on, it is the Pokémon's own species.
fn family_start(game: &GameData, number: DexNumber) -> DexNumber {
    if game.identity.generation.number() >= FORME_EVOLUTIONS_GENERATION {
        number
    } else {
        game.base_forme_of(number)
    }
}

/// Whether a TM or tutor move joins the pool: a strong enough damaging move with 60% chance, or by the level.
fn learnable_joins(mv: &Move, level: u8, chance: f64, rng: &mut StepRng) -> bool {
    let level_ = usize::from(level);
    if mv.power > 1 && f64::from(level) * 3.0 > strength(mv) && unit(rng) < chance {
        return true;
    }
    (mv.power <= 1 && below(rng, 100) < level_) || below(rng, 200) < level_
}

/// The moves to choose from: level-up moves by the Pokémon's level, some of its pre-evolutions', TM
/// and tutor moves it can learn, and some of its family's egg moves ([`family_start`]).
pub(super) fn selection_pool<'a>(
    game: &'a GameData,
    tp: &TrainerPokemon,
    cyclic_evolutions: bool,
    rng: &mut StepRng,
) -> Vec<&'a Move> {
    let learnset = |n: DexNumber| game.learnsets.get(&n).map(Vec::as_slice).unwrap_or_default();
    let mut pool: Vec<&Move> = learnset(tp.species)
        .iter()
        .filter(|l| (l.level != 0 && l.level <= tp.level) || (l.level == 0 && tp.level >= 30))
        .filter_map(|l| move_of(game, l.move_id))
        .collect();
    pool = distinct(&pool);
    let family_start = family_start(game, tp.species);
    let pre_evolutions = if cyclic_evolutions {
        Vec::new()
    } else {
        pre_evolutions(game, family_start)
    };
    for &pre in &pre_evolutions {
        let moves: Vec<&Move> = learnset(pre)
            .iter()
            .filter(|l| l.level <= tp.level)
            .filter(|_| unit(rng) < PRE_EVO_MOVE_CHANCE)
            .filter_map(|l| move_of(game, l.move_id))
            .collect();
        pool.extend(distinct(&moves));
    }
    for (moves, compat, chance) in [
        (&game.tm_moves, game.tm_hm_compat.get(&tp.species), TM_MOVE_CHANCE),
        (&game.tutor_moves, game.tutor_compat.get(&tp.species), TUTOR_MOVE_CHANCE),
    ] {
        for (i, &number) in moves.iter().enumerate() {
            let Some(mv) = move_of(game, number) else { continue };
            if compat.and_then(|c| c.get(i)).copied().unwrap_or_default() && learnable_joins(mv, tp.level, chance, rng)
            {
                pool.push(mv);
            }
        }
    }
    if !cyclic_evolutions {
        let first = pre_evolutions.last().copied().unwrap_or(family_start);
        for &number in game.egg_moves.get(&first).map(Vec::as_slice).unwrap_or_default() {
            if unit(rng) < EGG_MOVE_CHANCE
                && let Some(mv) = move_of(game, number)
            {
                pool.push(mv);
            }
        }
    }
    distinct(&pool)
}

/// The first four moves, 0 for none.
pub(super) fn fill(moves: &[&Move]) -> [MoveId; 4] {
    let mut out = [0; 4];
    for (slot, mv) in out.iter_mut().zip(moves) {
        *slot = mv.number;
    }
    out
}

/// Drops useless moves, double battle moves outside double battles, outdone moves, moves
/// missing the move they need, and moves at odds with the ability. `Err` holds the moves once four or fewer are
/// left.
pub(super) fn trim(
    pool: Vec<&Move>,
    ability: Option<u16>,
    only_multi_battles: bool,
) -> Result<Vec<&Move>, [MoveId; 4]> {
    let done = |pool: &[&Move]| if pool.len() <= 4 { Err(fill(pool)) } else { Ok(()) };
    done(&pool)?;
    let mut pool: Vec<&Move> = pool
        .into_iter()
        .filter(|m| {
            !USELESS_MOVES.contains(&m.number) && (only_multi_battles || !DOUBLE_BATTLE_MOVES.contains(&m.number))
        })
        .collect();
    done(&pool)?;
    let obsolete: BTreeSet<MoveId> = obsolete_moves(&pool).iter().map(|m| m.number).collect();
    pool.retain(|m| !obsolete.contains(&m.number));
    done(&pool)?;
    let dependent: Vec<&Move> = pool
        .iter()
        .copied()
        .filter(|m| REQUIRES_OTHER_MOVE.contains(&m.number))
        .collect();
    for dependent in dependent {
        if requires_other_move(dependent, &pool).is_empty()
            && let Some(at) = pool.iter().position(|m| m.number == dependent.number)
        {
            pool.remove(at);
        }
    }
    done(&pool)?;
    if let Some(ability) = ability {
        let anti: BTreeSet<MoveId> = hard_ability_anti_synergy(ability, &pool)
            .iter()
            .map(|m| m.number)
            .collect();
        let without: Vec<&Move> = pool.iter().copied().filter(|m| !anti.contains(&m.number)).collect();
        if !without.is_empty() {
            pool = without;
        }
    }
    done(&pool)?;
    Ok(pool)
}

/// A damaging move that lowers the target's stats or raises the user's.
fn has_beneficial_stat_change(mv: &Move) -> bool {
    let stages = mv.stat_changes.first().map_or(0, |s| s.stages);
    (mv.stat_change_move_type == Scm::DamageTarget && stages < 0)
        || (mv.stat_change_move_type == Scm::DamageUser && stages > 0)
}

/// Whether damaging move `mv` outdoes `other`: of the same type and category, stronger, at least as accurate and
/// fast, and with the same side effect or none worth having.
fn outdoes(mv: &Move, other: &Move) -> bool {
    let same_effect = (mv.stat_change_move_type == other.stat_change_move_type
        && mv.stat_changes.first() == other.stat_changes.first())
        || (other.stat_change_move_type == Scm::NoneOrUnknown && has_beneficial_stat_change(mv));
    !CANNOT_BE_OBSOLETED_MOVES.contains(&other.number)
        && mv.move_type == other.move_type
        && ((same_effect && mv.absorb_percent >= other.absorb_percent && !mv.is_charge && !mv.is_recharge)
            || strength(other) <= 30.0)
        && mv.accuracy >= other.accuracy
        && mv.category == other.category
        && mv.priority >= other.priority
        && other.power > 0
        && strength(mv) > strength(other)
}

/// Whether status move `mv`, changing `changes`, outdoes `other`: it changes the same stats, at least one more
/// or by more stages.
fn outdoes_status(mv: &Move, changes: &[StatChange], other: &Move) -> bool {
    let theirs = &other.stat_changes;
    if theirs.len() > changes.len() {
        return false;
    }
    let mine: Vec<&StatChange> = changes.iter().filter(|c| !theirs.contains(c)).collect();
    let theirs: Vec<&StatChange> = theirs.iter().filter(|c| !changes.contains(c)).collect();
    if theirs.is_empty() {
        return !mine.is_empty();
    }
    let mut maybe_better = false;
    for sc1 in mine {
        let mut can_still_be_better = false;
        for sc2 in &theirs {
            if sc1.stat == sc2.stat {
                can_still_be_better = true;
                let better = match mv.stat_change_move_type {
                    Scm::NoDamageUser => sc1.stages > sc2.stages,
                    Scm::NoDamageTarget => sc1.stages < sc2.stages,
                    _ => false,
                };
                if better {
                    maybe_better = true;
                } else {
                    can_still_be_better = false;
                }
            }
        }
        if !can_still_be_better {
            return false;
        }
    }
    maybe_better
}

/// The moves another in `moves` does better.
pub(super) fn obsolete_moves<'a>(moves: &[&'a Move]) -> Vec<&'a Move> {
    let mut obsolete = Vec::new();
    for &mv in moves {
        if CANNOT_OBSOLETE_MOVES.contains(&mv.number) {
            continue;
        }
        if mv.power > 0 {
            obsolete.extend(moves.iter().copied().filter(|other| outdoes(mv, other)));
        } else if matches!(mv.stat_change_move_type, Scm::NoDamageUser | Scm::NoDamageTarget) {
            obsolete.extend(moves.iter().copied().filter(|other| {
                other.number != mv.number
                    && other.power == 0
                    && other.stat_change_move_type == mv.stat_change_move_type
                    && (other.status == mv.status || other.status.is_none())
                    && !CANNOT_BE_OBSOLETED_MOVES.contains(&other.number)
                    && outdoes_status(mv, &mv.stat_changes, other)
            }));
        }
    }
    distinct(&obsolete)
}
