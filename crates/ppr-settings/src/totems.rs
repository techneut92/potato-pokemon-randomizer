use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// How Totem Pokémon, or their allies, are replaced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum TotemMode {
    /// Keep the game's species.
    #[default]
    Unchanged,
    /// Any species.
    Random,
    /// A species of similar base stat total.
    SimilarStrength,
}

/// How the auras of Totem Pokémon change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum AuraMode {
    /// Keep the game's auras.
    #[default]
    Unchanged,
    /// Any stat, or every stat, raised by 1 to 3 stages.
    Random,
    /// Another single stat, raised by as many stages; auras that raise every stat stay.
    SameStrength,
}

/// Totem Pokémon settings (Gen 7).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TotemSettings {
    /// How the Totem Pokémon are replaced.
    pub mode: TotemMode,
    /// How the allies they call are replaced.
    pub ally_mode: TotemMode,
    /// How their auras change.
    pub aura_mode: AuraMode,
    /// Totem Pokémon that hold an item get a random consumable one.
    pub randomize_held_items: bool,
    /// New totems and allies may be alternate formes.
    #[serde(default)]
    pub allow_alt_formes: bool,
    /// Change the levels of Totem Pokémon and their allies by this percentage, -100 to 155.
    pub level_modifier: i16,
}

impl TotemSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        if !(-100..=155).contains(&self.level_modifier) {
            return Err(InvalidSettings("the Totem Pokémon level modifier is -100% to 155%"));
        }
        Ok(())
    }

    /// Whether anything about the Totem Pokémon changes.
    pub fn changes_anything(&self) -> bool {
        self.mode != TotemMode::Unchanged
            || self.ally_mode != TotemMode::Unchanged
            || self.aura_mode != AuraMode::Unchanged
            || self.randomize_held_items
            || self.level_modifier != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Settings;

    #[test]
    fn level_modifier_is_limited() {
        let mut s = Settings::default();
        s.totems.level_modifier = 155;
        assert!(s.validate().is_ok());
        s.totems.level_modifier = 156;
        assert!(s.validate().is_err());
        s.totems.level_modifier = -101;
        assert!(s.validate().is_err());
    }

    #[test]
    fn anything_but_the_defaults_changes_something() {
        let mut s = TotemSettings::default();
        assert!(!s.changes_anything());
        s.aura_mode = AuraMode::SameStrength;
        assert!(s.changes_anything());
        let held = TotemSettings {
            randomize_held_items: true,
            ..TotemSettings::default()
        };
        assert!(held.changes_anything());
    }
}
