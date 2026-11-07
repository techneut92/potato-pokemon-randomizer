//! Whole stat points from a base stat total and ratios.
//!
//! Stats are in spreading order here: HP, Attack, Defense, Sp. Atk, Sp. Def, Speed. Shedinja has no HP to spread;
//! its HP stays 1 and its five other stats are spread instead. Gen 1 has five stats, in the order HP, Attack,
//! Defense, Speed, Special; its Special is kept in `sp_attack`, with `sp_defense` 0.
//!
//! The ratios are not kept between steps: a later step starts from the whole stats, so a step that
//! sets a total spreads it by those rather than by the ratios an earlier step chose.

use ppr_model::{BaseStats, DexNumber, Species};

/// Highest value a stat can have.
pub(crate) const STAT_MAX: u32 = 255;

/// Shedinja's national number.
pub(crate) const SHEDINJA: DexNumber = 292;

/// The stats in spreading order: HP, Attack, Defense, Sp. Atk, Sp. Def, Speed.
pub(crate) fn stat_order(s: BaseStats) -> [u8; 6] {
    [s.hp, s.attack, s.defense, s.sp_attack, s.sp_defense, s.speed]
}

/// A species' stats in spreading order: [`stat_order`], or Gen 1's five (`gen1`).
pub(crate) fn stats_of(s: BaseStats, gen1: bool) -> Vec<u8> {
    if gen1 {
        vec![s.hp, s.attack, s.defense, s.speed, s.sp_attack]
    } else {
        stat_order(s).to_vec()
    }
}

/// Stats from spreading order.
pub(crate) fn from_stat_order([hp, attack, defense, sp_attack, sp_defense, speed]: [u8; 6]) -> BaseStats {
    BaseStats {
        hp,
        attack,
        defense,
        speed,
        sp_attack,
        sp_defense,
    }
}

/// `bst` points spread over the stats in proportion to `ratios`, one point at a time to
/// the stat furthest below its share, none to stats of ratio 0 and none past [`STAT_MAX`]. `None` when the points
/// don't fit.
pub(crate) fn spread(bst: u32, ratios: &[f64]) -> Option<Vec<u32>> {
    let total: f64 = ratios.iter().sum();
    if total == 0.0 {
        return (bst == 0).then(|| vec![0; ratios.len()]);
    }
    let raw: Vec<f64> = ratios.iter().map(|r| f64::from(bst) * (r / total)).collect();
    let mut stats = vec![0u32; ratios.len()];
    let mut queue = TieOrderedHeap::default();
    // "+ 0.000001 ensures higher raw stats get priority before any points have been handed out."
    let neediness = |stats: &[u32], i: usize| (f64::from(stats[i]) - raw[i] + 0.000_001) / raw[i];
    for i in (0..raw.len()).filter(|&i| raw[i] != 0.0) {
        queue.add(i, |a, b| neediness(&stats, a).total_cmp(&neediness(&stats, b)));
    }
    for _ in 0..bst {
        let i = queue.poll(|a, b| neediness(&stats, a).total_cmp(&neediness(&stats, b)))?;
        stats[i] += 1;
        if stats[i] < STAT_MAX {
            queue.add(i, |a, b| neediness(&stats, a).total_cmp(&neediness(&stats, b)));
        }
    }
    Some(stats)
}

/// A binary min-heap with a fixed sift order, so ties are always broken in the same, known order.
#[derive(Default)]
struct TieOrderedHeap {
    items: Vec<usize>,
}

impl TieOrderedHeap {
    /// Adds `x` at the end and sifts it up.
    fn add(&mut self, x: usize, cmp: impl Fn(usize, usize) -> std::cmp::Ordering) {
        let mut k = self.items.len();
        self.items.push(x);
        while k > 0 {
            let parent = (k - 1) / 2;
            let e = self.items[parent];
            if cmp(x, e).is_ge() {
                break;
            }
            self.items[k] = e;
            k = parent;
        }
        self.items[k] = x;
    }

    /// Takes the head, then sifts the last item down from the root.
    fn poll(&mut self, cmp: impl Fn(usize, usize) -> std::cmp::Ordering) -> Option<usize> {
        let last = self.items.pop()?;
        let n = self.items.len();
        if n == 0 {
            return Some(last);
        }
        let head = self.items[0];
        let mut k = 0;
        while k < n / 2 {
            let mut child = 2 * k + 1;
            let right = child + 1;
            if right < n && cmp(self.items[child], self.items[right]).is_gt() {
                child = right;
            }
            if cmp(last, self.items[child]).is_le() {
                break;
            }
            self.items[k] = self.items[child];
            k = child;
        }
        self.items[k] = last;
        Some(head)
    }
}

/// The total a species' stats are spread from: Shedinja's leaves out its HP.
pub(crate) fn spread_total(sp: &Species) -> u32 {
    let total = u32::from(sp.stats.total());
    if sp.number == SHEDINJA {
        total - u32::from(sp.stats.hp)
    } else {
        total
    }
}

/// The species keeps its total, spread by `ratios` (in the order of [`stats_of`]; Shedinja's without HP). `false`, leaving the stats as they were, when the total doesn't fit.
pub(crate) fn set_ratios(sp: &mut Species, ratios: &[f64], gen1: bool) -> bool {
    set_total_and_ratios(sp, spread_total(sp), ratios, gen1)
}

/// Sets the total, spread by `ratios` (in the order of [`stats_of`]; Shedinja's without HP). `false`, leaving the
/// stats as they were, when the total doesn't fit.
pub(crate) fn set_total_and_ratios(sp: &mut Species, bst: u32, ratios: &[f64], gen1: bool) -> bool {
    let Some(points) = spread(bst, ratios) else {
        return false;
    };
    let byte = |v: u32| u8::try_from(v).unwrap_or(u8::MAX);
    let mut stats: Vec<u8> = points.into_iter().map(byte).collect();
    if gen1 {
        let Ok([hp, attack, defense, speed, special]) = <[u8; 5]>::try_from(stats) else {
            return false;
        };
        sp.stats = BaseStats {
            hp,
            attack,
            defense,
            speed,
            sp_attack: special,
            sp_defense: 0,
        };
        return true;
    }
    if sp.number == SHEDINJA {
        stats.insert(0, sp.stats.hp);
    }
    let Ok(stats) = <[u8; 6]>::try_from(stats) else {
        return false;
    };
    sp.stats = from_stat_order(stats);
    true
}

/// The ratios a species' current stats have (in the order of [`stats_of`]; Shedinja's without HP).
pub(crate) fn current_ratios(sp: &Species, gen1: bool) -> Vec<f64> {
    let stats = stats_of(sp.stats, gen1);
    let skip = usize::from(sp.number == SHEDINJA && !gen1);
    stats[skip..].iter().map(|&s| f64::from(s)).collect()
}

/// Highest total a species can have: five stats' worth in Gen 1, else six.
pub(crate) fn max_bst(gen1: bool) -> u32 {
    STAT_MAX * if gen1 { 5 } else { 6 }
}

/// Sets the total, the stats keeping their proportions. Shedinja's total is the one for power levels
/// ([`Species::bst`]), of which its five stats get 5/6. `false`, leaving the stats as they were, when the total
/// doesn't fit.
pub(crate) fn set_bst(sp: &mut Species, bst: u32, gen1: bool) -> bool {
    let ratios = current_ratios(sp, gen1);
    let total = if sp.number == SHEDINJA && !gen1 {
        bst * 5 / 6
    } else {
        bst
    };
    set_total_and_ratios(sp, total, &ratios, gen1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spread_keeps_whole_stats() {
        let stats = [45.0, 49.0, 49.0, 65.0, 65.0, 45.0];
        assert_eq!(spread(318, &stats), Some(vec![45, 49, 49, 65, 65, 45]));
    }

    #[test]
    fn spread_caps_stats_and_skips_zero_ratios() {
        // Blissey with a higher total puts no more into HP.
        let blissey = [255.0, 10.0, 10.0, 75.0, 135.0, 55.0];
        let stats = spread(640, &blissey).expect("fits");
        assert_eq!(stats[0], 255);
        assert_eq!(stats.iter().sum::<u32>(), 640);
        assert_eq!(spread(10, &[0.0, 1.0, 0.0]), Some(vec![0, 10, 0]));
        assert_eq!(spread(300, &[0.0, 1.0, 0.0]), None, "255 at most");
        assert_eq!(spread(5, &[0.0, 0.0]), None);
        assert_eq!(spread(0, &[0.0, 0.0]), Some(vec![0, 0]));
    }

    #[test]
    fn spread_matches_known_results() {
        // Known-good spreads, ties included.
        let bulbasaur = [45.0, 49.0, 49.0, 65.0, 65.0, 45.0];
        assert_eq!(spread(400, &bulbasaur), Some(vec![57, 62, 62, 81, 81, 57]));
        assert_eq!(spread(7, &[1.0; 6]), Some(vec![1, 1, 1, 1, 2, 1]));
        assert_eq!(spread(11, &[1.0; 6]), Some(vec![2, 2, 2, 1, 2, 2]));
        let blissey = [255.0, 10.0, 10.0, 75.0, 135.0, 55.0];
        assert_eq!(spread(640, &blissey), Some(vec![255, 14, 14, 101, 182, 74]));
        assert_eq!(
            spread(318, &[0.3, 0.7, 0.11, 0.5, 0.25, 0.9]),
            Some(vec![35, 80, 13, 58, 29, 103])
        );
    }

    #[test]
    fn shedinja_keeps_its_hp() {
        let mut sp = Species {
            number: SHEDINJA,
            name: "SHEDINJA".into(),
            primary_type: ppr_model::Type::Bug,
            secondary_type: Some(ppr_model::Type::Ghost),
            stats: from_stat_order([1, 90, 45, 30, 30, 40]),
            catch_rate: 45,
            abilities: Vec::new(),
            growth_curve: ppr_model::ExpCurve::MediumFast,
            forme: ppr_model::Forme::default(),
        };
        assert_eq!(current_ratios(&sp, false), [90.0, 45.0, 30.0, 30.0, 40.0]);
        assert!(set_ratios(&mut sp, &[0.3, 0.7, 0.11, 0.5, 0.25], false));
        assert_eq!(stat_order(sp.stats), [1, 38, 88, 14, 63, 32], "Shedinja's HP stays 1");
        assert_eq!(sp.bst(), 282);
        assert!(set_bst(&mut sp, 300, false));
        assert_eq!(sp.stats.hp, 1);
        assert_eq!(spread_total(&sp), 250);
    }

    #[test]
    fn set_bst_keeps_proportions() {
        let mut sp = Species {
            number: 1,
            name: "BULBASAUR".into(),
            primary_type: ppr_model::Type::Grass,
            secondary_type: Some(ppr_model::Type::Poison),
            stats: from_stat_order([45, 49, 49, 65, 65, 45]),
            catch_rate: 45,
            abilities: Vec::new(),
            growth_curve: ppr_model::ExpCurve::MediumFast,
            forme: ppr_model::Forme::default(),
        };
        assert!(set_bst(&mut sp, 400, false));
        assert_eq!(stat_order(sp.stats), [57, 62, 62, 81, 81, 57], "spread in proportion");
        assert!(!set_bst(&mut sp, max_bst(false) + 1, false));
    }

    #[test]
    fn gen1_has_five_stats() {
        let mut sp = Species {
            number: 1,
            name: "BULBASAUR".into(),
            primary_type: ppr_model::Type::Grass,
            secondary_type: Some(ppr_model::Type::Poison),
            stats: BaseStats {
                hp: 45,
                attack: 49,
                defense: 49,
                speed: 45,
                sp_attack: 65,
                sp_defense: 0,
            },
            catch_rate: 45,
            abilities: Vec::new(),
            growth_curve: ppr_model::ExpCurve::MediumSlow,
            forme: ppr_model::Forme::default(),
        };
        assert_eq!(current_ratios(&sp, true), [45.0, 49.0, 49.0, 45.0, 65.0]);
        assert!(set_bst(&mut sp, 400, true));
        assert_eq!(stats_of(sp.stats, true), [71, 78, 77, 71, 103], "Special is one stat");
        assert_eq!((sp.stats.sp_defense, sp.bst()), (0, 400));
        assert_eq!(max_bst(true), 1275);
        assert!(!set_bst(&mut sp, 1276, true));
    }
}
