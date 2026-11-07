use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Generation;

/// The physical types: before Gen 4 split them by move, a move's type makes it physical or special.
pub const PHYSICAL_TYPES: [Type; 9] = [
    Type::Normal,
    Type::Fighting,
    Type::Poison,
    Type::Ground,
    Type::Flying,
    Type::Bug,
    Type::Rock,
    Type::Ghost,
    Type::Steel,
];

/// Elemental type of a species or move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
#[allow(missing_docs)]
pub enum Type {
    Normal,
    Fighting,
    Flying,
    Poison,
    Ground,
    Rock,
    Bug,
    Ghost,
    Steel,
    Fire,
    Water,
    Grass,
    Electric,
    Psychic,
    Ice,
    Dragon,
    Dark,
    Fairy,
}

impl Type {
    /// Every type, in index order.
    pub const ALL: [Self; 18] = [
        Self::Normal,
        Self::Fighting,
        Self::Flying,
        Self::Poison,
        Self::Ground,
        Self::Rock,
        Self::Bug,
        Self::Ghost,
        Self::Steel,
        Self::Fire,
        Self::Water,
        Self::Grass,
        Self::Electric,
        Self::Psychic,
        Self::Ice,
        Self::Dragon,
        Self::Dark,
        Self::Fairy,
    ];

    /// The types that exist in a generation: Steel and Dark from Gen 2, Fairy from Gen 6.
    pub fn in_generation(generation: Generation) -> Vec<Self> {
        let n = generation.number();
        Self::ALL
            .into_iter()
            .filter(|t| match t {
                Self::Steel | Self::Dark => n >= 2,
                Self::Fairy => n >= 6,
                _ => true,
            })
            .collect()
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

/// A name that is not one of the 18 types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownType(pub String);

impl fmt::Display for UnknownType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown type {:?}", self.0)
    }
}

impl std::error::Error for UnknownType {}

impl FromStr for Type {
    type Err = UnknownType;

    /// Parses the English name, ignoring case: `"fire"` and `"Fire"` are both [`Type::Fire`].
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|t| t.to_string().eq_ignore_ascii_case(s))
            .ok_or_else(|| UnknownType(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_counts_per_generation() {
        let count = |n| Type::in_generation(Generation::new(n).expect("valid")).len();
        assert_eq!((count(1), count(2), count(5), count(6)), (15, 17, 17, 18));
    }

    #[test]
    fn names_round_trip_ignoring_case() {
        for t in Type::ALL {
            assert_eq!(t.to_string().to_lowercase().parse(), Ok(t));
        }
        assert_eq!("Shadow".parse::<Type>(), Err(UnknownType("Shadow".into())));
    }
}
