use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InvalidSettings;

/// How base stat totals are randomized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum BstMode {
    /// Keep the game's totals.
    #[default]
    Unchanged,
    /// Raise or lower each total by a random percentage.
    BuffNerf,
    /// Give each species the total of another.
    Shuffle,
    /// Give each species a random total.
    Random,
}

/// Base stat total settings; the stats keep their proportions.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BstSettings {
    /// How the totals are randomized.
    pub mode: BstMode,
    /// With [`BstMode::BuffNerf`]: the largest change, 0–50 percent.
    pub max_buff_nerf_percent: u8,
    /// With [`BstMode::BuffNerf`], evolutions get their pre-evolution's percentage; with [`BstMode::Shuffle`],
    /// evolution lines swap totals with lines as long.
    pub follow_evolutions: bool,
    /// With [`BstMode::Shuffle`]: legendaries swap totals only among themselves.
    pub separate_legendaries: bool,
}

/// The largest buff or nerf, in percent.
const MAX_BUFF_NERF: u8 = 50;

impl BstSettings {
    pub(crate) fn validate(&self) -> Result<(), InvalidSettings> {
        if self.max_buff_nerf_percent > MAX_BUFF_NERF {
            return Err(InvalidSettings("the base stat total buff or nerf is 0% to 50%"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Settings;

    #[test]
    fn buff_nerf_is_at_most_50_percent() {
        let mut s = Settings::default();
        s.bsts.max_buff_nerf_percent = 50;
        assert!(s.validate().is_ok());
        s.bsts.max_buff_nerf_percent = 51;
        assert!(s.validate().is_err());
    }
}
