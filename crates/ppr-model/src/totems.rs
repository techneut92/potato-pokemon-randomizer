//! Totem Pokémon (Gen 7): the boosted Pokémon at the end of each trial, with the allies they call and the aura
//! that raises their stats.

use serde::{Deserialize, Serialize};

use crate::{DexNumber, ItemId};

/// The stat an aura raises.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum AuraStat {
    /// No aura.
    None,
    /// Attack.
    Attack,
    /// Defense.
    Defense,
    /// Special Attack.
    SpecialAttack,
    /// Special Defense.
    SpecialDefense,
    /// Speed.
    Speed,
    /// Every stat.
    All,
}

impl AuraStat {
    /// The stats a single-stat aura can raise, in the game's order.
    pub const SINGLE: [Self; 5] = [
        Self::Attack,
        Self::Defense,
        Self::SpecialAttack,
        Self::SpecialDefense,
        Self::Speed,
    ];
    const ALL_IN_ORDER: [Self; 7] = [
        Self::None,
        Self::Attack,
        Self::Defense,
        Self::SpecialAttack,
        Self::SpecialDefense,
        Self::Speed,
        Self::All,
    ];

    fn index(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Attack => 1,
            Self::Defense => 2,
            Self::SpecialAttack => 3,
            Self::SpecialDefense => 4,
            Self::Speed => 5,
            Self::All => 6,
        }
    }
}

/// The aura of a Totem Pokémon: one stat (or every stat) raised by 1 to 3 stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Aura {
    /// The stat raised.
    pub stat: AuraStat,
    /// By how many stages, 1 to 3; 0 without an aura.
    pub stages: u8,
}

impl Aura {
    /// No aura.
    pub const NONE: Self = Self {
        stat: AuraStat::None,
        stages: 0,
    };

    /// The aura of the game's aura byte: 0 for none, then three stage counts per stat, from Attack +1 (1) to every
    /// stat +3 (18). A byte past 18 reads as no aura.
    pub fn from_byte(b: u8) -> Self {
        if b == 0 {
            return Self::NONE;
        }
        let stat = AuraStat::ALL_IN_ORDER.get(usize::from((b - 1) / 3 + 1)).copied();
        match stat {
            Some(stat) => Self {
                stat,
                stages: (b - 1) % 3 + 1,
            },
            None => Self::NONE,
        }
    }

    /// The game's aura byte.
    pub fn to_byte(self) -> u8 {
        if self.stat == AuraStat::None {
            0
        } else {
            (self.stat.index() - 1) * 3 + self.stages
        }
    }
}

impl std::fmt::Display for Aura {
    /// "None", "Attack +2", "Special Defense +1", "All +1".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self.stat {
            AuraStat::None => return f.write_str("None"),
            AuraStat::Attack => "Attack",
            AuraStat::Defense => "Defense",
            AuraStat::SpecialAttack => "Special Attack",
            AuraStat::SpecialDefense => "Special Defense",
            AuraStat::Speed => "Speed",
            AuraStat::All => "All",
        };
        write!(f, "{name} +{}", self.stages)
    }
}

/// A Pokémon a Totem Pokémon calls to its side.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotemAlly {
    /// The species: an alternate forme's own number for a forme with data of its own.
    pub species: DexNumber,
    /// The forme the game stores with it: the forme number of an alternate forme species, or a cosmetic forme of a
    /// base species; 0 otherwise.
    #[serde(default)]
    pub forme: u8,
    /// Its level.
    pub level: u8,
    /// Its held item.
    pub held_item: Option<ItemId>,
}

/// A Totem Pokémon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TotemPokemon {
    /// The species: an alternate forme's own number for a forme with data of its own (the Totem formes).
    pub species: DexNumber,
    /// The forme the game stores with it, as [`TotemAlly::forme`].
    #[serde(default)]
    pub forme: u8,
    /// Its level.
    pub level: u8,
    /// Its held item.
    pub held_item: Option<ItemId>,
    /// Its aura.
    pub aura: Aura,
    /// The allies it calls, in the game's order.
    pub allies: Vec<TotemAlly>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aura_bytes_round_trip() {
        for b in 0..=18 {
            assert_eq!(Aura::from_byte(b).to_byte(), b);
        }
        assert_eq!(Aura::from_byte(19), Aura::NONE);
        assert_eq!(
            Aura::from_byte(5),
            Aura {
                stat: AuraStat::Defense,
                stages: 2
            }
        );
        assert_eq!(
            Aura::from_byte(16),
            Aura {
                stat: AuraStat::All,
                stages: 1
            }
        );
    }

    #[test]
    fn auras_read_like_the_log() {
        assert_eq!(Aura::NONE.to_string(), "None");
        assert_eq!(Aura::from_byte(10).to_string(), "Special Defense +1");
        assert_eq!(Aura::from_byte(18).to_string(), "All +3");
    }
}
