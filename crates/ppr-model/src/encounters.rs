use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::DexNumber;

/// How the player meets a set of wild Pokémon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum EncounterKind {
    /// Walking in grass or caves.
    Walking,
    /// Surfing.
    Surfing,
    /// Interacting with the world, e.g. smashing rocks.
    Interact,
    /// Fishing, with any rod.
    Fishing,
    /// Ambush encounters, e.g. shaking grass.
    Ambush,
    /// Anything else.
    Special,
    /// Present in the game data but never met.
    Unused,
}

/// One encounter slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WildEncounter {
    /// The species.
    pub species: DexNumber,
    /// The forme the game stores with it: the forme number of an alternate forme species, or a cosmetic forme of
    /// a base species (Shellos's East Sea); 0 otherwise.
    #[serde(default)]
    pub forme: u8,
    /// Lowest level.
    pub min_level: u8,
    /// Highest level.
    pub max_level: u8,
}

/// Wild Pokémon met in one place by one action, e.g. surfing on Route 110.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncounterSet {
    /// How they are met.
    pub kind: EncounterKind,
    /// The map, as a game-specific number, when the game has one.
    pub map: Option<u16>,
    /// Display name, e.g. "ROUTE 110 Surfing".
    pub name: String,
    /// Location tag: sets with the same tag form one named location. `None` when untagged.
    pub location: Option<String>,
    /// Only reachable after the main story.
    pub post_game: bool,
    /// For a set whose later slots are only reachable after the main story, the first of those slots (Gen 2
    /// fishing: the Super Rod's); `None` when [`EncounterSet::post_game`] tells for the whole set.
    #[serde(default)]
    pub post_game_from: Option<usize>,
    /// Encounter rate, as the game stores it.
    pub rate: u8,
    /// Species that must not be put here, e.g. battle trappers where the game can't handle them.
    pub banned: Vec<DexNumber>,
    /// The slots, in the game's order.
    pub encounters: Vec<WildEncounter>,
}

impl EncounterSet {
    /// The slots reachable before the end of the main story: none of a post-game set, the first ones of a set
    /// that is partly post-game.
    pub fn main_game_encounters(&self) -> &[WildEncounter] {
        if self.post_game {
            &[]
        } else {
            &self.encounters[..self
                .post_game_from
                .unwrap_or(self.encounters.len())
                .min(self.encounters.len())]
        }
    }

    /// Each species in the set once, in slot order.
    pub fn species(&self) -> Vec<DexNumber> {
        let mut out: Vec<DexNumber> = Vec::new();
        for e in &self.encounters {
            if !out.contains(&e.species) {
                out.push(e.species);
            }
        }
        out
    }
}
