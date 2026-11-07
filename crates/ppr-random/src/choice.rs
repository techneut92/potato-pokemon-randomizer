//! Unbiased random choices on top of the step RNG.

use std::collections::BTreeSet;

use rand_core::Rng;

/// A uniform index in `0..n`, without modulo bias. `n` must be positive.
pub(crate) fn below(rng: &mut impl Rng, n: usize) -> usize {
    assert!(n > 0, "cannot choose from an empty range");
    let n = n as u64;
    // Values below 2^64 mod n would make the low results slightly more likely; reject them.
    let threshold = n.wrapping_neg() % n;
    loop {
        let x = rng.next_u64();
        if x >= threshold {
            return usize::try_from(x % n).unwrap_or_default();
        }
    }
}

/// A uniformly chosen member, or `None` when the set is empty.
pub(crate) fn pick<T: Copy + Ord>(rng: &mut impl Rng, set: &BTreeSet<T>) -> Option<T> {
    (!set.is_empty())
        .then(|| set.iter().nth(below(rng, set.len())).copied())
        .flatten()
}

/// Removes and returns a uniformly chosen member.
pub(crate) fn take<T: Copy + Ord>(rng: &mut impl Rng, set: &mut BTreeSet<T>) -> Option<T> {
    let chosen = pick(rng, set)?;
    set.remove(&chosen);
    Some(chosen)
}

/// A uniform number in `0.0..1.0`, from 53 random bits.
pub(crate) fn unit(rng: &mut impl Rng) -> f64 {
    /// 2^-53.
    const SCALE: f64 = 1.0 / 9_007_199_254_740_992.0;
    #[expect(clippy::cast_precision_loss, reason = "53 bits fit an f64 exactly")]
    let bits = (rng.next_u64() >> 11) as f64;
    bits * SCALE
}

/// Fisher–Yates shuffle.
pub(crate) fn shuffle<T>(rng: &mut impl Rng, items: &mut [T]) {
    for i in (1..items.len()).rev() {
        items.swap(i, below(rng, i + 1));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Seed;

    #[test]
    fn below_covers_the_range_evenly() {
        let mut rng = Seed(1).rng_for(0);
        let mut counts = [0u32; 3];
        for _ in 0..30_000 {
            counts[below(&mut rng, 3)] += 1;
        }
        assert!(counts.iter().all(|&c| (9_000..11_000).contains(&c)), "{counts:?}");
    }

    #[test]
    fn take_empties_a_set_without_repeats() {
        let mut rng = Seed(2).rng_for(0);
        let mut set: BTreeSet<u16> = (1..=10).collect();
        let mut seen = BTreeSet::new();
        while let Some(x) = take(&mut rng, &mut set) {
            assert!(seen.insert(x));
        }
        assert_eq!(seen.len(), 10);
        assert_eq!(pick(&mut rng, &set), None);
    }

    #[test]
    fn shuffle_is_a_permutation() {
        let mut rng = Seed(3).rng_for(0);
        let mut v: Vec<u8> = (0..20).collect();
        shuffle(&mut rng, &mut v);
        let mut sorted = v.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn unit_stays_below_one() {
        let mut rng = Seed(4).rng_for(0);
        let values: Vec<f64> = (0..1000).map(|_| unit(&mut rng)).collect();
        assert!(values.iter().all(|v| (0.0..1.0).contains(v)));
        let mean = values.iter().sum::<f64>() / 1000.0;
        assert!((0.45..0.55).contains(&mean), "{mean}");
    }
}
