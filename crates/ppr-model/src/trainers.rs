//! The trainers the player battles and their teams.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DexNumber, ItemId, MoveId};

/// How a trainer battles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum BattleStyle {
    /// One Pokémon on each side.
    #[default]
    Single,
    /// Two Pokémon on each side.
    Double,
    /// Three Pokémon on each side (Gen 5 and 6).
    Triple,
    /// Three Pokémon that rotate (Gen 5 and 6).
    Rotation,
}

impl BattleStyle {
    /// How many Pokémon a trainer needs to battle this way.
    pub fn required_pokemon(self) -> usize {
        match self {
            Self::Single => 1,
            Self::Double => 2,
            Self::Triple | Self::Rotation => 3,
        }
    }
}

/// Whether a trainer battles alongside another.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum MultiBattleStatus {
    /// Always alone.
    #[default]
    Never,
    /// Together with another trainer when both see the player at once.
    Potential,
    /// Always together with another trainer.
    Always,
}

/// One Pokémon of a trainer's team.
#[expect(clippy::struct_excessive_bools, reason = "independent flags of a trainer's Pokémon")]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerPokemon {
    /// Its species.
    pub species: DexNumber,
    /// The forme the game stores with it: the forme number of an alternate forme species, or a cosmetic forme of
    /// a base species (Shellos's East Sea); 0 otherwise.
    #[serde(default)]
    pub forme: u8,
    /// Its level.
    pub level: u8,
    /// Its IVs, 0–31, the same for every stat.
    pub ivs: u8,
    /// The item it holds.
    pub held_item: Option<ItemId>,
    /// Its moves, 0 for none; all 0 when it knows what its level-up moves teach by its level.
    pub moves: [MoveId; 4],
    /// Its moves are to be those its level-up moves teach by its level, as they are when the game is saved.
    pub reset_moves: bool,
    /// Which of its species' abilities it has, from 1.
    pub ability_slot: u8,
    /// Added to the team by the randomizer; not saved in the game.
    #[serde(default)]
    pub added: bool,
    /// It is shiny (Gen 7, which stores the flag with its IVs).
    #[serde(default)]
    pub shiny: bool,
    /// Added as a copy of a team member holding a Mega Stone; it holds nothing, and random held items leave it so
    /// while Mega Evolvables are swapped.
    #[serde(default)]
    pub added_from_mega_stone: bool,
}

impl TrainerPokemon {
    /// A Pokémon of `species` at `level` with no IVs, item or moves of its own.
    pub fn new(species: DexNumber, level: u8) -> Self {
        Self {
            species,
            forme: 0,
            level,
            ivs: 0,
            held_item: None,
            moves: [0; 4],
            reset_moves: false,
            ability_slot: 1,
            added: false,
            shiny: false,
            added_from_mega_stone: false,
        }
    }
}

/// A trainer battle: who, how and with which team. A character battled more than once has a trainer for each
/// battle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trainer {
    /// The game's number of this trainer, from 1.
    pub index: u16,
    /// Its name.
    pub name: String,
    /// The number of its trainer class.
    pub class: u16,
    /// Its tag: `GYM1`, `GYM1-LEADER`, `ELITE1`, `CHAMPION`, `UBER`, `RIVAL1-0`, `THEMED:WALLY-STRONG`, `NOTSTRONG` and
    /// the like.
    pub tag: Option<String>,
    /// Whether it battles alongside another trainer.
    pub multi_battle: MultiBattleStatus,
    /// How it battles.
    pub battle_style: BattleStyle,
    /// Its battle style is set by the randomizer, so the game must be told.
    pub forced_battle_style: bool,
    /// Where in its team the rival's starter must stay when rivals carry the starter (Gen 1,
    /// whose rival teams follow the player's choice).
    #[serde(default)]
    pub force_starter_position: Option<u8>,
    /// Its team.
    pub pokemon: Vec<TrainerPokemon>,
}

impl Trainer {
    /// A single-battle trainer with no tag, of a class numbered below 256.
    pub fn new(index: u16, name: impl Into<String>, class: u8, pokemon: Vec<TrainerPokemon>) -> Self {
        Self {
            index,
            name: name.into(),
            class: class.into(),
            tag: None,
            multi_battle: MultiBattleStatus::Never,
            battle_style: BattleStyle::Single,
            forced_battle_style: false,
            force_starter_position: None,
            pokemon,
        }
    }

    fn tag(&self) -> &str {
        self.tag.as_deref().unwrap_or_default()
    }

    /// Elite Four, Champion, post-game bosses and leaders.
    pub fn is_boss(&self) -> bool {
        let tag = self.tag();
        ["ELITE", "CHAMPION", "UBER"].iter().any(|p| tag.starts_with(p)) || tag.ends_with("LEADER")
    }

    /// Rivals, friends and strong trainers such as evil team admins.
    pub fn is_important(&self) -> bool {
        let tag = self.tag();
        tag.starts_with("RIVAL") || tag.starts_with("FRIEND") || tag.ends_with("STRONG")
    }

    /// Neither a boss nor important.
    pub fn is_regular(&self) -> bool {
        !self.is_boss() && !self.is_important()
    }

    /// The first battle against the rival or friend (`RIVAL1-…`, `FRIEND1-…`).
    pub fn is_first_rival_or_friend(&self) -> bool {
        let tag = self.tag();
        tag.starts_with("RIVAL1-") || tag.starts_with("FRIEND1-")
    }

    /// Gets no buffs: the first rival battle and trainers tagged `NOTSTRONG`.
    pub fn should_not_get_buffs(&self) -> bool {
        self.is_first_rival_or_friend() || self.tag().ends_with("NOTSTRONG")
    }

    /// A Pokémon of the team holds an item.
    pub fn pokemon_have_items(&self) -> bool {
        self.pokemon.iter().any(|p| p.held_item.is_some())
    }

    /// A Pokémon has moves of its own.
    pub fn pokemon_have_custom_moves(&self) -> bool {
        self.pokemon
            .iter()
            .any(|p| !p.reset_moves && p.moves.iter().any(|&m| m != 0))
    }

    /// The team's data layout in the game: 1 for custom moves plus 2 for held items.
    pub fn poke_type(&self) -> u8 {
        u8::from(self.pokemon_have_custom_moves()) + 2 * u8::from(self.pokemon_have_items())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_tell_bosses_and_important_trainers() {
        let mut t = Trainer::new(1, "A", 0, vec![TrainerPokemon::new(1, 5)]);
        assert!(t.is_regular());
        for (tag, boss, important, no_buffs) in [
            ("GYM1-LEADER", true, false, false),
            ("ELITE2", true, false, false),
            ("RIVAL1-0", false, true, true),
            ("RIVAL2-0", false, true, false),
            ("THEMED:WALLY-STRONG", false, true, false),
            ("NOTSTRONG", false, true, true),
            ("GYM3", false, false, false),
        ] {
            t.tag = Some(tag.into());
            assert_eq!(
                (t.is_boss(), t.is_important(), t.should_not_get_buffs()),
                (boss, important, no_buffs),
                "{tag}"
            );
        }
    }

    #[test]
    fn poke_type_counts_moves_and_items() {
        let mut t = Trainer::new(1, "A", 0, vec![TrainerPokemon::new(1, 5)]);
        assert_eq!(t.poke_type(), 0);
        t.pokemon[0].moves[2] = 33;
        assert_eq!(t.poke_type(), 1);
        t.pokemon[0].held_item = Some(1);
        assert_eq!(t.poke_type(), 3);
        t.pokemon[0].reset_moves = true;
        assert_eq!(t.poke_type(), 2);
        assert_eq!(BattleStyle::Rotation.required_pokemon(), 3);
    }
}
