use std::fmt;
use std::str::FromStr;

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The RNG every randomization step uses: specified output, identical on every platform (ADR 0005).
pub type StepRng = ChaCha8Rng;

/// Seed of a randomization run. Shown to users as a decimal number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Seed(pub u64);

impl Seed {
    /// The RNG for one step: the same seed with the step's own stream, so steps never shift each other's
    /// random choices.
    pub fn rng_for(self, stream: u64) -> StepRng {
        let mut rng = ChaCha8Rng::seed_from_u64(self.0);
        rng.set_stream(stream);
        rng
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A seed that is not a decimal number from -2^63 to 2^64 - 1.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a seed is a whole number from -9223372036854775808 to 18446744073709551615")]
pub struct SeedParseError;

impl FromStr for Seed {
    type Err = SeedParseError;

    /// Seeds are 64-bit; a negative one is taken as its 64 bits (two's complement).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        s.parse()
            .or_else(|_| s.parse::<i64>().map(i64::cast_unsigned))
            .map(Self)
            .map_err(|_| SeedParseError)
    }
}

/// A decimal string: a `u64` does not fit a JSON (JavaScript) number exactly.
impl Serialize for Seed {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Seed {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use rand_core::Rng;

    use super::*;

    #[test]
    fn serializes_as_a_decimal_string() {
        let json = serde_json::to_string(&Seed(u64::MAX)).expect("serializes");
        assert_eq!(json, "\"18446744073709551615\"");
        assert_eq!(serde_json::from_str::<Seed>(&json).expect("parses"), Seed(u64::MAX));
        assert!(serde_json::from_str::<Seed>("\"x\"").is_err());
    }

    #[test]
    fn negative_seeds_keep_their_bits() {
        assert_eq!("-1".parse::<Seed>(), Ok(Seed(u64::MAX)));
        assert_eq!("-9223372036854775808".parse::<Seed>(), Ok(Seed(1 << 63)));
        assert!("-9223372036854775809".parse::<Seed>().is_err());
        assert!("18446744073709551616".parse::<Seed>().is_err());
    }

    #[test]
    fn same_seed_and_stream_give_the_same_numbers() {
        let a: Vec<u32> = (0..8)
            .map({
                let mut r = Seed(42).rng_for(3);
                move |_| r.next_u32()
            })
            .collect();
        let b: Vec<u32> = (0..8)
            .map({
                let mut r = Seed(42).rng_for(3);
                move |_| r.next_u32()
            })
            .collect();
        assert_eq!(a, b);
    }

    #[test]
    fn streams_are_independent() {
        assert_ne!(Seed(42).rng_for(1).next_u64(), Seed(42).rng_for(2).next_u64());
    }

    #[test]
    fn output_is_stable_across_versions() {
        // Pinned so an RNG change cannot silently alter existing seeds.
        assert_eq!(Seed(0).rng_for(0).next_u64(), PINNED_FIRST_U64);
    }

    const PINNED_FIRST_U64: u64 = 13_080_132_717_333_068_652;

    proptest! {
        #[test]
        fn display_and_parse_round_trip(n: u64) {
            prop_assert_eq!(Seed(n).to_string().parse::<Seed>(), Ok(Seed(n)));
        }
    }

    #[test]
    fn rejects_non_numbers() {
        assert_eq!("1.5".parse::<Seed>(), Err(SeedParseError));
        assert_eq!("abc".parse::<Seed>(), Err(SeedParseError));
    }
}
