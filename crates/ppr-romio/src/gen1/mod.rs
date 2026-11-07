//! Generation 1 (Red, Green, Blue, Yellow).
//!
//! Like Gen 3, only what changed is written, so an unchanged game saves byte for byte.
//!
//! Deliberate choices (more in the submodules):
//! - TM prices are read and written in TM order: TM01 in the high nybble of the first byte.
//! - A Pokémon's fifth level-1 move is kept in its level-up moves (4 fit in the base stats).
//! - Starter and TM texts are only written when they fit where the old text was. A starter text that doesn't fit
//!   drops the "So! " in front.
//! - Species, move and item graphics are not ported yet.

mod constants;
mod encounters;
mod graphics;
mod maps;
mod moves;
#[cfg(test)]
mod tests;
mod trainers;

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{
    BaseStats, Capabilities, DexNumber, Effectiveness, Evolution, EvolutionType, ExpCurve, GameData, Generation,
    InGameTrade, Item, ItemId, LevelUpMove, MiscTweak, MoveId, PriceFormat, RomIdentity, Shop, Species,
    StaticEncounter, Type, TypeChart,
};

use self::constants::{self as c, Family};
use crate::entries::{RomEntry, StaticSpot, catalog};
use crate::gb::text::GbText;
use crate::gb::{GbRom, need, offsets, optional};
use crate::graphics::GbGames;
use crate::type_order::TYPE_ORDER;
use crate::{GameHandler, RomError, ips};

const Z80_NOP: u8 = 0x00;
const Z80_JUMP: u8 = 0xC3;
const Z80_JUMP_RELATIVE: u8 = 0x18;
const Z80_JUMP_RELATIVE_C: u8 = 0x38;
const Z80_RET: u8 = 0xC9;
const Z80_RET_C: u8 = 0xD8;
const Z80_XOR_A: u8 = 0xAF;
const Z80_LD_A: u8 = 0x3E;
const Z80_LD_A_TO_FAR: u8 = 0xEA;
const TYPE_CHART_END: u8 = 0xFF;
const TYPE_CHART_ROOM: usize = 82;
const EVOS_AND_MOVES_TERMINATOR: u8 = 0;

/// Loaded Gen 1 ROM.
#[derive(Debug)]
pub(crate) struct Gen1Handler {
    rom: GbRom,
    entry: RomEntry,
    family: Family,
    text: GbText,
    loaded: GameData,
    /// National number → internal index.
    dex_to_internal: Vec<u8>,
    /// Move number → internal index.
    move_internal: Vec<u8>,
    encounters: encounters::Encounters,
    statics: Vec<StaticSpot>,
    /// Offsets of the field items the randomizer may change.
    field_item_offsets: Vec<usize>,
}

impl Gen1Handler {
    #[expect(clippy::too_many_lines, reason = "a line for each field of GameData")]
    pub(crate) fn load(bytes: Vec<u8>, identity: RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(1).ok_or_else(|| RomError::Malformed("generation 1".into()))?;
        let entry = catalog(generation)
            .entries
            .iter()
            .find(|e| e.name == identity.name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {}", identity.name)))?;
        let family = Family::parse(&entry.family)
            .ok_or_else(|| RomError::Malformed(format!("unknown Gen 1 game type {}", entry.family)))?;
        let rom = GbRom::new(bytes, &entry)?;
        let text = GbText::for_entry(entry.value("ExtraTableFile"))?;

        let (internal_to_dex, dex_to_internal) = pokedex_order(&rom, &entry)?;
        let dex_of = |i: u8| internal_to_dex.get(usize::from(i)).copied().unwrap_or_default();
        let species = read_species(&rom, &entry, family, &text, &dex_to_internal)?;
        let (moves, move_internal) = moves::read(&rom, &entry, &text)?;
        let move_of = |i: u8| {
            move_internal
                .iter()
                .position(|&m| m == i && i != 0)
                .and_then(|n| MoveId::try_from(n).ok())
                .unwrap_or_default()
        };
        let learnsets = read_learnsets(&rom, &entry, family, &internal_to_dex, &move_of)?;
        let evolutions = read_evolutions(&rom, &entry, &internal_to_dex)?;
        let mut starters = read_starters(&rom, &entry, family, &dex_of)?;
        let items = read_items(&rom, &entry, &text)?;
        let maps = maps::load(&rom, &entry, &text)?;
        let statics = static_spots(&entry);
        let ghost_marowak = if optional(&entry, "StaticPokemonSupport") > 0 {
            let at = entry
                .list("StaticPokemonGhostMarowak{}")
                .first()
                .and_then(|s| StaticSpot::parse(s).species.first().copied())
                .ok_or_else(|| RomError::Malformed("no ghost Marowak in the ROM entry".into()))?;
            dex_of(rom.byte(at)?)
        } else {
            c::MAROWAK
        };
        let encounters = encounters::read(&rom, &entry, family, &maps, &dex_of, ghost_marowak)?;
        let static_encounters = statics
            .iter()
            .map(|s| {
                let at = *s
                    .species
                    .first()
                    .ok_or_else(|| RomError::Malformed("a static without species".into()))?;
                let level = match s.levels.first() {
                    Some(&l) => rom.byte(l)?,
                    None => 1,
                };
                Ok(StaticEncounter::new(dex_of(rom.byte(at)?), level))
            })
            .collect::<Result<Vec<_>, RomError>>()?;
        let trainers = trainers::read(&rom, &entry, family, &dex_of)?;
        for &(i, class, number, slot) in c::trainer_starters(family) {
            if let Some(p) = trainers::index_of(&trainers, class, number).and_then(|t| trainers[t].pokemon.get(slot)) {
                starters[i] = p.species;
            }
        }
        let field_item_offsets: Vec<usize> = maps::field_item_offsets(&rom, &entry, family, &maps)?
            .into_iter()
            .filter(|&o| rom.byte(o).is_ok_and(|b| !c::item_banned(c::item_standard(b))))
            .collect();
        let field_items = field_item_offsets
            .iter()
            .map(|&o| Ok(c::item_standard(rom.byte(o)?)))
            .collect::<Result<Vec<_>, RomError>>()?;
        let tm_offset = need(&entry, "TMMovesOffset")?;
        let tm_moves = (0..c::TM_COUNT)
            .map(|i| Ok(move_of(rom.byte(tm_offset + i)?)))
            .collect::<Result<Vec<_>, RomError>>()?;
        let hm_moves = (0..c::HM_COUNT)
            .map(|i| Ok(move_of(rom.byte(tm_offset + c::TM_COUNT + i)?)))
            .collect::<Result<Vec<_>, RomError>>()?;
        let types = Type::in_generation(generation);
        let all: Vec<DexNumber> = species.iter().map(|s| s.number).collect();

        let mut loaded = GameData {
            formes: ppr_model::FormeRules::default(),
            identity,
            type_chart: read_type_chart(&rom, &entry, &types)?,
            types,
            tm_hm_compat: read_tm_hm_compat(&rom, &entry, family, &species)?,
            species,
            evolutions,
            evolution_level_samples: Vec::new(),
            restricted_species: None,
            starters,
            starter_held_items: Vec::new(),
            starter_type_triangle: None,
            item_prices: read_prices(&rom, &entry, &items)?,
            items,
            wild_held_items: BTreeMap::new(),
            palettes: graphics::read_palettes(&rom, &entry, all.iter().copied())?,
            palette_files: None,
            player_graphics_packs: if entry.non_japanese == 0 {
                Vec::new()
            } else {
                crate::graphics::gb_packs(GbGames::Gen1)
            },
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
            trades: read_trades(&rom, &entry, &text, &dex_of)?,
            max_trade_nickname_length: 10,
            max_trade_ot_name_length: 7,
            moves,
            max_move_name_length: moves::MAX_NAME_LENGTH,
            learnsets,
            egg_moves: BTreeMap::new(),
            hm_moves,
            game_breaking_moves: c::GAME_BREAKING_MOVES.to_vec(),
            banned_level_up_moves: c::BANNED_LEVEL_UP_MOVES.to_vec(),
            four_starting_moves: true,
            tm_moves,
            tutor_moves: Vec::new(),
            tutor_compat: BTreeMap::new(),
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
            // No limit: Gen 1 trainers have no names, and names keep their length.
            max_trainer_name_length: usize::MAX,
            max_trainer_class_name_length: usize::MAX,
            trainer_name_shares_class_line: false,
            max_trainer_names_total: None,
            doubles_trainer_classes: Vec::new(),
            personal_trainer_classes: c::PERSONAL_TRAINER_CLASSES.to_vec(),
            fixed_trainer_text: optional(&entry, "CanChangeTrainerText") == 0,
            fixed_trainer_name_length: true,
            fixed_trainer_class_name_length: true,
            text_tokens: text.tokens(),
            intro_candidates: all.clone(),
            intro_pokemon: None,
            held_item_pools: ppr_model::HeldItemPools::default(),
            // The banks with trainer data have no room for more Pokémon of regular trainers, and the Japanese
            // games none at all; trainer Pokémon have no moves of their own.
            trainer_pokemon_additions: [entry.non_japanese != 0, entry.non_japanese != 0, false],
            trainer_custom_movesets: [false; 3],
            trainer_held_item_additions: [true; 3],
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
            dex_to_internal,
            move_internal,
            encounters,
            statics,
            field_item_offsets,
        })
    }

    fn internal(&self, number: DexNumber) -> Result<u8, RomError> {
        internal_index(&self.dex_to_internal, number)
    }

    fn move_internal(&self, number: MoveId) -> Result<u8, RomError> {
        self.move_internal
            .get(usize::from(number))
            .copied()
            .filter(|&m| m != 0)
            .ok_or_else(|| RomError::Malformed(format!("Gen 1 has no move {number}")))
    }

    /// Where a species' base stats are: Mew's are apart in Red and Blue.
    fn stats_offset(&self, number: DexNumber) -> Result<usize, RomError> {
        stats_offset(&self.entry, self.family, number)
    }

    /// The starters everywhere they are named, and in Red and Blue their text and the
    /// Pokédex entries the game turns on and off around choosing; in Yellow the Pikachu happiness check for the
    /// Bulbasaur gift goes when Pikachu is no longer the starter.
    fn write_starters(&mut self, data: &GameData) -> Result<(), RomError> {
        let count = starter_count(self.family);
        if data.starters.len() != count {
            return Err(RomError::Malformed(format!(
                "Gen 1 has {count} starters here, got {}",
                data.starters.len()
            )));
        }
        for (i, &s) in data.starters.iter().enumerate() {
            if c::trainer_starters(self.family).iter().any(|&(t, ..)| t == i) {
                // Written with the trainers.
                continue;
            }
            let internal = self.internal(s)?;
            for at in offsets(&self.entry, &format!("StarterOffsets{}", i + 1)) {
                self.rom.write_byte(at, internal)?;
            }
        }
        if self.family == Family::RedBlue {
            if optional(&self.entry, "CanChangeStarterText") > 0 {
                for (i, at) in offsets(&self.entry, "StarterTextOffsets")
                    .into_iter()
                    .enumerate()
                    .take(3)
                {
                    let Some(name) = data
                        .starters
                        .get(i)
                        .and_then(|&n| data.species(n))
                        .map(|s| s.name.clone())
                    else {
                        continue;
                    };
                    if !self.write_text_in_place(at, &format!("So! You want\\n{name}?\\e")) {
                        self.write_text_in_place(at, &format!("You want\\n{name}?\\e"));
                    }
                }
            }
            if optional(&self.entry, "PatchPokedex") > 0 {
                self.patch_starter_pokedex(&data.starters)?;
            }
        }
        let check = optional(&self.entry, "PikachuHappinessCheckOffset");
        if check != 0 && data.starters.first() != Some(&c::PIKACHU) {
            // ld a, [wPikachuHappiness]; cp 147; jr c, … ← the jump goes.
            self.rom.write(check, &[Z80_NOP, Z80_NOP])?;
        }
        Ok(())
    }

    /// The starters' Pokédex routine: turns their Pokédex bits on while choosing, and off
    /// after.
    fn patch_starter_pokedex(&mut self, starters: &[DexNumber]) -> Result<(), RomError> {
        let ram = need(&self.entry, "PokedexRamOffset")?;
        let mut on: BTreeMap<usize, u8> = BTreeMap::new();
        for &n in starters.iter().take(3) {
            let n = usize::from(n) - 1;
            *on.entry(n / 8 + ram).or_default() |= 1 << (n % 8);
        }
        let on_at = need(&self.entry, "StarterPokedexOnOffset")?;
        let off_at = need(&self.entry, "StarterPokedexOffOffset")?;
        let on_routine = need(&self.entry, "StarterPokedexBranchOffset")?;
        let off_routine = on_routine + 5 * on.len() + 3;
        let jump = |target: usize| {
            let [lo, hi] = crate::gb::gb_pointer(target).to_le_bytes();
            [Z80_JUMP, lo, hi]
        };
        self.rom
            .write(on_at, &[jump(on_routine).as_slice(), &[Z80_NOP, Z80_NOP]].concat())?;
        self.rom
            .write(off_at, &[jump(off_routine).as_slice(), &[Z80_NOP]].concat())?;
        let (mut on_code, mut off_code) = (Vec::new(), vec![Z80_XOR_A]);
        for (&address, &value) in &on {
            let [a_lo, a_hi] = u16::try_from(address).unwrap_or_default().to_le_bytes();
            on_code.extend([Z80_LD_A, value, Z80_LD_A_TO_FAR, a_lo, a_hi]);
            off_code.extend([Z80_LD_A_TO_FAR, a_lo, a_hi]);
        }
        on_code.extend(jump(on_at + 5));
        off_code.extend(jump(off_at + 4));
        self.rom.write(on_routine, &on_code)?;
        self.rom.write(off_routine, &off_code)
    }

    /// Text in the place of the old text at `offset`, if it fits there (terminator included).
    fn write_text_in_place(&mut self, offset: usize, text: &str) -> bool {
        crate::gb::text::write_in_place(&mut self.rom, &self.text, offset, text)
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
                let at = names + (usize::from(self.internal(s.number)?) - 1) * name_len;
                let bytes = self.text.fixed(&s.name, name_len);
                self.rom.write(at, &bytes)?;
            }
            let at = self.stats_offset(s.number)?;
            if old.stats != s.stats {
                let b = s.stats;
                self.rom
                    .write(at + c::BS_HP, &[b.hp, b.attack, b.defense, b.speed, b.sp_attack])?;
            }
            if (old.primary_type, old.secondary_type) != (s.primary_type, s.secondary_type) {
                let byte =
                    |t| c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 1 has no type {t:?}")));
                let primary = byte(s.primary_type)?;
                let secondary = s.secondary_type.map_or(Ok(primary), byte)?;
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

    /// Writes the evolutions and level-up moves of the species whose evolutions or level-up moves changed: the level-1
    /// moves in the base stats, the rest with the evolutions, moved to free space in their bank when they grow.
    fn write_evos_and_moves(&mut self, data: &GameData) -> Result<(), RomError> {
        let table = need(&self.entry, "PokemonMovesetsTableOffset")?;
        let evos_of = |d: &GameData, n: DexNumber| -> Vec<Evolution> {
            d.evolutions.iter().filter(|e| e.from == n).copied().collect()
        };
        for s in &data.species {
            let n = s.number;
            let new_moves = data.learnsets.get(&n).cloned().unwrap_or_default();
            let old_moves = self.loaded.learnsets.get(&n).cloned().unwrap_or_default();
            let new_evos = evos_of(data, n);
            if new_moves == old_moves && new_evos == evos_of(&self.loaded, n) {
                continue;
            }
            let mut level_1 = [0u8; 4];
            for (slot, m) in level_1.iter_mut().zip(&new_moves) {
                if m.level == 1 {
                    *slot = self.move_internal(m.move_id)?;
                }
            }
            let at = self.stats_offset(n)?;
            self.rom.write(at + c::BS_LEVEL_1_MOVES, &level_1)?;

            let mut bytes = Vec::new();
            for e in &new_evos {
                bytes.push(evolution_index(e.kind)?);
                match e.kind {
                    EvolutionType::Level => bytes.push(u8::try_from(e.parameter).unwrap_or(u8::MAX)),
                    EvolutionType::Stone => {
                        let item = c::item_internal(e.parameter)
                            .ok_or_else(|| RomError::Malformed(format!("Gen 1 has no item {}", e.parameter)))?;
                        // The item, then the lowest level.
                        bytes.extend([item, 1]);
                    }
                    // The lowest level.
                    _ => bytes.push(1),
                }
                bytes.push(self.internal(e.to)?);
            }
            bytes.push(EVOS_AND_MOVES_TERMINATOR);
            for (i, m) in new_moves.iter().enumerate() {
                if i < level_1.len() && m.level == 1 {
                    continue;
                }
                bytes.extend([m.level, self.move_internal(m.move_id)?]);
            }
            bytes.push(EVOS_AND_MOVES_TERMINATOR);
            let pointer = table + (usize::from(self.internal(n)?) - 1) * 2;
            let old = self.rom.pointer(pointer)?;
            let old_length = self.rom.length_with_terminators(old, EVOS_AND_MOVES_TERMINATOR, 2)?;
            self.rom.rewrite_same_bank(pointer, &bytes, old_length)?;
        }
        Ok(())
    }

    /// The TMs, the moves the gym leaders of Red and Blue use from their TMs, and what people
    /// say about them.
    fn write_tms(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.tm_moves.len() != c::TM_COUNT {
            return Err(RomError::Malformed(format!("Gen 1 has {} TMs", c::TM_COUNT)));
        }
        let tms = need(&self.entry, "TMMovesOffset")?;
        let internal: Vec<u8> = data
            .tm_moves
            .iter()
            .map(|&m| self.move_internal(m))
            .collect::<Result<_, _>>()?;
        self.rom.write(tms, &internal)?;
        if self.family == Family::RedBlue {
            let table = need(&self.entry, "GymLeaderMovesTableOffset")?;
            for (i, &tm) in c::GYM_LEADER_TMS.iter().enumerate() {
                self.rom.write_byte(table + i * 2, internal[tm - 1])?;
            }
        }
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

    fn write_tm_hm_compat(&mut self, data: &GameData) -> Result<(), RomError> {
        for (&n, flags) in &data.tm_hm_compat {
            if self.loaded.tm_hm_compat.get(&n) == Some(flags) {
                continue;
            }
            let mut bytes = [0u8; 7];
            for (i, _) in flags.iter().enumerate().filter(|&(_, &on)| on) {
                bytes[i / 8] |= 1 << (i % 8);
            }
            let at = self.stats_offset(n)? + c::BS_TM_HM_COMPAT;
            self.rom.write(at, &bytes)?;
        }
        Ok(())
    }

    /// Rewrites every shop, in order, when any changed: each frees its old bytes and takes the first free space that
    /// fits in its pointer's bank or bank 0. Unchanged shops can move too, as in the original.
    fn write_shops(&mut self, shops: &[Shop]) -> Result<(), RomError> {
        let pointers = offsets(&self.entry, "ShopPointerOffsets");
        if shops.len() != pointers.len() {
            return Err(RomError::Malformed(format!("Gen 1 has {} shops", pointers.len())));
        }
        for (shop, &pointer) in shops.iter().zip(&pointers) {
            let count = u8::try_from(shop.items.len()).map_err(|_| {
                RomError::Malformed(format!("a shop holds at most 255 items, not {}", shop.items.len()))
            })?;
            let mut bytes = vec![c::SHOP_ITEMS_SCRIPT, count];
            for &id in &shop.items {
                bytes.push(c::item_internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 1 has no item {id}")))?);
            }
            bytes.push(c::SHOP_ITEMS_TERMINATOR);
            let at = self.rom.pointer(pointer)?;
            let old_length = self.rom.length_with_terminators(at, c::SHOP_ITEMS_TERMINATOR, 1)?;
            self.rom.rewrite_same_bank(pointer, &bytes, old_length)?;
        }
        Ok(())
    }

    /// Writes the prices that changed.
    /// Prices that don't fit are written as [`PriceFormat::fit`] makes them.
    fn write_prices(&mut self, prices: &BTreeMap<ItemId, u32>) -> Result<(), RomError> {
        for (&id, &price) in prices {
            if self.loaded.item_prices.get(&id) == Some(&price) {
                continue;
            }
            match price_slot(&self.entry, id)? {
                Some(PriceSlot::Normal(at)) => {
                    write_decimal_price(&mut self.rom, at, PriceFormat::DECIMAL.fit(price))?;
                }
                Some(PriceSlot::Tm(at, high)) => {
                    let thousands = PriceFormat::THOUSANDS.stored(price);
                    self.rom
                        .write_nybble(at, high, u8::try_from(thousands).unwrap_or_default())?;
                }
                None => {}
            }
        }
        Ok(())
    }

    /// The species, and the nicknames where the ROM entry allows text changes.
    fn write_trades(&mut self, trades: &[InGameTrade]) -> Result<(), RomError> {
        let entries = trade_entries(&self.entry)?;
        if trades.len() != entries.len() {
            return Err(RomError::Malformed(format!("Gen 1 has {} trades here", entries.len())));
        }
        let name_len = need(&self.entry, "TradeNameLength")?;
        let texts = optional(&self.entry, "CanChangeTrainerText") > 0;
        for ((t, old), &at) in trades.iter().zip(&self.loaded.trades).zip(&entries) {
            if t == old {
                continue;
            }
            let requested = t.requested.map_or(Ok(0), |n| self.internal(n))?;
            self.rom.write(at, &[requested, self.internal(t.given)?])?;
            if texts {
                let bytes = self.text.fixed(&t.nickname, name_len);
                self.rom.write(at + 3, &bytes)?;
            }
        }
        Ok(())
    }

    /// Writes the type chart: the pairs that aren't neutral, in type order.
    fn write_type_chart(&mut self, chart: &TypeChart) -> Result<(), RomError> {
        if chart.non_neutral_count() > TYPE_CHART_ROOM {
            return Err(RomError::Malformed(format!(
                "the type chart has {} pairs that are not neutral; Gen 1 has room for {TYPE_CHART_ROOM}",
                chart.non_neutral_count()
            )));
        }
        let types: Vec<Type> = TYPE_ORDER.into_iter().filter(|t| chart.types().contains(t)).collect();
        let mut data = Vec::new();
        for &attacker in &types {
            for &defender in &types {
                let value = match chart.get(attacker, defender) {
                    Some(Effectiveness::Double) => 20,
                    Some(Effectiveness::Half) => 5,
                    Some(Effectiveness::Zero) => 0,
                    Some(Effectiveness::Neutral) | None => continue,
                };
                let byte = |t| c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 1 has no type {t}")));
                data.extend([byte(attacker)?, byte(defender)?, value]);
            }
        }
        data.push(TYPE_CHART_END);
        self.rom.write(need(&self.entry, "TypeEffectivenessOffset")?, &data)
    }

    /// Applies the tweaks newly chosen, and the PC Potion and catching tutorial.
    fn apply_tweaks(&mut self, data: &GameData) -> Result<(), RomError> {
        let new: Vec<MiscTweak> = data
            .applied_tweaks
            .difference(&self.loaded.applied_tweaks)
            .copied()
            .collect();
        for tweak in new {
            let patch = |key: &str| self.entry.value(key).map(str::to_owned);
            match tweak {
                MiscTweak::BwExpPatch => self.apply_patch(patch("BWXPTweak"))?,
                MiscTweak::NerfXAccuracy => self.apply_patch(patch("XAccNerfTweak"))?,
                MiscTweak::FixCritRate => self.apply_patch(patch("CritRateTweak"))?,
                MiscTweak::FastestText => {
                    let at = optional(&self.entry, "TextDelayFunctionOffset");
                    if at != 0 {
                        self.rom.write_byte(at, Z80_RET)?;
                    }
                }
                MiscTweak::AllowPikachuEvolution => {
                    let at = optional(&self.entry, "PikachuEvoJumpOffset");
                    if at != 0 {
                        self.rom.write_byte(at, Z80_JUMP_RELATIVE)?;
                    }
                }
                MiscTweak::ReusableTms => {
                    // A conditional "ret c" made unconditional, which skips using up the TM.
                    self.patch_byte("TMMovesReusableFunctionOffset", Z80_RET_C, &[Z80_RET])?;
                }
                MiscTweak::ForgettableHms => {
                    // The "jr c" to the special handling of HMs when forgetting moves, made two NOPs.
                    self.patch_byte(
                        "HMMovesForgettableFunctionOffset",
                        Z80_JUMP_RELATIVE_C,
                        &[Z80_NOP, Z80_NOP],
                    )?;
                }
                // Done to the game data by the randomizer.
                _ => {}
            }
        }
        if let Some(item) = data.pc_potion {
            let at = optional(&self.entry, "PCPotionOffset");
            if at != 0 {
                let internal =
                    c::item_internal(item).ok_or_else(|| RomError::Malformed(format!("Gen 1 has no item {item}")))?;
                self.rom.write_byte(at, internal)?;
            }
        }
        if let Some(t) = data.catching_tutorial {
            let at = optional(&self.entry, "CatchingTutorialMonOffset");
            if at != 0 {
                let internal = self.internal(t.opponent)?;
                self.rom.write_byte(at, internal)?;
            }
        }
        Ok(())
    }

    fn apply_patch(&mut self, name: Option<String>) -> Result<(), RomError> {
        match name {
            Some(name) => ips::named(&name)?.apply(self.rom.bytes_mut()),
            None => Ok(()),
        }
    }

    /// `bytes` at the entry's `key` offset, which must hold `expected`.
    fn patch_byte(&mut self, key: &str, expected: u8, bytes: &[u8]) -> Result<(), RomError> {
        let at = optional(&self.entry, key);
        if at == 0 {
            return Ok(());
        }
        if self.rom.byte(at)? != expected {
            return Err(RomError::Malformed(format!(
                "unexpected byte at {key}; the ROM entry may be faulty"
            )));
        }
        self.rom.write(at, bytes)
    }

    /// The Master Ball's "always catches" jump made unconditional.
    fn enable_guaranteed_catching(&mut self) -> Result<(), RomError> {
        if let Some(at) = self.rom.find_unique(&c::GUARANTEED_CATCH_PREFIX).filter(|&a| a > 0) {
            let jump = at + c::GUARANTEED_CATCH_PREFIX.len();
            if self.rom.byte(jump)? == 0xCA {
                self.rom.write_byte(jump, Z80_JUMP)?;
            }
        }
        Ok(())
    }
}

impl GameHandler for Gen1Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    /// Gen 1 supports an intro Pokémon, wild areas by map and location, shop sizes except in
    /// Japanese Yellow, and move names in the games with English text.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            intro_pokemon: true,
            map_indices: true,
            encounter_locations: true,
            shop_sizes: !(self.family == Family::Yellow && self.entry.non_japanese == 0),
            english: self
                .entry
                .value("ExtraTableFile")
                .is_some_and(|t| t.eq_ignore_ascii_case("rby_english")),
            ..Capabilities::default()
        }
    }

    /// Shops grow into free space in their bank up to 255 items, but not in Japanese Yellow, which has none to spare;
    /// trade nicknames fit the trade table, and stay where the ROM entry keeps the texts; Yellow's rival has the
    /// second starter in his first battle.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        rules.shops.size = if self.capabilities().shop_sizes {
            ppr_model::ShopSize::Grow { max: Some(255) }
        } else {
            ppr_model::ShopSize::Shrink
        };
        crate::gb::trade_text_rules(&mut rules.trades, &self.entry);
        rules.starter_links = c::trainer_starters(self.family)
            .iter()
            .filter_map(|&(starter, class, number, slot)| {
                let t = trainers::index_of(&self.loaded.trainers, class, number)?;
                Some(ppr_model::StarterLink {
                    starter,
                    trainer: self.loaded.trainers[t].index,
                    slot,
                })
            })
            .collect();
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.starters != self.loaded.starters {
            self.write_starters(data)?;
        }
        if data.encounters != self.loaded.encounters {
            let dex_to_internal = &self.dex_to_internal;
            encounters::write(&mut self.rom, &self.encounters, &data.encounters, &|n| {
                internal_index(dex_to_internal, n)
            })?;
        }
        if data.statics != self.loaded.statics {
            self.write_statics(&data.statics)?;
        }
        if data.trades != self.loaded.trades {
            self.write_trades(&data.trades)?;
        }
        if data.type_chart != self.loaded.type_chart {
            self.write_type_chart(&data.type_chart)?;
        }
        self.write_species(&data.species)?;
        if data.moves != self.loaded.moves {
            moves::write(
                &mut self.rom,
                &self.entry,
                &self.text,
                &self.move_internal,
                &self.loaded.moves,
                &data.moves,
            )?;
        }
        if data.evolutions != self.loaded.evolutions || data.learnsets != self.loaded.learnsets {
            self.write_evos_and_moves(data)?;
        }
        if data.tm_moves != self.loaded.tm_moves {
            self.write_tms(data)?;
        }
        if data.tm_hm_compat != self.loaded.tm_hm_compat {
            self.write_tm_hm_compat(data)?;
        }
        let mut new_trainers = Cow::Borrowed(&data.trainers);
        for &(i, class, number, slot) in c::trainer_starters(self.family) {
            let Some(&starter) = data
                .starters
                .get(i)
                .filter(|&&s| self.loaded.starters.get(i) != Some(&s))
            else {
                continue;
            };
            if let Some(t) = trainers::index_of(&new_trainers, class, number)
                && let Some(p) = new_trainers.to_mut()[t].pokemon.get_mut(slot)
            {
                p.species = starter;
            }
        }
        if *new_trainers != self.loaded.trainers {
            let dex_to_internal = &self.dex_to_internal;
            trainers::write(
                &mut self.rom,
                &self.entry,
                self.family,
                &self.loaded.trainers,
                &new_trainers,
                &|n| internal_index(dex_to_internal, n),
            )?;
        }
        if data.trainer_class_names != self.loaded.trainer_class_names {
            trainers::write_class_names(&mut self.rom, &self.entry, &self.text, &data.trainer_class_names)?;
        }
        if let Some(n) = data.intro_pokemon {
            let internal = self.internal(n)?;
            self.rom
                .write_byte(need(&self.entry, "IntroPokemonOffset")?, internal)?;
            self.rom.write_byte(need(&self.entry, "IntroCryOffset")?, internal)?;
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
                    "Gen 1 has {} field items here",
                    self.field_item_offsets.len()
                )));
            }
            for (&at, &id) in self.field_item_offsets.iter().zip(&data.field_items) {
                let internal =
                    c::item_internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 1 has no item {id}")))?;
                self.rom.write_byte(at, internal)?;
            }
        }
        if data.guaranteed_catching && !self.loaded.guaranteed_catching {
            self.enable_guaranteed_catching()?;
        }
        self.apply_tweaks(data)?;
        self.write_graphics(data)
    }

    /// One player character: the second can't be chosen.
    fn multiple_player_characters(&self) -> bool {
        false
    }

    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        out.write_all(self.rom.bytes())?;
        Ok(())
    }
}

impl Gen1Handler {
    /// The Pokémon palettes that changed, and the custom player graphics, if chosen, after everything else.
    fn write_graphics(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.palettes != self.loaded.palettes {
            graphics::write_palettes(&mut self.rom, &self.entry, &data.palettes, &self.loaded.palettes)?;
        }
        let Some(choice) = &data.player_graphics else {
            return Ok(());
        };
        if self.entry.non_japanese == 0 {
            return Err(RomError::Malformed(
                "the Japanese games can't get custom player graphics".into(),
            ));
        }
        let graphics = crate::graphics::gb_graphics(&choice.pack, GbGames::Gen1)
            .ok_or_else(|| RomError::Malformed(format!("no player graphics pack {} for this game", choice.pack)))?;
        graphics::write_player_graphics(&mut self.rom, &self.entry, self.family, &graphics)
    }

    /// The species everywhere it is named, and the first level.
    fn write_statics(&mut self, statics: &[StaticEncounter]) -> Result<(), RomError> {
        if statics.len() != self.statics.len() {
            return Err(RomError::Malformed(format!(
                "Gen 1 has {} static encounters here",
                self.statics.len()
            )));
        }
        for (i, s) in statics.iter().enumerate() {
            if self.loaded.statics.get(i) == Some(s) {
                continue;
            }
            let internal = self.internal(s.species)?;
            for &at in &self.statics[i].species {
                self.rom.write_byte(at, internal)?;
            }
            if let Some(&at) = self.statics[i].levels.first() {
                self.rom.write_byte(at, s.level)?;
            }
        }
        Ok(())
    }
}

fn internal_index(dex_to_internal: &[u8], number: DexNumber) -> Result<u8, RomError> {
    dex_to_internal
        .get(usize::from(number))
        .copied()
        .filter(|&i| i != 0)
        .ok_or_else(|| RomError::Malformed(format!("Gen 1 has no species {number}")))
}

/// 2 starters in Yellow (the second is the rival's Eevee), 3 in Red and Blue.
fn starter_count(family: Family) -> usize {
    match family {
        Family::Yellow => 2,
        Family::RedBlue => 3,
    }
}

/// Internal index → national number, and back (the first internal index wins).
fn pokedex_order(rom: &GbRom, entry: &RomEntry) -> Result<(Vec<DexNumber>, Vec<u8>), RomError> {
    let count = need(entry, "InternalPokemonCount")?;
    let order = need(entry, "PokedexOrder")?;
    let mut internal_to_dex = vec![0; count + 1];
    let mut dex_to_internal = vec![0u8; 256];
    for (i, slot) in internal_to_dex.iter_mut().enumerate().skip(1) {
        let dex = rom.byte(order + i - 1)?;
        *slot = DexNumber::from(dex);
        if dex != 0 && dex_to_internal[usize::from(dex)] == 0 {
            dex_to_internal[usize::from(dex)] =
                u8::try_from(i).map_err(|_| RomError::Malformed("too many species".into()))?;
        }
    }
    let highest = internal_to_dex.iter().copied().max().unwrap_or_default();
    dex_to_internal.truncate(usize::from(highest) + 1);
    Ok((internal_to_dex, dex_to_internal))
}

fn stats_offset(entry: &RomEntry, family: Family, number: DexNumber) -> Result<usize, RomError> {
    if number == c::MEW && family == Family::RedBlue {
        need(entry, "MewStatsOffset")
    } else {
        Ok(need(entry, "PokemonStatsOffset")? + (usize::from(number) - 1) * c::BASE_STATS_ENTRY_SIZE)
    }
}

/// Names, base stats (the Special as the special attack), types, catch rate and growth
/// curve.
fn read_species(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    text: &GbText,
    dex_to_internal: &[u8],
) -> Result<Vec<Species>, RomError> {
    let names = need(entry, "PokemonNamesOffset")?;
    let name_len = need(entry, "PokemonNamesLength")?;
    (1..dex_to_internal.len())
        .map(|i| {
            let number = DexNumber::try_from(i).map_err(|_| RomError::Malformed("too many species".into()))?;
            let internal = usize::from(dex_to_internal[i]);
            let name = text.read_string(
                rom.slice(names + (internal.max(1) - 1) * name_len, name_len)?,
                name_len,
                false,
            );
            let b = rom.slice(stats_offset(entry, family, number)?, c::BASE_STATS_ENTRY_SIZE)?;
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
                    sp_attack: b[c::BS_SPECIAL],
                    sp_defense: 0,
                },
                catch_rate: b[c::BS_CATCH_RATE],
                abilities: Vec::new(),
                growth_curve: ExpCurve::from_byte(b[c::BS_GROWTH_CURVE]).unwrap_or_default(),
            })
        })
        .collect()
}

/// The level-1 moves in the base stats, then the level-up moves after the evolutions.
fn read_learnsets(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    internal_to_dex: &[DexNumber],
    move_of: &dyn Fn(u8) -> MoveId,
) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let table = need(entry, "PokemonMovesetsTableOffset")?;
    let mut learnsets = BTreeMap::new();
    for (i, &n) in internal_to_dex.iter().enumerate().skip(1) {
        if n == 0 {
            continue;
        }
        let stats = stats_offset(entry, family, n)?;
        let mut moves: Vec<LevelUpMove> = rom
            .slice(stats + c::BS_LEVEL_1_MOVES, 4)?
            .iter()
            .filter(|&&m| m != 0)
            .map(|&m| LevelUpMove {
                level: 1,
                move_id: move_of(m),
            })
            .collect();
        let mut at = skip_evolutions(rom, rom.pointer(table + (i - 1) * 2)?)?;
        while rom.byte(at)? != 0 {
            moves.push(LevelUpMove {
                level: rom.byte(at)?,
                move_id: move_of(rom.byte(at + 1)?),
            });
            at += 2;
        }
        learnsets.insert(n, moves);
    }
    Ok(learnsets)
}

/// Past the evolutions that start at `at`, and their terminator.
fn skip_evolutions(rom: &GbRom, mut at: usize) -> Result<usize, RomError> {
    loop {
        match rom.byte(at)? {
            0 => return Ok(at + 1),
            2 => at += 4,
            1 | 3 => at += 3,
            other => return Err(RomError::Malformed(format!("evolution method {other} at {at:#x}"))),
        }
    }
}

fn evolution_index(kind: EvolutionType) -> Result<u8, RomError> {
    match kind {
        EvolutionType::Level => Ok(1),
        EvolutionType::Stone => Ok(2),
        EvolutionType::Trade => Ok(3),
        other => Err(RomError::Malformed(format!("Gen 1 has no {other:?} evolutions"))),
    }
}

/// A stone evolution's parameter is the stone; a trade's is its lowest level, 1.
fn read_evolutions(rom: &GbRom, entry: &RomEntry, internal_to_dex: &[DexNumber]) -> Result<Vec<Evolution>, RomError> {
    let table = need(entry, "PokemonMovesetsTableOffset")?;
    let mut evolutions: Vec<Evolution> = Vec::new();
    for (i, &from) in internal_to_dex.iter().enumerate().skip(1) {
        if from == 0 {
            continue;
        }
        let mut at = rom.pointer(table + (i - 1) * 2)?;
        loop {
            let (kind, to, parameter, size) = match rom.byte(at)? {
                0 => break,
                1 => (EvolutionType::Level, rom.byte(at + 2)?, u16::from(rom.byte(at + 1)?), 3),
                2 => (
                    EvolutionType::Stone,
                    rom.byte(at + 3)?,
                    c::item_standard(rom.byte(at + 1)?),
                    4,
                ),
                3 => (EvolutionType::Trade, rom.byte(at + 2)?, u16::from(rom.byte(at + 1)?), 3),
                other => return Err(RomError::Malformed(format!("evolution method {other} at {at:#x}"))),
            };
            let to = internal_to_dex.get(usize::from(to)).copied().unwrap_or_default();
            let evo = Evolution::new(from, to, kind, parameter);
            if !evolutions
                .iter()
                .any(|e| e.from == from && e.to == to && e.kind == kind && e.parameter == parameter)
            {
                evolutions.push(evo);
            }
            at += size;
        }
    }
    Ok(evolutions)
}

/// The starters, read from the first offset of each.
fn read_starters(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    dex_of: &dyn Fn(u8) -> DexNumber,
) -> Result<Vec<DexNumber>, RomError> {
    (1..=starter_count(family))
        .map(|i| {
            let at = *offsets(entry, &format!("StarterOffsets{i}"))
                .first()
                .ok_or_else(|| RomError::Malformed(format!("no StarterOffsets{i}")))?;
            Ok(dex_of(rom.byte(at)?))
        })
        .collect()
}

/// The names as the game finds them, one after another until the end of
/// their bank; TMs and HMs are named by number.
fn read_items(rom: &GbRom, entry: &RomEntry, text: &GbText) -> Result<Vec<Item>, RomError> {
    const NAME_MAX: usize = 20;
    let start = need(entry, "ItemNamesOffset")?;
    let mut names: Vec<Option<String>> = vec![None; 256];
    let mut at = start;
    for index in 1..=0x100usize {
        if at / crate::gb::BANK > start / crate::gb::BANK {
            // The game would read on into VRAM.
            break;
        }
        let name_at = at;
        while rom.byte(at)? != crate::gb::text::TERMINATOR {
            at += 1;
        }
        at += 1;
        let len = NAME_MAX.min(rom.len() - name_at);
        names[index % 256] = Some(text.read_string(rom.slice(name_at, len)?, len, false));
    }
    for i in 0..c::HM_COUNT {
        let hm = c::HM01 + ItemId::try_from(i).unwrap_or_default();
        if let Some(internal) = c::item_internal(hm) {
            names[usize::from(internal)] = Some(format!("HM{:02}", i + 1));
        }
    }
    for i in 0..c::TM_COUNT {
        let tm = c::TM01 + ItemId::try_from(i).unwrap_or_default();
        if let Some(internal) = c::item_internal(tm) {
            names[usize::from(internal)] = Some(format!("TM{:02}", i + 1));
        }
    }
    let tms = c::TM01..c::TM01 + ItemId::try_from(c::TM_COUNT).unwrap_or_default();
    let mut items: Vec<Item> = (1..=255u8)
        .map(|internal| {
            let id = c::item_standard(internal);
            Item {
                id,
                name: names[usize::from(internal)].clone().unwrap_or_default(),
                allowed: !c::item_banned(id),
                bad: false,
                tm: tms.contains(&id),
            }
        })
        .collect();
    items.sort_by_key(|i| i.id);
    Ok(items)
}

/// Where an item's price is stored.
enum PriceSlot {
    /// Three bytes of decimal digits.
    Normal(usize),
    /// A nybble of thousands: the byte, and whether it is the high nybble.
    Tm(usize, bool),
}

/// Where prices are: items up to Max Elixir by internal index; TMs two to a byte, TM01 high.
fn price_slot(entry: &RomEntry, id: ItemId) -> Result<Option<PriceSlot>, RomError> {
    let Some(internal) = c::item_internal(id) else {
        return Ok(None);
    };
    let max_elixir = c::item_internal(c::MAX_ELIXIR).unwrap_or_default();
    let tms = c::TM01..c::TM01 + ItemId::try_from(c::TM_COUNT).unwrap_or_default();
    if internal != 0 && internal <= max_elixir {
        Ok(Some(PriceSlot::Normal(
            need(entry, "ShopPricesOffset")? + 3 * (usize::from(internal) - 1),
        )))
    } else if tms.contains(&id) {
        let n = usize::from(id - c::TM01);
        Ok(Some(PriceSlot::Tm(
            need(entry, "TMShopPricesOffset")? + n / 2,
            n % 2 == 0,
        )))
    } else {
        Ok(None)
    }
}

/// Every item with a price slot.
fn read_prices(rom: &GbRom, entry: &RomEntry, items: &[Item]) -> Result<BTreeMap<ItemId, u32>, RomError> {
    let mut prices = BTreeMap::new();
    for item in items {
        let price = match price_slot(entry, item.id)? {
            Some(PriceSlot::Normal(at)) => read_decimal_price(rom, at)?,
            Some(PriceSlot::Tm(at, high)) => u32::from(rom.nybble(at, high)?) * 1000,
            None => continue,
        };
        prices.insert(item.id, price);
    }
    Ok(prices)
}

/// Six decimal digits, one per nybble.
fn read_decimal_price(rom: &GbRom, at: usize) -> Result<u32, RomError> {
    rom.slice(at, 3)?.iter().try_fold(0u32, |acc, &b| {
        Ok(acc * 100 + u32::from(b >> 4) * 10 + u32::from(b & 0xF))
    })
}

/// Writes six decimal digits, one per nybble.
fn write_decimal_price(rom: &mut GbRom, at: usize, value: u32) -> Result<(), RomError> {
    let digits = |v: u32| u8::try_from(v % 10).unwrap_or_default();
    let bytes = [
        digits(value / 100_000) << 4 | digits(value / 10_000),
        digits(value / 1000) << 4 | digits(value / 100),
        digits(value / 10) << 4 | digits(value),
    ];
    rom.write(at, &bytes)
}

/// The shops, with their names and whether they are main-game or special.
fn read_shops(rom: &GbRom, entry: &RomEntry) -> Result<Vec<Shop>, RomError> {
    offsets(entry, "ShopPointerOffsets")
        .into_iter()
        .enumerate()
        .map(|(i, pointer)| {
            let mut at = rom.pointer(pointer)?;
            if rom.byte(at)? != c::SHOP_ITEMS_SCRIPT {
                return Err(RomError::Malformed(format!("invalid start of shop data at {at:#x}")));
            }
            let count = usize::from(rom.byte(at + 1)?);
            at += 2;
            let items = rom.slice(at, count)?.iter().map(|&b| c::item_standard(b)).collect();
            if rom.byte(at + count)? != c::SHOP_ITEMS_TERMINATOR {
                return Err(RomError::Malformed("shop size mismatch or terminator missing".into()));
            }
            Ok(Shop {
                name: c::SHOP_NAMES.get(i).copied().unwrap_or_default().to_owned(),
                items,
                main_game: true,
                special: c::SPECIAL_SHOPS.contains(&i),
            })
        })
        .collect()
}

/// The offsets of the used trade entries.
fn trade_entries(entry: &RomEntry) -> Result<Vec<usize>, RomError> {
    let table = need(entry, "TradeTableOffset")?;
    let size = need(entry, "TradeTableSize")?;
    let length = need(entry, "TradeNameLength")? + 3;
    let unused = offsets(entry, "TradesUnused");
    Ok((0..size)
        .filter(|i| !unused.contains(i))
        .map(|i| table + i * length)
        .collect())
}

/// The species and the nickname; Gen 1 trades have nothing else.
fn read_trades(
    rom: &GbRom,
    entry: &RomEntry,
    text: &GbText,
    dex_of: &dyn Fn(u8) -> DexNumber,
) -> Result<Vec<InGameTrade>, RomError> {
    let name_len = need(entry, "TradeNameLength")?;
    trade_entries(entry)?
        .into_iter()
        .map(|at| {
            let requested = dex_of(rom.byte(at)?);
            Ok(InGameTrade {
                requested: (requested != 0).then_some(requested),
                given: dex_of(rom.byte(at + 1)?),
                nickname: text.read_string(rom.slice(at + 3, name_len)?, name_len, false),
                ot_name: String::new(),
                ot_id: 0,
                ivs: Vec::new(),
                held_item: None,
            })
        })
        .collect()
}

/// TM01 to TM50, then HM01 to HM05.
fn read_tm_hm_compat(
    rom: &GbRom,
    entry: &RomEntry,
    family: Family,
    species: &[Species],
) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    species
        .iter()
        .map(|s| {
            let bytes = rom.slice(stats_offset(entry, family, s.number)? + c::BS_TM_HM_COMPAT, 7)?;
            let flags = (0..c::TM_COUNT + c::HM_COUNT)
                .map(|i| bytes[i / 8] >> (i % 8) & 1 == 1)
                .collect();
            Ok((s.number, flags))
        })
        .collect()
}

/// The type chart: attacker, defender and effectiveness, until 0xFF.
fn read_type_chart(rom: &GbRom, entry: &RomEntry, types: &[Type]) -> Result<TypeChart, RomError> {
    let mut chart = TypeChart::neutral(types);
    let mut at = need(entry, "TypeEffectivenessOffset")?;
    while rom.byte(at)? != TYPE_CHART_END {
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

/// The static encounters of the ROM entry, the ghost Marowak last; none without `StaticPokemonSupport`.
fn static_spots(entry: &RomEntry) -> Vec<StaticSpot> {
    if optional(entry, "StaticPokemonSupport") == 0 {
        return Vec::new();
    }
    entry
        .list("StaticPokemon{}")
        .iter()
        .chain(entry.list("StaticPokemonGhostMarowak{}"))
        .map(|s| StaticSpot::parse(s))
        .collect()
}

/// The tweaks this ROM supports.
fn available_tweaks(entry: &RomEntry) -> Vec<MiscTweak> {
    let mut tweaks = vec![MiscTweak::LowerCaseNames];
    let has = |key: &str| entry.value(key).is_some();
    let set = |key: &str| optional(entry, key) != 0;
    for (on, tweak) in [
        (has("BWXPTweak"), MiscTweak::BwExpPatch),
        (has("XAccNerfTweak"), MiscTweak::NerfXAccuracy),
        (has("CritRateTweak"), MiscTweak::FixCritRate),
        (set("TextDelayFunctionOffset"), MiscTweak::FastestText),
        (set("PCPotionOffset"), MiscTweak::RandomizePcPotion),
        (set("PikachuEvoJumpOffset"), MiscTweak::AllowPikachuEvolution),
        (set("CatchingTutorialMonOffset"), MiscTweak::RandomizeCatchingTutorial),
        (set("TMMovesReusableFunctionOffset"), MiscTweak::ReusableTms),
        (set("HMMovesForgettableFunctionOffset"), MiscTweak::ForgettableHms),
    ] {
        if on {
            tweaks.push(tweak);
        }
    }
    tweaks
}
