use ppr_model::{DexNumber, Type};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// How starters are chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StarterMode {
    /// Keep the game's starters.
    #[default]
    Unchanged,
    /// Use [`StarterSettings::custom`]; slots set to `None` are chosen randomly.
    Custom,
    /// Any species.
    Random,
    /// Basic species (nothing evolves into them) that evolve twice.
    RandomWithTwoEvolutions,
    /// Any basic species.
    RandomBasic,
}

/// Type rule for random starters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StarterTypes {
    /// No rule.
    #[default]
    None,
    /// One of each of the game's standard triangle (Grass, Fire, Water from Gen 3; Fire, Water, Grass before).
    FireWaterGrass,
    /// Three types that beat each other in a circle.
    TypeTriangle,
    /// No type shared between starters.
    Unique,
    /// All of one type; `None` picks a random type with enough species.
    SingleType(Option<Type>),
}

/// Starter Pokémon settings.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent switches of the UI; not a state machine"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StarterSettings {
    /// How starters are chosen.
    pub mode: StarterMode,
    /// Custom starters by national number; `None` means random. Used with [`StarterMode::Custom`].
    pub custom: Vec<Option<DexNumber>>,
    /// Type rule for the randomly chosen starters.
    pub types: StarterTypes,
    /// Exclude legendaries.
    pub no_legendaries: bool,
    /// Exclude dual-type species.
    pub no_dual_types: bool,
    /// Minimum base stat total; 0 for none.
    pub bst_min: u16,
    /// Maximum base stat total; 0 for none.
    pub bst_max: u16,
    /// Give the starters random held items, whatever the mode.
    pub random_held_items: bool,
    /// Leave out items that are useless to hold.
    pub ban_bad_items: bool,
    /// Starters may be alternate formes, where the game's starters have a forme.
    #[serde(default)]
    pub allow_alt_formes: bool,
}

impl Default for StarterSettings {
    fn default() -> Self {
        Self {
            mode: StarterMode::Unchanged,
            custom: Vec::new(),
            types: StarterTypes::None,
            no_legendaries: false,
            no_dual_types: false,
            bst_min: 0,
            bst_max: 0,
            random_held_items: false,
            ban_bad_items: false,
            allow_alt_formes: false,
        }
    }
}

impl StarterSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        let custom = self.mode == StarterMode::Custom;
        if custom && matches!(self.types, StarterTypes::FireWaterGrass | StarterTypes::TypeTriangle) {
            return Err(InvalidSettings(
                "custom starters cannot use the Fire/Water/Grass or type triangle rule",
            ));
        }
        if !custom && !self.custom.is_empty() {
            return Err(InvalidSettings("custom starters are only used in Custom mode"));
        }
        if self.ban_bad_items && !self.random_held_items {
            return Err(InvalidSettings("banning bad items needs random held items"));
        }
        if self.bst_max != 0 && self.bst_min > self.bst_max {
            return Err(InvalidSettings("the minimum BST is higher than the maximum"));
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
        assert!(s.validate().is_ok());
        s.starters.mode = StarterMode::Custom;
        s.starters.types = StarterTypes::FireWaterGrass;
        assert!(s.validate().is_err());
        s.starters.types = StarterTypes::Unique;
        assert!(s.validate().is_ok());
        s.starters.ban_bad_items = true;
        assert!(s.validate().is_err());
        s.starters.random_held_items = true;
        assert!(s.validate().is_ok());
        s.starters.bst_min = 500;
        s.starters.bst_max = 400;
        assert!(s.validate().is_err());
    }
}
