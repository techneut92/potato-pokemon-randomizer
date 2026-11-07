use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// Whether wild Pokémon are replaced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WildMode {
    /// Keep the game's wild Pokémon.
    #[default]
    Unchanged,
    /// Replace them.
    Random,
}

/// Where a species is replaced by the same species every time.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WildZones {
    /// Nowhere: every slot is chosen on its own.
    Slot,
    /// Within one encounter set, e.g. a route's grass.
    Set,
    /// Within one map, for games that number their maps.
    Map,
    /// Within one named location, e.g. all of Route 101.
    Location,
    /// In the whole game.
    #[default]
    Game,
}

/// Type rule for wild Pokémon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WildTypes {
    /// No rule.
    #[default]
    None,
    /// Each zone gets one random type that all its Pokémon have.
    ZoneThemes,
    /// A replacement has the primary type of the Pokémon it replaces.
    KeepPrimary,
}

/// Evolution rule for wild Pokémon.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum WildEvolution {
    /// No rule.
    #[default]
    None,
    /// Only species that nothing evolves into.
    BasicOnly,
    /// A replacement is at the same evolution stage as the Pokémon it replaces.
    SameStage,
}

/// Wild Pokémon settings: zones, types, evolutions and the other wild Pokémon options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one field per option of the Wild Pokémon section"
)]
pub struct WildSettings {
    /// Whether wild Pokémon are replaced.
    pub mode: WildMode,
    /// Where replacements are consistent.
    pub zones: WildZones,
    /// Zones are split by encounter type (grass, water, rock smash, fishing).
    pub split_by_kind: bool,
    /// Type rule.
    pub types: WildTypes,
    /// A zone whose Pokémon all share a type keeps that type.
    pub keep_themes: bool,
    /// Evolution rule.
    pub evolution: WildEvolution,
    /// Members of one evolution line in a zone are replaced by members of one new line.
    pub keep_relations: bool,
    /// A replacement has a similar base stat total.
    pub similar_strength: bool,
    /// With similar strength: no strong Pokémon at low levels.
    pub balance_low_levels: bool,
    /// Use every species once before any species appears twice.
    pub catch_em_all: bool,
    /// Exclude legendaries.
    pub no_legendaries: bool,
    /// Minimum catch rate level, 1 to 5 (5: every ball catches); 0 leaves catch rates alone. Applies without
    /// randomizing too.
    pub min_catch_rate: u8,
    /// Change wild levels by this percentage, -100 to 155. Applies without randomizing too.
    pub level_modifier: i16,
    /// Randomize the items wild Pokémon hold. Applies without randomizing the Pokémon too.
    #[serde(default)]
    pub random_held_items: bool,
    /// With random held items: no bad items.
    #[serde(default)]
    pub ban_bad_held_items: bool,
    /// Wild encounters by time of day, where the game has them: a set per time of day, each randomized and written
    /// on its own. Without it, the daytime Pokémon are written to every time of day. Applies to the level modifier
    /// without randomizing too.
    #[serde(default)]
    pub time_of_day: bool,
    /// Replacements may be alternate formes, where wild Pokémon have a forme.
    #[serde(default)]
    pub allow_alt_formes: bool,
}

impl WildSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        if self.min_catch_rate > 5 {
            return Err(InvalidSettings("the minimum catch rate level is 1 to 5"));
        }
        if !(-100..=155).contains(&self.level_modifier) {
            return Err(InvalidSettings("the wild level modifier is -100% to 155%"));
        }
        if self.split_by_kind && matches!(self.zones, WildZones::Slot | WildZones::Set) {
            return Err(InvalidSettings(
                "splitting by encounter type needs zones per map, location or the whole game",
            ));
        }
        if self.types == WildTypes::ZoneThemes && self.zones == WildZones::Game && !self.split_by_kind {
            return Err(InvalidSettings("zone themes need more than one zone"));
        }
        if self.keep_relations && self.zones == WildZones::Slot {
            return Err(InvalidSettings("keeping relations needs zones"));
        }
        if self.balance_low_levels && !self.similar_strength {
            return Err(InvalidSettings("balancing low levels needs similar strength"));
        }
        if self.catch_em_all && self.zones == WildZones::Game && !self.split_by_kind {
            return Err(InvalidSettings("catch 'em all needs more than one zone"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;

    #[test]
    fn rejects_combinations_the_ui_forbids() {
        let mut s = Settings::default();
        s.wild.mode = WildMode::Random;
        assert!(s.validate().is_ok());
        s.wild.types = WildTypes::ZoneThemes;
        assert!(s.validate().is_err(), "one zone for the whole game");
        s.wild.split_by_kind = true;
        assert!(s.validate().is_ok());
        s.wild.zones = WildZones::Set;
        assert!(s.validate().is_err(), "sets can't be split");
        s.wild.split_by_kind = false;
        s.wild.balance_low_levels = true;
        assert!(s.validate().is_err());
        s.wild.similar_strength = true;
        assert!(s.validate().is_ok());
        s.wild.level_modifier = 156;
        assert!(s.validate().is_err());
        s.wild.level_modifier = -100;
        s.wild.min_catch_rate = 6;
        assert!(s.validate().is_err());
    }
}
