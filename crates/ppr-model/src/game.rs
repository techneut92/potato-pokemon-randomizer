use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::{
    CatchingTutorial, DexNumber, EncounterSet, Evolution, HeldItemPools, InGameTrade, Item, ItemId, LevelUpMove,
    MiscTweak, Move, MoveCategory, MoveId, PHYSICAL_TYPES, PlayerGraphicsChoice, PlayerGraphicsPack, RomIdentity, Shop,
    Species, SpeciesPalettes, StaticEncounter, TotemPokemon, Trainer, Type, TypeChart, WildHeldItems,
};

/// Species whose alternate formes depend on their ability or revert, by national number.
const CASTFORM: DexNumber = 351;
const GIRATINA: DexNumber = 487;
const SHAYMIN: DexNumber = 492;
const DARMANITAN: DexNumber = 555;
const AEGISLASH: DexNumber = 681;
const WISHIWASHI: DexNumber = 746;

/// Every kind of trainer: bosses, important and regular trainers.
fn all_trainers() -> [bool; 3] {
    [true; 3]
}

/// How a game stores formes beyond its species list: which Pokémon have a forme, and which formes trainers can't use.
#[expect(clippy::struct_excessive_bools, reason = "independent facts of a game's formes")]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FormeRules {
    /// Trainer Pokémon have a forme, so they can be alternate and cosmetic formes.
    pub trainers: bool,
    /// Wild Pokémon have a forme.
    pub wild: bool,
    /// Static encounters have a forme.
    pub statics: bool,
    /// Starters can be alternate formes.
    #[serde(default)]
    pub starters: bool,
    /// Alternate formes have evolutions of their own, so random evolutions take them in.
    #[serde(default)]
    pub evolutions: bool,
    /// Formes trainers can't use, e.g. Giratina's Origin Forme where it reverts without its orb.
    pub banned_for_trainers: Vec<DexNumber>,
    /// A trainer's alternate forme has the abilities of its base species.
    pub trainer_base_forme_abilities: bool,
    /// Static encounters that hold their Mega Stone are always replaced by species that can Mega Evolve, holding a
    /// Mega Stone of theirs (X and Y's Lucario, whose Mega Stone is given with it).
    #[serde(default)]
    pub force_swap_static_megas: bool,
}

/// Everything a run reads and changes, loaded from one ROM (ADR 0002).
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent facts and patches of a game, not a state machine"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameData {
    /// Which game this is.
    pub identity: RomIdentity,
    /// Types that exist in this game.
    pub types: Vec<Type>,
    /// Effectiveness of every type against every other.
    pub type_chart: TypeChart,
    /// Valid species, ordered by number: the base species by national number, then the alternate formes with data
    /// of their own ([`crate::Forme`]).
    pub species: Vec<Species>,
    /// How the game stores formes beyond its species list.
    #[serde(default)]
    pub formes: FormeRules,
    /// Evolution edges between valid species.
    pub evolutions: Vec<Evolution>,
    /// Each evolution at a level as loaded, with the base stat totals before and after it; set by
    /// [`GameData::estimate_evolution_levels`].
    #[serde(default)]
    pub evolution_level_samples: Vec<[u16; 3]>,
    /// The species the randomizers may choose from when "Limit Pokémon" is on, fixed before the first randomizer;
    /// `None` allows every species. See [`GameData::species_pool`].
    #[serde(default)]
    pub restricted_species: Option<BTreeSet<DexNumber>>,
    /// Starter species, in the game's order.
    pub starters: Vec<DexNumber>,
    /// Items the starters hold; Gen 3 has one for all three. `None` is no item.
    pub starter_held_items: Vec<Option<ItemId>>,
    /// The type triangle the starters were chosen from, each type beating the one before it; `None` while the game's
    /// standard triangle applies.
    pub starter_type_triangle: Option<[Type; 3]>,
    /// The game's items, by number.
    pub items: Vec<Item>,
    /// What wild Pokémon may hold, by species; species missing hold nothing.
    #[serde(default)]
    pub wild_held_items: BTreeMap<DexNumber, WildHeldItems>,
    /// The species' palettes, by species; empty when the game's palettes cannot be randomized.
    #[serde(default)]
    pub palettes: BTreeMap<DexNumber, SpeciesPalettes>,
    /// Which palette description file fits the game: `"E"` or `"FRLG"` in Gen 3.
    #[serde(default)]
    pub palette_files: Option<String>,
    /// The custom player graphics packs the game can use.
    #[serde(default)]
    pub player_graphics_packs: Vec<PlayerGraphicsPack>,
    /// The player graphics to put in the game; `None` keeps the game's own.
    #[serde(default)]
    pub player_graphics: Option<PlayerGraphicsChoice>,
    /// Wild Pokémon may always hold an item.
    #[serde(default)]
    pub guaranteed_wild_held_items: bool,
    /// Wild Pokémon may hold an item in dark grass.
    #[serde(default)]
    pub dark_grass_held_items: bool,
    /// Wild encounter sets; each set appears once, even when several maps share it.
    pub encounters: Vec<EncounterSet>,
    /// Every ball catches every Pokémon; a patch, so only ever switched on.
    pub guaranteed_catching: bool,
    /// Static encounters, in the handler's fixed order; empty when the game's are not supported.
    pub statics: Vec<StaticEncounter>,
    /// Legendaries the main story makes the player face, e.g. Groudon in Ruby.
    pub main_game_legendaries: Vec<DexNumber>,
    /// Species whose static encounters play special music; empty when the game can't move that music to their
    /// replacements.
    pub special_music_statics: Vec<DexNumber>,
    /// Give replaced special-music statics' music to their replacements; a patch, so only ever switched on.
    pub correct_static_music: bool,
    /// Totem Pokémon with their allies, in the handler's fixed order; empty in games without them.
    #[serde(default)]
    pub totems: Vec<TotemPokemon>,
    /// Happiness evolutions need less happiness (part of making evolutions easier); a patch, so only ever switched on.
    #[serde(default)]
    pub easier_happiness_evolutions: bool,
    /// In-game trades, in the game's order.
    pub trades: Vec<InGameTrade>,
    /// Longest nickname a trade can have, in characters.
    pub max_trade_nickname_length: usize,
    /// Longest original trainer name a trade can have, in characters.
    pub max_trade_ot_name_length: usize,
    /// Moves by number, from 1; empty when the handler doesn't read them yet.
    #[serde(default)]
    pub moves: Vec<Move>,
    /// Longest move name the game can hold; 0 when the handler doesn't write names.
    #[serde(default)]
    pub max_move_name_length: usize,
    /// Moves each species learns by level, in the game's order.
    #[serde(default)]
    pub learnsets: BTreeMap<DexNumber, Vec<LevelUpMove>>,
    /// Egg moves of the species that have them.
    #[serde(default)]
    pub egg_moves: BTreeMap<DexNumber, Vec<MoveId>>,
    /// The moves of the HMs, which random movesets leave out.
    #[serde(default)]
    pub hm_moves: Vec<MoveId>,
    /// Moves that break the game early: Sonic Boom and Dragon Rage.
    #[serde(default)]
    pub game_breaking_moves: Vec<MoveId>,
    /// Moves no random moveset, TM or tutor may have: Transform in Gen 1.
    #[serde(default)]
    pub banned_level_up_moves: Vec<MoveId>,
    /// Species may start with four moves.
    #[serde(default)]
    pub four_starting_moves: bool,
    /// The moves of the TMs, TM01 first.
    #[serde(default)]
    pub tm_moves: Vec<MoveId>,
    /// The moves of the move tutors; empty when the game has none.
    #[serde(default)]
    pub tutor_moves: Vec<MoveId>,
    /// Which TMs and HMs each species can learn: one flag per TM, then per HM.
    #[serde(default)]
    pub tm_hm_compat: BTreeMap<DexNumber, Vec<bool>>,
    /// Which move tutor moves each species can learn; empty without tutors.
    #[serde(default)]
    pub tutor_compat: BTreeMap<DexNumber, Vec<bool>>,
    /// HMs needed early in the game, which random compatibility favours.
    #[serde(default)]
    pub early_required_hm_moves: Vec<MoveId>,
    /// Moves used outside battle, which "Keep Field Move TMs" keeps.
    #[serde(default)]
    pub field_moves: Vec<MoveId>,
    /// Items on the ground and hidden items that may change, in the game's map order.
    #[serde(default)]
    pub field_items: Vec<ItemId>,
    /// TMs only found as field items, which random field items keep.
    #[serde(default)]
    pub required_field_tms: Vec<ItemId>,
    /// Shops, in the handler's fixed order; empty when the game's are not supported.
    #[serde(default)]
    pub shops: Vec<Shop>,
    /// What items cost, by item; items the game has no price for are missing. [`crate::PriceFormat`] says what fits.
    #[serde(default)]
    pub item_prices: BTreeMap<ItemId, u32>,
    /// Better prices for items whose price does not fit their use.
    #[serde(default)]
    pub balanced_item_prices: BTreeMap<ItemId, u32>,
    /// Items too good to sell.
    #[serde(default)]
    pub op_shop_items: Vec<ItemId>,
    /// Mega Stones: random field items place each only once, unless shop prices are balanced.
    #[serde(default)]
    pub mega_stones: Vec<ItemId>,
    /// Items that make species evolve.
    #[serde(default)]
    pub evolution_items: Vec<ItemId>,
    /// What Pickup finds, in the game's order; empty when the game has no table.
    #[serde(default)]
    pub pickup_items: Vec<ItemId>,
    /// Pickup may find TMs: they can be held and are used up.
    #[serde(default)]
    pub pickup_tms: bool,
    /// Ability names by number, 0 being "no ability"; empty when the game has no abilities.
    pub ability_names: Vec<String>,
    /// Abilities that do the same as another, by the one they vary, itself included: Insomnia → Insomnia, Vital Spirit.
    pub ability_variations: BTreeMap<u16, Vec<u16>>,
    /// Abilities that do nothing for most species, e.g. Forecast.
    pub useless_abilities: Vec<u16>,
    /// Species the game can't have as wild Pokémon, e.g. Unown outside the Tanoby Ruins in `FireRed` and `LeafGreen`.
    pub wild_banned: Vec<DexNumber>,
    /// The trainer battles, by index; empty when not read.
    #[serde(default)]
    pub trainers: Vec<Trainer>,
    /// Trainer class names by number.
    #[serde(default)]
    pub trainer_class_names: Vec<String>,
    /// Indices of the Elite Four and Champion trainers.
    #[serde(default)]
    pub elite_four_trainers: Vec<u16>,
    /// The types of the gyms, Elite Four and Champion by trainer group (`GYM1`, `ELITE1`, `CHAMPION`).
    #[serde(default)]
    pub gym_and_elite_themes: BTreeMap<String, Type>,
    /// Species' types as loaded, kept when a step changes them so the original primary type stays known; empty while
    /// unchanged.
    #[serde(default)]
    pub original_types: BTreeMap<DexNumber, (Type, Option<Type>)>,
    /// The longest trainer name the game has room for.
    #[serde(default)]
    pub max_trainer_name_length: usize,
    /// A trainer name shares [`GameData::max_trainer_name_length`] with its class name, as both are shown on one
    /// line (Gen 2).
    #[serde(default)]
    pub trainer_name_shares_class_line: bool,
    /// The most bytes all trainer names may take together, terminators not counted; `None` for no limit.
    #[serde(default)]
    pub max_trainer_names_total: Option<usize>,
    /// The longest trainer class name the game has room for.
    #[serde(default)]
    pub max_trainer_class_name_length: usize,
    /// The trainer classes of pairs battling together, like twins.
    #[serde(default)]
    pub doubles_trainer_classes: Vec<u8>,
    /// Trainer classes named after a person, like the gym leaders in Gen 1: their names are trainer names, not
    /// class names.
    #[serde(default)]
    pub personal_trainer_classes: Vec<u8>,
    /// Trainer and trainer class names can't be changed in this game.
    #[serde(default)]
    pub fixed_trainer_text: bool,
    /// A new trainer name must take as many bytes as the old one ([`GameData::text_length`]).
    #[serde(default)]
    pub fixed_trainer_name_length: bool,
    /// A new trainer class name must take as many bytes as the old one ([`GameData::text_length`]).
    #[serde(default)]
    pub fixed_trainer_class_name_length: bool,
    /// The texts that one byte of the game's text encoding stands for, sorted; empty when a character takes one
    /// byte. See [`GameData::text_length`].
    #[serde(default)]
    pub text_tokens: Vec<String>,
    /// The species the game's intro can show; empty when it can't be changed.
    #[serde(default)]
    pub intro_candidates: Vec<DexNumber>,
    /// The species the intro is to show; `None` keeps the game's.
    #[serde(default)]
    pub intro_pokemon: Option<DexNumber>,
    /// The items trainer Pokémon may be given to hold.
    #[serde(default)]
    pub held_item_pools: HeldItemPools,
    /// Which trainers can get more Pokémon: bosses, important and regular trainers. Room for trainer data limits
    /// it in Gen 1.
    #[serde(default = "all_trainers")]
    pub trainer_pokemon_additions: [bool; 3],
    /// Which trainers' Pokémon can get movesets of their own: bosses, important and regular trainers.
    #[serde(default = "all_trainers")]
    pub trainer_custom_movesets: [bool; 3],
    /// Which trainers' Pokémon can be given held items: bosses, important and regular trainers. Room for trainer
    /// data limits it in Gen 2.
    #[serde(default = "all_trainers")]
    pub trainer_held_item_additions: [bool; 3],
    /// The misc tweaks the game supports.
    #[serde(default)]
    pub available_tweaks: Vec<MiscTweak>,
    /// The misc tweaks the handler patches into the ROM when saving.
    #[serde(default)]
    pub applied_tweaks: BTreeSet<MiscTweak>,
    /// The item to put in the player's PC instead of the Potion; `None` keeps it.
    #[serde(default)]
    pub pc_potion: Option<ItemId>,
    /// The species the catching tutorial can show as the one caught and as the catcher; both empty when it can't be
    /// changed.
    #[serde(default)]
    pub catching_tutorial_opponents: Vec<DexNumber>,
    /// See [`GameData::catching_tutorial_opponents`].
    #[serde(default)]
    pub catching_tutorial_players: Vec<DexNumber>,
    /// The Pokémon the catching tutorial is to show; `None` keeps the game's.
    #[serde(default)]
    pub catching_tutorial: Option<CatchingTutorial>,
    /// The static encounters that are fossils, which "Balance Static Pokémon Levels" sets to level 30.
    #[serde(default)]
    pub fossils: Vec<usize>,
}

impl GameData {
    /// A damaging move is physical: by its type before Gen 4, by its category after.
    pub fn is_physical(&self, mv: &Move) -> bool {
        if self.identity.generation.number() < 4 {
            mv.move_type.is_some_and(|t| PHYSICAL_TYPES.contains(&t))
        } else {
            mv.category == MoveCategory::Physical
        }
    }

    /// The game is Pokémon Yellow, whose rival has the second of its two starters.
    pub fn is_yellow(&self) -> bool {
        self.identity.generation.number() == 1 && self.identity.family.eq_ignore_ascii_case("Yellow")
    }

    /// The bytes `text` takes in the game, terminator not counted: the longest token of [`GameData::text_tokens`]
    /// that matches is one byte, `\xNN` too, and a character nothing matches is dropped. Characters when the game
    /// has no tokens.
    pub fn text_length(&self, text: &str) -> usize {
        if self.text_tokens.is_empty() {
            return text.chars().count();
        }
        let longest = self.text_tokens.iter().map(|t| t.chars().count()).max().unwrap_or(1);
        let chars: Vec<char> = text.chars().collect();
        let (mut i, mut bytes) = (0, 0);
        while i < chars.len() {
            if chars[i] == '\\' && chars.get(i + 1) == Some(&'x') {
                bytes += 1;
                i += 4;
                continue;
            }
            let matched = (1..=longest.min(chars.len() - i)).rev().find(|&len| {
                let token: String = chars[i..i + len].iter().collect();
                self.text_tokens.binary_search(&token).is_ok()
            });
            match matched {
                Some(len) => {
                    bytes += 1;
                    i += len;
                }
                None => i += 1,
            }
        }
        bytes
    }

    /// The items randomizers may use, by number.
    pub fn allowed_items(&self) -> Vec<ItemId> {
        self.items.iter().filter(|i| i.allowed).map(|i| i.id).collect()
    }

    /// The allowed items that are not bad, by number.
    pub fn non_bad_items(&self) -> Vec<ItemId> {
        self.items
            .iter()
            .filter(|i| i.allowed && !i.bad)
            .map(|i| i.id)
            .collect()
    }

    /// The standard type triangle: Fire, Water, Grass before Gen 3; Grass, Fire, Water from Gen 3.
    pub fn standard_type_triangle(&self) -> [Type; 3] {
        if self.identity.generation.number() <= 2 {
            [Type::Fire, Type::Water, Type::Grass]
        } else {
            [Type::Grass, Type::Fire, Type::Water]
        }
    }

    /// The triangle the starters were chosen from, else the standard one.
    pub fn starter_type_triangle(&self) -> [Type; 3] {
        self.starter_type_triangle
            .unwrap_or_else(|| self.standard_type_triangle())
    }

    /// The last four different moves `species` learns by `level`, 0 for none.
    pub fn moves_at_level(&self, species: DexNumber, level: u8) -> [MoveId; 4] {
        let mut moves = [0; 4];
        let mut count = 0;
        for m in self.learnsets.get(&species).into_iter().flatten() {
            if m.level > level {
                break;
            }
            if moves[..count].contains(&m.move_id) {
                continue;
            }
            if count == 4 {
                moves.rotate_left(1);
                moves[3] = m.move_id;
            } else {
                moves[count] = m.move_id;
                count += 1;
            }
        }
        moves
    }

    /// Remembers every species' types as loaded, before a step changes them.
    pub fn keep_original_types(&mut self) {
        if self.original_types.is_empty() {
            self.original_types = self
                .species
                .iter()
                .map(|sp| (sp.number, (sp.primary_type, sp.secondary_type)))
                .collect();
        }
    }

    /// A species' types as loaded.
    pub fn original_types_of(&self, number: DexNumber) -> Option<(Type, Option<Type>)> {
        self.original_types
            .get(&number)
            .copied()
            .or_else(|| self.species(number).map(|sp| (sp.primary_type, sp.secondary_type)))
    }

    /// The base species the randomizers may choose from: those "Limit Pokémon" allows, else every base species of
    /// the game.
    pub fn species_pool(&self) -> BTreeSet<DexNumber> {
        let mut pool = self.species_pool_with_formes();
        pool.retain(|&n| self.species(n).is_some_and(Species::is_base_forme));
        pool
    }

    /// The species and alternate formes the randomizers may choose from, as [`GameData::species_pool`] with the
    /// alternate formes of the game.
    pub fn species_pool_with_formes(&self) -> BTreeSet<DexNumber> {
        self.restricted_species
            .clone()
            .unwrap_or_else(|| self.species.iter().map(|sp| sp.number).collect())
    }

    /// The base species, without alternate formes.
    pub fn base_species(&self) -> impl Iterator<Item = &Species> {
        self.species.iter().filter(|sp| sp.is_base_forme())
    }

    /// The alternate formes with data of their own.
    pub fn alt_formes(&self) -> impl Iterator<Item = &Species> {
        self.species.iter().filter(|sp| !sp.is_base_forme())
    }

    /// The base species of `number`: itself unless it is an alternate forme.
    pub fn base_forme_of(&self, number: DexNumber) -> DexNumber {
        self.species(number).map_or(number, Species::base_number)
    }

    /// The alternate formes of base species `base`, by forme number.
    pub fn formes_of(&self, base: DexNumber) -> impl Iterator<Item = &Species> {
        self.species.iter().filter(move |sp| sp.forme.base == Some(base))
    }

    /// The species that forme `forme` of `base` is: the alternate forme with that number, else `base` itself (forme
    /// 0, and cosmetic formes without data of their own).
    pub fn forme_species(&self, base: DexNumber, forme: u8) -> DexNumber {
        if forme == 0 {
            return base;
        }
        self.formes_of(base)
            .find(|sp| sp.forme.number == forme)
            .map_or(base, |sp| sp.number)
    }

    /// The forme number to store with a Pokémon of `number`: an alternate forme's own, else 0.
    pub fn forme_number(&self, number: DexNumber) -> u8 {
        self.species(number).map_or(0, |sp| sp.forme.number)
    }

    /// The generation that introduced `number` ([`Species::generation`]).
    pub fn generation_of(&self, number: DexNumber) -> u8 {
        self.species(number)
            .map_or_else(|| crate::generation_of(number), Species::generation)
    }

    /// The Mega Evolutions.
    pub fn mega_evolutions(&self) -> impl Iterator<Item = &Species> {
        self.species.iter().filter(|sp| sp.forme.mega)
    }

    /// Whether `number` is legendary, alternate formes by their base species.
    pub fn is_legendary(&self, number: DexNumber) -> bool {
        crate::is_legendary(self.base_forme_of(number))
    }

    /// Whether `number` is one of the strongest legendaries, alternate formes by their base species.
    pub fn is_strong_legendary(&self, number: DexNumber) -> bool {
        crate::is_strong_legendary(self.base_forme_of(number))
    }

    /// The Ultra Beasts of the species pool ([`GameData::species_pool`]), without alternate formes.
    pub fn ultra_beasts(&self) -> BTreeSet<DexNumber> {
        let mut pool = self.species_pool();
        pool.retain(|&n| crate::is_ultra_beast(n));
        pool
    }

    /// The alternate formes "No Irregular Alt Formes" leaves out.
    pub fn irregular_formes(&self) -> BTreeSet<DexNumber> {
        self.alt_formes()
            .filter(|sp| sp.forme.irregular)
            .map(|sp| sp.number)
            .collect()
    }

    /// The alternate formes that need their species' ability to keep, which are left out while abilities stay as
    /// they are: Castform's weather formes, Darmanitan's Zen Mode, Aegislash's Blade Forme and Wishiwashi's School
    /// Form.
    pub fn ability_dependent_formes(&self) -> BTreeSet<DexNumber> {
        self.alt_formes()
            .filter(|sp| match sp.base_number() {
                CASTFORM | AEGISLASH | WISHIWASHI => true,
                DARMANITAN => sp.forme.number == 1,
                _ => false,
            })
            .map(|sp| sp.number)
            .collect()
    }

    /// The alternate formes the player's Pokémon (wild, static, evolved into) can't be, as they revert: Giratina's
    /// Origin Forme without its orb, Shaymin's Sky Forme.
    pub fn banned_formes_for_player(&self) -> BTreeSet<DexNumber> {
        self.alt_formes()
            .filter(|sp| matches!(sp.base_number(), GIRATINA | SHAYMIN))
            .map(|sp| sp.number)
            .collect()
    }

    /// The abilities a trainer's Pokémon of `number` has: an alternate forme's base species' where the game does
    /// that.
    pub fn trainer_pokemon_abilities(&self, number: DexNumber) -> &[u16] {
        let number = if self.formes.trainer_base_forme_abilities {
            self.base_forme_of(number)
        } else {
            number
        };
        self.species(number).map_or(&[], |sp| sp.abilities.as_slice())
    }

    /// The species with this national number.
    pub fn species(&self, number: DexNumber) -> Option<&Species> {
        self.species
            .binary_search_by_key(&number, |s| s.number)
            .ok()
            .map(|i| &self.species[i])
    }

    /// Species this one evolves into directly; an alternate forme also those of its base species.
    pub fn evolved_species(&self, number: DexNumber) -> BTreeSet<DexNumber> {
        let base = self.base_forme_of(number);
        self.evolutions
            .iter()
            .filter(|e| e.from == number || e.from == base)
            .map(|e| e.to)
            .collect()
    }

    /// Species that evolve into this one directly; for an alternate forme also those of its base species.
    pub fn pre_evolved_species(&self, number: DexNumber) -> BTreeSet<DexNumber> {
        let base = self.base_forme_of(number);
        self.evolutions
            .iter()
            .filter(|e| e.to == number || e.to == base)
            .map(|e| e.from)
            .collect()
    }

    /// How many evolution stages follow this species: 2 for Bulbasaur, 0 for Venusaur. Cycles count each step once.
    pub fn stages_after(&self, number: DexNumber) -> usize {
        let mut stages = 0;
        let mut checked = BTreeSet::new();
        let mut current = BTreeSet::from([number]);
        while !current.is_empty() {
            let mut next = BTreeSet::new();
            for n in current {
                if checked.insert(n) {
                    next.extend(self.evolved_species(n));
                }
            }
            if !next.is_empty() {
                stages += 1;
            }
            current = next;
        }
        stages
    }

    /// How many evolutions lead to this species: 0 for basic species (Pichu, Tauros), 1 for Pikachu. Cycles count each
    /// step once.
    pub fn stages_before(&self, number: DexNumber) -> usize {
        let mut stages = 0;
        let mut checked = BTreeSet::new();
        let mut current = BTreeSet::from([number]);
        while !current.is_empty() {
            let mut previous = BTreeSet::new();
            for n in current {
                if checked.insert(n) {
                    previous.extend(self.pre_evolved_species(n));
                }
            }
            if !previous.is_empty() {
                stages += 1;
            }
            current = previous;
        }
        stages
    }

    /// Every species related to this one by evolution, itself included.
    pub fn family(&self, number: DexNumber) -> BTreeSet<DexNumber> {
        let mut family = BTreeSet::new();
        let mut queue = VecDeque::from([number]);
        while let Some(n) = queue.pop_front() {
            if family.insert(n) {
                queue.extend(self.evolved_species(n));
                queue.extend(self.pre_evolved_species(n));
            }
        }
        family
    }

    /// Breadth-first walk over the family with each member's position relative to `number`: +1 for what it evolves
    /// into, -1 for what evolves into it. The first path wins.
    fn positions(&self, number: DexNumber) -> Vec<(DexNumber, i32)> {
        let mut out = Vec::new();
        let mut checked = BTreeSet::new();
        let mut queue = VecDeque::from([(number, 0)]);
        while let Some((n, position)) = queue.pop_front() {
            if !checked.insert(n) {
                continue;
            }
            out.push((n, position));
            queue.extend(self.evolved_species(n).into_iter().map(|e| (e, position + 1)));
            queue.extend(self.pre_evolved_species(n).into_iter().map(|e| (e, position - 1)));
        }
        out
    }

    /// Position of `relative` seen from `number`: -1 if `relative` evolves directly into `number`. `None` when they are
    /// not related.
    pub fn relation(&self, number: DexNumber, relative: DexNumber) -> Option<i32> {
        self.positions(number)
            .into_iter()
            .find(|&(n, _)| n == relative)
            .map(|(_, p)| p)
    }

    /// Relatives at a position, on any branch: +1 on Cascoon gives Beautifly and Dustox.
    pub fn relatives_at_position(&self, number: DexNumber, position: i32) -> BTreeSet<DexNumber> {
        self.positions(number)
            .into_iter()
            .filter(|&(_, p)| p == position)
            .map(|(n, _)| n)
            .collect()
    }

    /// Relatives that many evolutions after (or, negative, before) this species, on its own branch.
    pub fn relatives_at_position_same_branch(&self, number: DexNumber, position: i32) -> BTreeSet<DexNumber> {
        let mut current = BTreeSet::from([number]);
        for _ in 0..position.unsigned_abs() {
            current = current
                .iter()
                .flat_map(|&n| {
                    if position > 0 {
                        self.evolved_species(n)
                    } else {
                        self.pre_evolved_species(n)
                    }
                })
                .collect();
        }
        current
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{BaseStats, EvolutionType, Generation, Platform};

    #[expect(clippy::too_many_lines, reason = "a line for each field of GameData")]
    /// A Gen 3 game with species 1–6 of the Normal type and these evolutions, and nothing else.
    pub(crate) fn data(evolutions: &[(DexNumber, DexNumber)]) -> GameData {
        GameData {
            identity: RomIdentity {
                name: "Test".into(),
                code: "TEST".into(),
                version: Some(0),
                generation: Generation::new(3).expect("valid"),
                platform: Platform::GameBoyAdvance,
                family: "Em".into(),
            },
            types: vec![Type::Normal],
            type_chart: crate::TypeChart::default(),
            species: (1..=6)
                .map(|number| Species {
                    number,
                    name: format!("S{number}"),
                    primary_type: Type::Normal,
                    secondary_type: None,
                    stats: BaseStats::default(),
                    catch_rate: 45,
                    abilities: Vec::new(),
                    growth_curve: crate::ExpCurve::MediumFast,
                    forme: crate::Forme::default(),
                })
                .collect(),
            formes: FormeRules::default(),
            evolutions: evolutions
                .iter()
                .map(|&(from, to)| Evolution::new(from, to, EvolutionType::Level, 16))
                .collect(),
            evolution_level_samples: Vec::new(),
            restricted_species: None,
            starters: vec![1, 4, 6],
            starter_type_triangle: None,
            starter_held_items: Vec::new(),
            items: Vec::new(),
            wild_held_items: BTreeMap::new(),
            guaranteed_wild_held_items: true,
            dark_grass_held_items: false,
            palettes: BTreeMap::new(),
            palette_files: None,
            player_graphics_packs: Vec::new(),
            player_graphics: None,
            encounters: Vec::new(),
            guaranteed_catching: false,
            wild_banned: Vec::new(),
            statics: Vec::new(),
            main_game_legendaries: Vec::new(),
            special_music_statics: Vec::new(),
            correct_static_music: false,
            totems: Vec::new(),
            easier_happiness_evolutions: false,
            moves: Vec::new(),
            max_move_name_length: 0,
            learnsets: BTreeMap::new(),
            egg_moves: BTreeMap::new(),
            hm_moves: Vec::new(),
            game_breaking_moves: Vec::new(),
            banned_level_up_moves: Vec::new(),
            four_starting_moves: false,
            tm_moves: Vec::new(),
            tutor_moves: Vec::new(),
            tm_hm_compat: BTreeMap::new(),
            tutor_compat: BTreeMap::new(),
            field_items: Vec::new(),
            required_field_tms: Vec::new(),
            shops: Vec::new(),
            item_prices: BTreeMap::new(),
            balanced_item_prices: BTreeMap::new(),
            op_shop_items: Vec::new(),
            mega_stones: Vec::new(),
            evolution_items: Vec::new(),
            pickup_items: Vec::new(),
            pickup_tms: false,
            early_required_hm_moves: Vec::new(),
            field_moves: Vec::new(),
            trades: Vec::new(),
            max_trade_nickname_length: 10,
            max_trade_ot_name_length: 7,
            ability_names: Vec::new(),
            ability_variations: BTreeMap::new(),
            useless_abilities: Vec::new(),
            trainers: Vec::new(),
            trainer_class_names: Vec::new(),
            elite_four_trainers: Vec::new(),
            gym_and_elite_themes: BTreeMap::new(),
            original_types: BTreeMap::new(),
            max_trainer_name_length: 0,
            max_trainer_class_name_length: 0,
            trainer_name_shares_class_line: false,
            max_trainer_names_total: None,
            doubles_trainer_classes: Vec::new(),
            personal_trainer_classes: Vec::new(),
            fixed_trainer_text: false,
            fixed_trainer_name_length: false,
            fixed_trainer_class_name_length: false,
            text_tokens: Vec::new(),
            intro_candidates: Vec::new(),
            intro_pokemon: None,
            held_item_pools: HeldItemPools::default(),
            trainer_pokemon_additions: [true; 3],
            trainer_custom_movesets: [true; 3],
            trainer_held_item_additions: [true; 3],
            available_tweaks: Vec::new(),
            applied_tweaks: BTreeSet::new(),
            pc_potion: None,
            catching_tutorial_opponents: Vec::new(),
            catching_tutorial_players: Vec::new(),
            catching_tutorial: None,
            fossils: Vec::new(),
        }
    }

    #[test]
    fn text_length_counts_tokens() {
        let mut d = data(&[]);
        assert_eq!(d.text_length("PKMN 'd"), 7, "characters without tokens");
        d.text_tokens = ["'d", "A", "B", "M", "N", "P", "K", "[PK]", "[MN]"]
            .map(String::from)
            .to_vec();
        d.text_tokens.sort();
        assert_eq!(d.text_length("[PK][MN] 'd"), 3, "the space is not in the table");
        assert_eq!(d.text_length("AB\\x50"), 3);
    }

    #[test]
    fn evolution_queries() {
        let d = data(&[(1, 2), (2, 3), (4, 5), (4, 6)]);
        assert_eq!(d.stages_after(1), 2);
        assert_eq!(d.stages_after(4), 1);
        assert_eq!(d.stages_after(3), 0);
        assert_eq!(d.pre_evolved_species(2), BTreeSet::from([1]));
        assert_eq!(d.evolved_species(4), BTreeSet::from([5, 6]));
        assert_eq!(d.species(5).map(|s| s.name.as_str()), Some("S5"));
        assert!(d.species(9).is_none());
    }

    #[test]
    fn cycles_terminate() {
        let d = data(&[(1, 2), (2, 1)]);
        assert_eq!(d.stages_after(1), 2);
        assert_eq!(d.stages_before(1), 2, "the step back to itself counts");
        assert_eq!(d.family(1), BTreeSet::from([1, 2]));
    }

    #[test]
    fn stages_and_relations() {
        let d = data(&[(1, 2), (2, 3), (4, 5), (4, 6)]);
        assert_eq!(
            [
                d.stages_before(1),
                d.stages_before(2),
                d.stages_before(3),
                d.stages_before(6)
            ],
            [0, 1, 2, 1]
        );
        assert_eq!(d.family(2), BTreeSet::from([1, 2, 3]));
        assert_eq!(
            (d.relation(2, 1), d.relation(2, 3), d.relation(1, 3)),
            (Some(-1), Some(1), Some(2))
        );
        assert_eq!(d.relation(1, 4), None);
        assert_eq!(d.relatives_at_position(5, 0), BTreeSet::from([5, 6]), "branches count");
        assert_eq!(d.relatives_at_position_same_branch(5, 0), BTreeSet::from([5]));
        assert_eq!(d.relatives_at_position_same_branch(4, 1), BTreeSet::from([5, 6]));
        assert_eq!(d.relatives_at_position_same_branch(3, -2), BTreeSet::from([1]));
    }
}
