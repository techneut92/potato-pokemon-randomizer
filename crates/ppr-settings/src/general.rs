use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// General options: rules that apply across the randomizers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GeneralSettings {
    /// The Pokémon of the game's intro is a random one (on unless "No Random Intro Mon" is ticked).
    pub random_intro_pokemon: bool,
    /// Wild and trainer Pokémon are never evolved beyond what their level allows.
    pub no_premature_evolutions: bool,
    /// Only Pokémon of these generations appear; `None` allows all.
    #[serde(default)]
    #[ts(optional)]
    pub limit_pokemon: Option<GenerationLimit>,
    /// Alternate formes that only last in battle or under some condition (Castform's weather formes, Darmanitan's
    /// Zen Mode, Kyurem's fused formes) are never chosen.
    #[serde(default)]
    pub no_irregular_alt_formes: bool,
}

/// The generations whose Pokémon may appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct GenerationLimit {
    /// Whether generation `i + 1` is allowed. Generations past the game's own are ignored.
    pub generations: [bool; 7],
    /// The evolutionary relatives of allowed Pokémon are allowed too, whatever their generation.
    pub allow_evolutionary_relatives: bool,
}

impl Default for GenerationLimit {
    /// Every generation, with relatives.
    fn default() -> Self {
        Self {
            generations: [true; 7],
            allow_evolutionary_relatives: true,
        }
    }
}

impl GenerationLimit {
    /// Whether Pokémon of `generation` (1–7) are allowed.
    pub fn allows(&self, generation: u8) -> bool {
        usize::from(generation)
            .checked_sub(1)
            .and_then(|i| self.generations.get(i))
            .copied()
            .unwrap_or(false)
    }
}

impl GeneralSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        if self.limit_pokemon.is_some_and(|l| !l.generations.contains(&true)) {
            return Err(InvalidSettings("Limit Pokémon needs at least one generation"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_limit_needs_a_generation() {
        let mut s = GeneralSettings {
            limit_pokemon: Some(GenerationLimit::default()),
            ..GeneralSettings::default()
        };
        assert!(s.validate().is_ok());
        s.limit_pokemon = Some(GenerationLimit {
            generations: [false; 7],
            allow_evolutionary_relatives: true,
        });
        assert!(s.validate().is_err());
        let limit = GenerationLimit {
            generations: [true, false, true, false, false, false, false],
            allow_evolutionary_relatives: false,
        };
        assert!(limit.allows(1) && !limit.allows(2) && limit.allows(3) && !limit.allows(0) && !limit.allows(8));
    }
}
