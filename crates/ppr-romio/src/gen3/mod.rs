//! Generation 3 (Ruby, Sapphire, Emerald, `FireRed`, `LeafGreen`).
//!
//! Ported so far: species names, types and base stats, the national/internal index maps, evolutions, the type
//! chart (read), starters (read and write, with the starter text of `FireRed` and `LeafGreen` and the obedience
//! patch for Mew and Deoxys), wild encounters and catch rates ([`encounters`]), items, and static encounters with
//! their patches ([`statics`]). Text that grows is written to free space ([`space`], [`events`]).

mod abilities;
mod compat;
mod constants;
mod encounters;
pub(crate) mod events;
mod evolutions;
mod field_items;
#[cfg(any(test, feature = "test-fixtures"))]
pub(crate) mod fixtures;
mod hack;
mod intro;
mod items;
mod learnsets;
mod moves;
mod palettes;
mod pickup;
mod player_graphics;
mod shops;
mod space;
mod statics;
mod tags;
mod tms;
mod trades;
mod trainer_tags;
mod trainers;
mod tweaks;

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{
    BaseStats, Capabilities, DexNumber, Effectiveness, ExpCurve, GameData, Generation, Item, ItemId, MiscTweak,
    RomIdentity, Species, StaticEncounter, Type, TypeChart, WildHeldItems,
};

use self::constants as c;
use crate::entries::{RomEntry, catalog};
use crate::freespace::FreedSpace;
use crate::gba::{Found, GbaRom, hex};
use crate::text::TextTable;
use crate::{GameHandler, RomError};

/// Game family (`Type=` in the ROM entries).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    RubySapphire,
    Emerald,
    FireRedLeafGreen,
}

impl Family {
    /// The games the family's player graphics packs are for.
    fn graphics(self) -> crate::graphics::Gen3Games {
        match self {
            Self::FireRedLeafGreen => crate::graphics::Gen3Games::Frlg,
            Self::RubySapphire | Self::Emerald => crate::graphics::Gen3Games::Rse,
        }
    }

    /// `frlg` for `FireRed` and `LeafGreen`, `rse` for Ruby, Sapphire and Emerald.
    fn pick(self, frlg: &[u16], rse: &[u16]) -> Vec<u16> {
        if self == Self::FireRedLeafGreen { frlg } else { rse }.to_vec()
    }

    fn parse(s: &str) -> Result<Self, RomError> {
        match s.to_ascii_lowercase().as_str() {
            "ruby" | "sapp" => Ok(Self::RubySapphire),
            "em" => Ok(Self::Emerald),
            "frlg" => Ok(Self::FireRedLeafGreen),
            other => Err(RomError::Malformed(format!("unknown Gen 3 game type {other}"))),
        }
    }
}

/// Loaded Gen 3 ROM.
#[derive(Debug)]
pub(crate) struct Gen3Handler {
    rom: GbaRom,
    entry: RomEntry,
    family: Family,
    loaded: GameData,
    /// National number → internal index (0 when absent); the first internal slot wins.
    dex_to_internal: Vec<u16>,
    obedience_patched: bool,
    stats_offset: usize,
    encounters: encounters::Encounters,
    text: TextTable,
    free_space: FreedSpace,
    texts: EventTexts,
    /// Where each field item in `GameData::field_items` is.
    field_item_offsets: Vec<usize>,
    /// Where the Pickup table is and how big its entries are.
    pickup_table: Option<(usize, usize)>,
    /// Where the palette pointer tables are, when the entry knows.
    palette_tables: Option<palettes::Tables>,
    statics: Option<statics::Statics>,
    /// Offsets of the used in-game trade entries.
    trade_entries: Vec<usize>,
    /// National number by internal index.
    internal_to_dex: Vec<DexNumber>,
    learnset_format: learnsets::Format,
}

/// The location tags of a family's encounter areas (`Type=` in the ROM entries).
#[cfg(any(test, feature = "test-fixtures"))]
fn tags_for(family: &str) -> &'static [&'static str] {
    match Family::parse(family) {
        Ok(Family::RubySapphire) => &tags::LOCATION_TAGS_RS,
        Ok(Family::Emerald) => &tags::LOCATION_TAGS_EM,
        _ => &tags::LOCATION_TAGS_FRLG,
    }
}

fn need(entry: &RomEntry, key: &str) -> Result<usize, RomError> {
    entry
        .int(key)
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no valid {key}", entry.name)))
}

fn found_at(found: Found) -> Option<usize> {
    // A pattern found more than once, or at offset 0, counts as not found.
    match found {
        Found::Unique(at) if at > 0 => Some(at),
        _ => None,
    }
}

impl Gen3Handler {
    #[expect(
        clippy::too_many_lines,
        reason = "a line for each field of GameData, which grows with every randomizer"
    )]
    pub(crate) fn load(bytes: Vec<u8>, identity: RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(3).ok_or_else(|| RomError::Malformed("generation 3".into()))?;
        let mut entry = catalog(generation)
            .entries
            .iter()
            .find(|e| e.name == identity.name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {}", identity.name)))?;
        let family = Family::parse(&entry.family)?;
        let rom = GbaRom::new(bytes);
        let text = TextTable::named(entry.value("TableFile").unwrap_or_default())?;
        let hack = hack::adjust(&rom, &mut entry, &text)?;
        let entry = entry;

        let stats_offset = entry_offset(&rom, &entry, "PokemonStats")?;
        let internal_count = need(&entry, "PokemonCount")?;
        let names = read_species_names(&rom, &entry, &text, internal_count)?;
        // names[i - 1] is internal index i.
        let name_of = |i: usize| names[i - 1].as_str();

        let mut order = PokedexOrder::read(&rom, &entry, internal_count, stats_offset, name_of)?;
        order.is_rom_hack |= hack;

        let species = read_all_species(&rom, &entry, family, &order, stats_offset, internal_count, name_of)?;

        let palette_tables = palettes::Tables::find(&rom, &entry);
        let species_palettes = match palette_tables {
            Some(t) => palettes::read(&rom, t, species.iter().map(|s| s.number), &order.dex_to_internal)?,
            None => BTreeMap::new(),
        };
        let wild_held_items = read_wild_held_items(&rom, &species, &order, stats_offset, internal_count)?;
        let evolutions = evolutions::read(&rom, &entry, &species, &order, internal_count)?;
        let starters = read_starters(&rom, &entry, family, &order)?;
        let battle_trappers = battle_trappers(&rom, stats_offset, &order.internal_to_dex);
        let encounters = encounters::read(&rom, &entry, family, &text, &order.internal_to_dex, &battle_trappers)?;
        let items = read_items(&rom, &entry, family, &text)?;
        let (field_item_offsets, field_items) = field_items::load(&rom, &entry, &items)?;
        let pickup_table = pickup::table(&rom, &entry, family)?;
        let starter_held_items = vec![read_starter_held_item(&rom, &entry, family)?];
        let types = Type::in_generation(generation);
        let type_chart = read_type_chart(&rom, &entry, &types)?;
        let mut free_space = space::unused_space(&rom, need(&entry, "FreeSpace")?)?;
        let trade_entries = trades::entries(&entry);
        let trades = trades::read(&rom, &trade_entries, &text, &order.internal_to_dex)?;
        let (statics, static_encounters) = read_statics(&rom, &entry, family, internal_count, &order, &mut free_space)?;
        let learnset_format = learnsets::Format::of(&rom, &entry)?;
        let internal_of = |n: DexNumber| internal_index(&order.dex_to_internal, n).ok();
        let learnsets = learnsets::read(&rom, learnset_format, &species, internal_of)?;
        let egg_moves = learnsets::read_egg_moves(&rom, &entry, &order.internal_to_dex)?;
        let tm_hm_compat = compat::read_tm_hm(&rom, &entry, &species, internal_of)?;
        let tutor_compat = compat::read_tutors(&rom, &entry, family, &species, internal_of)?;
        let texts = EventTexts::read(&rom, &entry)?;
        let trainers = trainers::read(&rom, &entry, family, &text, &order.internal_to_dex)?;
        let trainer_class_names = trainers::class_names(&rom, &entry, &text)?;
        let (catching_tutorial_opponents, catching_tutorial_players) =
            if tweaks::available(&entry, family).contains(&MiscTweak::RandomizeCatchingTutorial) {
                tweaks::catching_tutorial_candidates(&rom, &entry, family, stats_offset, &order.dex_to_internal)
            } else {
                (Vec::new(), Vec::new())
            };
        let fossils = statics
            .as_ref()
            .map(|st| st.at_levels(&entry.array("FossilLevelOffsets").unwrap_or_default()))
            .unwrap_or_default();

        let mut loaded = GameData {
            formes: ppr_model::FormeRules::default(),
            identity,
            types,
            type_chart,
            species,
            evolutions,
            evolution_level_samples: Vec::new(),
            restricted_species: None,
            starters,
            starter_type_triangle: None,
            starter_held_items,
            items,
            wild_held_items,
            // Gen 3 wild Pokémon may always hold an item, and has no dark grass.
            guaranteed_wild_held_items: true,
            dark_grass_held_items: false,
            palettes: species_palettes,
            // Ruby and Sapphire use Emerald's palette descriptions.
            palette_files: palette_tables.map(|_| {
                if family == Family::FireRedLeafGreen {
                    "FRLG"
                } else {
                    "E"
                }
                .to_owned()
            }),
            player_graphics_packs: crate::graphics::gen3_packs(family.graphics()),
            player_graphics: None,
            encounters: encounters.sets.clone(),
            guaranteed_catching: false,
            statics: static_encounters,
            main_game_legendaries: main_game_legendaries(&entry),
            special_music_statics: statics
                .as_ref()
                .map(|st| st.special_music().to_vec())
                .unwrap_or_default(),
            correct_static_music: false,
            totems: Vec::new(),
            easier_happiness_evolutions: false,
            trades,
            max_trade_nickname_length: trades::MAX_NICKNAME_LENGTH,
            max_trade_ot_name_length: trades::MAX_OT_NAME_LENGTH,
            wild_banned: wild_banned(family),
            ability_names: abilities::names(&rom, &entry, &text)?,
            ability_variations: abilities::variations(),
            useless_abilities: abilities::useless(),
            moves: moves::read(&rom, &entry, &text)?,
            max_move_name_length: moves::MAX_NAME_LENGTH,
            learnsets,
            egg_moves,
            hm_moves: c::HM_MOVES.to_vec(),
            game_breaking_moves: c::GAME_BREAKING_MOVES.to_vec(),
            banned_level_up_moves: Vec::new(),
            four_starting_moves: true,
            tm_moves: tms::read(&rom, &entry)?,
            tutor_moves: tms::read_tutors(&rom, &entry, family)?,
            tm_hm_compat,
            tutor_compat,
            early_required_hm_moves: family.pick(&c::FRLG_EARLY_REQUIRED_HM_MOVES, &c::RSE_EARLY_REQUIRED_HM_MOVES),
            field_moves: family.pick(&c::FRLG_FIELD_MOVES, &c::RSE_FIELD_MOVES),
            field_items,
            required_field_tms: field_items::required_tms(family),
            shops: shops::read(&rom, &entry, family)?,
            item_prices: shops::read_prices(&rom, &entry)?,
            balanced_item_prices: shops::balanced_prices(),
            op_shop_items: c::OP_SHOP_ITEMS.to_vec(),
            mega_stones: Vec::new(),
            evolution_items: c::EVOLUTION_ITEMS.to_vec(),
            pickup_items: pickup::read(&rom, &entry, pickup_table)?,
            // Gen 3 TMs can be held and are used up, so Pickup may find them.
            pickup_tms: true,
            trainers,
            trainer_class_names,
            elite_four_trainers: entry
                .array("EliteFourIndices")
                .unwrap_or_default()
                .iter()
                .filter_map(|&i| u16::try_from(i).ok())
                .collect(),
            gym_and_elite_themes: trainers::gym_and_elite_themes(family),
            original_types: BTreeMap::new(),
            max_trainer_name_length: need(&entry, "TrainerNameLength")?.saturating_sub(1),
            max_trainer_class_name_length: need(&entry, "TrainerClassNameLength")?.saturating_sub(1),
            trainer_name_shares_class_line: false,
            max_trainer_names_total: None,
            doubles_trainer_classes: entry
                .array("DoublesTrainerClasses")
                .unwrap_or_default()
                .iter()
                .filter_map(|&i| u8::try_from(i).ok())
                .collect(),
            personal_trainer_classes: Vec::new(),
            fixed_trainer_text: false,
            fixed_trainer_name_length: false,
            fixed_trainer_class_name_length: false,
            text_tokens: Vec::new(),
            intro_candidates: intro::candidates(&rom, &entry, family, &order.dex_to_internal),
            intro_pokemon: None,
            held_item_pools: c::held_item_pools(),
            trainer_pokemon_additions: [true; 3],
            trainer_custom_movesets: [true; 3],
            trainer_held_item_additions: [true; 3],
            available_tweaks: tweaks::available(&entry, family),
            applied_tweaks: BTreeSet::new(),
            pc_potion: None,
            catching_tutorial_opponents,
            catching_tutorial_players,
            catching_tutorial: None,
            fossils,
        };
        // The evolution level estimates are taken from the game as loaded.
        loaded.estimate_evolution_levels();
        Ok(Self {
            rom,
            entry,
            family,
            loaded,
            dex_to_internal: order.dex_to_internal,
            internal_to_dex: order.internal_to_dex,
            obedience_patched: false,
            stats_offset,
            encounters,
            text,
            free_space,
            texts,
            field_item_offsets,
            pickup_table,
            palette_tables,
            statics,
            trade_entries,
            learnset_format,
        })
    }

    fn internal_index(&self, number: DexNumber) -> Result<u16, RomError> {
        internal_index(&self.dex_to_internal, number)
    }

    /// For the species where `field` changed: `bytes` at `offset` of every internal slot
    /// of the species.
    fn write_species_field<T: PartialEq>(
        &mut self,
        species: &[Species],
        offset: usize,
        field: impl Fn(&Species) -> T,
        bytes: impl Fn(&Species) -> Result<Vec<u8>, RomError>,
    ) -> Result<(), RomError> {
        for s in species {
            if self.loaded.species(s.number).is_none_or(|old| field(old) == field(s)) {
                continue;
            }
            let bytes = bytes(s)?;
            for (i, _) in self.internal_to_dex.iter().enumerate().filter(|&(_, &n)| n == s.number) {
                self.rom
                    .write(self.stats_offset + i * c::BASE_STATS_ENTRY_SIZE + offset, &bytes)?;
            }
        }
        Ok(())
    }

    /// Species names and palettes, where they changed.
    fn write_names_and_palettes(&mut self, data: &GameData) -> Result<(), RomError> {
        self.write_species_names(&data.species)?;
        if let Some(tables) = self.palette_tables
            && data.palettes != self.loaded.palettes
        {
            let mut space = space::Space {
                rom: &mut self.rom,
                free: &mut self.free_space,
            };
            palettes::write(
                &mut space,
                tables,
                &data.palettes,
                &self.loaded.palettes,
                &self.dex_to_internal,
            )?;
        }
        Ok(())
    }

    /// The custom player graphics, if chosen; written after everything else.
    fn write_player_graphics(&mut self, data: &GameData) -> Result<(), RomError> {
        let Some(choice) = &data.player_graphics else {
            return Ok(());
        };
        let graphics = crate::graphics::gen3_graphics(&choice.pack, self.family.graphics())
            .ok_or_else(|| RomError::Malformed(format!("no player graphics pack {} for this game", choice.pack)))?;
        let mut space = space::Space {
            rom: &mut self.rom,
            free: &mut self.free_space,
        };
        player_graphics::write(&mut space, &self.entry, self.family, &graphics, choice.replace)
    }

    fn write_held_items(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.starter_held_items != self.loaded.starter_held_items {
            self.write_starter_held_items(&data.starter_held_items)?;
        }
        if data.wild_held_items != self.loaded.wild_held_items {
            self.write_wild_held_items(&data.wild_held_items)?;
        }
        Ok(())
    }

    /// The wild held items, where they changed: a guaranteed item in both words, else the
    /// common and the rare one, 0 for none.
    fn write_wild_held_items(&mut self, held: &BTreeMap<DexNumber, WildHeldItems>) -> Result<(), RomError> {
        let word = |item: Option<ItemId>| {
            item.map_or(Ok(0), |id| {
                items::internal(id).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {id}")))
            })
        };
        for (&number, h) in held {
            if self.loaded.wild_held_items.get(&number).copied().unwrap_or_default() == *h {
                continue;
            }
            let (common, rare) = match h.guaranteed {
                Some(_) => (word(h.guaranteed)?, word(h.guaranteed)?),
                None => (word(h.common)?, word(h.rare)?),
            };
            for (i, _) in self.internal_to_dex.iter().enumerate().filter(|&(_, &n)| n == number) {
                let at = self.stats_offset + i * c::BASE_STATS_ENTRY_SIZE;
                self.rom.write_u16(at + c::BS_COMMON_HELD_ITEM, common)?;
                self.rom.write_u16(at + c::BS_RARE_HELD_ITEM, rare)?;
            }
        }
        Ok(())
    }

    /// The species data, where it changed: base stats (and Deoxys's hard-coded
    /// ones outside Ruby and Sapphire), types (one type stored twice), catch rate, growth curve and abilities (an
    /// empty second slot gets the first ability, "to not break evos with random ability").
    fn write_species(&mut self, species: &[Species]) -> Result<(), RomError> {
        self.write_species_field(
            species,
            c::BS_HP,
            |s| s.stats,
            |s| {
                let b = s.stats;
                let mut bytes = vec![0; 6];
                bytes[c::BS_HP] = b.hp;
                bytes[c::BS_ATTACK] = b.attack;
                bytes[c::BS_DEFENSE] = b.defense;
                bytes[c::BS_SPEED] = b.speed;
                bytes[c::BS_SP_ATTACK] = b.sp_attack;
                bytes[c::BS_SP_DEFENSE] = b.sp_defense;
                Ok(bytes)
            },
        )?;
        if self.family != Family::RubySapphire
            && let Some(deoxys) = species.iter().find(|s| s.number == c::DEOXYS)
            && self
                .loaded
                .species(c::DEOXYS)
                .is_some_and(|old| old.stats != deoxys.stats)
        {
            write_deoxys_stats(&mut self.rom, &self.entry, deoxys.stats)?;
        }
        let type_byte = |t| c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no type {t:?}")));
        self.write_species_field(
            species,
            c::BS_PRIMARY_TYPE,
            |s| (s.primary_type, s.secondary_type),
            |s| {
                Ok(vec![
                    type_byte(s.primary_type)?,
                    type_byte(s.secondary_type.unwrap_or(s.primary_type))?,
                ])
            },
        )?;
        self.write_species_field(species, c::BS_CATCH_RATE, |s| s.catch_rate, |s| Ok(vec![s.catch_rate]))?;
        self.write_species_field(
            species,
            c::BS_GROWTH_CURVE,
            |s| s.growth_curve,
            |s| Ok(vec![s.growth_curve.to_byte()]),
        )?;
        let ability_byte = |a: u16| {
            u8::try_from(a)
                .ok()
                .filter(|&b| u16::from(b) <= abilities::HIGHEST)
                .ok_or_else(|| RomError::Malformed(format!("Gen 3 has no ability {a}")))
        };
        self.write_species_field(
            species,
            c::BS_ABILITY_1,
            |s| s.abilities.clone(),
            |s| {
                let first = ability_byte(s.abilities.first().copied().unwrap_or(0))?;
                let second = match s.abilities.get(1).copied().unwrap_or(0) {
                    0 => first,
                    a => ability_byte(a)?,
                };
                Ok(vec![first, second])
            },
        )
    }

    /// The species names, where they changed.
    fn write_species_names(&mut self, species: &[Species]) -> Result<(), RomError> {
        let names_offset = species_names_offset(&self.rom, &self.entry)?;
        let name_len = need(&self.entry, "PokemonNameLength")?;
        for s in species {
            if self.loaded.species(s.number).is_none_or(|old| old.name == s.name) {
                continue;
            }
            for (i, _) in self.internal_to_dex.iter().enumerate().filter(|&(_, &n)| n == s.number) {
                trades::write_name(
                    &mut self.rom,
                    names_offset + i * name_len,
                    &s.name,
                    name_len,
                    &self.text,
                )?;
            }
        }
        Ok(())
    }

    /// The misc tweaks newly applied, the PC Potion and the catching tutorial.
    fn apply_tweaks(&mut self, data: &GameData) -> Result<(), RomError> {
        let slots: Vec<usize> = self
            .internal_to_dex
            .iter()
            .enumerate()
            .filter(|&(i, &n)| i > 0 && n != 0)
            .map(|(i, _)| self.stats_offset + i * c::BASE_STATS_ENTRY_SIZE)
            .collect();
        for &tweak in data.applied_tweaks.difference(&self.loaded.applied_tweaks) {
            let mut space = space::Space {
                rom: &mut self.rom,
                free: &mut self.free_space,
            };
            tweaks::apply(&mut space, &self.entry, self.family, &slots, tweak)?;
        }
        if let Some(item) = data.pc_potion {
            tweaks::write_pc_potion(&mut self.rom, &self.entry, item)?;
        }
        if let Some(t) = data.catching_tutorial {
            let opponent = self.internal_index(t.opponent)?;
            let player = self.internal_index(t.player)?;
            tweaks::write_catching_tutorial(&mut self.rom, &self.entry, self.family, opponent, player)?;
        }
        Ok(())
    }

    /// Removes the "odds above 254" check of the ball throw, so every
    /// ball acts as if the odds were 255.
    fn enable_guaranteed_catching(&mut self) -> Result<(), RomError> {
        if let Some(at) = found_at(self.rom.find(&hex(c::PERFECT_ODDS_BRANCH_LOCATOR)?)) {
            self.rom.write(at, &[0, 0, 0, 0])?;
        }
        Ok(())
    }

    /// The three starters, at every place the game keeps them.
    fn write_starters(&mut self, starters: &[DexNumber]) -> Result<(), RomError> {
        let [s0, s1, s2] = <[DexNumber; 3]>::try_from(starters)
            .map_err(|_| RomError::Malformed(format!("Gen 3 has 3 starters, got {}", starters.len())))?;
        let (s0, s1, s2) = (
            self.internal_index(s0)?,
            self.internal_index(s1)?,
            self.internal_index(s2)?,
        );
        let base = need(&self.entry, "StarterPokemon")?;
        if self.family == Family::FireRedLeafGreen {
            self.rom.write_u16(base, s0)?;
            self.rom.write_u16(base + c::FRLG_STARTER_REPEAT, s1)?;
            self.rom.write_u16(base + c::FRLG_STARTER_2, s1)?;
            self.rom
                .write_u16(base + c::FRLG_STARTER_2 + c::FRLG_STARTER_REPEAT, s2)?;
            self.rom.write_u16(base + c::FRLG_STARTER_3, s2)?;
            self.rom
                .write_u16(base + c::FRLG_STARTER_3 + c::FRLG_STARTER_REPEAT, s0)?;
        } else {
            self.rom.write_u16(base, s0)?;
            self.rom.write_u16(base + c::RSE_STARTER_2, s1)?;
            self.rom.write_u16(base + c::RSE_STARTER_3, s2)?;
        }
        Ok(())
    }

    /// The starters' held item: one item for all starters.
    fn write_starter_held_items(&mut self, held: &[Option<ItemId>]) -> Result<(), RomError> {
        let [item] = held else {
            return Err(RomError::Malformed(format!(
                "Gen 3 has 1 starter held item, got {}",
                held.len()
            )));
        };
        let internal = match item {
            None => 0,
            Some(id) => self
                .loaded
                .items
                .iter()
                .any(|i| i.id == *id)
                .then(|| items::internal(*id))
                .flatten()
                .ok_or_else(|| RomError::Malformed(format!("item {id} does not exist in this game")))?,
        };
        if self.family == Family::FireRedLeafGreen {
            let at = need(&self.entry, "StarterPokemon")? + c::FRLG_STARTER_ITEMS;
            return self.rom.write_u16(at, internal);
        }
        // A MOVS of one byte, plus an ADD for items past 0xFF.
        let at = need(&self.entry, "StarterItems")?;
        let (low, add) = match u8::try_from(internal) {
            Ok(b) => (b, 0),
            Err(_) => (0xFF, u8::try_from(internal - 0xFF).unwrap_or(u8::MAX)),
        };
        self.rom.write(at, &[low])?;
        self.rom.write(at + 2, &[add, c::GBA_ADD_R2])
    }

    /// The starter text: `… the [type] POKéMON [starter]?` in `FireRed` and `LeafGreen`. A Normal type
    /// with a second type is named by the second.
    fn write_starter_text(&mut self, data: &GameData) -> Result<(), RomError> {
        if self.texts.starter.is_empty() {
            return Ok(());
        }
        let values = |id: usize| {
            let starter = data.species(*data.starters.get(id)?)?;
            let shown = match (starter.primary_type, starter.secondary_type) {
                (Type::Normal, Some(second)) => second,
                (primary, _) => primary,
            };
            Some(vec![starter.name.clone(), shown.to_string().to_uppercase()])
        };
        let mut space = space::Space {
            rom: &mut self.rom,
            free: &mut self.free_space,
        };
        events::write(
            &mut space,
            &self.text,
            &self.texts.starter,
            &["[starter]", "[type]"],
            values,
            "Starter",
        )
    }

    /// The TMs, their item colours and descriptions, and what people say about them.
    fn write_tms(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.tm_moves.len() != tms::TM_COUNT {
            return Err(RomError::Malformed(format!("Gen 3 has {} TMs", tms::TM_COUNT)));
        }
        tms::write_moves(&mut self.rom, &self.entry, &data.tm_moves)?;
        tms::write_palettes(&mut self.rom, &self.entry, &data.tm_moves, &data.moves)?;
        let mut space = space::Space {
            rom: &mut self.rom,
            free: &mut self.free_space,
        };
        tms::write_item_text(&mut space, &self.entry, self.family, &self.text, &data.tm_moves)?;
        let name = |id: usize| {
            let number = *data.tm_moves.get(id.checked_sub(1)?)?;
            Some(vec![data.moves.iter().find(|m| m.number == number)?.name.clone()])
        };
        events::write(&mut space, &self.text, &self.texts.tm, &["[move]"], name, "TM")
    }

    /// Shops, prices, Pickup and field items, where they changed.
    fn write_items(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.shops != self.loaded.shops {
            let mut space = space::Space {
                rom: &mut self.rom,
                free: &mut self.free_space,
            };
            shops::write(&mut space, &self.entry, &data.shops)?;
        }
        if data.item_prices != self.loaded.item_prices {
            shops::write_prices(&mut self.rom, &self.entry, &data.item_prices)?;
        }
        if data.pickup_items != self.loaded.pickup_items {
            pickup::write(&mut self.rom, self.pickup_table, &data.pickup_items)?;
        }
        if data.field_items != self.loaded.field_items {
            let (old, all) = (&self.loaded.field_items, &self.loaded.items);
            field_items::write(&mut self.rom, &self.field_item_offsets, old, &data.field_items, all)?;
        }
        Ok(())
    }

    /// Level-up moves, egg moves and trainers, where they changed; trainers last, as their reset moves come
    /// from the level-up moves.
    fn write_learnt(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.learnsets != self.loaded.learnsets {
            let dex_to_internal = &self.dex_to_internal;
            let mut space = space::Space {
                rom: &mut self.rom,
                free: &mut self.free_space,
            };
            learnsets::write(&mut space, self.learnset_format, &data.species, &data.learnsets, |n| {
                internal_index(dex_to_internal, n).ok()
            })?;
        }
        if data.trainers != self.loaded.trainers {
            let dex_to_internal = &self.dex_to_internal;
            let mut space = space::Space {
                rom: &mut self.rom,
                free: &mut self.free_space,
            };
            trainers::write(
                &mut space,
                &self.entry,
                self.family,
                &self.text,
                &self.loaded.trainers,
                data,
                |n| internal_index(dex_to_internal, n).ok(),
            )?;
        }
        if let Some(n) = data.intro_pokemon.filter(|n| data.intro_candidates.contains(n)) {
            let internal = internal_index(&self.dex_to_internal, n)?;
            intro::write(&mut self.rom, &self.entry, self.family, internal)?;
        }
        if data.trainer_class_names != self.loaded.trainer_class_names {
            trainers::write_class_names(&mut self.rom, &self.entry, &self.text, &data.trainer_class_names)?;
        }
        if data.egg_moves != self.loaded.egg_moves {
            let dex_to_internal = &self.dex_to_internal;
            learnsets::write_egg_moves(
                &mut self.rom,
                &self.entry,
                &self.loaded.egg_moves,
                &data.egg_moves,
                |n| internal_index(dex_to_internal, n).ok(),
            )?;
        }
        Ok(())
    }

    /// What TMs, HMs and tutors teach, and who can learn it, where it changed.
    fn write_teachables(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.tm_moves != self.loaded.tm_moves {
            self.write_tms(data)?;
        }
        if data.tutor_moves != self.loaded.tutor_moves {
            self.write_tutors(data)?;
        }
        let dex_to_internal = &self.dex_to_internal;
        let internal_of = |n| internal_index(dex_to_internal, n).ok();
        if data.tm_hm_compat != self.loaded.tm_hm_compat {
            compat::write_tm_hm(&mut self.rom, &self.entry, &data.tm_hm_compat, internal_of)?;
        }
        if data.tutor_compat != self.loaded.tutor_compat {
            compat::write_tutors(&mut self.rom, &self.entry, self.family, &data.tutor_compat, internal_of)?;
        }
        Ok(())
    }

    fn write_tutors(&mut self, data: &GameData) -> Result<(), RomError> {
        tms::write_tutors(&mut self.rom, &self.entry, &data.tutor_moves)?;
        let mut space = space::Space {
            rom: &mut self.rom,
            free: &mut self.free_space,
        };
        let name = |id: usize| {
            let number = *data.tutor_moves.get(id)?;
            Some(vec![data.moves.iter().find(|m| m.number == number)?.name.clone()])
        };
        events::write(
            &mut space,
            &self.text,
            &self.texts.tutor,
            &["[move]"],
            name,
            "MoveTutor",
        )
    }

    /// Lets Mew and Deoxys obey, and lets FR/LG evolve species beyond
    /// the Kanto Pokédex without the National Pokédex. Applied at most once.
    fn patch_obedience_and_evolution(&mut self) -> Result<(), RomError> {
        if self.obedience_patched {
            return Ok(());
        }
        self.obedience_patched = true;
        if let Some(at) = found_at(self.rom.find(&hex(c::DEOXYS_OBEY_CODE)?)) {
            // MOVS R1, 0 twice: only the glitch species 0 would disobey.
            let movs_r1_0 = [0x00, c::GBA_SET_RX_OPCODE | c::GBA_R1];
            self.rom.write(at, &[movs_r1_0, movs_r1_0].concat())?;
            let mew_check = at + c::MEW_OBEY_OFFSET_FROM_DEOXYS_OBEY;
            let cmp_r0 = (c::GBA_CMP_RX_OPCODE | c::GBA_R0) << 8;
            if self.rom.u16(mew_check)? == cmp_r0 | c::MEW {
                self.rom.write_u16(mew_check, cmp_r0)?;
            }
        }
        if self.family == Family::FireRedLeafGreen {
            for (code, jump) in [
                (c::LEVEL_EVO_KANTO_DEX_CHECK_CODE, c::LEVEL_EVO_KANTO_DEX_JUMP_AMOUNT),
                (c::STONE_EVO_KANTO_DEX_CHECK_CODE, c::STONE_EVO_KANTO_DEX_JUMP_AMOUNT),
            ] {
                if let Some(at) = found_at(self.rom.find(&hex(code)?)) {
                    self.rom.write_u16(at, c::GBA_NOP_OPCODE)?;
                    self.rom
                        .write_u16(at + 2, (c::GBA_UNCONDITIONAL_JUMP_OPCODE << 8) | jump)?;
                }
            }
        }
        Ok(())
    }
}

impl GameHandler for Gen3Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    /// Tutors in Emerald and FireRed/LeafGreen only, move names in English games
    /// (a game code ending in `E`); no formes, Mega Evolutions or physical/special split in Gen 3.
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            intro_pokemon: true,
            starter_held_items: true,
            move_tutors: matches!(self.family, Family::Emerald | Family::FireRedLeafGreen),
            encounter_locations: true,
            shop_sizes: true,
            english: self.loaded.identity.code.ends_with('E'),
            ..Capabilities::default()
        }
    }

    /// Shops and trainer teams move to free space, so they grow freely; Emerald's Mossdeep Steven is a fixed block
    /// of three Pokémon with moves and no held items.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        if self.family == Family::Emerald
            && let Some(steven) = self.loaded.trainers.last()
            && let Some(limits) = rules.trainers.trainers.iter_mut().find(|t| t.index == steven.index)
        {
            limits.min_pokemon = trainers::STEVEN_TEAM;
            limits.max_pokemon = trainers::STEVEN_TEAM;
            limits.held_items = false;
            limits.custom_moves = true;
        }
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.starters != self.loaded.starters {
            // New starters may be Mew or Deoxys: patch first, then write.
            self.patch_obedience_and_evolution()?;
            self.write_starters(&data.starters)?;
            self.write_starter_text(data)?;
        }
        self.write_held_items(data)?;
        if data.encounters != self.loaded.encounters {
            // Mew and Deoxys may now be met in the wild.
            self.patch_obedience_and_evolution()?;
            let dex_to_internal = &self.dex_to_internal;
            encounters::write(&mut self.rom, &self.encounters, &data.encounters, |n| {
                internal_index(dex_to_internal, n)
            })?;
        }
        let statics_changed = self.statics.is_some() && data.statics != self.loaded.statics;
        if statics_changed
            && data
                .statics
                .iter()
                .zip(&self.loaded.statics)
                .any(|(a, b)| a.species != b.species)
        {
            // Mew and Deoxys may now be met.
            self.patch_obedience_and_evolution()?;
        }
        if let Some(st) = &self.statics {
            let internal = |n| internal_index(&self.dex_to_internal, n);
            if data.correct_static_music && !self.loaded.correct_static_music {
                st.correct_music(
                    &mut self.rom,
                    &self.loaded.statics,
                    &data.statics,
                    &self.internal_to_dex,
                    internal,
                )?;
            }
            // "Balance Static Pokémon Levels" alone sets fossil levels, written by the tweak; anything more is a
            // full static encounter write.
            let only_fossil_levels = data.statics.len() == self.loaded.statics.len()
                && data
                    .statics
                    .iter()
                    .zip(&self.loaded.statics)
                    .enumerate()
                    .all(|(i, (a, b))| {
                        a == b
                            || (data.fossils.contains(&i)
                                && a.species == b.species
                                && a.egg == b.egg
                                && a.linked_levels == b.linked_levels)
                    });
            if statics_changed && only_fossil_levels {
                for (i, s) in data
                    .statics
                    .iter()
                    .enumerate()
                    .filter(|&(i, _)| data.fossils.contains(&i))
                {
                    st.write_level(&mut self.rom, i, s.level)?;
                }
            } else if statics_changed {
                let stats_offset = self.stats_offset;
                let gender_ratio = |rom: &GbaRom, n| {
                    let at = stats_offset + usize::from(internal(n)?) * c::BASE_STATS_ENTRY_SIZE;
                    Ok(rom.slice(at + c::BS_GENDER_RATIO, 1)?[0])
                };
                st.write(&mut self.rom, &data.statics, internal, gender_ratio)?;
            }
        }
        if data.trades != self.loaded.trades {
            let dex_to_internal = &self.dex_to_internal;
            trades::write(&mut self.rom, &self.trade_entries, &data.trades, &self.text, |n| {
                internal_index(dex_to_internal, n)
            })?;
        }
        if data.type_chart != self.loaded.type_chart {
            write_type_chart(&mut self.rom, &self.entry, &data.type_chart)?;
        }
        self.write_species(&data.species)?;
        if data.evolutions != self.loaded.evolutions {
            // Any evolution change patches, not only Change Impossible Evolutions; random evolutions need it as much.
            self.patch_obedience_and_evolution()?;
            let dex_to_internal = &self.dex_to_internal;
            evolutions::write(&mut self.rom, &self.entry, &data.species, &data.evolutions, |n| {
                internal_index(dex_to_internal, n).ok()
            })?;
        }
        if data.moves != self.loaded.moves {
            moves::write(&mut self.rom, &self.entry, &self.text, &self.loaded.moves, &data.moves)?;
        }
        self.write_teachables(data)?;
        self.write_learnt(data)?;
        self.write_items(data)?;
        if data.easier_happiness_evolutions && !self.loaded.easier_happiness_evolutions {
            evolutions::ease_happiness(&mut self.rom, self.family != Family::FireRedLeafGreen)?;
        }
        if data.guaranteed_catching && !self.loaded.guaranteed_catching {
            self.enable_guaranteed_catching()?;
        }
        self.write_names_and_palettes(data)?;
        self.apply_tweaks(data)?;
        self.write_player_graphics(data)
    }

    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        out.write_all(self.rom.bytes())?;
        Ok(())
    }
}

/// What people say about the starters, TMs and move tutor moves.
#[derive(Debug)]
struct EventTexts {
    starter: Vec<events::EventText>,
    tm: Vec<events::EventText>,
    tutor: Vec<events::EventText>,
}

impl EventTexts {
    fn read(rom: &GbaRom, entry: &RomEntry) -> Result<Self, RomError> {
        Ok(Self {
            starter: event_texts(rom, entry, "StarterText[]")?,
            tm: event_texts(rom, entry, "TMText[]")?,
            tutor: event_texts(rom, entry, "MoveTutorText[]")?,
        })
    }
}

/// The entry's event texts under `key`, found on their maps.
fn event_texts(rom: &GbaRom, entry: &RomEntry, key: &str) -> Result<Vec<events::EventText>, RomError> {
    let mut texts = events::parse(entry.list(key));
    events::resolve(rom, need(entry, "MapHeaders")?, &mut texts);
    Ok(texts)
}

/// Species names by internal index, from 1.
/// Where the species names start: the pointer in pointer block 1, or else the pointer right before
/// [`c::RS_POKEMON_NAMES_POINTER_SUFFIX`].
fn species_names_offset(rom: &GbaRom, entry: &RomEntry) -> Result<usize, RomError> {
    if entry.int("HasPointerBlock1") == Some(1) {
        rom.pointer(c::POKEMON_NAMES_POINTER)
    } else {
        let at = match rom.find(&hex(c::RS_POKEMON_NAMES_POINTER_SUFFIX)?) {
            Found::Unique(at) if at >= 4 => at,
            _ => return Err(RomError::Malformed("species names pointer not found".into())),
        };
        rom.pointer(at - 4)
    }
}

fn read_species_names(
    rom: &GbaRom,
    entry: &RomEntry,
    text: &TextTable,
    internal_count: usize,
) -> Result<Vec<String>, RomError> {
    let names_offset = species_names_offset(rom, entry)?;
    let name_len = need(entry, "PokemonNameLength")?;
    (1..=internal_count)
        .map(|i| Ok(text.decode(rom.slice(names_offset + i * name_len, name_len)?, name_len)))
        .collect()
}

/// The statics, if the game's are supported; the space their patches write to is taken out of `free_space`.
fn read_statics(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    internal_count: usize,
    order: &PokedexOrder,
    free_space: &mut FreedSpace,
) -> Result<(Option<statics::Statics>, Vec<StaticEncounter>), RomError> {
    let latios = order
        .dex_to_internal
        .get(usize::from(c::LATIOS))
        .copied()
        .unwrap_or_default();
    let Some(statics) = statics::Statics::new(entry, family, internal_count, latios)? else {
        return Ok((None, Vec::new()));
    };
    statics.reserve(free_space);
    let encounters = statics.read(rom, &order.internal_to_dex)?;
    Ok((Some(statics), encounters))
}

/// Species banned from wild encounters: `FireRed` and `LeafGreen` crash on Unown outside the Tanoby Ruins
/// (`GenerateWildMon` in pokefirered).
fn wild_banned(family: Family) -> Vec<DexNumber> {
    if family == Family::FireRedLeafGreen {
        vec![c::UNOWN]
    } else {
        Vec::new()
    }
}

/// The legendaries the main story makes the player face, from the entry's `MainGameLegendaries`.
fn main_game_legendaries(entry: &RomEntry) -> Vec<DexNumber> {
    entry
        .array("MainGameLegendaries")
        .unwrap_or_default()
        .into_iter()
        .filter_map(|n| DexNumber::try_from(n).ok())
        .collect()
}

/// An offset the entry names, or that is read from the ROM header when the entry
/// says the ROM has the pointer block.
fn entry_offset(rom: &GbaRom, entry: &RomEntry, key: &str) -> Result<usize, RomError> {
    let pointer = match key {
        "MoveNames" => Some(("HasPointerBlock1", c::MOVE_NAMES_POINTER)),
        "PokemonFrontImages" => Some(("HasPointerBlock1", c::POKEMON_FRONT_IMAGES_POINTER)),
        "PokemonNormalPalettes" => Some(("HasPointerBlock1", c::POKEMON_NORMAL_PALETTES_POINTER)),
        "PokemonShinyPalettes" => Some(("HasPointerBlock1", c::POKEMON_SHINY_PALETTES_POINTER)),
        "PokemonStats" => Some(("HasPointerBlock2", c::POKEMON_STATS_POINTER)),
        "ItemData" => Some(("HasPointerBlock2", c::ITEM_DATA_POINTER)),
        "AbilityNames" => Some(("HasPointerBlock2", c::ABILITY_NAMES_POINTER)),
        "MoveData" => Some(("HasPointerBlock2", c::MOVE_DATA_POINTER)),
        _ => None,
    };
    match pointer {
        Some((block, at)) if entry.int(block) == Some(1) => rom.pointer(at),
        _ => need(entry, key),
    }
}

/// The items: names from the item data, numbers made standard; key items, unused items and HMs are not
/// allowed, the family's bad items are bad, TM01 to TM50 are TMs.
fn read_items(rom: &GbaRom, entry: &RomEntry, family: Family, text: &TextTable) -> Result<Vec<Item>, RomError> {
    const NAME_MAX: usize = 32;
    let data = entry_offset(rom, entry, "ItemData")?;
    let size = need(entry, "ItemEntrySize")?;
    let count = u16::try_from(need(entry, "ItemCount")?).map_err(|_| RomError::Malformed("item count".into()))?;
    let bad = |id: ItemId| {
        items::BAD_RSE.contains(&id) || (family == Family::FireRedLeafGreen && items::BAD_FRLG_EXTRA.contains(&id))
    };
    let mut out: Vec<Item> = (1..=count)
        .map(|internal| {
            let at = data + size * usize::from(internal);
            let len = NAME_MAX.min(rom.bytes().len().saturating_sub(at));
            let id = items::standard(internal);
            Ok(Item {
                id,
                name: text.decode(rom.slice(at, len)?, len),
                allowed: !items::HMS.contains(&id) && !items::UNIQUE_BANNED.contains(&id),
                bad: bad(id),
                tm: (items::TM01..items::TM01 + items::TM_COUNT).contains(&id),
            })
        })
        .collect::<Result<_, RomError>>()?;
    out.sort_by_key(|i| i.id);
    Ok(out)
}

/// The starters' held item. In Ruby, Sapphire and Emerald the item is a MOVS of one byte, plus an ADD of a
/// second one for items past 0xFF.
fn read_starter_held_item(rom: &GbaRom, entry: &RomEntry, family: Family) -> Result<Option<ItemId>, RomError> {
    let internal = if family == Family::FireRedLeafGreen {
        rom.u16(need(entry, "StarterPokemon")? + c::FRLG_STARTER_ITEMS)?
    } else {
        let at = need(entry, "StarterItems")?;
        let b = rom.slice(at, 3)?;
        if b[2] == 0 {
            u16::from(b[0])
        } else {
            u16::from(b[2]) + 0xFF
        }
    };
    Ok((internal != 0).then(|| items::standard(internal)))
}

/// Deoxys's alternate formes use stats hard-coded next to the entry's `DeoxysStatPrefix` almost all the time;
/// we use them.
fn read_deoxys_stats(rom: &GbaRom, entry: &RomEntry, deoxys: &mut Species) -> Result<(), RomError> {
    let Some(prefix) = entry.value("DeoxysStatPrefix") else {
        return Ok(());
    };
    let Some(at) = found_at(rom.find(&hex(prefix)?)) else {
        return Ok(());
    };
    let at = at + prefix.len() / 2;
    let word = |k: usize| rom.u16(at + k).map(|w| u8::try_from(w & 0xFF).unwrap_or_default());
    deoxys.stats = BaseStats {
        hp: word(0)?,
        attack: word(2)?,
        defense: word(4)?,
        speed: word(6)?,
        sp_attack: word(8)?,
        sp_defense: word(10)?,
    };
    Ok(())
}

/// Writes Deoxys's hard-coded stats where [`read_deoxys_stats`] found them, as words.
fn write_deoxys_stats(rom: &mut GbaRom, entry: &RomEntry, b: BaseStats) -> Result<(), RomError> {
    let Some(prefix) = entry.value("DeoxysStatPrefix") else {
        return Ok(());
    };
    let Some(at) = found_at(rom.find(&hex(prefix)?)) else {
        return Ok(());
    };
    let at = at + prefix.len() / 2;
    for (k, stat) in [b.hp, b.attack, b.defense, b.speed, b.sp_attack, b.sp_defense]
        .into_iter()
        .enumerate()
    {
        rom.write_u16(at + 2 * k, u16::from(stat))?;
    }
    Ok(())
}

/// The type chart: (attacker, defender, effectiveness) triples up to `0xFF`; the `0xFE` marker before the
/// immunities Foresight removes is skipped.
fn read_type_chart(rom: &GbaRom, entry: &RomEntry, types: &[Type]) -> Result<TypeChart, RomError> {
    let mut chart = TypeChart::neutral(types);
    let start = need(entry, "TypeEffectivenessOffset")?;
    // 18 × 18 pairs at most, plus the markers.
    for at in (start..).step_by(c::TYPE_CHART_ENTRY_SIZE).take(18 * 18 + 2) {
        let b = rom.slice(at, c::TYPE_CHART_ENTRY_SIZE)?;
        match b[0] {
            c::TYPE_CHART_END => return Ok(chart),
            c::TYPE_CHART_FORESIGHT => continue,
            _ => {}
        }
        let effectiveness = match b[2] {
            20 => Effectiveness::Double,
            10 => Effectiveness::Neutral,
            5 => Effectiveness::Half,
            0 => Effectiveness::Zero,
            _ => continue,
        };
        let (Some(attacker), Some(defender)) = (c::type_from_byte(b[0]), c::type_from_byte(b[1])) else {
            return Err(RomError::Malformed(format!(
                "type chart entry at {at:#x} has an unknown type"
            )));
        };
        chart.set(attacker, defender, effectiveness);
    }
    Err(RomError::Malformed("the type chart has no end".into()))
}

/// The type chart: the pairs that are not neutral, in [`c::TYPE_ORDER`], Ghost immunities after the
/// Foresight marker, as the game checks them only when Foresight is not used.
fn write_type_chart(rom: &mut GbaRom, entry: &RomEntry, chart: &TypeChart) -> Result<(), RomError> {
    if chart.non_neutral_count() > c::TYPE_CHART_ROOM {
        return Err(RomError::Malformed(format!(
            "the type chart has {} pairs that are not neutral; Gen 3 has room for {}",
            chart.non_neutral_count(),
            c::TYPE_CHART_ROOM
        )));
    }
    let (mut main, mut ghost) = (Vec::new(), Vec::new());
    for &attacker in &c::TYPE_ORDER {
        for &defender in &c::TYPE_ORDER {
            let value = match chart.get(attacker, defender) {
                Some(Effectiveness::Double) => 20,
                Some(Effectiveness::Half) => 5,
                Some(Effectiveness::Zero) => 0,
                Some(Effectiveness::Neutral) | None => continue,
            };
            let part = if defender == Type::Ghost && value == 0 {
                &mut ghost
            } else {
                &mut main
            };
            let byte = |t| c::type_to_byte(t).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no type {t}")));
            part.extend([byte(attacker)?, byte(defender)?, value]);
        }
    }
    let mut data = main;
    data.extend([c::TYPE_CHART_FORESIGHT, c::TYPE_CHART_FORESIGHT, 0]);
    data.extend(ghost);
    data.extend([c::TYPE_CHART_END, c::TYPE_CHART_END, 0]);
    rom.write(need(entry, "TypeEffectivenessOffset")?, &data)
}

/// Species with Shadow Tag, Magnet Pull or Arena Trap.
fn battle_trappers(rom: &GbaRom, stats_offset: usize, internal_to_dex: &[DexNumber]) -> BTreeSet<DexNumber> {
    internal_to_dex
        .iter()
        .enumerate()
        .skip(1)
        .filter(|&(i, &n)| {
            n != 0
                && rom
                    .slice(stats_offset + i * c::BASE_STATS_ENTRY_SIZE, c::BASE_STATS_ENTRY_SIZE)
                    .is_ok_and(|b| {
                        [b[c::BS_ABILITY_1], b[c::BS_ABILITY_2]]
                            .iter()
                            .any(|a| c::BATTLE_TRAPPING_ABILITIES.contains(a))
                    })
        })
        .map(|(_, &n)| n)
        .collect()
}

fn internal_index(dex_to_internal: &[u16], number: DexNumber) -> Result<u16, RomError> {
    dex_to_internal
        .get(usize::from(number))
        .copied()
        .filter(|&i| i != 0)
        .ok_or_else(|| RomError::Malformed(format!("species #{number} has no internal index")))
}

fn read_species(rom: &GbaRom, at: usize, number: DexNumber, name: &str) -> Result<Species, RomError> {
    let b = rom.slice(at, c::BASE_STATS_ENTRY_SIZE)?;
    let ty = |off: usize| {
        c::type_from_byte(b[off])
            .ok_or_else(|| RomError::Malformed(format!("species #{number} has unknown type byte {:#04x}", b[off])))
    };
    let primary_type = ty(c::BS_PRIMARY_TYPE)?;
    let secondary = ty(c::BS_SECONDARY_TYPE)?;
    Ok(Species {
        forme: ppr_model::Forme::default(),
        number,
        name: name.to_owned(),
        primary_type,
        secondary_type: (secondary != primary_type).then_some(secondary),
        stats: BaseStats {
            hp: b[c::BS_HP],
            attack: b[c::BS_ATTACK],
            defense: b[c::BS_DEFENSE],
            speed: b[c::BS_SPEED],
            sp_attack: b[c::BS_SP_ATTACK],
            sp_defense: b[c::BS_SP_DEFENSE],
        },
        catch_rate: b[c::BS_CATCH_RATE],
        abilities: vec![u16::from(b[c::BS_ABILITY_1]), u16::from(b[c::BS_ABILITY_2])],
        growth_curve: ExpCurve::from_byte(b[c::BS_GROWTH_CURVE]).ok_or_else(|| {
            RomError::Malformed(format!(
                "species #{number} has unknown growth curve {:#04x}",
                b[c::BS_GROWTH_CURVE]
            ))
        })?,
    })
}

/// Every species by national number. A later internal slot with the same number replaces an earlier one;
/// Deoxys has its hard-coded stats outside Ruby and Sapphire.
fn read_all_species<'a>(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    order: &PokedexOrder,
    stats_offset: usize,
    internal_count: usize,
    name_of: impl Fn(usize) -> &'a str,
) -> Result<Vec<Species>, RomError> {
    let mut by_dex: Vec<Option<Species>> = vec![None; order.pokedex_count + 1];
    for i in 1..=internal_count {
        let number = order.internal_to_dex[i];
        if number == 0 {
            continue;
        }
        let slot = by_dex
            .get_mut(usize::from(number))
            .ok_or_else(|| RomError::Malformed(format!("national number {number} beyond the Pokédex")))?;
        *slot = Some(read_species(
            rom,
            stats_offset + i * c::BASE_STATS_ENTRY_SIZE,
            number,
            name_of(i),
        )?);
    }

    if family != Family::RubySapphire
        && let Some(Some(deoxys)) = by_dex.get_mut(usize::from(c::DEOXYS))
    {
        read_deoxys_stats(rom, entry, deoxys)?;
    }

    // In hacked ROMs, skip placeholder names.
    Ok(by_dex
        .into_iter()
        .flatten()
        .filter(|s| {
            let lower = s.name.to_lowercase();
            !order.is_rom_hack || (!lower.contains("unused") && lower != "?")
        })
        .collect())
}

/// The wild held items of the species loaded: two equal words are a guaranteed item,
/// others a common and a rare one (0 for none). Like the species, a later internal slot replaces an earlier one.
fn read_wild_held_items(
    rom: &GbaRom,
    species: &[Species],
    order: &PokedexOrder,
    stats_offset: usize,
    internal_count: usize,
) -> Result<BTreeMap<DexNumber, WildHeldItems>, RomError> {
    let item = |w: u16| (w != 0).then(|| items::standard(w));
    let mut out = BTreeMap::new();
    for i in 1..=internal_count {
        let number = order.internal_to_dex[i];
        if !species.iter().any(|s| s.number == number) {
            continue;
        }
        let at = stats_offset + i * c::BASE_STATS_ENTRY_SIZE;
        let common = rom.u16(at + c::BS_COMMON_HELD_ITEM)?;
        let rare = rom.u16(at + c::BS_RARE_HELD_ITEM)?;
        let held = if common == rare {
            WildHeldItems {
                guaranteed: item(common),
                ..WildHeldItems::default()
            }
        } else {
            WildHeldItems {
                common: item(common),
                rare: item(rare),
                ..WildHeldItems::default()
            }
        };
        out.insert(number, held);
    }
    Ok(out)
}

/// The national ↔ internal index maps.
struct PokedexOrder {
    internal_to_dex: Vec<DexNumber>,
    dex_to_internal: Vec<u16>,
    pokedex_count: usize,
    is_rom_hack: bool,
}

impl PokedexOrder {
    fn read<'a>(
        rom: &GbaRom,
        entry: &RomEntry,
        internal_count: usize,
        stats_offset: usize,
        name_of: impl Fn(usize) -> &'a str,
    ) -> Result<Self, RomError> {
        let order_offset = need(entry, "PokedexOrder")?;
        let mut internal_to_dex = vec![0; internal_count + 1];
        let mut dex_to_internal = vec![0u16; internal_count + 1];
        let mut max_dex = 0;
        for (i, to_dex) in internal_to_dex.iter_mut().enumerate().skip(1) {
            let dex = rom.u16(order_offset + (i - 1) * 2)?;
            if dex != 0 {
                *to_dex = dex;
                let slot = dex_to_internal
                    .get_mut(usize::from(dex))
                    .ok_or_else(|| RomError::Malformed(format!("national number {dex} beyond the internal count")))?;
                if *slot == 0 {
                    *slot = u16::try_from(i).unwrap_or_default();
                }
                max_dex = max_dex.max(usize::from(dex));
            }
        }
        if max_dex != c::UNHACKED_MAX_POKEDEX {
            return Ok(Self {
                internal_to_dex,
                dex_to_internal,
                pokedex_count: max_dex,
                is_rom_hack: true,
            });
        }
        // The slots between Johto and Hoenn: old hacks used them instead of adding species.
        let spare = c::UNHACKED_MAX_POKEDEX - c::UNHACKED_REAL_POKEDEX;
        let mut used = 0;
        for k in 0..spare {
            let slot = c::HOENN_POKES_START + k;
            let lower = name_of(slot).to_lowercase();
            let empty = rom.slice(
                stats_offset + slot * c::BASE_STATS_ENTRY_SIZE,
                c::EMPTY_POKEMON_SIG.len(),
            )? == c::EMPTY_POKEMON_SIG;
            if !empty && !lower.contains("unused") && lower != "?" && lower != "-" {
                used += 1;
                let dex = c::UNHACKED_REAL_POKEDEX + used;
                dex_to_internal[dex] = u16::try_from(slot).unwrap_or_default();
                internal_to_dex[slot] = u16::try_from(dex).unwrap_or_default();
            } else {
                internal_to_dex[slot] = 0;
            }
        }
        for k in used + 1..=spare {
            dex_to_internal[c::UNHACKED_REAL_POKEDEX + k] = 0;
        }
        Ok(Self {
            internal_to_dex,
            dex_to_internal,
            pokedex_count: c::UNHACKED_REAL_POKEDEX + used,
            is_rom_hack: used > 0,
        })
    }
}

/// The three starters, from the entry's `StarterPokemon`.
fn read_starters(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    order: &PokedexOrder,
) -> Result<Vec<DexNumber>, RomError> {
    let base = need(entry, "StarterPokemon")?;
    let (second, third) = if family == Family::FireRedLeafGreen {
        (c::FRLG_STARTER_2, c::FRLG_STARTER_3)
    } else {
        (c::RSE_STARTER_2, c::RSE_STARTER_3)
    };
    [base, base + second, base + third]
        .into_iter()
        .map(|at| {
            let internal = usize::from(rom.u16(at)?);
            order
                .internal_to_dex
                .get(internal)
                .copied()
                .filter(|&n| n != 0)
                .ok_or_else(|| RomError::Malformed(format!("starter at {at:#x} is not a species ({internal})")))
        })
        .collect()
}

#[cfg(test)]
mod tests;
