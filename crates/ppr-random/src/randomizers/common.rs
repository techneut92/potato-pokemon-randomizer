//! Helpers several randomizers share: species sets, level modifiers, similar-strength picks and damaging moves.

use std::collections::BTreeSet;

use ppr_model::{DexNumber, GameData, Move, MoveId, SWIFT, Type};

use crate::StepRng;
use crate::choice::{pick, take};

/// Species by national number, always walked in national order.
pub(super) type SpeciesSet = BTreeSet<DexNumber>;

/// The fixed type order, which decides ties between themes and the order types are weighed in.
pub(super) const TYPE_ORDER: [Type; 18] = [
    Type::Normal,
    Type::Fighting,
    Type::Flying,
    Type::Grass,
    Type::Water,
    Type::Fire,
    Type::Rock,
    Type::Ground,
    Type::Psychic,
    Type::Bug,
    Type::Dragon,
    Type::Electric,
    Type::Ghost,
    Type::Poison,
    Type::Ice,
    Type::Steel,
    Type::Dark,
    Type::Fairy,
];

/// A similar-strength pool widens until it holds `min(MINIMUM_POOL, size / POOL_FACTOR)` species.
const SIMILAR_STRENGTH_MINIMUM_POOL: usize = 5;
const SIMILAR_STRENGTH_POOL_FACTOR: usize = 4;

/// `level` changed by `percent`: rounded, then clamped to 1–100.
pub(super) fn modified_level(level: u8, percent: i16) -> u8 {
    let scaled = (f64::from(level) * (1.0 + f64::from(percent) / 100.0)).round();
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to 1..=100"
    )]
    let level = scaled.clamp(1.0, 100.0) as u8;
    level
}

/// The species a holder (a trainer's, wild or static Pokémon) may become: the base species of the pool, with
/// `allow_alt_formes` also the alternate formes that are not only cosmetic.
pub(super) fn pool_with_formes(game: &GameData, allow_alt_formes: bool) -> SpeciesSet {
    if allow_alt_formes {
        let mut pool = game.species_pool_with_formes();
        pool.retain(|&n| game.species(n).is_some_and(|sp| !sp.forme.cosmetic));
        pool
    } else {
        game.species_pool()
    }
}

/// The alternate formes holders leave out: those that revert for the player (`player`), those that need their
/// species' ability while abilities stay as they are, and with `ban_irregular` the irregular ones.
pub(super) fn banned_formes(game: &GameData, player: bool, abilities_random: bool, ban_irregular: bool) -> SpeciesSet {
    let mut banned = SpeciesSet::new();
    if player {
        banned.extend(game.banned_formes_for_player());
    }
    if !abilities_random {
        banned.extend(game.ability_dependent_formes());
    }
    if ban_irregular {
        banned.extend(game.irregular_formes());
    }
    banned
}

/// The forme a holder of `n` gets: an alternate forme's number, or for a base species, when holders have formes
/// (`holder_formes`), one of its cosmetic formes (or the plain one) at random.
pub(super) fn forme_for(rng: &mut StepRng, game: &GameData, n: DexNumber, holder_formes: bool) -> u8 {
    let Some(sp) = game.species(n) else { return 0 };
    if !sp.is_base_forme() {
        return sp.forme.number;
    }
    if !holder_formes || sp.forme.cosmetic_formes.is_empty() {
        return 0;
    }
    let choices = sp.forme.cosmetic_formes.len() + 1;
    match crate::choice::below(rng, choices) {
        0 => 0,
        i => sp.forme.cosmetic_formes[i - 1],
    }
}

/// A species' base stat total (sum of its base stats); 0 for species the game doesn't have.
pub(super) fn bst(game: &GameData, n: DexNumber) -> u32 {
    game.species(n).map_or(0, |sp| u32::from(sp.bst()))
}

/// A random species of similar strength: within 10% of `target`, widened by 5% each way until the
/// pool holds at least `min(5, size / 4)` species. `set` must not be empty.
pub(super) fn similar_strength(rng: &mut StepRng, game: &GameData, set: &SpeciesSet, target: u32) -> DexNumber {
    let minimum = SIMILAR_STRENGTH_MINIMUM_POOL
        .min(set.len() / SIMILAR_STRENGTH_POOL_FACTOR)
        .max(1);
    if minimum >= set.len() {
        return pick(rng, set).unwrap_or_default();
    }
    let target = i64::from(target);
    // Integer division; at least 1 so a tiny BST still widens.
    let step = (target / 20).max(1);
    let (mut low, mut high) = (target - target / 10, target + target / 10);
    let mut pool: Vec<DexNumber> = set.iter().copied().collect();
    let mut can_pick = SpeciesSet::new();
    while can_pick.len() < minimum {
        pool.retain(|&n| {
            let b = i64::from(bst(game, n));
            let fits = b >= low && b <= high;
            if fits {
                can_pick.insert(n);
            }
            !fits
        });
        low -= step;
        high += step;
    }
    pick(rng, &can_pick).unwrap_or_default()
}

/// Species left to choose from, refilled from the full pool once used up.
pub(super) struct Pool {
    full: SpeciesSet,
    /// What is left.
    pub(super) left: SpeciesSet,
}

impl Pool {
    pub(super) fn new(full: SpeciesSet) -> Self {
        Self {
            left: full.clone(),
            full,
        }
    }

    /// Removes `n` from what is left, refilling when nothing is.
    pub(super) fn remove(&mut self, n: DexNumber) {
        self.left.remove(&n);
        if self.left.is_empty() {
            self.left.clone_from(&self.full);
        }
    }

    /// The species left that `keep` accepts, else those of the full pool that it accepts.
    pub(super) fn filtered(&self, keep: impl Fn(DexNumber) -> bool) -> SpeciesSet {
        let left: SpeciesSet = self.left.iter().copied().filter(|&n| keep(n)).collect();
        if left.is_empty() {
            self.full.iter().copied().filter(|&n| keep(n)).collect()
        } else {
            left
        }
    }

    /// A random species from what is left, then removed from it; `None` when the pool is empty.
    pub(super) fn take(&mut self, rng: &mut StepRng) -> Option<DexNumber> {
        let n = take(rng, &mut self.left)?;
        self.remove(n);
        Some(n)
    }
}

/// A species of similar strength, other than `current` when `ban_current` is set (`current` when it is all there
/// is), no stronger than it when `limit` is set; `None` when `pool` is empty.
pub(super) fn power_level_replacement(
    rng: &mut StepRng,
    game: &GameData,
    pool: &SpeciesSet,
    current: DexNumber,
    limit: bool,
    ban_current: bool,
) -> Option<DexNumber> {
    let target = bst(game, current);
    let mut candidates = pool.clone();
    if limit {
        let weaker: SpeciesSet = pool.iter().copied().filter(|&n| bst(game, n) <= target).collect();
        if !weaker.is_empty() {
            candidates = weaker;
        }
    }
    if candidates.is_empty() {
        return None;
    }
    if !ban_current {
        return Some(similar_strength(rng, game, &candidates, target));
    }
    // The current species only if it is all there is.
    let mut without_current = candidates.clone();
    without_current.remove(&current);
    if without_current.is_empty() {
        return Some(current);
    }
    Some(similar_strength(rng, game, &without_current, target))
}

/// Whether `n` could have evolved by `level`: it evolves from nothing, or from something by `level` at the
/// evolution's estimated level times `factor`.
pub(super) fn is_legal_evolution_at(game: &GameData, n: DexNumber, level: u8, factor: f64) -> bool {
    let mut to = game.evolutions.iter().filter(|e| e.to == n).peekable();
    to.peek().is_none() || to.any(|e| f64::from(level) >= factor * f64::from(e.estimated_level))
}

/// The power a damaging move needs to count as good (twice that, or this with good accuracy).
const MIN_DAMAGING_POWER: f64 = 50.0;
/// Moves that make poor forced damaging moves.
pub(super) const BANNED_FOR_DAMAGING: [MoveId; 26] = [
    120, 138, 153, 173, 206, 248, 252, 264, 353, 364, 387, 389, 132, 99, 205, 301, 485, 704, 492, 255, 49, 82, 32, 12,
    90, 329,
];
/// A move's power times its number of hits.
pub(super) fn strength(mv: &Move) -> f64 {
    f64::from(mv.power) * f64::from(mv.hit_count_hundredths) / 100.0
}

/// A strong damaging move: strength of 100 or more, or 50 or more with an accuracy of 90 or more or perfect accuracy.
pub(super) fn is_good_damaging(mv: &Move, perfect_accuracy: u8) -> bool {
    let s = strength(mv);
    s >= 2.0 * MIN_DAMAGING_POWER || (s >= MIN_DAMAGING_POWER && (mv.accuracy >= 90 || mv.accuracy == perfect_accuracy))
}

/// The "never misses" accuracy: the accuracy of Swift, which never misses.
pub(super) fn perfect_accuracy(game: &GameData) -> u8 {
    game.moves.iter().find(|m| m.number == SWIFT).map_or(0, |m| m.accuracy)
}
