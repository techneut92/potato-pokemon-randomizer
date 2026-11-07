//! The limits of one game for manual edits, as its ROM handler reports them (ADR 0008).
//!
//! [`Capabilities`](crate::Capabilities) says which features a game has, one flag each; these rules carry the
//! numbers and lists the editor needs on top: how big a shop or team may get, how long a nickname may be, which
//! items the game needs where they are. [`EditList::validate`](super::EditList::validate) checks an edit list
//! against them, and the frontend uses them to offer only what fits.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{DexNumber, GameData, Generation, ItemId, MultiBattleStatus, PriceFormat, Trainer};

/// What one game allows manual edits to do, computed by its ROM handler from the loaded game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EditRules {
    /// What shops can hold.
    pub shops: ShopRules,
    /// What item prices the game can store.
    pub prices: PriceRules,
    /// What trainer teams can hold.
    pub trainers: TrainerRules,
    /// How wild encounter levels are stored, and which species may not appear where.
    pub wild: WildRules,
    /// Which fields in-game trades have, and how long their texts may be.
    pub trades: TradeRules,
    /// Field items the game needs where they are.
    pub field_items: FieldItemRules,
    /// Trainer Pokémon the game sets from a starter, so they change with the starter, not with the trainer.
    pub starter_links: Vec<StarterLink>,
}

impl EditRules {
    /// The rules as far as the loaded game's data tells them; a handler then sets what only it knows, like how
    /// shops are stored or which wild sets keep one level per slot.
    ///
    /// - Shops can grow without a limit but free space.
    /// - Trainers: teams of up to 6, 3 for one that always battles together with another; a team grows, and its
    ///   Pokémon get held items or moves of their own, where the game data allows it for the trainer's kind (boss,
    ///   important, regular) or the team already has them.
    /// - Wild sets keep one level per slot before Gen 3 and a range from Gen 3 on.
    /// - Trades have an original trainer, IVs and a held item from Gen 2 on, and texts that can change.
    /// - The field items the game needs are its TMs only found in the field.
    pub fn from_game(data: &GameData) -> Self {
        let generation = data.identity.generation.number();
        Self {
            shops: ShopRules {
                size: ShopSize::Grow { max: None },
            },
            prices: PriceRules::of(data.identity.generation),
            trainers: TrainerRules::from_game(data),
            wild: WildRules {
                levels: vec![
                    if generation <= 2 {
                        WildLevels::Single
                    } else {
                        WildLevels::Range
                    };
                    data.encounters.len()
                ],
                banned: data.wild_banned.clone(),
                banned_in_sets: data
                    .encounters
                    .iter()
                    .enumerate()
                    .filter(|(_, set)| !set.banned.is_empty())
                    .map(|(set, s)| WildSetBan {
                        set,
                        species: s.banned.clone(),
                    })
                    .collect(),
            },
            trades: TradeRules {
                texts: true,
                max_nickname_length: data.max_trade_nickname_length,
                max_ot_name_length: if generation >= 2 {
                    data.max_trade_ot_name_length
                } else {
                    0
                },
                original_trainer: generation >= 2,
                max_ot_id: max_trade_ot_id(generation),
                held_item: generation >= 2,
                ivs: data.trades.first().map_or(0, |t| t.ivs.len()),
                max_iv: if generation <= 2 { 15 } else { 31 },
                random_iv: random_trade_iv(generation),
            },
            field_items: FieldItemRules {
                required: data.required_field_tms.clone(),
            },
            starter_links: Vec::new(),
        }
    }

    /// The limits of trainer `index`.
    pub fn trainer(&self, index: u16) -> Option<&TrainerLimits> {
        self.trainers.trainers.iter().find(|t| t.index == index)
    }

    /// Sets every wild set the handler lists to `levels`, by place.
    pub fn set_wild_levels(&mut self, levels: impl IntoIterator<Item = (usize, WildLevels)>) {
        for (set, l) in levels {
            if let Some(slot) = self.wild.levels.get_mut(set) {
                *slot = l;
            }
        }
    }
}

/// How shops can change size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
#[ts(export)]
pub enum ShopSize {
    /// Every shop keeps as many items as it has; items can only be swapped.
    Fixed,
    /// A shop can lose items but not get more than it has: the game has no free space to move a shop to.
    Shrink,
    /// Each shop is a list of its own that can grow, up to `max` items; `None` when only the free space in the ROM
    /// limits it.
    Grow {
        /// The most items one shop can hold.
        max: Option<usize>,
    },
    /// The shops share one area of `bytes` bytes. Shop `i` takes `costs[i].base + costs[i].per_item × items`.
    Shared {
        /// The size of the area.
        bytes: usize,
        /// What each shop takes, by shop.
        costs: Vec<ShopCost>,
        /// Only shops that sell something else than in the loaded game take room in the area; the others stay where
        /// they are. Otherwise every shop does.
        only_changed: bool,
        /// The most items one shop can hold.
        max: usize,
    },
}

/// What one shop takes of a shared area: `base + per_item × items` bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShopCost {
    /// Bytes the shop takes whatever it sells (its terminator).
    pub base: usize,
    /// Bytes each item takes.
    pub per_item: usize,
}

impl ShopCost {
    /// The bytes a shop of `items` items takes.
    pub fn of(self, items: usize) -> usize {
        self.base + self.per_item * items
    }
}

/// What shops can hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ShopRules {
    /// How shops can change size.
    pub size: ShopSize,
}

/// What item prices the game can store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PriceRules {
    /// Most items.
    pub items: PriceFormat,
    /// TMs, which Gen 1 stores in whole thousands.
    pub tms: PriceFormat,
}

impl PriceRules {
    /// The price formats of a generation.
    pub const fn of(generation: Generation) -> Self {
        Self {
            items: PriceFormat::of(generation, false),
            tms: PriceFormat::of(generation, true),
        }
    }

    /// The format of `item`'s price in `data`.
    pub fn of_item(&self, data: &GameData, item: ItemId) -> PriceFormat {
        if data.items.iter().any(|i| i.id == item && i.tm) {
            self.tms
        } else {
            self.items
        }
    }
}

/// What trainer teams can hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrainerRules {
    /// The game has held items for trainer Pokémon.
    pub held_items: bool,
    /// The game has moves of their own for trainer Pokémon.
    pub custom_moves: bool,
    /// The limits of each trainer, by trainer number.
    pub trainers: Vec<TrainerLimits>,
}

impl TrainerRules {
    /// The limits [`EditRules::from_game`] describes.
    pub fn from_game(data: &GameData) -> Self {
        let later = data.identity.generation.number() >= 2;
        Self {
            held_items: later,
            custom_moves: later,
            trainers: data
                .trainers
                .iter()
                .map(|t| TrainerLimits::from_game(data, t, later))
                .collect(),
        }
    }
}

impl TrainerLimits {
    fn from_game(data: &GameData, t: &Trainer, later: bool) -> Self {
        let kind = if t.is_boss() {
            0
        } else if t.is_important() {
            1
        } else {
            2
        };
        let size = t.pokemon.len();
        let most = if t.multi_battle == MultiBattleStatus::Always {
            3
        } else {
            6
        };
        Self {
            index: t.index,
            // Some double battles of the game itself have one Pokémon.
            min_pokemon: t.battle_style.required_pokemon().min(size.max(1)),
            max_pokemon: if data.trainer_pokemon_additions[kind] {
                most.max(size)
            } else {
                size
            },
            shares_battle: t.multi_battle == MultiBattleStatus::Potential,
            levels: true,
            held_items: later
                && (data.trainer_held_item_additions[kind] || t.pokemon.iter().any(|p| p.held_item.is_some())),
            custom_moves: later
                && (data.trainer_custom_movesets[kind] || t.pokemon.iter().any(|p| p.moves.iter().any(|&m| m != 0))),
            starter_slot: t.force_starter_position.map(usize::from),
        }
    }
}

/// The limits of one trainer's team.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
#[expect(clippy::struct_excessive_bools, reason = "independent limits of one team")]
pub struct TrainerLimits {
    /// The trainer number.
    pub index: u16,
    /// The fewest Pokémon its battle style needs, 2 for a double battle, or as many as it has when that is fewer.
    pub min_pokemon: usize,
    /// The most Pokémon its team can have.
    pub max_pokemon: usize,
    /// It battles together with another trainer when both see the player, and then brings only its first 3.
    pub shares_battle: bool,
    /// Its Pokémon's levels can change; the game sets them otherwise (the Driftveil tournament of Black 2 and
    /// White 2).
    pub levels: bool,
    /// Its Pokémon can hold items.
    pub held_items: bool,
    /// Its Pokémon can have moves of their own.
    pub custom_moves: bool,
    /// The place in its team (from 0) of the rival's starter, for information: a randomizer that gives rivals the
    /// player's starters puts it there.
    pub starter_slot: Option<usize>,
}

/// How the levels of one wild set are stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum WildLevels {
    /// A lowest and a highest level per slot.
    Range,
    /// One level per slot: a slot's lowest and highest level are the same.
    Single,
    /// One level range for the whole set: every slot has the same lowest and highest level.
    Shared,
    /// The levels can't change: the set only stores species, its levels come from elsewhere.
    Fixed,
}

/// How wild encounter levels are stored, and which species may not appear where.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WildRules {
    /// How each set stores its levels, by place in the list of sets (`GameData::encounters`).
    pub levels: Vec<WildLevels>,
    /// Species no wild set may have.
    pub banned: Vec<DexNumber>,
    /// Species particular sets may not have, for the sets that have such species.
    pub banned_in_sets: Vec<WildSetBan>,
}

/// Species one wild set may not have.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WildSetBan {
    /// The set, by place in the list of sets.
    pub set: usize,
    /// The species.
    pub species: Vec<DexNumber>,
}

/// Which fields in-game trades have, and how long their texts may be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TradeRules {
    /// The nicknames and original trainer names can change; not in games whose ROM entry keeps their texts.
    pub texts: bool,
    /// The longest nickname, in the game's characters.
    pub max_nickname_length: usize,
    /// The longest original trainer name, in the game's characters; 0 when trades have none.
    pub max_ot_name_length: usize,
    /// Trades have an original trainer (name and ID).
    pub original_trainer: bool,
    /// The highest original trainer ID: 65535 up to Gen 6, which store 16 bits, 4294967295 in Gen 7, which stores
    /// 32; 0 when trades have none.
    pub max_ot_id: u32,
    /// Trades have a held item.
    pub held_item: bool,
    /// How many IVs a trade has; 0 when it has none.
    pub ivs: usize,
    /// The highest IV: 15 for the DVs of Gen 2, 31 later.
    pub max_iv: u8,
    /// The value that stands for an IV the game picks at random, when the game has one: 255 in Gen 5 and 7, 32 in
    /// Gen 6.
    pub random_iv: Option<u8>,
}

/// The value that stands for a random IV of a trade in a generation, when it has one.
pub(super) const fn random_trade_iv(generation: u8) -> Option<u8> {
    match generation {
        5 | 7 => Some(255),
        6 => Some(32),
        _ => None,
    }
}

/// The highest original trainer ID a trade can have in a generation; 0 when its trades have none.
pub(super) const fn max_trade_ot_id(generation: u8) -> u32 {
    match generation {
        1 => 0,
        7 => u32::MAX,
        _ => 0xFFFF,
    }
}

/// Field items the game needs where they are.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FieldItemRules {
    /// Items the game only gives as field items, like TMs of moves needed to get on: replacing one leaves the player
    /// without it.
    pub required: Vec<ItemId>,
}

/// A trainer Pokémon the game sets from a starter (Yellow: the rival's first Eevee is the second starter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StarterLink {
    /// The starter, by place (from 0).
    pub starter: usize,
    /// The trainer, by number.
    pub trainer: u16,
    /// The Pokémon's place in the team (from 0).
    pub slot: usize,
}
