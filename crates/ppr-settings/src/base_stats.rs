use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// How base stats are randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum BaseStatMode {
    /// Keep the game's base stats.
    #[default]
    Unchanged,
    /// Shuffle each species' stats among themselves; the total stays.
    Shuffle,
    /// Spread each species' total over its stats at random.
    Random,
}

/// Base statistics settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BaseStatSettings {
    /// How the stats are randomized.
    pub mode: BaseStatMode,
    /// Evolutions follow their pre-evolution's stat order or spread.
    pub follow_evolutions: bool,
    /// With [`Self::follow_evolutions`]: Mega Evolutions follow their base species' stat order or spread like
    /// evolutions.
    #[serde(default)]
    pub follow_megas: bool,
    /// With [`BaseStatMode::Random`] and [`Self::follow_evolutions`] or [`Self::follow_megas`]: spread the stats an
    /// evolution adds at random instead of in its pre-evolution's proportions.
    pub random_evolution_stats: bool,
    /// Give species the base stats of this later generation (6–9) first, where it changed them.
    pub update_to_generation: Option<u8>,
}

impl BaseStatSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        if self.random_evolution_stats
            && !(self.mode == BaseStatMode::Random && (self.follow_evolutions || self.follow_megas))
        {
            return Err(InvalidSettings(
                "randomizing added stats on evolution needs random base stats that follow evolutions or Mega Evolutions",
            ));
        }
        if self.update_to_generation.is_some_and(|g| !(6..=9).contains(&g)) {
            return Err(InvalidSettings("base stats are updated to generation 6 to 9"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;

    #[test]
    fn random_evolution_stats_need_random_followed_stats() {
        let mut s = Settings::default();
        s.base_stats.random_evolution_stats = true;
        assert!(s.validate().is_err());
        s.base_stats.mode = BaseStatMode::Random;
        assert!(s.validate().is_err());
        s.base_stats.follow_evolutions = true;
        assert!(s.validate().is_ok());
        // As in the original, following Mega Evolutions alone allows it too.
        s.base_stats.follow_evolutions = false;
        s.base_stats.follow_megas = true;
        assert!(s.validate().is_ok());
    }

    #[test]
    fn updates_go_to_generation_6_to_9() {
        let mut s = Settings::default();
        for (g, ok) in [(5, false), (6, true), (9, true), (10, false)] {
            s.base_stats.update_to_generation = Some(g);
            assert_eq!(s.validate().is_ok(), ok, "{g}");
        }
    }
}
