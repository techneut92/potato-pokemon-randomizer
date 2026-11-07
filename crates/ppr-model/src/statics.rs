//! Static encounters: Pokémon met once, in a fixed place, at a fixed level.

use serde::{Deserialize, Serialize};

use crate::{DexNumber, ItemId};

/// One static encounter: a legendary, a gift, a fossil, an egg or an overworld Pokémon.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticEncounter {
    /// The species met.
    pub species: DexNumber,
    /// The forme the game stores with it: the forme number of an alternate forme species, or a cosmetic forme of
    /// a base species (Shellos's East Sea); 0 otherwise.
    #[serde(default)]
    pub forme: u8,
    /// Its level; 1 for eggs, which have none.
    pub level: u8,
    /// An egg, which hatches at level 5 whatever its level says.
    pub egg: bool,
    /// The item it holds, where the game data has static encounters' held items (Gen 6 and 7); `None` for none.
    #[serde(default)]
    pub held_item: Option<ItemId>,
    /// Levels of encounters that are always the same species as this one, such as Emerald's roaming Latios and Latias,
    /// who are the ones met on Southern Island.
    pub linked_levels: Vec<u8>,
    /// The only species a replacement may be, where the game limits it (the hidden hollows of Black 2 and White 2);
    /// `None` for any species.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_species: Option<Vec<DexNumber>>,
}

impl StaticEncounter {
    /// A plain encounter of `species` at `level`.
    pub fn new(species: DexNumber, level: u8) -> Self {
        Self {
            species,
            forme: 0,
            level,
            egg: false,
            held_item: None,
            linked_levels: Vec::new(),
            allowed_species: None,
        }
    }
}
