//! Evolutions, and an estimate of the level at which evolutions that don't happen at a level would.

use serde::{Deserialize, Serialize};

use crate::{DexNumber, GameData};

/// What makes a species evolve, the same in every generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[allow(missing_docs, reason = "the names say what they are")]
pub enum EvolutionType {
    // Gen 1+
    Level,
    Stone,
    Trade,
    // Gen 2+
    TradeItem,
    Happiness,
    HappinessDay,
    HappinessNight,
    /// Tyrogue, with the next two.
    LevelAttackHigher,
    LevelDefenseHigher,
    LevelAtkDefSame,
    // Gen 3+
    /// Wurmple, with the next.
    LevelLowPv,
    LevelHighPv,
    /// Nincada, with the next.
    LevelCreateExtra,
    LevelIsExtra,
    /// Feebas.
    HighBeauty,
    // Gen 4+
    StoneMaleOnly,
    StoneFemaleOnly,
    ItemDay,
    ItemNight,
    WithMove,
    /// Mantyke.
    WithOther,
    LevelMaleOnly,
    LevelFemaleOnly,
    MagneticField,
    MossRock,
    IceRock,
    // Gen 5+
    /// Karrablast and Shelmet.
    TradeSpecial,
    // Gen 6+
    /// Eevee into Sylveon.
    FairyAffection,
    /// Pancham.
    LevelWithDark,
    /// Inkay.
    LevelUpsideDown,
    /// Sliggoo.
    LevelRain,
    LevelDay,
    LevelNight,
    // Gen 7+
    /// Cosmoem, with the next; the latter can only happen in another game.
    LevelGameThis,
    LevelGameOther,
    /// Rockruff, with the next three.
    LevelGameThisDay,
    LevelGameThisNight,
    LevelGameOtherDay,
    LevelGameOtherNight,
    /// Crabrawler.
    Snowy,
    /// Rockruff with Own Tempo.
    LevelDusk,
    /// Cubone into Alolan Marowak, in Ultra Space.
    LevelUltra,
    /// Pikachu and Exeggcute into their Alolan forms, in Ultra Space.
    StoneUltra,
    /// In no game.
    Item,
    /// Not an evolution the game stores: links a species to the alternate formes its evolution can take (Burmy to
    /// Wormadam's Sandy and Trash Cloaks), so those count as its split evolutions.
    None,
}

use EvolutionType as E;

impl EvolutionType {
    /// The evolution happens on reaching a level, the parameter.
    pub fn uses_level_threshold(self) -> bool {
        matches!(
            self,
            E::Level
                | E::LevelAttackHigher
                | E::LevelDefenseHigher
                | E::LevelAtkDefSame
                | E::LevelLowPv
                | E::LevelHighPv
                | E::LevelCreateExtra
                | E::LevelIsExtra
                | E::LevelMaleOnly
                | E::LevelFemaleOnly
                | E::LevelWithDark
                | E::LevelUpsideDown
                | E::LevelRain
                | E::LevelDay
                | E::LevelNight
                | E::LevelGameThis
                | E::LevelGameOther
                | E::LevelGameThisDay
                | E::LevelGameOtherDay
                | E::LevelGameThisNight
                | E::LevelGameOtherNight
                | E::LevelDusk
                | E::LevelUltra
        )
    }

    /// The parameter is an item.
    pub fn uses_item(self) -> bool {
        matches!(
            self,
            E::Stone
                | E::TradeItem
                | E::StoneMaleOnly
                | E::StoneFemaleOnly
                | E::ItemDay
                | E::ItemNight
                | E::StoneUltra
                | E::Item
        )
    }

    /// The same evolution at any time of day; `None` for evolutions that don't depend on it.
    pub fn timeless(self) -> Option<Self> {
        match self {
            E::HappinessDay | E::HappinessNight => Some(E::Happiness),
            E::ItemDay | E::ItemNight => Some(E::Item),
            E::LevelDay | E::LevelNight | E::LevelDusk => Some(E::Level),
            E::LevelGameThisDay | E::LevelGameThisNight => Some(E::LevelGameThis),
            E::LevelGameOtherDay | E::LevelGameOtherNight => Some(E::LevelGameOther),
            _ => None,
        }
    }

    /// The evolution depends on the time of day.
    pub fn uses_time(self) -> bool {
        self.timeless().is_some()
    }

    /// The day version of an evolution with a night twin.
    pub fn is_day_type(self) -> bool {
        matches!(
            self,
            E::HappinessDay | E::ItemDay | E::LevelDay | E::LevelGameThisDay | E::LevelGameOtherDay
        )
    }

    /// The twin at the other time of day; `None` without one, as for dusk.
    pub fn opposite_time(self) -> Option<Self> {
        match self {
            E::HappinessDay => Some(E::HappinessNight),
            E::HappinessNight => Some(E::HappinessDay),
            E::ItemDay => Some(E::ItemNight),
            E::ItemNight => Some(E::ItemDay),
            E::LevelDay => Some(E::LevelNight),
            E::LevelNight => Some(E::LevelDay),
            E::LevelGameThisDay => Some(E::LevelGameThisNight),
            E::LevelGameThisNight => Some(E::LevelGameThisDay),
            E::LevelGameOtherDay => Some(E::LevelGameOtherNight),
            E::LevelGameOtherNight => Some(E::LevelGameOtherDay),
            _ => None,
        }
    }
}

/// One evolution edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Evolution {
    /// Species that evolves.
    pub from: DexNumber,
    /// Species it evolves into.
    pub to: DexNumber,
    /// What makes it evolve.
    pub kind: EvolutionType,
    /// The level, item (a standard [`crate::ItemId`]), move, species … the kind needs.
    pub parameter: u16,
    /// The level, for evolutions at a level; otherwise an estimate of a fair level.
    pub estimated_level: u16,
}

impl Evolution {
    /// An evolution; its estimated level is its level, or 0 until [`GameData::estimate_evolution_levels`].
    pub fn new(from: DexNumber, to: DexNumber, kind: EvolutionType, parameter: u16) -> Self {
        Self {
            from,
            to,
            kind,
            parameter,
            estimated_level: if kind.uses_level_threshold() { parameter } else { 0 },
        }
    }

    /// A new kind and parameter, keeping the estimated level equal to the level of evolutions at a level. One that
    /// stops being at a level keeps its estimate.
    pub fn update_method(&mut self, kind: EvolutionType, parameter: u16) {
        if kind.uses_level_threshold() && (!self.kind.uses_level_threshold() || parameter != self.estimated_level) {
            self.estimated_level = parameter;
        }
        self.kind = kind;
        self.parameter = parameter;
    }

    /// [`Evolution::update_method`], except that with `use_estimated` an evolution at a level becomes one at the
    /// estimated level instead of at `parameter`.
    pub fn update_method_estimated(&mut self, kind: EvolutionType, parameter: u16, use_estimated: bool) {
        let parameter = if use_estimated && kind.uses_level_threshold() {
            self.estimated_level
        } else {
            parameter
        };
        self.update_method(kind, parameter);
    }
}

/// `factor * level`, rounded up to a whole level.
fn ceil_times(factor: f64, level: u16) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small positive levels"
    )]
    let out = (factor * f64::from(level)).ceil() as u16;
    out
}

/// `x` rounded to a whole level, halves up.
fn round_half_up(x: f64) -> u16 {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "small positive levels"
    )]
    let out = (x + 0.5).floor() as u16;
    out
}

/// The estimated level for an evolution between totals `pre` and `post`: the levels of evolutions at a level, scaled to
/// the target's totals and weighted by how close their totals are.
fn find_evolution_level(samples: &[[u16; 3]], pre: u16, post: u16) -> u16 {
    const PRE_FACTOR: f64 = 1.5;
    const POST_FACTOR: f64 = 3.0;
    let (pre, post) = (f64::from(pre), f64::from(post));
    let mut weighted_sum = 0.0;
    let mut weight_sum = 0.0;
    for &[sample_pre, sample_post, level] in samples {
        let (sample_pre, sample_post) = (f64::from(sample_pre), f64::from(sample_post));
        let (dx, dy) = (pre - sample_pre, post - sample_post);
        let dist = (dx * dx + dy * dy).sqrt();
        let weight = if dist == 0.0 { 1.0 } else { 1.0 / dist };
        let scaled_pre = (pre / sample_pre).powf(PRE_FACTOR);
        let scaled_post = (post / sample_post).powf(POST_FACTOR);
        let adjusted = f64::from(level) * f64::midpoint(scaled_pre, scaled_post);
        weighted_sum += adjusted * weight;
        weight_sum += weight;
    }
    round_half_up(weighted_sum / weight_sum)
}

impl GameData {
    /// The highest (estimated) evolution level.
    pub fn highest_evolution_level(&self) -> u16 {
        self.evolutions
            .iter()
            .map(|e| e.estimated_level)
            .max()
            .unwrap_or_default()
    }

    /// The samples of the game as it is: each evolution at a level with the totals before and after it.
    fn current_level_samples(&self) -> Vec<[u16; 3]> {
        let bst = |n| self.species(n).map_or(0, crate::Species::bst);
        self.evolutions
            .iter()
            .filter(|e| e.kind.uses_level_threshold())
            .map(|e| [bst(e.from), bst(e.to), e.parameter])
            .collect()
    }

    /// Estimates every evolution's level: evolutions at a level are estimated at it; the others at a level fitting
    /// their totals, judged by `samples`. Handlers call it on loading.
    pub fn estimate_evolution_levels(&mut self) {
        for e in &mut self.evolutions {
            if e.kind.uses_level_threshold() {
                e.estimated_level = e.parameter;
            }
        }
        self.evolution_level_samples = self.current_level_samples();
        let targets: Vec<usize> = (0..self.evolutions.len())
            .filter(|&i| !self.evolutions[i].kind.uses_level_threshold())
            .collect();
        self.estimate_levels(&targets);
    }

    /// Every evolution estimated anew from the current totals, and evolutions at a level happen at their estimate.
    pub fn adjust_evolution_levels(&mut self) {
        let all: Vec<usize> = (0..self.evolutions.len()).collect();
        self.estimate_levels(&all);
        for e in &mut self.evolutions {
            if e.kind.uses_level_threshold() {
                e.update_method_estimated(e.kind, e.parameter, true);
            }
        }
    }

    /// Estimates the levels of the evolutions at `targets`, from the samples taken on loading.
    fn estimate_levels(&mut self, targets: &[usize]) {
        let samples = std::mem::take(&mut self.evolution_level_samples);
        self.estimate_levels_from(&samples, targets);
        self.evolution_level_samples = samples;
    }

    fn estimate_levels_from(&mut self, samples: &[[u16; 3]], targets: &[usize]) {
        if samples.is_empty() {
            return;
        }
        let bst = |g: &Self, n| g.species(n).map_or(0, crate::Species::bst);
        for &i in targets {
            let e = self.evolutions[i];
            self.evolutions[i].estimated_level = find_evolution_level(samples, bst(self, e.from), bst(self, e.to));
        }
        // At least 25% above the evolution before, and at most 80% of the ones after.
        for &i in targets {
            let e = self.evolutions[i];
            let mut level = e.estimated_level;
            if let Some(previous) = self.evolutions.iter().find(|p| p.to == e.from) {
                level = level.max(ceil_times(1.25, previous.estimated_level));
            }
            for next in self.evolutions.iter().filter(|n| n.from == e.to) {
                level = level.min(ceil_times(0.8, next.estimated_level));
            }
            self.evolutions[i].estimated_level = level;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updating_keeps_the_estimate_of_level_evolutions_in_step() {
        let mut e = Evolution::new(1, 2, E::Trade, 0);
        e.estimated_level = 33;
        e.update_method_estimated(E::Level, 37, true);
        assert_eq!((e.kind, e.parameter, e.estimated_level), (E::Level, 33, 33));
        e.update_method(E::Level, 30);
        assert_eq!(e.estimated_level, 30);
        e.update_method(E::Stone, 84);
        assert_eq!((e.parameter, e.estimated_level), (84, 30), "keeps the estimate");
        e.update_method_estimated(E::Stone, 81, true);
        assert_eq!(e.parameter, 81, "items aren't levels");
    }

    #[test]
    fn time_pairs() {
        assert_eq!(E::HappinessDay.opposite_time(), Some(E::HappinessNight));
        assert_eq!(E::LevelDusk.timeless(), Some(E::Level));
        assert!(E::LevelDusk.opposite_time().is_none());
        assert!(E::ItemDay.is_day_type() && !E::ItemNight.is_day_type());
        assert!(!E::Level.uses_time());
    }

    #[test]
    fn a_sample_with_the_same_totals_gives_its_level() {
        assert_eq!(find_evolution_level(&[[300, 400, 20]], 300, 400), 20);
        // Twice the totals: 20 * (2^1.5 + 2^3) / 2 = 108.28…
        assert_eq!(find_evolution_level(&[[150, 200, 20]], 300, 400), 108);
    }
}
