//! Misc tweaks: small game-specific patches, by the IDs the settings and the frontend use.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DexNumber, ItemId};

/// One misc tweak. The declaration order is fixed: it is also the order they apply in, after the ones with priority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum MiscTweak {
    /// Black and White's EXP formula.
    #[serde(rename = "bwPatch")]
    BwExpPatch,
    /// X Accuracy no longer makes every move hit.
    #[serde(rename = "nerfXAcc")]
    NerfXAccuracy,
    /// The critical hit rate of later games.
    #[serde(rename = "critRateFix")]
    FixCritRate,
    /// Text appears at once.
    #[serde(rename = "fastestText")]
    FastestText,
    /// Running Shoes work indoors.
    #[serde(rename = "runningShoes")]
    RunningShoesIndoors,
    /// The Potion in the player's PC is a random item.
    #[serde(rename = "pcPotion")]
    RandomizePcPotion,
    /// Pikachu can evolve (Yellow).
    #[serde(rename = "pikachuEvo")]
    AllowPikachuEvolution,
    /// The National Dex from the start.
    #[serde(rename = "nationalDex")]
    NationalDexAtStart,
    /// Eggs hatch after the fewest steps.
    #[serde(rename = "fastEggHatching")]
    FastEggHatching,
    /// Challenge Mode, always (Black 2 and White 2).
    #[serde(rename = "forceChallengeMode")]
    ForceChallengeMode,
    /// Species names in Title Case.
    #[serde(rename = "lowerCaseNames")]
    LowerCaseNames,
    /// The Pokémon of the catching tutorial are random.
    #[serde(rename = "catchingTutorial")]
    RandomizeCatchingTutorial,
    /// The Lucky Egg is never a random item.
    #[serde(rename = "luckyEgg")]
    BanLuckyEgg,
    /// No Lucky Egg as a gift.
    #[serde(rename = "freeLuckyEgg")]
    NoFreeLuckyEgg,
    /// The items only worth money are never random items.
    #[serde(rename = "maniacItems")]
    BanBigManiacItems,
    /// Every wild Pokémon can call allies (Sun and Moon).
    #[serde(rename = "sosBattles")]
    SosBattlesForAll,
    /// Gift and static Pokémon at levels that fit the story.
    #[serde(rename = "balanceStaticLevels")]
    BalanceStaticLevels,
    /// Temporary alternate formes stay after battle.
    #[serde(rename = "retainAltFormes")]
    RetainAltFormes,
    /// Running without the Running Shoes.
    #[serde(rename = "runWithoutRunningShoes")]
    RunWithoutRunningShoes,
    /// HP and EXP bars fill faster.
    #[serde(rename = "fasterHpAndExpBars")]
    FasterHpAndExpBars,
    /// The Distortion World goes faster (Platinum).
    #[serde(rename = "fastDistortionWorld")]
    FastDistortionWorld,
    /// Rotom's appliance formes get their later types.
    #[serde(rename = "updateRotomFormeTyping")]
    UpdateRotomFormeTyping,
    /// No low-HP music.
    #[serde(rename = "disableLowHpMusic")]
    DisableLowHpMusic,
    /// TMs are never used up.
    #[serde(rename = "reusableTMs")]
    ReusableTms,
    /// HM moves can be forgotten.
    #[serde(rename = "forgettableHMs")]
    ForgettableHms,
    /// Defeated Pokémon give no EVs.
    #[serde(rename = "noEVYields")]
    NoEvYields,
}

impl MiscTweak {
    /// Tweaks with priority apply first.
    pub fn has_priority(self) -> bool {
        matches!(self, Self::BanLuckyEgg | Self::BanBigManiacItems)
    }
}

/// Lucky Egg.
pub const LUCKY_EGG: ItemId = 231;

/// The two Pokémon of the catching tutorial: the one caught and the one catching it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatchingTutorial {
    /// The wild Pokémon that is caught.
    pub opponent: DexNumber,
    /// The Pokémon on the catcher's side.
    pub player: DexNumber,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_order_are_fixed() {
        assert_eq!(
            serde_json::to_string(&MiscTweak::ReusableTms).ok().as_deref(),
            Some("\"reusableTMs\"")
        );
        assert!(MiscTweak::FastestText < MiscTweak::NoEvYields);
        assert!(MiscTweak::BanLuckyEgg.has_priority() && !MiscTweak::FastestText.has_priority());
    }
}
