use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Pickup item settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PickupItemSettings {
    /// Pickup finds random items, each as likely as the item it replaces.
    pub random: bool,
    /// No bad items.
    pub ban_bad: bool,
}
