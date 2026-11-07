use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// How static Pokémon are replaced.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum StaticMode {
    /// Keep the game's static Pokémon.
    #[default]
    Unchanged,
    /// Legendaries become legendaries, the others non-legendaries.
    RandomMatching,
    /// Any species.
    Random,
    /// A species of similar base stat total.
    SimilarStrength,
}

/// Static Pokémon settings.
#[expect(clippy::struct_excessive_bools, reason = "one flag per option of the UI")]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StaticSettings {
    /// How they are replaced.
    pub mode: StaticMode,
    /// With similar strength, the legendaries the story makes the player face are replaced by species no
    /// stronger than them.
    pub limit_main_game_legendaries: bool,
    /// With similar strength, statics with a base stat total of 600 or more are replaced by any species
    /// ("Randomize 600+ BST").
    pub randomize_600_bst: bool,
    /// Their special battle music follows replaced legendaries, where the game
    /// supports it.
    pub fix_music: bool,
    /// Change their levels by this percentage, -100 to 155. Applies without randomizing too.
    pub level_modifier: i16,
    /// Replacements may be alternate formes, where static Pokémon have a forme.
    #[serde(default)]
    pub allow_alt_formes: bool,
    /// Static Pokémon that hold their Mega Stone become species that can Mega Evolve, holding a Mega Stone of
    /// theirs (Gen 6).
    #[serde(default)]
    pub swap_mega_evos: bool,
}

impl StaticSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        if !(-100..=155).contains(&self.level_modifier) {
            return Err(InvalidSettings("the static level modifier is -100% to 155%"));
        }
        if self.limit_main_game_legendaries && self.mode != StaticMode::SimilarStrength {
            return Err(InvalidSettings("limiting main-game legendaries needs similar strength"));
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
        s.statics.limit_main_game_legendaries = true;
        assert!(s.validate().is_err());
        s.statics.mode = StaticMode::SimilarStrength;
        assert!(s.validate().is_ok());
        s.statics.level_modifier = -101;
        assert!(s.validate().is_err());
    }
}
