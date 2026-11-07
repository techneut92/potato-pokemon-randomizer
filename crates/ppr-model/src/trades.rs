//! In-game trades: a Pokémon an NPC trades the player.

use serde::{Deserialize, Serialize};

use crate::{DexNumber, ItemId};

/// One in-game trade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InGameTrade {
    /// The species the NPC asks for; `None` when any will do.
    pub requested: Option<DexNumber>,
    /// The species the NPC gives.
    pub given: DexNumber,
    /// The given Pokémon's nickname.
    pub nickname: String,
    /// Its original trainer's name.
    pub ot_name: String,
    /// Its original trainer's ID: the public 16 bits up to Gen 6, all 32 in Gen 7.
    pub ot_id: u32,
    /// Its individual values, HP, Attack, Defense, Speed, Special Attack, Special Defense.
    pub ivs: Vec<u8>,
    /// The item it holds, if any.
    pub held_item: Option<ItemId>,
}
