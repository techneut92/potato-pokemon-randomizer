use std::fmt;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Console a ROM runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum Platform {
    /// Game Boy / Game Boy Color.
    GameBoy,
    /// Game Boy Advance.
    GameBoyAdvance,
    /// Nintendo DS.
    NintendoDs,
    /// Nintendo 3DS.
    Nintendo3ds,
}

impl Platform {
    /// Name as shown to users.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::GameBoy => "Game Boy",
            Self::GameBoyAdvance => "Game Boy Advance",
            Self::NintendoDs => "Nintendo DS",
            Self::Nintendo3ds => "Nintendo 3DS",
        }
    }
}

/// Main-series generation, 1 to 7.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Generation(u8);

impl Generation {
    /// The supported generations, oldest first.
    pub const ALL: [Self; 7] = [Self(1), Self(2), Self(3), Self(4), Self(5), Self(6), Self(7)];

    /// Returns the generation, or `None` outside 1..=7.
    pub const fn new(n: u8) -> Option<Self> {
        if n >= 1 && n <= 7 { Some(Self(n)) } else { None }
    }

    /// The generation number.
    pub const fn number(self) -> u8 {
        self.0
    }

    /// The console the generation's games run on.
    pub const fn platform(self) -> Platform {
        match self.0 {
            1 | 2 => Platform::GameBoy,
            3 => Platform::GameBoyAdvance,
            4 | 5 => Platform::NintendoDs,
            _ => Platform::Nintendo3ds,
        }
    }
}

impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Gen {}", self.0)
    }
}

/// Which game a ROM is, as matched against the supported ROM entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct RomIdentity {
    /// Entry name, e.g. `Platinum (U)`.
    pub name: String,
    /// Game code or product code from the header, e.g. `CPUE`.
    pub code: String,
    /// Header version byte, when the platform has one.
    pub version: Option<u8>,
    /// Generation of the game.
    pub generation: Generation,
    /// Console the game runs on.
    pub platform: Platform,
    /// Game family within the generation, e.g. `Plat` or `HGSS`, as named in the ROM entries.
    pub family: String,
}

impl RomIdentity {
    /// Short description such as `Gen 4 · Nintendo DS`.
    pub fn summary(&self) -> String {
        format!("{} · {}", self.generation, self.platform.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_bounds_and_platforms() {
        assert_eq!(Generation::new(0), None);
        assert_eq!(Generation::new(8), None);
        let platforms: Vec<_> = Generation::ALL.iter().map(|g| g.platform()).collect();
        assert_eq!(
            platforms,
            [
                Platform::GameBoy,
                Platform::GameBoy,
                Platform::GameBoyAdvance,
                Platform::NintendoDs,
                Platform::NintendoDs,
                Platform::Nintendo3ds,
                Platform::Nintendo3ds
            ]
        );
    }
}
