use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// How much experience a species needs per level.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum ExpCurve {
    /// 1,059,860 points to level 100.
    #[default]
    MediumFast,
    /// 600,000 points to level 100.
    Erratic,
    /// 1,640,000 points to level 100.
    Fluctuating,
    /// 1,059,860 points to level 100, slower early on.
    MediumSlow,
    /// 800,000 points to level 100.
    Fast,
    /// 1,250,000 points to level 100.
    Slow,
}

impl ExpCurve {
    /// The curve stored as a byte of the Gen 3+ base stats (and Gen 1–2's), `None` for others.
    pub fn from_byte(b: u8) -> Option<Self> {
        Some(match b {
            0 => Self::MediumFast,
            1 => Self::Erratic,
            2 => Self::Fluctuating,
            3 => Self::MediumSlow,
            4 => Self::Fast,
            5 => Self::Slow,
            _ => return None,
        })
    }

    /// The byte the curve is stored as, the inverse of [`ExpCurve::from_byte`].
    pub fn to_byte(self) -> u8 {
        match self {
            Self::MediumFast => 0,
            Self::Erratic => 1,
            Self::Fluctuating => 2,
            Self::MediumSlow => 3,
            Self::Fast => 4,
            Self::Slow => 5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_round_trip() {
        for b in 0..=5 {
            assert_eq!(ExpCurve::from_byte(b).map(ExpCurve::to_byte), Some(b));
        }
        assert_eq!(ExpCurve::from_byte(6), None);
    }
}
