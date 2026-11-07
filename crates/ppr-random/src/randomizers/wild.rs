//! Wild encounters: new species by area, zone or slot, level modifiers and catch rates, with similar-strength
//! picks.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005), and species are always taken in national order, so
//!   a seed gives the same result every run.
//! - With Only Basic Pokémon and a type rule, the per-type pools hold only basic species, as the setting says (they
//!   are sorted by type after filtering, not before).
//! - Not supported yet: `ORAS`'s `DexNav` handling and forced multiple species (no Gen 3 area needs either).
//!
//! Where wild Pokémon have a forme, "Allow Alternate Formes" lets replacements be alternate formes (but those that
//! revert for the player, those that need their species' ability while abilities stay, and with "No Irregular Alt
//! Formes" the irregular ones), and a base species with cosmetic formes gets one at random.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};

use ppr_model::{DexNumber, EncounterKind, EncounterSet, GameData, Type};
use ppr_settings::{WildEvolution, WildMode, WildSettings, WildTypes, WildZones};

use super::common::{self, SpeciesSet, TYPE_ORDER, modified_level};
use crate::choice::{pick, shuffle};
use crate::{CancelToken, RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

const LABEL: &str = "Wild Pokémon";

/// Replaces wild Pokémon and changes their levels and catch rates according to [`WildSettings`].
#[derive(Debug, Clone, Copy, Default)]
pub struct WildRandomizer;

impl Step<RunData> for WildRandomizer {
    fn id(&self) -> u64 {
        step_ids::WILD
    }

    fn label(&self) -> &'static str {
        LABEL
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.wild;
        s.mode != WildMode::Unchanged || s.min_catch_rate > 0 || s.level_modifier != 0
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, cancel } = ctx;
        let s = data.settings.wild.clone();
        // The level modifier comes first, whether species change or not.
        apply_level_modifier(&mut data.game.encounters, s.level_modifier);
        if s.mode == WildMode::Random {
            let no_premature = data.settings.general.no_premature_evolutions;
            let banned_formes = common::banned_formes(
                &data.game,
                true,
                data.settings.abilities.mode != ppr_settings::AbilityMode::Unchanged,
                data.settings.general.no_irregular_alt_formes,
            );
            data.game.encounters = randomize_encounters(&data.game, &s, no_premature, banned_formes, &mut rng, cancel)?;
        }
        if s.min_catch_rate > 0 {
            change_catch_rates(&mut data.game, s.min_catch_rate);
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

fn apply_level_modifier(sets: &mut [EncounterSet], percent: i16) {
    if percent == 0 {
        return;
    }
    for e in sets.iter_mut().flat_map(|s| &mut s.encounters) {
        e.min_level = modified_level(e.min_level, percent);
        e.max_level = modified_level(e.max_level, percent);
    }
}

/// Level 5 patches the game so every ball catches; levels 1–4 raise catch rates.
fn change_catch_rates(game: &mut GameData, level: u8) {
    let (normal, legendary) = match level {
        5 => {
            game.guaranteed_catching = true;
            return;
        }
        2 => (128, 64),
        3 => (200, 100),
        4 => (255, 255),
        _ => (75, 37),
    };
    for s in &mut game.species {
        let minimum = if s.is_legendary() { legendary } else { normal };
        s.catch_rate = s.catch_rate.max(minimum);
    }
}

/// The new encounter sets, after the level modifier.
fn randomize_encounters(
    game: &GameData,
    s: &WildSettings,
    no_premature_evolutions: bool,
    banned_formes: SpeciesSet,
    rng: &mut StepRng,
    cancel: &CancelToken,
) -> Result<Vec<EncounterSet>, RandomizeError> {
    // The areas to randomize: no unused ones, in random order.
    let mut prepped: Vec<usize> = (0..game.encounters.len())
        .filter(|&i| {
            let set = &game.encounters[i];
            set.kind != EncounterKind::Unused && set.location.as_deref() != Some("UNUSED")
        })
        .collect();
    shuffle(rng, &mut prepped);

    let mut banned: SpeciesSet = game.wild_banned.iter().copied().collect();
    banned.extend(banned_formes);
    let pool = common::pool_with_formes(game, s.allow_alt_formes && game.formes.wild);
    let allowed: SpeciesSet = game
        .species
        .iter()
        .filter(|sp| !(s.no_legendaries && sp.is_legendary()))
        .map(|sp| sp.number)
        .filter(|n| pool.contains(n) && !banned.contains(n))
        .collect();

    let zones: Vec<Vec<usize>> = match s.zones {
        WildZones::Slot | WildZones::Set => prepped.iter().map(|&i| vec![i]).collect(),
        WildZones::Map => split(game, group(&prepped, |i| game.encounters[i].map), s.split_by_kind),
        WildZones::Location => {
            let mut untagged = 0;
            let by_location = group(&prepped, |i| {
                game.encounters[i].location.clone().unwrap_or_else(|| {
                    untagged += 1;
                    format!("UNTAGGED-{untagged}")
                })
            });
            split(game, by_location, s.split_by_kind)
        }
        WildZones::Game if s.split_by_kind => group(&prepped, |i| game.encounters[i].kind),
        WildZones::Game => vec![prepped],
    };
    let mut inner = Inner::new(game, s, allowed, banned, rng, cancel)?;
    inner.use_mapping = s.zones != WildZones::Slot;
    // "No Premature Evolutions" is dropped when other evolution rules are on.
    inner.no_premature_evolutions =
        no_premature_evolutions && s.evolution != WildEvolution::SameStage && !s.keep_relations;
    inner.randomize_zones(zones)?;
    Ok(inner.sets)
}

/// Groups area indices by a key, keeping the order in which keys first appear.
fn group<K: Eq + std::hash::Hash>(indices: &[usize], mut key: impl FnMut(usize) -> K) -> Vec<Vec<usize>> {
    let mut index: HashMap<K, usize> = HashMap::new();
    let mut groups: Vec<Vec<usize>> = Vec::new();
    for &i in indices {
        let g = *index.entry(key(i)).or_insert_with(|| {
            groups.push(Vec::new());
            groups.len() - 1
        });
        groups[g].push(i);
    }
    groups
}

/// With Split by Encounter Types, each zone becomes one zone per encounter type.
fn split(game: &GameData, zones: Vec<Vec<usize>>, by_kind: bool) -> Vec<Vec<usize>> {
    if !by_kind {
        return zones;
    }
    zones
        .into_iter()
        .flat_map(|zone| group(&zone, |i| game.encounters[i].kind))
        .collect()
}

/// What a zone tells about one of its species: possible themes, banned replacements, family and lowest level.
/// Themes are keyed by their index in [`TYPE_ORDER`].
#[derive(Debug, Clone)]
struct AreaInfo {
    species: DexNumber,
    possible_themes: BTreeMap<usize, usize>,
    banned_for_replacement: SpeciesSet,
    family: SpeciesSet,
    lowest_level: u32,
}

impl AreaInfo {
    fn new(species: DexNumber) -> Self {
        Self {
            species,
            possible_themes: BTreeMap::new(),
            banned_for_replacement: SpeciesSet::new(),
            family: SpeciesSet::new(),
            lowest_level: 100,
        }
    }
}

/// One run of the wild encounter randomizer: its pools and what earlier picks left.
struct Inner<'a> {
    game: &'a GameData,
    s: &'a WildSettings,
    rng: &'a mut StepRng,
    cancel: &'a CancelToken,
    sets: Vec<EncounterSet>,
    needs_types: bool,
    use_mapping: bool,
    /// Only species that could have evolved by an encounter's level. Zones keep the first pick for a species, checked
    /// at the level it was first met at.
    no_premature_evolutions: bool,
    allowed: SpeciesSet,
    allowed_by_type: BTreeMap<Type, SpeciesSet>,
    banned: SpeciesSet,
    remaining: SpeciesSet,
    remaining_by_type: BTreeMap<Type, SpeciesSet>,
    zone_map: HashMap<DexNumber, DexNumber>,
    area_info: Option<BTreeMap<DexNumber, AreaInfo>>,
}

impl<'a> Inner<'a> {
    fn new(
        game: &'a GameData,
        s: &'a WildSettings,
        allowed: SpeciesSet,
        banned: SpeciesSet,
        rng: &'a mut StepRng,
        cancel: &'a CancelToken,
    ) -> Result<Self, RandomizeError> {
        let allowed: SpeciesSet = if s.evolution == WildEvolution::BasicOnly && !s.keep_relations {
            allowed
                .into_iter()
                .filter(|&n| game.pre_evolved_species(n).is_empty())
                .collect()
        } else {
            allowed
        };
        if allowed.is_empty() {
            return Err(impossible("no Pokémon match the wild Pokémon restrictions"));
        }
        let needs_types = s.types != WildTypes::None || s.keep_themes;
        let mut allowed_by_type = BTreeMap::new();
        if needs_types {
            for &t in &game.types {
                let of_type: SpeciesSet = allowed.iter().copied().filter(|&n| has_type(game, n, t)).collect();
                allowed_by_type.insert(t, of_type);
            }
        }
        let mut inner = Self {
            game,
            s,
            rng,
            cancel,
            sets: game.encounters.clone(),
            needs_types,
            use_mapping: false,
            no_premature_evolutions: false,
            allowed,
            allowed_by_type,
            banned,
            remaining: SpeciesSet::new(),
            remaining_by_type: BTreeMap::new(),
            zone_map: HashMap::new(),
            area_info: None,
        };
        inner.refill_remaining();
        Ok(inner)
    }

    fn refill_remaining(&mut self) {
        self.remaining.clone_from(&self.allowed);
        if self.needs_types {
            self.remaining_by_type.clone_from(&self.allowed_by_type);
        }
    }

    fn randomize_zones(&mut self, mut zones: Vec<Vec<usize>>) -> Result<(), RandomizeError> {
        // Shuffled, or large zones would tend to come first.
        shuffle(self.rng, &mut zones);
        for zone in zones {
            if self.cancel.is_cancelled() {
                return Err(RandomizeError::Cancelled);
            }
            let zone_type = self.pick_zone_type(&zone);
            if self.use_mapping {
                self.zone_map.clear();
                self.setup_area_info(&zone);
                if self.s.keep_relations {
                    self.spread_themes_through_families();
                }
            }
            for &area in &zone {
                self.randomize_area(area, zone_type)?;
            }
            if self.use_mapping && !self.s.catch_em_all {
                self.refill_remaining();
            }
        }
        Ok(())
    }

    fn randomize_area(&mut self, area: usize, zone_type: Option<Type>) -> Result<(), RandomizeError> {
        for slot in 0..self.sets[area].encounters.len() {
            let current = self.game.encounters[area].encounters[slot].species;
            let mapped = if self.use_mapping {
                self.zone_map.get(&current).copied()
            } else {
                None
            };
            let replacement = if let Some(mapped) = mapped {
                mapped
            } else {
                let replacement = if self.s.keep_relations && self.map_has_family_member(current) {
                    self.pick_family_member_replacement(current)?
                } else {
                    let level = self.game.encounters[area].encounters[slot].min_level;
                    let allowed = self.allowed_for_replacement(current, area, zone_type, level)?;
                    self.pick_replacement(current, &allowed)
                };
                if self.use_mapping {
                    self.zone_map.insert(current, replacement);
                }
                if self.use_mapping || self.s.catch_em_all {
                    self.remove_from_remaining(replacement);
                }
                replacement
            };
            let forme = common::forme_for(self.rng, self.game, replacement, self.game.formes.wild);
            let e = &mut self.sets[area].encounters[slot];
            e.species = replacement;
            e.forme = forme;
        }
        Ok(())
    }

    fn info(&self, species: DexNumber) -> Option<&AreaInfo> {
        self.area_info.as_ref()?.get(&species)
    }

    fn map_has_family_member(&self, current: DexNumber) -> bool {
        self.info(current)
            .is_some_and(|info| info.family.iter().any(|r| self.zone_map.contains_key(r)))
    }

    fn pick_family_member_replacement(&mut self, to_replace: DexNumber) -> Result<DexNumber, RandomizeError> {
        let info = self
            .info(to_replace)
            .cloned()
            .ok_or_else(|| impossible("no area information"))?;
        let relative = info
            .family
            .iter()
            .copied()
            .find(|r| self.zone_map.contains_key(r))
            .ok_or_else(|| impossible("no replaced relative"))?;
        let relation = self.game.relation(relative, to_replace).unwrap_or(0);
        let relative_replacement = self.zone_map[&relative];
        let at_position = self.game.relatives_at_position(relative_replacement, relation);
        for from_remaining in [true, false] {
            let pool = if from_remaining { &self.remaining } else { &self.allowed };
            let possible: SpeciesSet = at_position
                .iter()
                .copied()
                .filter(|n| pool.contains(n) && !info.banned_for_replacement.contains(n))
                .collect();
            if !possible.is_empty() {
                return Ok(self.pick_replacement(to_replace, &possible));
            }
        }
        Err(impossible(format!(
            "no relative of #{relative_replacement} can stand in for #{to_replace}"
        )))
    }

    /// The species that may replace `current` in `area`; an error when there are none.
    fn allowed_for_replacement(
        &self,
        current: DexNumber,
        area: usize,
        zone_type: Option<Type>,
        level: u8,
    ) -> Result<SpeciesSet, RandomizeError> {
        let allowed = match self.info(current) {
            Some(info) => self.allowed_using_info(info, zone_type, level),
            None => self.allowed_without_info(current, area, zone_type, level),
        };
        if allowed.is_empty() {
            return Err(impossible(format!(
                "could not find a wild Pokémon to replace #{current} in {}",
                self.game.encounters[area].name
            )));
        }
        Ok(allowed)
    }

    fn pool(&self, remaining: bool, of_type: Option<Type>) -> SpeciesSet {
        match (remaining, of_type) {
            (true, None) => self.remaining.clone(),
            (true, Some(t)) => self.remaining_by_type.get(&t).cloned().unwrap_or_default(),
            (false, None) => self.allowed.clone(),
            (false, Some(t)) => self.allowed_by_type.get(&t).cloned().unwrap_or_default(),
        }
    }

    /// With zone information: from the species not used yet, else from all allowed.
    fn allowed_using_info(&self, info: &AreaInfo, theme: Option<Type>, level: u8) -> SpeciesSet {
        let of_type = theme.or_else(|| self.theme_of(info));
        let set = self.narrow_with_info(info, self.pool(true, of_type), level);
        if set.is_empty() {
            self.narrow_with_info(info, self.pool(false, of_type), level)
        } else {
            set
        }
    }

    fn narrow_with_info(&self, info: &AreaInfo, pool: SpeciesSet, level: u8) -> SpeciesSet {
        let mut set: SpeciesSet = pool
            .into_iter()
            .filter(|n| !info.banned_for_replacement.contains(n))
            .collect();
        if self.s.evolution == WildEvolution::SameStage {
            let stage = self.game.stages_before(info.species);
            set.retain(|&n| self.game.stages_before(n) == stage);
        }
        if self.s.keep_relations {
            set = self.allowed_for_family(set, info);
        }
        self.without_premature(set, level)
    }

    /// With "No Premature Evolutions", only species that could have evolved by `level`
    /// (evolution levels unscaled).
    fn without_premature(&self, mut set: SpeciesSet, level: u8) -> SpeciesSet {
        if self.no_premature_evolutions {
            set.retain(|&n| common::is_legal_evolution_at(self.game, n, level, 1.0));
        }
        set
    }

    /// The species that may replace `current` without zones.
    fn allowed_without_info(&self, current: DexNumber, area: usize, theme: Option<Type>, level: u8) -> SpeciesSet {
        let primary = self.game.species(current).map(|sp| sp.primary_type);
        let of_type = theme.or(if self.s.types == WildTypes::KeepPrimary {
            primary
        } else {
            None
        });
        let banned = &self.game.encounters[area].banned;
        let narrow = |pool: SpeciesSet| {
            let mut set: SpeciesSet = pool.into_iter().filter(|n| !banned.contains(n)).collect();
            if self.s.evolution == WildEvolution::SameStage {
                let stage = self.game.stages_before(current);
                set.retain(|&n| self.game.stages_before(n) == stage);
            }
            self.without_premature(set, level)
        };
        let set = narrow(self.pool(true, of_type));
        if set.is_empty() {
            narrow(self.pool(false, of_type))
        } else {
            set
        }
    }

    /// Species whose line can hold every relative found in the zone, preferring
    /// families not used in the zone yet.
    fn allowed_for_family(&self, pool: SpeciesSet, info: &AreaInfo) -> SpeciesSet {
        let game = self.game;
        let before = stages_within(&info.family, info.species, |n| game.pre_evolved_species(n));
        let after = stages_within(&info.family, info.species, |n| game.evolved_species(n));
        let mut set: SpeciesSet = pool
            .into_iter()
            .filter(|&n| game.stages_before(n) >= before && game.stages_after(n) >= after)
            .collect();
        if self.s.evolution == WildEvolution::BasicOnly {
            set.retain(|&n| game.stages_before(n) <= before);
        }
        for &relative in &info.family {
            let relation = game.relation(info.species, relative).unwrap_or(0);
            let relative_banned = self.info(relative).map(|i| &i.banned_for_replacement);
            set.retain(|&p| {
                game.relatives_at_position_same_branch(p, relation)
                    .iter()
                    .any(|r| self.allowed.contains(r) && relative_banned.is_none_or(|b| !b.contains(r)))
            });
        }
        let used: SpeciesSet = self.zone_map.values().copied().collect();
        let without_used: SpeciesSet = set
            .iter()
            .copied()
            .filter(|&p| game.family(p).is_disjoint(&used))
            .collect();
        if without_used.is_empty() { set } else { without_used }
    }

    /// Picks the replacement for `current` from `allowed`.
    fn pick_replacement(&mut self, current: DexNumber, allowed: &SpeciesSet) -> DexNumber {
        // Catch 'Em All keeps banned species (e.g. Unown) where they are, or they couldn't be caught at all.
        if self.s.catch_em_all && self.banned.contains(&current) {
            return current;
        }
        if self.s.similar_strength {
            let mut bst = self.bst(current);
            // Only with zones: without them there is no area information, so no lowest level.
            if self.s.balance_low_levels
                && let Some(info) = self.info(current)
            {
                bst = bst.min(info.lowest_level * 10 + 250);
            }
            return self.similar_strength(allowed, bst);
        }
        pick(self.rng, allowed).unwrap_or(current)
    }

    fn bst(&self, n: DexNumber) -> u32 {
        common::bst(self.game, n)
    }

    fn similar_strength(&mut self, set: &SpeciesSet, bst: u32) -> DexNumber {
        common::similar_strength(self.rng, self.game, set, bst)
    }

    fn remove_from_remaining(&mut self, replacement: DexNumber) {
        self.remaining.remove(&replacement);
        if self.needs_types {
            for set in self.remaining_by_type.values_mut() {
                set.remove(&replacement);
            }
        }
        if self.remaining.is_empty() {
            self.refill_remaining();
        }
    }

    /// The type theme of a zone, if it gets one.
    fn pick_zone_type(&mut self, zone: &[usize]) -> Option<Type> {
        let mut picked = None;
        if self.s.keep_themes {
            let mut possible: Vec<Type> = TYPE_ORDER.to_vec();
            for &area in zone {
                let shared = shared_types(self.game, &self.game.encounters[area].species());
                possible.retain(|t| shared.contains(t));
                if possible.is_empty() {
                    break;
                }
            }
            if let Some(&first) = possible.first() {
                picked = Some(first);
                if let Some(&second) = possible.get(1) {
                    let preferred = zone
                        .first()
                        .and_then(|&a| self.game.encounters[a].encounters.first())
                        .and_then(|e| self.game.species(e.species))
                        .map(|sp| sp.primary_type);
                    // Prefer anything over Normal, else the first species' primary type.
                    if first == Type::Normal || preferred != Some(first) {
                        picked = Some(second);
                    }
                }
            }
        }
        if picked.is_none() && self.s.types == WildTypes::ZoneThemes {
            picked = self.random_type_with_species_remaining();
            // Unown clause: with Catch 'Em All, banned species stay, so the theme must be one of their types.
            if self.s.catch_em_all {
                let in_zone: Vec<DexNumber> = zone
                    .iter()
                    .flat_map(|&a| self.game.encounters[a].species())
                    .filter(|n| self.banned.contains(n))
                    .collect::<SpeciesSet>()
                    .into_iter()
                    .collect();
                if let Some(t) = shared_type(self.game, &in_zone) {
                    picked = Some(t);
                }
            }
        }
        picked
    }

    fn random_type_with_species_remaining(&mut self) -> Option<Type> {
        let mut types = self.game.types.clone();
        shuffle(self.rng, &mut types);
        types
            .into_iter()
            .find(|t| self.remaining_by_type.get(t).is_some_and(|s| !s.is_empty()))
    }

    /// Gathers the [`AreaInfo`] of every species in the zone.
    fn setup_area_info(&mut self, zone: &[usize]) {
        let game = self.game;
        let mut existing = SpeciesSet::new();
        let mut map: BTreeMap<DexNumber, AreaInfo> = BTreeMap::new();
        for &area in zone {
            let set = &game.encounters[area];
            let species = set.species();
            let area_theme = if self.s.keep_themes {
                shared_type(game, &species)
            } else {
                None
            };
            for &sp in &species {
                if let Entry::Vacant(slot) = map.entry(sp) {
                    slot.insert(AreaInfo::new(sp));
                    if self.s.keep_relations {
                        existing.insert(sp);
                        let family: SpeciesSet = game.family(sp).intersection(&existing).copied().collect();
                        if family.len() > 1 {
                            for relative in &family {
                                if let Some(info) = map.get_mut(relative) {
                                    info.family.extend(&family);
                                }
                            }
                        }
                    }
                }
                if let Some(info) = map.get_mut(&sp) {
                    if let Some(t) = area_theme {
                        *info.possible_themes.entry(type_index(t)).or_default() += species.len();
                    }
                    info.banned_for_replacement.extend(&set.banned);
                }
            }
            if self.s.balance_low_levels {
                for e in &set.encounters {
                    if let Some(info) = map.get_mut(&e.species) {
                        let average = u32::midpoint(u32::from(e.min_level), u32::from(e.max_level));
                        info.lowest_level = info.lowest_level.min(average);
                    }
                }
            }
        }
        self.area_info = Some(map);
    }

    /// Every member of a family gets the themes of the whole family.
    fn spread_themes_through_families(&mut self) {
        let Some(map) = self.area_info.as_mut() else {
            return;
        };
        let mut completed = SpeciesSet::new();
        let species: Vec<DexNumber> = map.keys().copied().collect();
        for sp in species {
            if completed.contains(&sp) {
                continue;
            }
            let family = map[&sp].family.clone();
            completed.extend(&family);
            let mut themes: BTreeMap<usize, usize> = BTreeMap::new();
            for relative in &family {
                for (&t, &count) in &map[relative].possible_themes {
                    *themes.entry(t).or_default() += count;
                }
            }
            for relative in &family {
                if let Some(info) = map.get_mut(relative) {
                    info.possible_themes.clone_from(&themes);
                }
            }
        }
    }

    /// The theme with the most species, the original
    /// primary type on a tie; without themes, the primary type with Keep Primary Type, else none.
    fn theme_of(&self, info: &AreaInfo) -> Option<Type> {
        let primary = self.game.species(info.species).map(|sp| sp.primary_type);
        if info.possible_themes.is_empty() {
            return if self.s.types == WildTypes::KeepPrimary {
                primary
            } else {
                None
            };
        }
        let mut best: Option<(Type, usize)> = None;
        for (&t, &count) in &info.possible_themes {
            let t = TYPE_ORDER[t];
            match best {
                None => best = Some((t, count)),
                Some((_, best_count)) if count > best_count => best = Some((t, count)),
                Some((_, best_count)) if count == best_count && Some(t) == primary => best = Some((t, count)),
                Some(_) => {}
            }
        }
        best.map(|(t, _)| t)
    }
}

fn type_index(t: Type) -> usize {
    TYPE_ORDER.iter().position(|&u| u == t).unwrap_or_default()
}

fn has_type(game: &GameData, n: DexNumber, t: Type) -> bool {
    game.species(n).is_some_and(|sp| sp.has_type(t))
}

/// The types every species has, in [`TYPE_ORDER`].
fn shared_types(game: &GameData, species: &[DexNumber]) -> Vec<Type> {
    if species.is_empty() {
        return Vec::new();
    }
    TYPE_ORDER
        .into_iter()
        .filter(|&t| species.iter().all(|&n| has_type(game, n, t)))
        .collect()
}

/// The type all species share, preferring the first one's primary type, and
/// anything over Normal.
fn shared_type(game: &GameData, species: &[DexNumber]) -> Option<Type> {
    let first = game.species(*species.first()?)?;
    let mut primary = Some(first.primary_type);
    let mut secondary = first.secondary_type;
    for &n in &species[1..] {
        if secondary.is_some_and(|t| !has_type(game, n, t)) {
            secondary = None;
        }
        if primary.is_some_and(|t| !has_type(game, n, t)) {
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

/// Evolution stages from `species`, only through members of `set`.
fn stages_within(set: &SpeciesSet, species: DexNumber, next: impl Fn(DexNumber) -> SpeciesSet) -> usize {
    let mut stages = 0;
    let mut checked = SpeciesSet::new();
    let mut current = SpeciesSet::from([species]);
    while !current.is_empty() {
        let mut following = SpeciesSet::new();
        for n in current {
            if checked.insert(n) {
                following.extend(next(n).into_iter().filter(|m| set.contains(m)));
            }
        }
        if !following.is_empty() {
            stages += 1;
        }
        current = following;
    }
    stages
}

#[cfg(test)]
mod tests;
