use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Pokémon palette settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "one field per option of the Pokémon Palettes group"
)]
pub struct PaletteSettings {
    /// Pokémon get random colors.
    pub random: bool,
    /// The colors follow the species' types.
    pub follow_types: bool,
    /// Evolutions keep the colors of their pre-evolutions.
    pub follow_evolutions: bool,
    /// The shiny palette becomes the original normal one.
    pub shiny_from_normal: bool,
}
