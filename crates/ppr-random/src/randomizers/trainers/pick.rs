//! Choosing trainers' Pokémon: the replacements, the type themes and the Elite Four's unique Pokémon.

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{DexNumber, GameData, ItemId, Trainer, TrainerPokemon, Type};
use ppr_settings::{TrainerMode, TrainerSettings};

use super::super::common::{
    SpeciesSet, TYPE_ORDER, banned_formes, bst, forme_for, is_legal_evolution_at, pool_with_formes, similar_strength,
};
use super::{LABEL, evolved_as_far_as_legal, keep_valid_ability_slot};
use crate::choice::{below, pick, shuffle};
use crate::{RandomizeError, StepRng};

/// Wonder Guard's ability number.
const WONDER_GUARD: u16 = 25;
/// Below this level, no Wonder Guard with "No Early Wonder Guard".
const WONDER_GUARD_LEVEL: u8 = 20;
/// With random shiny trainer Pokémon, one in this many is shiny.
const SHINY_ODDS: usize = 256;

fn impossible(message: impl Into<String>) -> RandomizeError {
    RandomizeError::Impossible {
        step: LABEL,
        message: message.into(),
    }
}

/// The species `game` gives each type, by its current types.
fn by_type(game: &GameData, set: &SpeciesSet) -> BTreeMap<Type, SpeciesSet> {
    let mut map: BTreeMap<Type, SpeciesSet> = BTreeMap::new();
    for &n in set {
        if let Some(sp) = game.species(n) {
            map.entry(sp.primary_type).or_default().insert(n);
            if let Some(t) = sp.secondary_type {
                map.entry(t).or_default().insert(n);
            }
        }
    }
    map
}

fn has_type(game: &GameData, n: DexNumber, t: Type) -> bool {
    game.species(n).is_some_and(|sp| sp.has_type(t))
}

/// The type all species shared as loaded, preferring the first one's primary type,
/// and anything over Normal.
fn shared_original_type(game: &GameData, species: &SpeciesSet) -> Option<Type> {
    let original_has = |n, t| game.original_types_of(n).is_some_and(|(p, s)| p == t || s == Some(t));
    let mut iter = species.iter();
    let (mut primary, mut secondary) = game.original_types_of(*iter.next()?).map(|(p, s)| (Some(p), s))?;
    for &n in iter {
        if secondary.is_some_and(|t| !original_has(n, t)) {
            secondary = None;
        }
        if primary.is_some_and(|t| !original_has(n, t)) {
            primary = secondary;
            secondary = None;
        }
        primary?;
    }
    if primary == Some(Type::Normal) && secondary.is_some() {
        secondary
    } else {
        primary
    }
}

/// Whether `n` has an evolution whose estimated level, times `factor`, `level` reaches.
fn has_legal_evolution_at(game: &GameData, n: DexNumber, level: u8, factor: f64) -> bool {
    game.evolutions
        .iter()
        .any(|e| e.from == n && f64::from(level) >= factor * f64::from(e.estimated_level))
}

/// A random slot holding an ability, from 1; 0 for games without abilities.
pub(super) fn random_ability_slot(game: &GameData, n: DexNumber, rng: &mut StepRng) -> u8 {
    let abilities = game.trainer_pokemon_abilities(n).to_vec();
    if abilities.iter().all(|&a| a == 0) {
        return u8::from(!abilities.is_empty());
    }
    loop {
        let slot = below(rng, abilities.len());
        if abilities[slot] != 0 {
            return u8::try_from(slot + 1).unwrap_or(1);
        }
    }
}

/// Whether `tp` holds the Mega Stone of one of its species' Mega Evolutions.
pub(super) fn can_mega_evolve(game: &GameData, tp: &TrainerPokemon) -> bool {
    tp.held_item.is_some()
        && game
            .mega_evolutions()
            .any(|m| m.forme.base == Some(tp.species) && m.forme.mega_item == tp.held_item)
}

/// The species of the pool ("Limit Pokémon") with a Mega Evolution that needs a Mega Stone.
pub(super) fn mega_evolvables(game: &GameData) -> SpeciesSet {
    let pool = game.species_pool_with_formes();
    game.mega_evolutions()
        .filter(|m| m.forme.mega_item.is_some() && pool.contains(&m.number))
        .filter_map(|m| m.forme.base)
        .collect()
}

/// The Mega Stone of a random Mega Evolution of `n`; `None` when it has none or that Mega Evolution needs no stone.
pub(super) fn mega_stone(rng: &mut StepRng, game: &GameData, n: DexNumber) -> Option<ItemId> {
    let megas: SpeciesSet = game
        .mega_evolutions()
        .filter(|m| m.forme.base == Some(n))
        .map(|m| m.number)
        .collect();
    pick(rng, &megas)
        .and_then(|m| game.species(m))
        .and_then(|m| m.forme.mega_item)
}

/// Trainers (by position) grouped by tag before its `-`: gyms and the Elite Four, and
/// unless `league_only` the Champion and themed groups. Giovanni (`GIO`) joins the eighth gym.
fn trainer_groups(trainers: &[Trainer], league_only: bool) -> BTreeMap<String, Vec<usize>> {
    let mut groups: BTreeMap<String, Vec<usize>> = BTreeMap::new();
    for (i, t) in trainers.iter().enumerate() {
        let tag = t.tag.as_deref().unwrap_or_default();
        let group = tag.split('-').next().unwrap_or_default();
        let league = group.starts_with("GYM") || group.starts_with("ELITE");
        let other = group.starts_with("CHAMPION") || group.starts_with("THEMED");
        if league || (other && !league_only) {
            groups.entry(group.to_owned()).or_default().push(i);
        } else if group.starts_with("GIO") {
            groups.entry("GYM8".to_owned()).or_default().push(i);
        }
    }
    groups
}

/// Choosing trainers' Pokémon: the pools and what earlier choices left.
struct Picker<'a> {
    game: &'a GameData,
    s: &'a TrainerSettings,
    /// Every species trainers may get.
    all: SpeciesSet,
    /// `all` by type.
    by_type: BTreeMap<Type, SpeciesSet>,
    /// Evolution levels are multiplied by this.
    evolution_factor: f64,
    /// Only species that could have evolved by their level.
    no_premature_evolutions: bool,
    /// How often each species was placed, for the even distributions.
    placements: BTreeMap<DexNumber, u32>,
    /// Type weights, in [`TYPE_ORDER`]; computed when first needed.
    type_weights: Option<Vec<(Type, usize)>>,
    used_uber_types: BTreeSet<Type>,
    /// Group types by trainer position.
    trainer_types: BTreeMap<usize, Type>,
}

impl Picker<'_> {
    /// A random type that has species, weighed by how many with "Weight Types".
    fn pick_type(&mut self, rng: &mut StepRng) -> Result<Type, RandomizeError> {
        let weights = self.type_weights.get_or_insert_with(|| {
            let mut all: SpeciesSet = self.game.species_pool();
            if self.s.no_legendaries {
                all.retain(|&n| !self.game.is_legendary(n));
            }
            let sorted = by_type(self.game, &all);
            TYPE_ORDER
                .into_iter()
                .filter(|t| self.game.types.contains(t))
                .map(|t| (t, sorted.get(&t).map_or(0, BTreeSet::len)))
                .collect()
        });
        let total: usize = weights.iter().map(|&(_, w)| w).sum();
        if total == 0 {
            return Err(impossible("no type has species"));
        }
        if self.s.weight_types {
            let mut at = below(rng, total);
            for &(t, w) in weights.iter() {
                if at < w {
                    return Ok(t);
                }
                at -= w;
            }
        }
        loop {
            let t = self.game.types[below(rng, self.game.types.len())];
            if weights.iter().any(|&(w_t, w)| w_t == t && w > 0) {
                return Ok(t);
            }
        }
    }

    /// Gives the trainer groups their types: the game's themes when kept, else random ones.
    fn assign_group_types(&mut self, trainers: &[Trainer], rng: &mut StepRng) -> Result<(), RandomizeError> {
        let groups = trainer_groups(trainers, self.s.mode == TrainerMode::TypeThemedEliteFourGyms);
        let themes = if matches!(
            self.s.mode,
            TrainerMode::KeepThemed | TrainerMode::KeepThemesOrPrimary | TrainerMode::Unchanged
        ) {
            self.game.gym_and_elite_themes.clone()
        } else {
            self.random_group_types(groups.keys(), rng)?
        };
        for (group, members) in &groups {
            if let Some(&t) = themes.get(group) {
                for &i in members {
                    self.trainer_types.insert(i, t);
                }
            }
        }
        Ok(())
    }

    /// Random group types: gyms, the Elite Four and the Champion get different types
    /// while there are enough, gyms past the eighth last; other groups any type.
    fn random_group_types<'g>(
        &mut self,
        groups: impl Iterator<Item = &'g String>,
        rng: &mut StepRng,
    ) -> Result<BTreeMap<String, Type>, RandomizeError> {
        // Only types with species to choose from.
        let types: Vec<Type> = self
            .game
            .types
            .iter()
            .copied()
            .filter(|t| self.by_type.get(t).is_some_and(|set| !set.is_empty()))
            .collect();
        if types.is_empty() {
            return Err(impossible("no type has species"));
        }
        let mut remaining = types.clone();
        shuffle(rng, &mut remaining);
        let mut groups: Vec<&String> = groups.collect();
        shuffle(rng, &mut groups);
        let mut used_gym_types = BTreeSet::new();
        let mut post_eight = Vec::new();
        let mut out = BTreeMap::new();
        for group in groups {
            if (group.starts_with("GYM1") && group != "GYM1") || group == "GYM9" {
                post_eight.push(group);
                continue;
            }
            if group.starts_with("THEMED") {
                out.insert(group.clone(), types[below(rng, types.len())]);
                continue;
            }
            if remaining.is_empty() {
                return Err(impossible("more gyms, Elite Four and Champions than types"));
            }
            let t = remaining.remove(0);
            out.insert(group.clone(), t);
            if group.starts_with("GYM") {
                used_gym_types.insert(t);
            }
            if group == "CHAMPION" {
                self.used_uber_types.insert(t);
            }
        }
        for group in post_eight {
            let t = if remaining.is_empty() {
                let unused: Vec<Type> = types.iter().copied().filter(|t| !used_gym_types.contains(t)).collect();
                if unused.is_empty() {
                    types[below(rng, types.len())]
                } else {
                    unused[below(rng, unused.len())]
                }
            } else {
                remaining.remove(0)
            };
            out.insert(group.clone(), t);
            used_gym_types.insert(t);
        }
        Ok(out)
    }

    /// The type theme of trainer `i`, if any.
    fn type_for_trainer(&mut self, i: usize, t: &Trainer, rng: &mut StepRng) -> Result<Option<Type>, RandomizeError> {
        let mut theme = self.trainer_types.get(&i).copied();
        if theme.is_none() && self.s.mode == TrainerMode::TypeThemed {
            let mut picked = self.pick_type(rng)?;
            // Post-game bosses don't share a type.
            if t.tag.as_deref() == Some("UBER") {
                if self.used_uber_types.len() >= self.game.types.len() {
                    return Err(impossible("more post-game bosses than types"));
                }
                while self.used_uber_types.contains(&picked) {
                    picked = self.pick_type(rng)?;
                }
                self.used_uber_types.insert(picked);
            }
            theme = Some(picked);
        }
        let keep = matches!(
            self.s.mode,
            TrainerMode::KeepThemed | TrainerMode::KeepThemesOrPrimary | TrainerMode::Unchanged
        );
        if keep && theme.is_none() {
            let species: SpeciesSet = t.pokemon.iter().map(|p| p.species).collect();
            theme = shared_original_type(self.game, &species);
        }
        Ok(theme)
    }

    /// The species replacing `current`. `instead` replaces the pool (the Elite Four's non-local species). `mega`,
    /// for a Pokémon that can Mega Evolve when Mega Evolvables are swapped, holds the species that can: they replace
    /// the pool, or with a type theme the species of the theme that can, if any.
    #[expect(clippy::too_many_arguments, reason = "one parameter per rule")]
    fn replacement(
        &self,
        current: DexNumber,
        level: u8,
        placed: &mut SpeciesSet,
        theme: Option<Type>,
        instead: Option<&SpeciesSet>,
        mega: Option<&SpeciesSet>,
        banned_types: &BTreeSet<Type>,
        banned: &SpeciesSet,
        rng: &mut StepRng,
    ) -> Result<DexNumber, RandomizeError> {
        let game = self.game;
        let mut pool = match (theme, instead) {
            (Some(t), None) => self
                .by_type
                .get(&t)
                .cloned()
                .ok_or_else(|| impossible(format!("no {t:?} species for trainers")))?,
            (Some(t), Some(set)) => set.iter().copied().filter(|&n| has_type(game, n, t)).collect(),
            (None, set) => set.unwrap_or(&self.all).clone(),
        };
        if let Some(evolvables) = mega {
            // As in the original, they don't have to be in the pool, unless they share the theme.
            if theme.is_some() {
                let of_theme: SpeciesSet = pool.intersection(evolvables).copied().collect();
                if !of_theme.is_empty() {
                    pool = of_theme;
                }
            } else {
                pool.clone_from(evolvables);
            }
        }
        if !banned_types.is_empty() {
            // When every type is used up, repeat one rather than fail.
            let diverse: SpeciesSet = pool
                .iter()
                .copied()
                .filter(|&n| !banned_types.iter().any(|&t| has_type(game, n, t)))
                .collect();
            if !diverse.is_empty() {
                pool = diverse;
            }
        }
        if self.no_premature_evolutions {
            pool.retain(|&n| is_legal_evolution_at(game, n, level, self.evolution_factor));
        }
        if self.s.evolve.is_some() {
            pool.retain(|&n| !has_legal_evolution_at(game, n, level, self.evolution_factor));
        }
        if self.even_distribution() {
            #[expect(clippy::cast_precision_loss, reason = "counts of a few thousand")]
            let average = if self.placements.is_empty() {
                0.0
            } else {
                f64::from(self.placements.values().sum::<u32>()) / self.placements.len() as f64
            };
            let below_average: SpeciesSet = pool
                .iter()
                .copied()
                .filter(|n| f64::from(self.placements.get(n).copied().unwrap_or_default()) < average * 2.0)
                .collect();
            if !below_average.is_empty() {
                pool = below_average;
            }
        }
        if pool.is_empty() && instead.is_some() {
            return self.replacement(current, level, placed, theme, None, mega, banned_types, banned, rng);
        }
        let allowed: SpeciesSet = pool.difference(banned).copied().collect();
        if !allowed.is_empty() {
            pool = allowed;
        }
        // Last, so duplicates are avoided as long as possible.
        if !placed.is_empty() {
            let unplaced: SpeciesSet = pool.difference(placed).copied().collect();
            if unplaced.is_empty() {
                placed.clear();
            } else {
                pool = unplaced;
            }
        }
        if pool.is_empty() {
            return Err(impossible("no species fit the trainer settings"));
        }
        Ok(if self.s.similar_strength {
            similar_strength(rng, game, &pool, bst(game, current))
        } else {
            pick(rng, &pool).unwrap_or(current)
        })
    }

    fn even_distribution(&self) -> bool {
        // Gen 3 has no main-game trainer list, so every trainer counts.
        self.s.mode == TrainerMode::RandomEvenDistribution
    }
}

/// With diverse types and no theme, the types of `n` are used up.
fn use_types(game: &GameData, diverse: bool, theme: Option<Type>, used: &mut BTreeSet<Type>, n: DexNumber) {
    if diverse
        && theme.is_none()
        && let Some(sp) = game.species(n)
    {
        used.insert(sp.primary_type);
        used.extend(sp.secondary_type);
    }
}

/// The rules from outside the trainer settings.
#[derive(Debug, Clone, Copy)]
pub(super) struct Rules {
    pub(super) no_premature_evolutions: bool,
    pub(super) no_irregular_alt_formes: bool,
    /// Abilities are randomized, so alternate formes that need their species' ability may be used.
    pub(super) abilities_random: bool,
}

/// Replaces trainers' Pokémon according to [`TrainerSettings`].
pub(super) fn randomize(
    game: &mut GameData,
    s: &TrainerSettings,
    rules: Rules,
    rng: &mut StepRng,
) -> Result<(), RandomizeError> {
    let mut trainers = std::mem::take(&mut game.trainers);
    let result = randomize_trainers(game, &mut trainers, s, rules, rng);
    game.trainers = trainers;
    result
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass over the trainers, easier to follow in one piece"
)]
fn randomize_trainers(
    game: &GameData,
    trainers: &mut [Trainer],
    s: &TrainerSettings,
    rules: Rules,
    rng: &mut StepRng,
) -> Result<(), RandomizeError> {
    let no_premature_evolutions = rules.no_premature_evolutions;
    // With Unchanged only added Pokémon are chosen, keeping the team's theme.
    let only_added = s.mode == TrainerMode::Unchanged;
    let any_theme = only_added
        || matches!(
            s.mode,
            TrainerMode::TypeThemed
                | TrainerMode::TypeThemedEliteFourGyms
                | TrainerMode::KeepThemed
                | TrainerMode::KeepThemesOrPrimary
        );
    let mut everything = pool_with_formes(game, s.allow_alt_formes && game.formes.trainers);
    if s.no_legendaries {
        everything.retain(|&n| !game.is_legendary(n));
    }
    // As in the original, added Pokémon of unchanged teams are never irregular formes.
    let mut banned = banned_formes(
        game,
        false,
        rules.abilities_random,
        rules.no_irregular_alt_formes || only_added,
    );
    banned.extend(game.formes.banned_for_trainers.iter().copied());
    everything.retain(|n| !banned.contains(n));
    let mut all = everything.clone();
    if s.local_pokemon {
        let local: SpeciesSet = game
            .encounters
            .iter()
            .flat_map(|set| set.main_game_encounters().iter().map(|e| e.species))
            .flat_map(|n| game.family(n))
            .collect();
        all.retain(|n| local.contains(n));
    }
    let wonder_guard: SpeciesSet = all
        .iter()
        .copied()
        .filter(|&n| game.species(n).is_some_and(|sp| sp.abilities.contains(&WONDER_GUARD)))
        .collect();
    let mut picker = Picker {
        game,
        s,
        by_type: if any_theme {
            by_type(game, &all)
        } else {
            BTreeMap::new()
        },
        all,
        evolution_factor: 1.0 + f64::from(s.evolve.unwrap_or_default()) / 100.0,
        no_premature_evolutions,
        placements: BTreeMap::new(),
        type_weights: None,
        used_uber_types: BTreeSet::new(),
        trainer_types: BTreeMap::new(),
    };
    if any_theme {
        picker.assign_group_types(trainers, rng)?;
    }

    // A random order, so modifiers don't always favour the same trainers.
    let mut order: Vec<usize> = (0..trainers.len()).collect();
    shuffle(rng, &mut order);
    let unique = s.elite_four_unique.map_or(0, usize::from);
    let elite_positions: BTreeSet<usize> = (0..trainers.len())
        .filter(|&i| game.elite_four_trainers.contains(&trainers[i].index))
        .collect();
    let is_elite = |i: usize| elite_positions.contains(&i);
    let mut banned_from_unique = SpeciesSet::new();
    let mut elite_exceptions = None;
    let mut used_as_unique = SpeciesSet::new();
    let evolvables = if s.swap_mega_evos {
        mega_evolvables(game)
    } else {
        SpeciesSet::new()
    };
    if unique > 0 {
        order.sort_by_key(|&i| !is_elite(i));
        if s.rival_carries_starter {
            // Their families stay with the rivals, but for the final evolution.
            for &starter in game.starters.iter().take(3) {
                banned_from_unique.extend(game.family(starter));
            }
        }
        if s.local_pokemon {
            // The Elite Four's unique Pokémon come from outside the local ones first.
            elite_exceptions = Some(everything.difference(&picker.all).copied().collect::<SpeciesSet>());
        }
    }

    for i in order {
        let theme = picker.type_for_trainer(i, &trainers[i], rng)?;
        let t = &trainers[i];
        let mut positions: Vec<usize> = (0..t.pokemon.len()).collect();
        let mut skip_starter = s.rival_carries_starter
            && t.tag
                .as_deref()
                .is_some_and(|tag| tag.contains("RIVAL") || tag.contains("FRIEND"));
        let elite = unique > 0 && is_elite(i);
        let forced = t
            .force_starter_position
            .map(usize::from)
            .filter(|&p| skip_starter && p < positions.len());
        if let Some(starter) = forced {
            // The starter's place is fixed: it goes first, the others follow.
            positions.remove(starter);
            if elite {
                strongest_first(&mut positions, t);
            }
            positions.insert(0, starter);
        } else if elite || skip_starter {
            strongest_first(&mut positions, t);
        }
        let diverse = (t.is_boss() && s.diverse_types[0])
            || (t.is_important() && s.diverse_types[1])
            || (t.is_regular() && s.diverse_types[2]);
        let mut used_types = BTreeSet::new();
        let mut placed = SpeciesSet::new();
        if (s.avoid_duplicates || diverse) && only_added {
            for p in t.pokemon.iter().filter(|p| !p.added) {
                if s.avoid_duplicates {
                    placed.insert(p.species);
                }
                use_types(game, diverse, theme, &mut used_types, p.species);
            }
        }

        for (rank, &p) in positions.iter().enumerate() {
            let set_unique = elite && unique > rank;
            let swap_mega = s.swap_mega_evos && can_mega_evolve(game, &trainers[i].pokemon[p]);
            let (old, level, added) = {
                let tp = &trainers[i].pokemon[p];
                (tp.species, tp.level, tp.added)
            };
            let new = if skip_starter {
                // The rival's starter is already set.
                skip_starter = false;
                old
            } else if only_added && !added {
                if s.evolve.is_some() {
                    let evolved = evolved_as_far_as_legal(game, old, level, picker.evolution_factor, rng);
                    let tp = &mut trainers[i].pokemon[p];
                    if tp.species != evolved {
                        tp.species = evolved;
                        tp.forme = forme_for(rng, game, evolved, game.formes.trainers);
                    }
                    keep_valid_ability_slot(game, tp);
                    evolved
                } else {
                    old
                }
            } else {
                let mut banned = used_as_unique.clone();
                let mut instead = None;
                if set_unique {
                    banned.extend(banned_from_unique.iter().copied());
                    instead = elite_exceptions.as_ref();
                }
                if s.no_early_wonder_guard && level < WONDER_GUARD_LEVEL {
                    banned.extend(wonder_guard.iter().copied());
                }
                let wanted = if s.mode == TrainerMode::KeepThemesOrPrimary && theme.is_none() {
                    game.original_types_of(old).map(|(primary, _)| primary)
                } else {
                    theme
                };
                let mega = swap_mega.then_some(&evolvables);
                let new = picker.replacement(
                    old,
                    level,
                    &mut placed,
                    wanted,
                    instead,
                    mega,
                    &used_types,
                    &banned,
                    rng,
                )?;
                let forme = forme_for(rng, game, new, game.formes.trainers);
                let slot = random_ability_slot(game, new, rng);
                let tp = &mut trainers[i].pokemon[p];
                tp.species = new;
                tp.forme = forme;
                tp.ability_slot = slot;
                tp.reset_moves = true;
                new
            };
            if picker.even_distribution() {
                *picker.placements.entry(new).or_default() += 1;
            }
            if s.avoid_duplicates {
                placed.insert(new);
            }
            if set_unique {
                used_as_unique.insert(new);
            }
            if elite {
                banned_from_unique.insert(new);
            }
            use_types(game, diverse, theme, &mut used_types, new);
            if swap_mega {
                trainers[i].pokemon[p].held_item = mega_stone(rng, game, new);
            }
            // Last for each Pokémon, after its species, as the original rolls it.
            if s.random_shiny && below(rng, SHINY_ODDS) == 0 {
                trainers[i].pokemon[p].shiny = true;
            }
        }
    }
    Ok(())
}

/// Team positions strongest first, the last one first among equals.
fn strongest_first(positions: &mut [usize], t: &Trainer) {
    positions.reverse();
    positions.sort_by_key(|&p| std::cmp::Reverse(t.pokemon[p].level));
}
