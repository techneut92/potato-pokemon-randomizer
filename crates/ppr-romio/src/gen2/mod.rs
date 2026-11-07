//! Generation 2 (Gold, Silver, Crystal).
//!
//! Like Gen 1 and Gen 3, only what changed is written, so an unchanged game saves byte for byte. Data that grows
//! (level-up moves and evolutions, egg moves, trainer classes, shops, move names, the move tutor menu) moves to free
//! space.
//!
//! Deliberate choices (more in the submodules):
//! - The TM reuse patch replaces the call that uses up the TM with a relative jump (`jr`) to the code after it, as
//!   the entry's jump length means; an absolute jump (`jp`) with that one byte as its address would land in the
//!   wrong place.
//! - Starter and TM texts are only written when they fit where the old text was.
//! - Texts the ROM entries store encoded to UTF-8 twice (TM texts, shop names, the German "back" of the move tutor
//!   menu) are repaired before use, so `[POKé]` and `…` are written as the game's own characters.
//! - An egg move list that other species share stays where it is when one species' list changes; the changed list
//!   goes to free space.
//! - The Odd Egg Pokémon are eggs at level 1, as every egg; their moves are written again when their species, its
//!   level-up moves or the moves change.
//! - Trainer class names are only written when they take as many bytes as before; trainer names share a line with
//!   their class name and all of them a fixed room, which the randomizer respects.
//! - The fleeing and Unown patches are applied when wild or static Pokémon change, not on every save.
//! - Species, trainer and item graphics, trainer palettes, the Vietnamese Crystal translation and the check value
//!   of the speedchoice hacks are not ported yet.

mod constants;
mod encounters;
mod graphics;
mod maps;
mod moves;
#[cfg(test)]
mod tests;
mod trainers;

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{
    BaseStats, Capabilities, DexNumber, Effectiveness, Evolution, EvolutionType, ExpCurve, GameData, Generation,
    InGameTrade, Item, ItemId, LevelUpMove, MiscTweak, MoveId, PriceFormat, RomIdentity, Shop, Species,
    StaticEncounter, Type, TypeChart, WildHeldItems,
};

use self::constants::{self as c, Family};
use crate::entries::{RomEntry, StaticSpot, catalog};
use crate::gb::text::{GbText, TERMINATOR, repair_double_encoding};
use crate::gb::{GbRom, need, offsets, optional};
use crate::graphics::GbGames;
use crate::type_order::TYPE_ORDER;
use crate::{GameHandler, RomError, ips};

const Z80_NOP: u8 = 0x00;
const Z80_JUMP: u8 = 0xC3;
const Z80_JUMP_Z: u8 = 0xCA;
const Z80_JUMP_RELATIVE: u8 = 0x18;
const Z80_JUMP_RELATIVE_NZ: u8 = 0x20;
const Z80_JUMP_RELATIVE_C: u8 = 0x38;
const Z80_CALL: u8 = 0xCD;
const Z80_CALL_SIZE: usize = 3;
const Z80_RET: u8 = 0xC9;
const EVOS_AND_MOVES_TERMINATOR: u8 = 0;
const NO_TRADE_ITEM: u8 = 0xFF;

/// Flags by species, as of TMs or move tutors.
type Compat = BTreeMap<DexNumber, Vec<bool>>;

/// A static encounter of the ROM entry.
#[derive(Debug, Clone)]
struct StaticSlot {
    spot: StaticSpot,
    /// A Game Corner prize: its last species offset is the prize's name instead.
    game_corner: bool,
}

/// Loaded Gen 2 ROM.
#[derive(Debug)]
pub(crate) struct Gen2Handler {
    rom: GbRom,
    entry: RomEntry,
    family: Family,
    text: GbText,
    loaded: GameData,
    encounters: encounters::Encounters,
    statics: Vec<StaticSlot>,
    /// Offsets of the field items the randomizer may change.
    field_item_offsets: Vec<usize>,
}

impl Gen2Handler {
    #[expect(clippy::too_many_lines, reason = "a line for each field of GameData")]
    pub(crate) fn load(bytes: Vec<u8>, identity: RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(2).ok_or_else(|| RomError::Malformed("generation 2".into()))?;
        let entry = catalog(generation)
            .entries
            .iter()
            .find(|e| e.name == identity.name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {}", identity.name)))?;
        let family = Family::parse(&entry.family)
            .ok_or_else(|| RomError::Malformed(format!("unknown Gen 2 game type {}", entry.family)))?;
        let rom = GbRom::new(bytes, &entry)?;
        let text = GbText::for_entry(entry.value("ExtraTableFile"))?;

        let species = read_species(&rom, &entry, &text)?;
        let moves = moves::read(&rom, &entry, &text)?;
        let items = read_items(&rom, &entry, &text)?;
        let maps = maps::load(&rom, &entry, &text)?;
        let encounters = encounters::read(&rom, &entry, family, &maps, false)?;
        let statics = static_slots(&entry);
        let mut static_encounters = Vec::new();
        let eggs = offsets(&entry, "StaticEggPokemonOffsets");
        for (i, s) in statics.iter().enumerate() {
            let at = *s
                .spot
                .species
                .first()
                .ok_or_else(|| RomError::Malformed("a static without species".into()))?;
            let level = match s.spot.levels.first() {
                Some(&l) => rom.byte(l)?,
                None => 1,
            };
            static_encounters.push(StaticEncounter {
                egg: eggs.contains(&i),
                ..StaticEncounter::new(DexNumber::from(rom.byte(at)?), level)
            });
        }
        for at in odd_egg_offsets(&entry) {
            static_encounters.push(StaticEncounter {
                egg: true,
                ..StaticEncounter::new(DexNumber::from(rom.byte(at)?), 1)
            });
        }
        let trainers = trainers::read(&rom, &entry, &text)?;
        let field_item_offsets: Vec<usize> = maps
            .item_offsets
            .iter()
            .copied()
            .filter(|&o| rom.byte(o).is_ok_and(|b| !c::item_banned(c::item_standard(b))))
            .collect();
        let field_items = field_item_offsets
            .iter()
            .map(|&o| Ok(c::item_standard(rom.byte(o)?)))
            .collect::<Result<Vec<_>, RomError>>()?;
        let tm_offset = need(&entry, "TMMovesOffset")?;
        let tm_moves = (0..c::TM_COUNT)
            .map(|i| Ok(MoveId::from(rom.byte(tm_offset + i)?)))
            .collect::<Result<Vec<_>, RomError>>()?;
        let hm_moves = (0..c::HM_COUNT)
            .map(|i| Ok(MoveId::from(rom.byte(tm_offset + c::TM_COUNT + i)?)))
            .collect::<Result<Vec<_>, RomError>>()?;
        let (tutor_moves, tutor_compat) = if family == Family::Crystal {
            read_tutors(&rom, &entry, &species)?
        } else {
            (Vec::new(), BTreeMap::new())
        };
        let types = Type::in_generation(generation);
        let all: Vec<DexNumber> = species.iter().map(|s| s.number).collect();
        let non_japanese = entry.non_japanese != 0;

        let mut loaded = GameData {
            formes: ppr_model::FormeRules::default(),
            identity,
            type_chart: read_type_chart(&rom, &entry, &types)?,
            types,
            tm_hm_compat: read_tm_hm_compat(&rom, &entry, &species)?,
            wild_held_items: read_wild_held_items(&rom, &entry, &species)?,
            species,
            evolutions: read_evolutions(&rom, &entry)?,
            evolution_level_samples: Vec::new(),
            restricted_species: None,
            starters: read_starters(&rom, &entry)?,
            starter_held_items: offsets(&entry, "StarterHeldItems")
                .into_iter()
                .map(|at| Ok(item_or_none(rom.byte(at)?)))
                .collect::<Result<_, RomError>>()?,
            starter_type_triangle: None,
            item_prices: read_prices(&rom, &entry)?,
            items,
            palettes: graphics::read_palettes(&rom, &entry, all.iter().copied())?,
            palette_files: None,
            player_graphics_packs: crate::graphics::gb_packs(GbGames::Gen2),
            player_graphics: None,
            guaranteed_wild_held_items: false,
            dark_grass_held_items: false,
            encounters: encounters.sets.clone(),
            guaranteed_catching: false,
            statics: static_encounters,
            main_game_legendaries: Vec::new(),
            special_music_statics: Vec::new(),
            correct_static_music: false,
            totems: Vec::new(),
            easier_happiness_evolutions: false,
            trades: read_trades(&rom, &entry, &text)?,
            max_trade_nickname_length: 10,
            max_trade_ot_name_length: 7,
            moves,
            max_move_name_length: moves::MAX_NAME_LENGTH,
            learnsets: read_learnsets(&rom, &entry)?,
            egg_moves: read_egg_moves(&rom, &entry)?,
            hm_moves,
            game_breaking_moves: c::GAME_BREAKING_MOVES.to_vec(),
            banned_level_up_moves: c::BANNED_LEVEL_UP_MOVES.to_vec(),
            four_starting_moves: optional(&entry, "SupportsFourStartingMoves") > 0,
            tm_moves,
            tutor_moves,
            tutor_compat,
            early_required_hm_moves: c::EARLY_REQUIRED_HM_MOVES.to_vec(),
            field_moves: c::FIELD_MOVES.to_vec(),
            field_items,
            required_field_tms: c::REQUIRED_FIELD_TMS.to_vec(),
            shops: read_shops(&rom, &entry)?,
            balanced_item_prices: c::BALANCED_PRICES.into_iter().collect(),
            op_shop_items: c::OP_SHOP_ITEMS.to_vec(),
            mega_stones: Vec::new(),
            evolution_items: c::EVOLUTION_ITEMS.to_vec(),
            pickup_items: Vec::new(),
            pickup_tms: false,
            ability_names: Vec::new(),
            ability_variations: BTreeMap::new(),
            useless_abilities: Vec::new(),
            wild_banned: Vec::new(),
            elite_four_trainers: trainers::elite_four(&trainers),
            trainers,
            trainer_class_names: trainers::class_names(&rom, &entry, &text)?,
            gym_and_elite_themes: c::GYM_AND_ELITE_THEMES
                .iter()
                .map(|&(k, t)| (k.to_owned(), t))
                .collect(),
            original_types: BTreeMap::new(),
            max_trainer_name_length: c::MAX_TRAINER_NAME_LENGTH,
            trainer_name_shares_class_line: true,
            max_trainer_names_total: entry
                .int("MaxSumOfTrainerNameLengths")
                .and_then(|n| usize::try_from(n).ok()),
            max_trainer_class_name_length: usize::MAX,
            doubles_trainer_classes: offsets(&entry, "DoublesTrainerClasses")
                .into_iter()
                .filter_map(|c| u8::try_from(c).ok())
                .collect(),
            personal_trainer_classes: Vec::new(),
            fixed_trainer_text: optional(&entry, "CanChangeTrainerText") == 0,
            fixed_trainer_name_length: false,
            fixed_trainer_class_name_length: true,
            text_tokens: text.tokens(),
            intro_candidates: all.iter().copied().filter(|&n| n != c::UNOWN).collect(),
            intro_pokemon: None,
            held_item_pools: c::held_item_pools(),
            // The banks with trainer data have no room for more Pokémon in the Japanese games, nor for items of
            // regular trainers in Crystal.
            trainer_pokemon_additions: [non_japanese, non_japanese, false],
            trainer_custom_movesets: [non_japanese, non_japanese, false],
            trainer_held_item_additions: [non_japanese, non_japanese, non_japanese && family != Family::Crystal],
            available_tweaks: available_tweaks(&entry),
            applied_tweaks: BTreeSet::new(),
            pc_potion: None,
            catching_tutorial_opponents: all.clone(),
            catching_tutorial_players: all,
            catching_tutorial: None,
            fossils: Vec::new(),
        };
        loaded.estimate_evolution_levels();
        Ok(Self {
            rom,
            entry,
            family,
            text,
            loaded,
            encounters,
            statics,
            field_item_offsets,
        })
    }

    fn stats_offset(&self, number: DexNumber) -> Result<usize, RomError> {
        stats_offset(&self.entry, number)
    }

    /// Text in the place of the old text at `offset`, if it fits there (terminator included).
    fn write_text_in_place(&mut self, offset: usize, text: &str) -> bool {
        crate::gb::text::write_in_place(&mut self.rom, &self.text, offset, text)
    }

    /// The starters everywhere they are named, and their texts.
    fn write_starters(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.starters.len() != 3 {
            return Err(RomError::Malformed(format!(
                "Gen 2 has 3 starters, got {}",
                data.starters.len()
            )));
        }
        for (i, &s) in data.starters.iter().enumerate() {
            let byte = species_byte(s)?;
            for at in offsets(&self.entry, &format!("StarterOffsets{}", i + 1)) {
                self.rom.write_byte(at, byte)?;
            }
        }
        if optional(&self.entry, "CanChangeStarterText") > 0 {
            for (i, at) in offsets(&self.entry, "StarterTextOffsets")
                .into_iter()
                .enumerate()
                .take(3)
            {
                if let Some(name) = data
                    .starters
                    .get(i)
                    .and_then(|&n| data.species(n))
                    .map(|s| s.name.clone())
                {
                    self.write_text_in_place(at, &format!("{name}?\\e"));
                }
            }
        }
        Ok(())
    }

    fn write_starter_held_items(&mut self, held: &[Option<ItemId>]) -> Result<(), RomError> {
        let at = offsets(&self.entry, "StarterHeldItems");
        if held.len() != at.len() {
            return Err(RomError::Malformed(format!(
                "Gen 2 has {} starter held items here",
                at.len()
            )));
        }
        for (&at, &item) in at.iter().zip(held) {
            self.rom.write_byte(at, item_byte(item)?)?;
        }
        Ok(())
    }

    /// Writes what changed of the species: names, base stats, types, catch rate and growth curve.
    fn write_species(&mut self, species: &[Species]) -> Result<(), RomError> {
        let names = need(&self.entry, "PokemonNamesOffset")?;
        let name_len = need(&self.entry, "PokemonNamesLength")?;
        for s in species {
            let Some(old) = self.loaded.species(s.number).cloned() else {
                continue;
            };
            if old == *s {
                continue;
            }
            if old.name != s.name {
                let at = names + (usize::from(s.number) - 1) * name_len;
                let bytes = self.text.fixed(&s.name, name_len);
                self.rom.write(at, &bytes)?;
            }
            let at = self.stats_offset(s.number)?;
            if old.stats != s.stats {
                let b = s.stats;
                self.rom.write(
                    at + c::BS_HP,
                    &[b.hp, b.attack, b.defense, b.speed, b.sp_attack, b.sp_defense],
                )?;
            }
            if (old.primary_type, old.secondary_type) != (s.primary_type, s.secondary_type) {
                let primary = type_byte(s.primary_type)?;
                let secondary = s.secondary_type.map_or(Ok(primary), type_byte)?;
                self.rom.write(at + c::BS_PRIMARY_TYPE, &[primary, secondary])?;
            }
            if old.catch_rate != s.catch_rate {
                self.rom.write_byte(at + c::BS_CATCH_RATE, s.catch_rate)?;
            }
            if old.growth_curve != s.growth_curve {
                self.rom.write_byte(at + c::BS_GROWTH_CURVE, s.growth_curve.to_byte())?;
            }
        }
        Ok(())
    }

    /// The common and rare items wild Pokémon hold; a guaranteed item is both.
    fn write_wild_held_items(&mut self, held: &BTreeMap<DexNumber, WildHeldItems>) -> Result<(), RomError> {
        for (&n, h) in held {
            if self.loaded.wild_held_items.get(&n) == Some(h) {
                continue;
            }
            let (common, rare) = match h.guaranteed {
                Some(item) => (Some(item), Some(item)),
                None => (h.common, h.rare),
            };
            let at = self.stats_offset(n)?;
            self.rom
                .write(at + c::BS_COMMON_HELD_ITEM, &[item_byte(common)?, item_byte(rare)?])?;
        }
        Ok(())
    }

    /// Writes the evolutions and level-up moves of the species whose evolutions or level-up moves changed, moved to
    /// free space in their bank when they grow.
    fn write_evos_and_moves(&mut self, data: &GameData) -> Result<(), RomError> {
        let table = need(&self.entry, "PokemonMovesetsTableOffset")?;
        let evos_of = |d: &GameData, n: DexNumber| -> Vec<Evolution> {
            d.evolutions.iter().filter(|e| e.from == n).copied().collect()
        };
        for s in &data.species {
            let n = s.number;
            let new_moves = data.learnsets.get(&n).cloned().unwrap_or_default();
            let new_evos = evos_of(data, n);
            if Some(&new_moves) == self.loaded.learnsets.get(&n) && new_evos == evos_of(&self.loaded, n) {
                continue;
            }
            let mut bytes = Vec::new();
            for e in &new_evos {
                bytes.extend(evolution_bytes(e)?);
            }
            bytes.push(EVOS_AND_MOVES_TERMINATOR);
            for m in &new_moves {
                bytes.extend([m.level, move_byte(m.move_id)?]);
            }
            bytes.push(EVOS_AND_MOVES_TERMINATOR);
            let pointer = table + (usize::from(n) - 1) * 2;
            let old = self.rom.pointer(pointer)?;
            let old_length = self.rom.length_with_terminators(old, EVOS_AND_MOVES_TERMINATOR, 2)?;
            self.rom.rewrite_same_bank(pointer, &bytes, old_length)?;
        }
        Ok(())
    }

    /// Writes the egg moves of the species whose egg moves changed. A list shared with other species stays for
    /// them.
    fn write_egg_moves(&mut self, egg_moves: &BTreeMap<DexNumber, Vec<MoveId>>) -> Result<(), RomError> {
        let table = need(&self.entry, "EggMovesTableOffset")?;
        for number in 1..=c::SPECIES_COUNT {
            let n = DexNumber::try_from(number).unwrap_or_default();
            let new = egg_moves.get(&n).cloned().unwrap_or_default();
            if new == self.loaded.egg_moves.get(&n).cloned().unwrap_or_default() {
                continue;
            }
            let mut bytes = new.iter().map(|&m| move_byte(m)).collect::<Result<Vec<_>, _>>()?;
            bytes.push(c::EGG_MOVES_TERMINATOR);
            let pointer = table + (number - 1) * 2;
            let old = self.rom.pointer(pointer)?;
            let shared = (1..=c::SPECIES_COUNT)
                .filter(|&other| other != number)
                .any(|other| self.rom.pointer(table + (other - 1) * 2).is_ok_and(|p| p == old));
            if shared {
                self.rom.repoint_same_bank(pointer, &bytes)?;
            } else {
                let old_length = self.rom.length_with_terminators(old, c::EGG_MOVES_TERMINATOR, 1)?;
                self.rom.rewrite_same_bank(pointer, &bytes, old_length)?;
            }
        }
        Ok(())
    }

    /// The TMs and what people say about them.
    fn write_tms(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.tm_moves.len() != c::TM_COUNT {
            return Err(RomError::Malformed(format!("Gen 2 has {} TMs", c::TM_COUNT)));
        }
        let tms = need(&self.entry, "TMMovesOffset")?;
        let bytes: Vec<u8> = data.tm_moves.iter().map(|&m| move_byte(m)).collect::<Result<_, _>>()?;
        self.rom.write(tms, &bytes)?;
        for template in self.entry.list("TMText[]").to_vec() {
            let Some((number, offset, text)) = crate::gb::tm_text(&template) else {
                continue;
            };
            let Some(mv) = data
                .tm_moves
                .get(number - 1)
                .and_then(|&m| data.moves.iter().find(|x| x.number == m))
            else {
                continue;
            };
            self.write_text_in_place(offset, &text.replace("%m", &mv.name));
        }
        Ok(())
    }

    /// TM01 to TM50, then HM01 to HM07; the other bits of the last byte (the move tutors in Crystal) stay.
    fn write_tm_hm_compat(&mut self, data: &GameData) -> Result<(), RomError> {
        for (&n, flags) in &data.tm_hm_compat {
            if self.loaded.tm_hm_compat.get(&n) == Some(flags) {
                continue;
            }
            let at = self.stats_offset(n)? + c::BS_TM_HM_COMPAT;
            let mut bytes = [0u8; 8];
            for (i, _) in flags.iter().enumerate().filter(|&(_, &on)| on) {
                bytes[i / 8] |= 1 << (i % 8);
            }
            let flag_count = c::TM_COUNT + c::HM_COUNT;
            let kept = !((1u8 << (flag_count % 8)) - 1);
            bytes[7] |= self.rom.byte(at + 7)? & kept;
            self.rom.write(at, &bytes)?;
        }
        Ok(())
    }

    /// The move tutors' moves and menu (Crystal), which moves to free space in its bank when it grows.
    fn write_tutor_moves(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.tutor_moves.len() != c::TUTOR_COUNT {
            return Err(RomError::Malformed(format!("Gen 2 has {} move tutors", c::TUTOR_COUNT)));
        }
        let menu = optional(&self.entry, "MoveTutorMenuOffset");
        if menu == 0 {
            return Err(RomError::Malformed("this ROM's move tutors can't be changed".into()));
        }
        for (at, &m) in offsets(&self.entry, "MoveTutorMoves")
            .into_iter()
            .zip(&data.tutor_moves)
        {
            self.rom.write_byte(at, move_byte(m)?)?;
        }
        let cancel = repair_double_encoding(self.entry.value("CancelString").unwrap_or("CANCEL"));
        let mut options: Vec<String> = data
            .tutor_moves
            .iter()
            .map(|&m| {
                data.moves
                    .iter()
                    .find(|x| x.number == m)
                    .map(|x| x.name.clone())
                    .unwrap_or_default()
            })
            .collect();
        options.push(cancel);
        let mut bytes = vec![c::DIALOGUE_OPTION_START, u8::try_from(options.len()).unwrap_or(u8::MAX)];
        for o in &options {
            bytes.extend(self.text.terminated(o));
        }
        let old = self.rom.pointer(menu)?;
        if self.rom.byte(old)? != c::DIALOGUE_OPTION_START {
            return Err(RomError::Malformed(format!("no menu of choices at {old:#x}")));
        }
        let count = usize::from(self.rom.byte(old + 1)?);
        let old_length = 2 + self.rom.length_with_terminators(old + 2, TERMINATOR, count)?;
        self.rom.rewrite_same_bank(menu, &bytes, old_length)?;
        Ok(())
    }

    /// Bits 1 to 3 of the last TM and HM byte; the others stay.
    fn write_tutor_compat(&mut self, data: &GameData) -> Result<(), RomError> {
        for (&n, flags) in &data.tutor_compat {
            if self.loaded.tutor_compat.get(&n) == Some(flags) {
                continue;
            }
            let at = self.stats_offset(n)? + c::BS_TUTOR_COMPAT;
            let mut byte = self.rom.byte(at)? & !0b1110;
            for (i, _) in flags.iter().enumerate().take(c::TUTOR_COUNT).filter(|&(_, &on)| on) {
                byte |= 1 << (i + 1);
            }
            self.rom.write_byte(at, byte)?;
        }
        Ok(())
    }

    /// Rewrites every shop, in order, when any changed: each frees its old bytes and takes the first free space that
    /// fits in the table's bank or bank 0. Unchanged shops can move too, as in the original.
    fn write_shops(&mut self, shops: &[Shop]) -> Result<(), RomError> {
        let table = need(&self.entry, "ShopItemOffset")?;
        if shops.len() != self.loaded.shops.len() {
            return Err(RomError::Malformed(format!(
                "Gen 2 has {} shops here",
                self.loaded.shops.len()
            )));
        }
        let longer = optional(&self.entry, "LongerShopItems") == 1;
        for (i, shop) in shops.iter().enumerate() {
            let pointer = table + i * 2;
            let bytes = shop_bytes(&shop.items, longer)?;
            let at = self.rom.pointer(pointer)?;
            let old_length = shop_bytes(&read_shop(&self.rom, at, longer)?, longer)?.len();
            self.rom.rewrite_same_bank(pointer, &bytes, old_length)?;
        }
        Ok(())
    }

    /// Writes the prices that changed.
    /// Prices that don't fit are written as [`PriceFormat::fit`] makes them.
    fn write_prices(&mut self, prices: &BTreeMap<ItemId, u32>) -> Result<(), RomError> {
        let table = need(&self.entry, "ItemAttributesOffset")?;
        for (&id, &price) in prices {
            if self.loaded.item_prices.get(&id) == Some(&price) {
                continue;
            }
            if let Some(internal) = c::item_internal(id).filter(|&i| i != 0) {
                let at = table + (usize::from(internal) - 1) * c::ITEM_ATTRIBUTES_ENTRY_SIZE;
                let stored = PriceFormat::WORD.stored_u16(price);
                self.rom.write(at, &stored.to_le_bytes())?;
            }
        }
        Ok(())
    }

    /// The trades that changed; nicknames and trainer names where the ROM entry allows text changes. The gender the
    /// trader asks for goes.
    fn write_trades(&mut self, trades: &[InGameTrade]) -> Result<(), RomError> {
        let entries = trade_entries(&self.entry)?;
        if trades.len() != entries.len() {
            return Err(RomError::Malformed(format!("Gen 2 has {} trades here", entries.len())));
        }
        let name_len = need(&self.entry, "TradeNameLength")?;
        let ot_len = need(&self.entry, "TradeOTLength")?;
        let texts = optional(&self.entry, "CanChangeTrainerText") > 0;
        for ((t, old), &at) in trades.iter().zip(&self.loaded.trades).zip(&entries) {
            if t == old {
                continue;
            }
            let requested = t.requested.map_or(Ok(0), species_byte)?;
            self.rom.write(at + 1, &[requested, species_byte(t.given)?])?;
            if texts {
                let bytes = self.text.fixed(&t.nickname, name_len);
                self.rom.write(at + 3, &bytes)?;
            }
            let iv = |i: usize| t.ivs.get(i).copied().unwrap_or_default() & 0xF;
            self.rom.write(
                at + 3 + name_len,
                &[iv(0) << 4 | iv(1), iv(2) << 4 | iv(3), item_byte(t.held_item)?],
            )?;
            self.rom
                .write(at + 6 + name_len, &RomError::short_ot_id(t.ot_id)?.to_le_bytes())?;
            if texts {
                let bytes = self.text.fixed(&t.ot_name, ot_len);
                self.rom.write(at + 8 + name_len, &bytes)?;
            }
            self.rom.write_byte(at + 8 + name_len + ot_len, 0)?;
        }
        Ok(())
    }

    /// Writes the type chart: the pairs that aren't neutral in type order, then the immunities of Ghost, which
    /// Foresight skips.
    fn write_type_chart(&mut self, chart: &TypeChart) -> Result<(), RomError> {
        if chart.non_neutral_count() > c::TYPE_CHART_ROOM {
            return Err(RomError::Malformed(format!(
                "the type chart has {} pairs that are not neutral; Gen 2 has room for {}",
                chart.non_neutral_count(),
                c::TYPE_CHART_ROOM
            )));
        }
        let types: Vec<Type> = TYPE_ORDER.into_iter().filter(|t| chart.types().contains(t)).collect();
        let (mut main, mut ghost) = (Vec::new(), Vec::new());
        for &attacker in &types {
            for &defender in &types {
                let (value, part) = match chart.get(attacker, defender) {
                    Some(Effectiveness::Double) => (20, &mut main),
                    Some(Effectiveness::Half) => (5, &mut main),
                    Some(Effectiveness::Zero) if defender == Type::Ghost => (0, &mut ghost),
                    Some(Effectiveness::Zero) => (0, &mut main),
                    Some(Effectiveness::Neutral) | None => continue,
                };
                part.extend([type_byte(attacker)?, type_byte(defender)?, value]);
            }
        }
        main.push(c::TYPE_CHART_FORESIGHT);
        main.extend(ghost);
        main.push(c::TYPE_CHART_END);
        self.rom.write(need(&self.entry, "TypeEffectivenessOffset")?, &main)
    }

    /// The species everywhere it is named, the first level, the Game Corner prizes' names, and the Odd Egg
    /// Pokémon with their moves.
    fn write_statics(&mut self, data: &GameData) -> Result<(), RomError> {
        if optional(&self.entry, "StaticPokemonSupport") == 0 {
            return Ok(());
        }
        let odd_eggs = odd_egg_offsets(&self.entry);
        let statics = &data.statics;
        if statics.len() != self.statics.len() + odd_eggs.len() {
            return Err(RomError::Malformed(format!(
                "Gen 2 has {} static encounters here",
                self.statics.len() + odd_eggs.len()
            )));
        }
        if *statics != self.loaded.statics {
            self.patch_fleeing()?;
        }
        for (i, s) in statics.iter().enumerate().take(self.statics.len()) {
            if self.loaded.statics.get(i) == Some(s) {
                continue;
            }
            let slot = self.statics[i].clone();
            let byte = species_byte(s.species)?;
            let mut species_at = slot.spot.species.clone();
            if slot.game_corner
                && let Some(name_at) = species_at.pop()
            {
                let name = data.species(s.species).map(|sp| sp.name.clone()).unwrap_or_default();
                let length = need(&self.entry, "GameCornerPokemonNameLength")?;
                let mut bytes = self.text.encode(&" ".repeat(length));
                let name = self.text.encode(&name);
                for (b, n) in bytes.iter_mut().zip(name) {
                    *b = n;
                }
                self.rom.write(name_at, &bytes)?;
            }
            for at in species_at {
                self.rom.write_byte(at, byte)?;
            }
            if let Some(&at) = slot.spot.levels.first() {
                self.rom.write_byte(at, s.level)?;
            }
        }
        let size = optional(&self.entry, "StaticPokemonOddEggDataSize");
        for (s, (i, at)) in statics[self.statics.len()..]
            .iter()
            .zip(odd_eggs.into_iter().enumerate())
        {
            let unchanged = self.loaded.statics.get(self.statics.len() + i) == Some(s)
                && data.learnsets.get(&s.species) == self.loaded.learnsets.get(&s.species)
                && data.moves == self.loaded.moves;
            if unchanged || size == 0 {
                continue;
            }
            self.rom.write_byte(at, species_byte(s.species)?)?;
            // The last four moves learnt by level 5, then Dizzy Punch, which takes the place of the first of four.
            let mut moves: Vec<MoveId> = data
                .learnsets
                .get(&s.species)
                .into_iter()
                .flatten()
                .take_while(|m| m.level <= 5)
                .map(|m| m.move_id)
                .collect();
            moves.push(c::DIZZY_PUNCH);
            let moves = &moves[moves.len().saturating_sub(4)..];
            for i in 0..4 {
                let (m, pp) = match moves.get(i) {
                    Some(&m) => (m, data.moves.iter().find(|x| x.number == m).map_or(0, |x| x.pp)),
                    None => (0, 0),
                };
                self.rom.write_byte(at + 2 + i, move_byte(m)?)?;
                self.rom.write_byte(at + 23 + i, pp)?;
            }
        }
        Ok(())
    }

    /// Wild and static Pokémon no longer flee.
    fn patch_fleeing(&mut self) -> Result<(), RomError> {
        let at = need(&self.entry, "FleeingDataOffset")?;
        for set in c::FLEEING_SET_OFFSETS {
            self.rom.write_byte(at + set, 0xFF)?;
        }
        Ok(())
    }

    /// Unown can be met in the wild before the Ruins of Alph puzzles are solved: the check for Unown in the wild
    /// encounter routine always jumps past its special handling, and the loop that rolls another form until it
    /// finds one unlocked goes.
    fn patch_unown(&mut self) -> Result<(), RomError> {
        if optional(&self.entry, "AllowUnown") != 0 {
            return Ok(());
        }
        let at = offsets(&self.entry, "UnownAvailabilityPatchOffsets");
        let [jump, roll] = at.as_slice() else {
            return Err(RomError::Malformed(
                "the ROM entry needs two UnownAvailabilityPatchOffsets".into(),
            ));
        };
        if self.rom.byte(*jump)? != Z80_JUMP_RELATIVE_NZ || self.rom.byte(*roll)? != Z80_JUMP_RELATIVE_C {
            return Err(RomError::Malformed(
                "unexpected bytes at UnownAvailabilityPatchOffsets; the ROM entry may be faulty".into(),
            ));
        }
        self.rom.write_byte(*jump, Z80_JUMP_RELATIVE)?;
        self.rom.write(*roll, &[Z80_NOP, Z80_NOP])
    }

    /// Applies the tweaks newly chosen, and the catching tutorial.
    fn apply_tweaks(&mut self, data: &GameData) -> Result<(), RomError> {
        let new: Vec<MiscTweak> = data
            .applied_tweaks
            .difference(&self.loaded.applied_tweaks)
            .copied()
            .collect();
        for tweak in new {
            match tweak {
                MiscTweak::BwExpPatch => {
                    if let Some(name) = self.entry.value("BWXPTweak").map(str::to_owned) {
                        ips::named(&name)?.apply(self.rom.bytes_mut())?;
                    }
                }
                MiscTweak::FastestText => {
                    let at = optional(&self.entry, "TextDelayFunctionOffset");
                    if at != 0 {
                        self.rom.write_byte(at, Z80_RET)?;
                    }
                }
                MiscTweak::ReusableTms => self.make_tms_reusable()?,
                MiscTweak::ForgettableHms => {
                    // The "jr c" to the special handling of HMs when forgetting moves, made two NOPs.
                    let at = optional(&self.entry, "HMMovesForgettableFunctionOffset");
                    if at != 0 {
                        if self.rom.byte(at)? != Z80_JUMP_RELATIVE_C {
                            return Err(RomError::Malformed(
                                "unexpected byte at HMMovesForgettableFunctionOffset; the ROM entry may be faulty"
                                    .into(),
                            ));
                        }
                        self.rom.write(at, &[Z80_NOP, Z80_NOP])?;
                    }
                }
                MiscTweak::FastEggHatching => {
                    for n in 1..=c::SPECIES_COUNT {
                        let at = self.stats_offset(DexNumber::try_from(n).unwrap_or_default())?;
                        self.rom.write_byte(at + c::BS_EGG_CYCLES, 1)?;
                    }
                }
                // Done to the game data by the randomizer.
                _ => {}
            }
        }
        if let Some(t) = data.catching_tutorial {
            let byte = species_byte(t.opponent)?;
            for at in offsets(&self.entry, "CatchingTutorialOffsets") {
                self.rom.write_byte(at, byte)?;
            }
        }
        Ok(())
    }

    /// The call that uses up a taught TM becomes a relative jump to the code after the jump that follows it.
    fn make_tms_reusable(&mut self) -> Result<(), RomError> {
        let at = optional(&self.entry, "TMMovesReusableFunctionOffset");
        let length = optional(&self.entry, "TMMovesReusableFunctionJumpLength");
        if at == 0 || length == 0 {
            return Ok(());
        }
        if self.rom.byte(at)? != Z80_CALL || self.rom.byte(at + Z80_CALL_SIZE)? != Z80_JUMP_RELATIVE {
            return Err(RomError::Malformed(
                "unexpected bytes at TMMovesReusableFunctionOffset; the ROM entry may be faulty".into(),
            ));
        }
        let length = u8::try_from(length).map_err(|_| RomError::Malformed("jump too long".into()))?;
        self.rom.write(at, &[Z80_JUMP_RELATIVE, length])
    }

    /// The tutorial's "always catches" jump made unconditional.
    fn enable_guaranteed_catching(&mut self) -> Result<(), RomError> {
        let Some(prefix) = self.entry.value("GuaranteedCatchPrefix").and_then(hex_bytes) else {
            return Ok(());
        };
        if let Some(at) = self.rom.find_unique(&prefix).filter(|&a| a > 0) {
            let jump = at + prefix.len();
            if self.rom.byte(jump)? == Z80_JUMP_Z {
                self.rom.write_byte(jump, Z80_JUMP)?;
            }
        }
        Ok(())
    }

    /// Happiness evolutions at 160 instead of 220.
    fn ease_happiness(&mut self) -> Result<(), RomError> {
        if let Some(at) = self
            .rom
            .find_unique(&c::HAPPINESS_FOR_EVOLUTION_PREFIX)
            .filter(|&a| a > 0)
        {
            // The value is one byte into what is searched for.
            let value = at + 1;
            if self.rom.byte(value)? == c::VANILLA_HAPPINESS_TO_EVOLVE {
                self.rom.write_byte(value, c::EASIER_HAPPINESS_TO_EVOLVE)?;
            }
        }
        Ok(())
    }
}

impl GameHandler for Gen2Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    /// An intro Pokémon, starter held items, move tutors in Crystal, wild areas by location and time of day, shop
    /// sizes except in the Japanese games, and move names in the games with English text.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            intro_pokemon: true,
            starter_held_items: true,
            move_tutors: self.family == Family::Crystal,
            encounter_locations: true,
            shop_sizes: self.entry.non_japanese != 0,
            time_based_encounters: true,
            english: self.entry.name.contains("(U)") || self.entry.name.contains("SpeedChoice"),
            ..Capabilities::default()
        }
    }

    /// Shops grow into free space in their bank up to 255 items, but not in the Japanese games, which have none to
    /// spare; trade texts fit the trade table, and stay where the ROM entry keeps them; wild slots keep one level,
    /// but for the Bug Catching Contest's.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        rules.shops.size = if self.capabilities().shop_sizes {
            ppr_model::ShopSize::Grow { max: Some(255) }
        } else {
            ppr_model::ShopSize::Shrink
        };
        crate::gb::trade_text_rules(&mut rules.trades, &self.entry);
        let contest = self
            .loaded
            .encounters
            .iter()
            .enumerate()
            .filter(|(_, s)| s.name.eq_ignore_ascii_case("Bug Catching Contest"))
            .map(|(i, _)| (i, ppr_model::WildLevels::Range));
        rules.set_wild_levels(contest);
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.starters != self.loaded.starters {
            self.write_starters(data)?;
        }
        if data.starter_held_items != self.loaded.starter_held_items {
            self.write_starter_held_items(&data.starter_held_items)?;
        }
        if data.encounters != self.loaded.encounters {
            self.patch_fleeing()?;
            self.patch_unown()?;
            encounters::write(&mut self.rom, &self.encounters, &data.encounters, &species_byte)?;
        }
        if data.trades != self.loaded.trades {
            self.write_trades(&data.trades)?;
        }
        if data.type_chart != self.loaded.type_chart {
            self.write_type_chart(&data.type_chart)?;
        }
        self.write_species(&data.species)?;
        if data.wild_held_items != self.loaded.wild_held_items {
            self.write_wild_held_items(&data.wild_held_items)?;
        }
        if data.moves != self.loaded.moves {
            moves::write(&mut self.rom, &self.entry, &self.text, &self.loaded.moves, &data.moves)?;
        }
        if data.evolutions != self.loaded.evolutions || data.learnsets != self.loaded.learnsets {
            self.write_evos_and_moves(data)?;
        }
        if data.egg_moves != self.loaded.egg_moves {
            self.write_egg_moves(&data.egg_moves)?;
        }
        if data.statics != self.loaded.statics
            || data.learnsets != self.loaded.learnsets
            || data.moves != self.loaded.moves
        {
            self.write_statics(data)?;
        }
        if data.tm_moves != self.loaded.tm_moves {
            self.write_tms(data)?;
        }
        if data.tm_hm_compat != self.loaded.tm_hm_compat {
            self.write_tm_hm_compat(data)?;
        }
        if self.family == Family::Crystal {
            if data.tutor_moves != self.loaded.tutor_moves {
                self.write_tutor_moves(data)?;
            }
            if data.tutor_compat != self.loaded.tutor_compat {
                self.write_tutor_compat(data)?;
            }
        }
        if data.trainers != self.loaded.trainers {
            trainers::write(
                &mut self.rom,
                &self.entry,
                &self.text,
                &self.loaded.trainers,
                data,
                &species_byte,
            )?;
        }
        if data.trainer_class_names != self.loaded.trainer_class_names {
            trainers::write_class_names(
                &mut self.rom,
                &self.entry,
                &self.text,
                &self.loaded.trainer_class_names,
                &data.trainer_class_names,
            )?;
        }
        if let Some(n) = data.intro_pokemon.filter(|&n| n != c::UNOWN) {
            let byte = species_byte(n)?;
            self.rom.write_byte(need(&self.entry, "IntroSpriteOffset")?, byte)?;
            self.rom.write_byte(need(&self.entry, "IntroCryOffset")?, byte)?;
        }
        if data.shops != self.loaded.shops {
            self.write_shops(&data.shops)?;
        }
        if data.item_prices != self.loaded.item_prices {
            self.write_prices(&data.item_prices)?;
        }
        if data.field_items != self.loaded.field_items {
            if data.field_items.len() != self.field_item_offsets.len() {
                return Err(RomError::Malformed(format!(
                    "Gen 2 has {} field items here",
                    self.field_item_offsets.len()
                )));
            }
            for (&at, &id) in self.field_item_offsets.iter().zip(&data.field_items) {
                self.rom.write_byte(at, item_byte(Some(id))?)?;
            }
        }
        if data.guaranteed_catching && !self.loaded.guaranteed_catching {
            self.enable_guaranteed_catching()?;
        }
        if data.easier_happiness_evolutions && !self.loaded.easier_happiness_evolutions {
            self.ease_happiness()?;
        }
        self.apply_tweaks(data)?;
        self.write_graphics(data)
    }

    /// Chris and Kris in Crystal; Gold and Silver only have Chris.
    fn multiple_player_characters(&self) -> bool {
        self.family == Family::Crystal
    }

    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        out.write_all(self.rom.bytes())?;
        Ok(())
    }
}

impl Gen2Handler {
    /// The Pokémon palettes that changed, and the custom player graphics, if chosen, after everything else.
    fn write_graphics(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.palettes != self.loaded.palettes {
            graphics::write_palettes(&mut self.rom, &self.entry, &data.palettes, &self.loaded.palettes)?;
        }
        let Some(choice) = &data.player_graphics else {
            return Ok(());
        };
        let graphics = crate::graphics::gb_graphics(&choice.pack, GbGames::Gen2)
            .ok_or_else(|| RomError::Malformed(format!("no player graphics pack {} for this game", choice.pack)))?;
        graphics::write_player_graphics(
            &mut self.rom,
            &self.entry,
            self.family == Family::Crystal,
            &graphics,
            choice.replace,
        )
    }
}

/// The species byte: Gen 2 numbers species as the national Pokédex does.
fn species_byte(number: DexNumber) -> Result<u8, RomError> {
    u8::try_from(number)
        .ok()
        .filter(|&b| b != 0 && usize::from(b) <= c::SPECIES_COUNT)
        .ok_or_else(|| RomError::Malformed(format!("Gen 2 has no species {number}")))
}

/// The move byte; 0 for none.
fn move_byte(number: MoveId) -> Result<u8, RomError> {
    u8::try_from(number)
        .ok()
        .filter(|&b| usize::from(b) <= c::MOVE_COUNT)
        .ok_or_else(|| RomError::Malformed(format!("Gen 2 has no move {number}")))
}

/// The internal item index; 0 for none.
fn item_byte(item: Option<ItemId>) -> Result<u8, RomError> {
    match item {
        Some(id) => c::item_internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 2 has no item {id}"))),
        None => Ok(0),
    }
}

/// The item of an internal index; `None` for 0.
fn item_or_none(internal: u8) -> Option<ItemId> {
    (internal != 0).then(|| c::item_standard(internal))
}

fn type_byte(t: Type) -> Result<u8, RomError> {
    c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 2 has no type {t:?}")))
}

/// Bytes from a string of hexadecimal digits.
fn hex_bytes(s: &str) -> Option<Vec<u8>> {
    let s = s.trim();
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

fn stats_offset(entry: &RomEntry, number: DexNumber) -> Result<usize, RomError> {
    Ok(need(entry, "PokemonStatsOffset")? + (usize::from(number) - 1) * c::BASE_STATS_ENTRY_SIZE)
}

/// Names, base stats, types, catch rate and growth curve.
fn read_species(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<Species>, RomError> {
    let names = need(entry, "PokemonNamesOffset")?;
    let name_len = need(entry, "PokemonNamesLength")?;
    (1..=c::SPECIES_COUNT)
        .map(|i| {
            let number = DexNumber::try_from(i).map_err(|_| RomError::Malformed("too many species".into()))?;
            let name = text.read_string(rom.slice(names + (i - 1) * name_len, name_len)?, name_len, false);
            let b = rom.slice(stats_offset(entry, number)?, c::BASE_STATS_ENTRY_SIZE)?;
            let primary = c::type_from_byte(b[c::BS_PRIMARY_TYPE]).unwrap_or(Type::Normal);
            let secondary = c::type_from_byte(b[c::BS_SECONDARY_TYPE]).filter(|&t| t != primary);
            Ok(Species {
                forme: ppr_model::Forme::default(),
                number,
                name,
                primary_type: primary,
                secondary_type: secondary,
                stats: BaseStats {
                    hp: b[c::BS_HP],
                    attack: b[c::BS_ATTACK],
                    defense: b[c::BS_DEFENSE],
                    speed: b[c::BS_SPEED],
                    sp_attack: b[c::BS_SP_ATK],
                    sp_defense: b[c::BS_SP_DEF],
                },
                catch_rate: b[c::BS_CATCH_RATE],
                abilities: Vec::new(),
                growth_curve: ExpCurve::from_byte(b[c::BS_GROWTH_CURVE]).unwrap_or_default(),
            })
        })
        .collect()
}

/// The common and rare items of every species.
fn read_wild_held_items(
    rom: &GbRom,
    entry: &RomEntry,
    species: &[Species],
) -> Result<BTreeMap<DexNumber, WildHeldItems>, RomError> {
    species
        .iter()
        .map(|s| {
            let at = stats_offset(entry, s.number)?;
            Ok((
                s.number,
                WildHeldItems {
                    common: item_or_none(rom.byte(at + c::BS_COMMON_HELD_ITEM)?),
                    rare: item_or_none(rom.byte(at + c::BS_RARE_HELD_ITEM)?),
                    ..WildHeldItems::default()
                },
            ))
        })
        .collect()
}

/// Past the evolutions that start at `at`, and their terminator.
fn skip_evolutions(rom: &GbRom, mut at: usize) -> Result<usize, RomError> {
    loop {
        match rom.byte(at)? {
            0 => return Ok(at + 1),
            5 => at += 4,
            _ => at += 3,
        }
    }
}

/// The level-up moves after the evolutions.
fn read_learnsets(rom: &GbRom, entry: &RomEntry) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let table = need(entry, "PokemonMovesetsTableOffset")?;
    let mut learnsets = BTreeMap::new();
    for i in 1..=c::SPECIES_COUNT {
        let mut at = skip_evolutions(rom, rom.pointer(table + (i - 1) * 2)?)?;
        let mut moves = Vec::new();
        while rom.byte(at)? != 0 {
            moves.push(LevelUpMove {
                level: rom.byte(at)?,
                move_id: MoveId::from(rom.byte(at + 1)?),
            });
            at += 2;
        }
        learnsets.insert(DexNumber::try_from(i).unwrap_or_default(), moves);
    }
    Ok(learnsets)
}

/// The evolutions: by level, stone, trade (with an item or without), happiness (at any time, by day or by night)
/// and Tyrogue's level and stat comparison.
fn read_evolutions(rom: &GbRom, entry: &RomEntry) -> Result<Vec<Evolution>, RomError> {
    let table = need(entry, "PokemonMovesetsTableOffset")?;
    let mut evolutions: Vec<Evolution> = Vec::new();
    for i in 1..=c::SPECIES_COUNT {
        let from = DexNumber::try_from(i).unwrap_or_default();
        let mut at = rom.pointer(table + (i - 1) * 2)?;
        loop {
            let method = rom.byte(at)?;
            if method == 0 {
                break;
            }
            let b1 = rom.byte(at + 1)?;
            let (kind, parameter) = match method {
                1 => (EvolutionType::Level, u16::from(b1)),
                2 => (EvolutionType::Stone, c::item_standard(b1)),
                3 if b1 == NO_TRADE_ITEM => (EvolutionType::Trade, 0),
                3 => (EvolutionType::TradeItem, c::item_standard(b1)),
                4 => match b1 {
                    2 => (EvolutionType::HappinessDay, 0),
                    3 => (EvolutionType::HappinessNight, 0),
                    _ => (EvolutionType::Happiness, 0),
                },
                5 => match rom.byte(at + 2)? {
                    2 => (EvolutionType::LevelDefenseHigher, u16::from(b1)),
                    3 => (EvolutionType::LevelAtkDefSame, u16::from(b1)),
                    _ => (EvolutionType::LevelAttackHigher, u16::from(b1)),
                },
                other => return Err(RomError::Malformed(format!("evolution method {other} at {at:#x}"))),
            };
            let size = if method == 5 { 4 } else { 3 };
            let to = DexNumber::from(rom.byte(at + size - 1)?);
            if !evolutions
                .iter()
                .any(|e| e.from == from && e.to == to && e.kind == kind && e.parameter == parameter)
            {
                evolutions.push(Evolution::new(from, to, kind, parameter));
            }
            at += size;
        }
    }
    Ok(evolutions)
}

/// An evolution's bytes: the method, what it needs, and the species.
fn evolution_bytes(e: &Evolution) -> Result<Vec<u8>, RomError> {
    let level = u8::try_from(e.parameter).unwrap_or(u8::MAX);
    let item = || {
        c::item_internal(e.parameter).ok_or_else(|| RomError::Malformed(format!("Gen 2 has no item {}", e.parameter)))
    };
    let mut bytes = match e.kind {
        EvolutionType::Level => vec![1, level],
        EvolutionType::Stone => vec![2, item()?],
        EvolutionType::TradeItem => vec![3, item()?],
        EvolutionType::Trade => vec![3, NO_TRADE_ITEM],
        EvolutionType::Happiness => vec![4, 1],
        EvolutionType::HappinessDay => vec![4, 2],
        EvolutionType::HappinessNight => vec![4, 3],
        EvolutionType::LevelAttackHigher => vec![5, level, 1],
        EvolutionType::LevelDefenseHigher => vec![5, level, 2],
        EvolutionType::LevelAtkDefSame => vec![5, level, 3],
        other => return Err(RomError::Malformed(format!("Gen 2 has no {other:?} evolutions"))),
    };
    bytes.push(species_byte(e.to)?);
    Ok(bytes)
}

/// The egg moves of the species that have any.
fn read_egg_moves(rom: &GbRom, entry: &RomEntry) -> Result<BTreeMap<DexNumber, Vec<MoveId>>, RomError> {
    let table = need(entry, "EggMovesTableOffset")?;
    let mut out = BTreeMap::new();
    for i in 1..=c::SPECIES_COUNT {
        let mut at = rom.pointer(table + (i - 1) * 2)?;
        let mut moves = Vec::new();
        while rom.byte(at)? != c::EGG_MOVES_TERMINATOR {
            moves.push(MoveId::from(rom.byte(at)?));
            at += 1;
        }
        if !moves.is_empty() {
            out.insert(DexNumber::try_from(i).unwrap_or_default(), moves);
        }
    }
    Ok(out)
}

/// The starters, read from the first offset of each.
fn read_starters(rom: &GbRom, entry: &RomEntry) -> Result<Vec<DexNumber>, RomError> {
    (1..=3)
        .map(|i| {
            let at = *offsets(entry, &format!("StarterOffsets{i}"))
                .first()
                .ok_or_else(|| RomError::Malformed(format!("no StarterOffsets{i}")))?;
            Ok(DexNumber::from(rom.byte(at)?))
        })
        .collect()
}

/// The names as the game finds them, one after another until the end of their bank.
fn read_items(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<Item>, RomError> {
    const NAME_MAX: usize = 20;
    let start = need(entry, "ItemNamesOffset")?;
    let mut names: Vec<String> = vec![String::new(); 256];
    let mut at = start;
    for index in 1..=0x100usize {
        if at / crate::gb::BANK > start / crate::gb::BANK {
            // The game would read on into VRAM.
            break;
        }
        let name_at = at;
        while rom.byte(at)? != TERMINATOR {
            at += 1;
        }
        at += 1;
        let len = NAME_MAX.min(rom.len() - name_at);
        names[index % 256] = text.read_string(rom.slice(name_at, len)?, len, false);
    }
    let tms = c::TM01..c::TM01 + ItemId::try_from(c::TM_COUNT).unwrap_or_default();
    let mut items: Vec<Item> = (1..=255u8)
        .map(|internal| {
            let id = c::item_standard(internal);
            Item {
                id,
                name: names[usize::from(internal)].clone(),
                allowed: !c::item_banned(id),
                bad: c::item_bad(id),
                tm: tms.contains(&id),
            }
        })
        .collect();
    items.sort_by_key(|i| i.id);
    Ok(items)
}

/// Every item's price.
fn read_prices(rom: &GbRom, entry: &RomEntry) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let table = need(entry, "ItemAttributesOffset")?;
    (1..=255u8)
        .map(|internal| {
            let at = table + (usize::from(internal) - 1) * c::ITEM_ATTRIBUTES_ENTRY_SIZE;
            Ok((c::item_standard(internal), u32::from(rom.word(at)?)))
        })
        .collect()
}

/// A shop's items: their count, the items (each after a 1 in the speedchoice hacks), and 0xFF.
fn read_shop(rom: &GbRom, mut at: usize, longer: bool) -> Result<Vec<ItemId>, RomError> {
    let count = usize::from(rom.byte(at)?);
    at += 1 + usize::from(longer);
    let mut items = Vec::with_capacity(count);
    for _ in 0..count {
        items.push(c::item_standard(rom.byte(at)?));
        at += 1 + usize::from(longer);
    }
    if rom.byte(at)? != c::SHOP_ITEMS_TERMINATOR {
        return Err(RomError::Malformed(format!("invalid shop data at {at:#x}")));
    }
    Ok(items)
}

fn shop_bytes(items: &[ItemId], longer: bool) -> Result<Vec<u8>, RomError> {
    let mut bytes = vec![u8::try_from(items.len()).map_err(|_| RomError::Malformed("too many shop items".into()))?];
    for &id in items {
        if longer {
            bytes.push(1);
        }
        bytes.push(item_byte(Some(id))?);
    }
    if longer {
        bytes.push(1);
    }
    bytes.push(c::SHOP_ITEMS_TERMINATOR);
    Ok(bytes)
}

/// The shops, with their names and whether they are main-game or special.
fn read_shops(rom: &GbRom, entry: &RomEntry) -> Result<Vec<Shop>, RomError> {
    let table = need(entry, "ShopItemOffset")?;
    let longer = optional(entry, "LongerShopItems") == 1;
    let mut names: BTreeMap<usize, String> = BTreeMap::new();
    for value in entry.list("ShopName[]") {
        let inner = value.trim().trim_start_matches('[').trim_end_matches(']');
        if let Some((i, name)) = inner.split_once(',')
            && let Some(i) = crate::entries::ini::parse_int(i.trim()).and_then(|i| usize::try_from(i).ok())
        {
            names.insert(i, repair_double_encoding(name.trim()));
        }
    }
    let special = Some(offsets(entry, "SpecialShops"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| c::SPECIAL_SHOPS.to_vec());
    let main_game = Some(offsets(entry, "MainGameShops"))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| c::MAIN_GAME_SHOPS.to_vec());
    (0..need(entry, "ShopAmount")?)
        .map(|i| {
            let name = if names.is_empty() {
                c::SHOP_NAMES.get(i).copied().unwrap_or_default().to_owned()
            } else {
                names.get(&i).cloned().unwrap_or_default()
            };
            Ok(Shop {
                name,
                items: read_shop(rom, rom.pointer(table + i * 2)?, longer)?,
                main_game: main_game.contains(&i),
                special: special.contains(&i),
            })
        })
        .collect()
}

/// The offsets of the used trade entries, each the length of two names and nine bytes, made even.
fn trade_entries(entry: &RomEntry) -> Result<Vec<usize>, RomError> {
    let table = need(entry, "TradeTableOffset")?;
    let size = need(entry, "TradeTableSize")?;
    let length = (need(entry, "TradeNameLength")? + need(entry, "TradeOTLength")? + 9).next_multiple_of(2);
    let unused = offsets(entry, "TradesUnused");
    Ok((0..size)
        .filter(|i| !unused.contains(i))
        .map(|i| table + i * length)
        .collect())
}

/// The species, nickname, DVs, held item and original trainer of each trade.
fn read_trades(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<InGameTrade>, RomError> {
    let name_len = need(entry, "TradeNameLength")?;
    let ot_len = need(entry, "TradeOTLength")?;
    trade_entries(entry)?
        .into_iter()
        .map(|at| {
            let requested = DexNumber::from(rom.byte(at + 1)?);
            let (atk_def, spd_spc) = (rom.byte(at + 3 + name_len)?, rom.byte(at + 4 + name_len)?);
            Ok(InGameTrade {
                requested: (requested != 0).then_some(requested),
                given: DexNumber::from(rom.byte(at + 2)?),
                nickname: text.read_string(rom.slice(at + 3, name_len)?, name_len, false),
                ot_name: text.read_string(rom.slice(at + 8 + name_len, ot_len)?, ot_len, false),
                ot_id: rom.word(at + 6 + name_len)?.into(),
                ivs: vec![atk_def >> 4, atk_def & 0xF, spd_spc >> 4, spd_spc & 0xF],
                held_item: item_or_none(rom.byte(at + 5 + name_len)?),
            })
        })
        .collect()
}

/// TM01 to TM50, then HM01 to HM07.
fn read_tm_hm_compat(rom: &GbRom, entry: &RomEntry, species: &[Species]) -> Result<Compat, RomError> {
    species
        .iter()
        .map(|s| {
            let bytes = rom.slice(stats_offset(entry, s.number)? + c::BS_TM_HM_COMPAT, 8)?;
            let flags = (0..c::TM_COUNT + c::HM_COUNT)
                .map(|i| bytes[i / 8] >> (i % 8) & 1 == 1)
                .collect();
            Ok((s.number, flags))
        })
        .collect()
}

/// The move tutors' moves, and who can learn them (bits 1 to 3 of the last TM and HM byte).
fn read_tutors(rom: &GbRom, entry: &RomEntry, species: &[Species]) -> Result<(Vec<MoveId>, Compat), RomError> {
    let moves = offsets(entry, "MoveTutorMoves")
        .into_iter()
        .map(|at| Ok(MoveId::from(rom.byte(at)?)))
        .collect::<Result<Vec<_>, RomError>>()?;
    let compat = species
        .iter()
        .map(|s| {
            let byte = rom.byte(stats_offset(entry, s.number)? + c::BS_TUTOR_COMPAT)?;
            Ok((s.number, (1..=c::TUTOR_COUNT).map(|j| byte >> j & 1 == 1).collect()))
        })
        .collect::<Result<_, RomError>>()?;
    Ok((moves, compat))
}

/// The type chart: attacker, defender and effectiveness, until 0xFF; the 0xFE before the immunities of Ghost is
/// skipped.
fn read_type_chart(rom: &GbRom, entry: &RomEntry, types: &[Type]) -> Result<TypeChart, RomError> {
    let mut chart = TypeChart::neutral(types);
    let mut at = need(entry, "TypeEffectivenessOffset")?;
    while rom.byte(at)? != c::TYPE_CHART_END {
        if rom.byte(at)? == c::TYPE_CHART_FORESIGHT {
            at += 1;
            continue;
        }
        let b = rom.slice(at, 3)?;
        let effectiveness = match b[2] {
            20 => Some(Effectiveness::Double),
            10 => Some(Effectiveness::Neutral),
            5 => Some(Effectiveness::Half),
            0 => Some(Effectiveness::Zero),
            _ => None,
        };
        if let (Some(e), Some(attacker), Some(defender)) =
            (effectiveness, c::type_from_byte(b[0]), c::type_from_byte(b[1]))
        {
            chart.set(attacker, defender, e);
        }
        at += 3;
    }
    Ok(chart)
}

/// The static encounters of the ROM entry, the Game Corner prizes last; none without `StaticPokemonSupport`.
fn static_slots(entry: &RomEntry) -> Vec<StaticSlot> {
    if optional(entry, "StaticPokemonSupport") == 0 {
        return Vec::new();
    }
    let slots = |key: &str, game_corner: bool| -> Vec<StaticSlot> {
        entry
            .list(key)
            .iter()
            .map(|s| StaticSlot {
                spot: StaticSpot::parse(s),
                game_corner,
            })
            .collect()
    };
    let mut out = slots("StaticPokemon{}", false);
    out.extend(slots("StaticPokemonGameCorner{}", true));
    out
}

/// Where the Odd Egg Pokémon are; none when the ROM entry has no Odd Egg.
fn odd_egg_offsets(entry: &RomEntry) -> Vec<usize> {
    let at = optional(entry, "StaticPokemonOddEggOffset");
    let size = optional(entry, "StaticPokemonOddEggDataSize");
    if at == 0 {
        return Vec::new();
    }
    (0..c::ODD_EGG_COUNT).map(|i| at + i * size).collect()
}

/// The tweaks this ROM supports.
fn available_tweaks(entry: &RomEntry) -> Vec<MiscTweak> {
    let mut tweaks = vec![MiscTweak::LowerCaseNames];
    let set = |key: &str| optional(entry, key) != 0;
    for (on, tweak) in [
        (entry.value("BWXPTweak").is_some(), MiscTweak::BwExpPatch),
        (set("TextDelayFunctionOffset"), MiscTweak::FastestText),
        (
            !offsets(entry, "CatchingTutorialOffsets").is_empty(),
            MiscTweak::RandomizeCatchingTutorial,
        ),
        (true, MiscTweak::BanLuckyEgg),
        (
            set("TMMovesReusableFunctionOffset") && set("TMMovesReusableFunctionJumpLength"),
            MiscTweak::ReusableTms,
        ),
        (set("HMMovesForgettableFunctionOffset"), MiscTweak::ForgettableHms),
        (true, MiscTweak::FastEggHatching),
    ] {
        if on {
            tweaks.push(tweak);
        }
    }
    tweaks
}
