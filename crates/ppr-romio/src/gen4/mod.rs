//! Generation 4 (Diamond, Pearl, Platinum, `HeartGold`, `SoulSilver`).
//!
//! The games keep their data in NARC archives, overlays and the ARM9 binary of a DS ROM ([`files`]); text is in
//! one NARC of encrypted text files. Species numbers are national numbers. The alternate formes with data of their
//! own (Deoxys, Wormadam, and from Platinum on Giratina, Shaymin and Rotom) follow them from 494, their files two
//! after their number; they have their base species' name. Trainer and static Pokémon use them as their species,
//! with their forme number, and are written as their base species and forme; wild Pokémon have no forme. Burmy's
//! links to Wormadam's formes are evolutions of kind `None`, which the game doesn't store.
//!
//! Deliberate choices:
//! - Unown, Burmy, Shellos and Gastrodon get forme numbers 1 up to their number of formes less one for their
//!   cosmetic formes; the original gave them one more, a forme the game doesn't have.
//! - Only what changed is written, so an unchanged game saves byte for byte; text files are written unpacked.
//! - The ARM9 is extended when loaded, as the ROM entries' offsets count with that, but it is only written back
//!   when something else in it changed.

mod arm9;
mod constants;
mod encounter_tags;
mod encounters;
mod entry;
mod evolutions;
mod field_items;
mod files;
mod intro;
mod learnsets;
mod moves;
mod palettes;
mod pokedex_maps;
mod shops;
mod starters;
mod statics;
mod tms;
mod trades;
mod trainer_tags;
mod trainers;
mod tweaks;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use ppr_model::{
    BaseStats, Capabilities, DexNumber, Effectiveness, ExpCurve, Forme, FormeRules, GameData, Generation, Item, ItemId,
    RomIdentity, Species, Type, TypeChart, WildHeldItems,
};

use self::arm9::Arm9Room;
use self::constants::{self as c, Family};
use self::entry::Gen4Entry;
use self::files::{DsFiles, hex_bytes, put_word, word};
use crate::type_order::TYPE_ORDER;
use crate::{GameHandler, RomError};

/// The loaded ROM: its files, its entry and the room in its ARM9.
#[derive(Debug)]
pub(super) struct Rom {
    files: DsFiles,
    entry: Gen4Entry,
    room: Arm9Room,
}

impl Rom {
    fn open(bytes: Vec<u8>, entry: Gen4Entry) -> Result<Self, RomError> {
        let mut room = Arm9Room::default();
        let files = DsFiles::open(bytes, entry.file("Text")?, |arm9| {
            let patch = entry.int("Arm9PatchExtensionSize");
            let repoint = entry.int("Arm9RepointExtensionSize");
            if patch + repoint != 0 {
                let prefix = hex_bytes(entry.string("TCMCopyingPrefix"))
                    .ok_or_else(|| RomError::Malformed("no valid TCMCopyingPrefix".into()))?;
                room = arm9::extend(arm9, patch + repoint, repoint, &prefix)?;
            }
            // The new catching tutorial routine makes the tutorial show the player's Pokémon; as the tutorial
            // may be changed later and needs it then, it is part of the ARM9 from the start.
            if entry.family == Family::HeartGoldSoulSilver && entry.has_tweak("NewCatchingTutorialSubroutineTweak") {
                ips_patch(&entry, arm9, "NewCatchingTutorialSubroutineTweak")?;
            }
            Ok(())
        })?;
        Ok(Self { files, entry, room })
    }

    fn family(&self) -> Family {
        self.entry.family
    }

    /// The strings of the text file the entry's `key` names.
    fn strings_of(&mut self, key: &str) -> Result<Vec<String>, RomError> {
        let index = self.entry.need(key)?;
        self.files.strings(index)
    }

    /// The text file's lines are as long as this.
    fn chars_per_line(&self) -> usize {
        if self.family().is_dppt() {
            c::DPPT_TEXT_CHARS_PER_LINE
        } else {
            c::HGSS_TEXT_CHARS_PER_LINE
        }
    }

    /// Species stats file of `number`.
    fn stats(&mut self, number: DexNumber) -> Result<&mut Vec<u8>, RomError> {
        let path = self.entry.file("PokemonStats")?;
        self.files.narc_file(&path, c::species_file(number))
    }

    /// The numbers of the species and the alternate formes with data of their own: 1 to 493, then the formes from
    /// 494.
    fn species_numbers(&self) -> std::ops::RangeInclusive<DexNumber> {
        let last = c::SPECIES_COUNT + self.family().forme_count();
        1..=DexNumber::try_from(last).unwrap_or(DexNumber::MAX)
    }
}

/// Applies the patch the entry names for `key` to `data`.
fn ips_patch(entry: &Gen4Entry, data: &mut [u8], key: &str) -> Result<(), RomError> {
    let name = entry.string(key);
    if name.is_empty() {
        return Err(RomError::Malformed(format!(
            "ROM entry [{}] has no {key}",
            entry.name()
        )));
    }
    crate::ips::named(name)?.apply(data)
}

/// Loaded Gen 4 ROM.
#[derive(Debug)]
pub(crate) struct Gen4Handler {
    rom: Rom,
    loaded: GameData,
    /// Where each field item in `GameData::field_items` is.
    field_item_places: Vec<field_items::Place>,
    pickup_tables: field_items::PickupTables,
    /// What the trainers' teams hold beyond the game data.
    teams: trainers::Teams,
    statics: statics::Statics,
    encounters: encounters::Encounters,
}

impl Gen4Handler {
    #[expect(clippy::too_many_lines, reason = "a line for each field of GameData")]
    pub(crate) fn load(bytes: Vec<u8>, identity: RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(4).ok_or_else(|| RomError::Malformed("generation 4".into()))?;
        let entry = Gen4Entry::named(&identity.name)?;
        let mut rom = Rom::open(bytes, entry)?;
        let family = rom.family();

        let items = read_items(&mut rom)?;
        let (species, wild_held_items) = read_species(&mut rom)?;
        let types = Type::in_generation(generation);
        let type_chart = read_type_chart(&mut rom, &types)?;
        let moves = moves::read(&mut rom)?;
        let evolutions = evolutions::read(&mut rom)?;
        let learnsets = learnsets::read(&mut rom)?;
        let egg_moves = learnsets::read_egg_moves(&mut rom)?;
        let (tm_moves, hm_moves) = tms::read(&mut rom)?;
        let tm_hm_compat = tms::read_compat(&mut rom)?;
        let tutor_moves = tms::read_tutors(&mut rom)?;
        let tutor_compat = tms::read_tutor_compat(&mut rom, tutor_moves.len())?;
        let starters = starters::read(&mut rom)?;
        let starter_held_items = starters::read_held_items(&mut rom)?;
        let ability_names = rom.strings_of("AbilityNamesTextOffset")?;
        let shops = shops::read(&mut rom, items.len())?;
        let item_prices = shops::read_prices(&mut rom)?;
        let (field_item_places, field_items) = field_items::read(&mut rom, &items)?;
        let (pickup_tables, pickup_items) = field_items::read_pickup(&mut rom)?;
        let available_tweaks = tweaks::available(&rom);
        let (trainers, teams) = trainers::read(&mut rom)?;
        let trainer_class_names = trainers::class_names(&mut rom)?;
        let (statics, static_encounters) = statics::read(&mut rom)?;
        let trades = trades::read(&mut rom)?;
        let intro_candidates = intro::candidates(&mut rom)?;
        let (catching_tutorial_opponents, catching_tutorial_players) = intro::tutorial_candidates(&rom);
        let (encounter_places, encounters) = encounters::read(&mut rom)?;
        let palettes = palettes::read(&mut rom)?;

        let mut loaded = GameData {
            formes: FormeRules {
                trainers: true,
                wild: false,
                statics: true,
                starters: false,
                evolutions: false,
                banned_for_trainers: if family == Family::DiamondPearl {
                    Vec::new()
                } else {
                    vec![c::GIRATINA_ORIGIN]
                },
                trainer_base_forme_abilities: true,
                force_swap_static_megas: false,
            },
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
            // Gen 4 wild Pokémon may always hold an item, and has no dark grass.
            guaranteed_wild_held_items: true,
            dark_grass_held_items: false,
            palettes,
            palette_files: Some("DP".to_owned()),
            player_graphics_packs: Vec::new(),
            player_graphics: None,
            encounters,
            guaranteed_catching: false,
            statics: static_encounters,
            main_game_legendaries: rom
                .entry
                .array("MainGameLegendaries")
                .into_iter()
                .filter_map(|n| DexNumber::try_from(n).ok())
                .collect(),
            special_music_statics: statics::special_music(&rom),
            correct_static_music: false,
            totems: Vec::new(),
            easier_happiness_evolutions: false,
            trades,
            max_trade_nickname_length: trades::MAX_NICKNAME_LENGTH,
            max_trade_ot_name_length: trades::MAX_OT_NAME_LENGTH,
            wild_banned: encounters::WILD_BANNED.to_vec(),
            ability_names,
            ability_variations: c::ABILITY_VARIATIONS
                .iter()
                .map(|&(base, twins)| (base, twins.to_vec()))
                .collect(),
            useless_abilities: c::USELESS_ABILITIES.to_vec(),
            moves,
            max_move_name_length: moves::MAX_NAME_LENGTH,
            learnsets,
            egg_moves,
            hm_moves,
            game_breaking_moves: c::GAME_BREAKING_MOVES.to_vec(),
            banned_level_up_moves: Vec::new(),
            four_starting_moves: true,
            tm_moves,
            tutor_moves,
            tm_hm_compat,
            tutor_compat,
            early_required_hm_moves: if family.is_dppt() {
                c::DPPT_EARLY_REQUIRED_HM_MOVES.to_vec()
            } else {
                c::HGSS_EARLY_REQUIRED_HM_MOVES.to_vec()
            },
            field_moves: if family.is_dppt() {
                c::DPPT_FIELD_MOVES.to_vec()
            } else {
                c::HGSS_FIELD_MOVES.to_vec()
            },
            field_items,
            required_field_tms: match family {
                Family::DiamondPearl => c::DP_REQUIRED_FIELD_TMS.to_vec(),
                Family::Platinum => c::PT_REQUIRED_FIELD_TMS.to_vec(),
                Family::HeartGoldSoulSilver => Vec::new(),
            },
            shops,
            item_prices,
            balanced_item_prices: c::BALANCED_ITEM_PRICES.into_iter().collect(),
            op_shop_items: c::OP_SHOP_ITEMS.to_vec(),
            mega_stones: Vec::new(),
            evolution_items: c::EVOLUTION_ITEMS.to_vec(),
            pickup_items,
            pickup_tms: true,
            trainers,
            trainer_class_names,
            elite_four_trainers: rom
                .entry
                .array("EliteFourIndices")
                .into_iter()
                .filter_map(|i| u16::try_from(i).ok())
                .collect(),
            gym_and_elite_themes: trainers::gym_and_elite_themes(family),
            original_types: BTreeMap::new(),
            max_trainer_name_length: 10,
            trainer_name_shares_class_line: false,
            max_trainer_names_total: None,
            max_trainer_class_name_length: 12,
            doubles_trainer_classes: rom
                .entry
                .array("DoublesTrainerClasses")
                .into_iter()
                .filter_map(|i| u8::try_from(i).ok())
                .collect(),
            personal_trainer_classes: Vec::new(),
            fixed_trainer_text: false,
            fixed_trainer_name_length: false,
            fixed_trainer_class_name_length: false,
            text_tokens: Vec::new(),
            intro_candidates,
            intro_pokemon: None,
            held_item_pools: c::held_item_pools(),
            trainer_pokemon_additions: [true; 3],
            trainer_custom_movesets: [true; 3],
            trainer_held_item_additions: [true; 3],
            available_tweaks,
            applied_tweaks: BTreeSet::new(),
            pc_potion: None,
            catching_tutorial_opponents,
            catching_tutorial_players,
            catching_tutorial: None,
            fossils: statics.fossils(),
        };
        // The evolution level estimates are taken from the game as loaded.
        loaded.estimate_evolution_levels();
        Ok(Self {
            rom,
            loaded,
            field_item_places,
            pickup_tables,
            teams,
            statics,
            encounters: encounter_places,
        })
    }

    /// Shops, prices, field items and Pickup, where they changed.
    fn write_items(&mut self, data: &GameData) -> Result<(), RomError> {
        let loaded = &self.loaded;
        if data.shops != loaded.shops {
            shops::write(&mut self.rom, &loaded.shops, &data.shops, loaded.items.len())?;
        }
        if data.item_prices != loaded.item_prices {
            shops::write_prices(&mut self.rom, &loaded.item_prices, &data.item_prices)?;
        }
        if data.field_items != loaded.field_items {
            field_items::write(
                &mut self.rom,
                &self.field_item_places,
                &loaded.field_items,
                &data.field_items,
            )?;
        }
        if data.pickup_items != loaded.pickup_items {
            field_items::write_pickup(&mut self.rom, self.pickup_tables, &data.pickup_items)?;
        }
        Ok(())
    }

    /// The species data and names, where they changed.
    fn write_species(&mut self, data: &GameData) -> Result<(), RomError> {
        let loaded = &self.loaded;
        for s in &data.species {
            let Some(old) = loaded.species(s.number) else {
                continue;
            };
            let held = data.wild_held_items.get(&s.number).copied().unwrap_or_default();
            let old_held = loaded.wild_held_items.get(&s.number).copied().unwrap_or_default();
            if old == s && held == old_held {
                continue;
            }
            let stats = self.rom.stats(s.number)?;
            write_species_stats(stats, s, held)?;
        }
        // Alternate formes have their base species' name, which is written for the base species.
        let names_changed = data
            .base_species()
            .any(|s| loaded.species(s.number).is_some_and(|old| old.name != s.name));
        if names_changed {
            let index = self.rom.entry.need("PokemonNamesTextOffset")?;
            let mut names = self.rom.files.strings(index)?;
            // The names with an article ("a Bulbasaur") follow, when the game has them.
            let mut with_article = if self
                .rom
                .entry
                .string("HasExtraPokemonNames")
                .eq_ignore_ascii_case("yes")
            {
                Some(self.rom.files.strings(index + 1)?)
            } else {
                None
            };
            for s in data.base_species() {
                let i = usize::from(s.number);
                let Some(old) = names.get(i).cloned() else {
                    continue;
                };
                if old == s.name {
                    continue;
                }
                names[i].clone_from(&s.name);
                if let Some(other) = with_article.as_mut().and_then(|w| w.get_mut(i)) {
                    *other = other.replace(&old, &s.name);
                }
            }
            self.rom.files.set_strings(index, names)?;
            if let Some(w) = with_article {
                self.rom.files.set_strings(index + 1, w)?;
            }
        }
        Ok(())
    }
}

/// Writes a species' base stats, types, catch rate, growth curve, abilities and wild held items into its stats
/// file. One type is stored twice; a guaranteed held item is the common and the rare one.
fn write_species_stats(stats: &mut [u8], s: &Species, held: WildHeldItems) -> Result<(), RomError> {
    let b = s.stats;
    let mut fixed = [0u8; 8];
    fixed[c::BS_HP] = b.hp;
    fixed[c::BS_ATTACK] = b.attack;
    fixed[c::BS_DEFENSE] = b.defense;
    fixed[c::BS_SPEED] = b.speed;
    fixed[c::BS_SP_ATK] = b.sp_attack;
    fixed[c::BS_SP_DEF] = b.sp_defense;
    fixed[c::BS_PRIMARY_TYPE] = c::type_to_byte(s.primary_type);
    fixed[c::BS_SECONDARY_TYPE] = c::type_to_byte(s.secondary_type.unwrap_or(s.primary_type));
    files::put(stats, 0, &fixed)?;
    files::put(stats, c::BS_CATCH_RATE, &[s.catch_rate])?;
    files::put(stats, c::BS_GROWTH_CURVE, &[s.growth_curve.to_byte()])?;
    let ability = |i: usize| {
        let a = s.abilities.get(i).copied().unwrap_or_default();
        u8::try_from(a).map_err(|_| RomError::Malformed(format!("Gen 4 has no ability {a}")))
    };
    files::put(stats, c::BS_ABILITY_1, &[ability(0)?, ability(1)?])?;
    let (common, rare) = match held.guaranteed {
        Some(item) => (item, item),
        None => (held.common.unwrap_or(0), held.rare.unwrap_or(0)),
    };
    put_word(stats, c::BS_COMMON_HELD_ITEM, common)?;
    put_word(stats, c::BS_RARE_HELD_ITEM, rare)
}

/// The items, 1 on, by their names; their numbers are the standard ones.
fn read_items(rom: &mut Rom) -> Result<Vec<Item>, RomError> {
    let names = rom.strings_of("ItemNamesTextOffset")?;
    let tms = c::TM01..c::TM01 + u16::try_from(c::TM_COUNT).unwrap_or_default();
    names
        .into_iter()
        .enumerate()
        .skip(1)
        .map(|(i, name)| {
            let id = ItemId::try_from(i).map_err(|_| RomError::Malformed("too many items".into()))?;
            Ok(Item {
                id,
                name,
                allowed: !c::item_banned(id),
                bad: c::item_bad(id),
                tm: tms.contains(&id),
            })
        })
        .collect()
}

/// The forme links of species or alternate forme `number`: alternate formes from 494 by [`c::FORMES`], cosmetic
/// formes from Platinum on.
fn forme_of(family: Family, number: DexNumber) -> Forme {
    let n = usize::from(number);
    if let Some(&(base, forme, suffix)) = n.checked_sub(c::SPECIES_COUNT + 1).and_then(|i| c::FORMES.get(i)) {
        return Forme {
            base: Some(base),
            number: forme,
            suffix: suffix.to_owned(),
            ..Forme::default()
        };
    }
    let cosmetic = c::COSMETIC_FORMES
        .iter()
        .find(|&&(b, _)| b == number && family != Family::DiamondPearl)
        .map_or(0, |&(_, count)| count);
    Forme {
        suffix: c::BASE_FORME_SUFFIXES
            .iter()
            .find(|&&(b, _)| b == number)
            .map(|&(_, s)| s.to_owned())
            .unwrap_or_default(),
        cosmetic_formes: (1..cosmetic).collect(),
        ..Forme::default()
    }
}

/// The species 1 to 493 and the alternate formes after them, and what they may hold in the wild: two equal held
/// item words are a guaranteed item. A forme has its base species' name.
fn read_species(rom: &mut Rom) -> Result<(Vec<Species>, BTreeMap<DexNumber, WildHeldItems>), RomError> {
    let names = rom.strings_of("PokemonNamesTextOffset")?;
    let family = rom.family();
    let mut species = Vec::with_capacity(c::SPECIES_COUNT + family.forme_count());
    let mut held = BTreeMap::new();
    for number in rom.species_numbers() {
        let i = usize::from(number);
        let forme = forme_of(family, number);
        let name_index = usize::from(forme.base.unwrap_or(number));
        let name = names
            .get(name_index)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no name for species {name_index}")))?;
        let b = rom.stats(number)?.clone();
        let byte = |at: usize| files::byte(&b, at);
        let ty = |at: usize| {
            let v = byte(at)?;
            c::type_from_byte(v).ok_or_else(|| RomError::Malformed(format!("species {i} has type byte {v:#04x}")))
        };
        let primary_type = ty(c::BS_PRIMARY_TYPE)?;
        let secondary = ty(c::BS_SECONDARY_TYPE)?;
        let curve = byte(c::BS_GROWTH_CURVE)?;
        species.push(Species {
            forme,
            number,
            name,
            primary_type,
            secondary_type: (secondary != primary_type).then_some(secondary),
            stats: BaseStats {
                hp: byte(c::BS_HP)?,
                attack: byte(c::BS_ATTACK)?,
                defense: byte(c::BS_DEFENSE)?,
                speed: byte(c::BS_SPEED)?,
                sp_attack: byte(c::BS_SP_ATK)?,
                sp_defense: byte(c::BS_SP_DEF)?,
            },
            catch_rate: byte(c::BS_CATCH_RATE)?,
            abilities: vec![u16::from(byte(c::BS_ABILITY_1)?), u16::from(byte(c::BS_ABILITY_2)?)],
            growth_curve: ExpCurve::from_byte(curve)
                .ok_or_else(|| RomError::Malformed(format!("species {i} has growth curve {curve:#04x}")))?,
        });
        let item = |at: usize| word(&b, at).map(|w| (w != 0).then_some(w));
        let (common, rare) = (item(c::BS_COMMON_HELD_ITEM)?, item(c::BS_RARE_HELD_ITEM)?);
        let h = if common == rare {
            WildHeldItems {
                guaranteed: common,
                ..WildHeldItems::default()
            }
        } else {
            WildHeldItems {
                common,
                rare,
                ..WildHeldItems::default()
            }
        };
        if !h.is_empty() {
            held.insert(number, h);
        }
    }
    Ok((species, held))
}

/// The type chart in the battle overlay: triples of attacker, defender and effectiveness (20, 10, 5 or 0),
/// with a marker before the Ghost immunities that Foresight lifts, and one at the end.
fn read_type_chart(rom: &mut Rom, types: &[Type]) -> Result<TypeChart, RomError> {
    let mut chart = TypeChart::neutral(types);
    let start = rom.entry.need("TypeEffectivenessOffset")?;
    let overlay = rom.files.overlay(rom.entry.need("BattleOvlNumber")?)?;
    for at in (start..overlay.len()).step_by(3) {
        let b = overlay
            .get(at..at + 3)
            .ok_or_else(|| RomError::Malformed("the type chart has no end".into()))?;
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

/// The type chart: the pairs that are not neutral, Ghost immunities after the Foresight marker.
fn write_type_chart(rom: &mut Rom, chart: &TypeChart) -> Result<(), RomError> {
    if chart.non_neutral_count() > c::TYPE_CHART_ROOM {
        return Err(RomError::Malformed(format!(
            "the type chart has {} pairs that are not neutral; Gen 4 has room for {}",
            chart.non_neutral_count(),
            c::TYPE_CHART_ROOM
        )));
    }
    let (mut main, mut ghost) = (Vec::new(), Vec::new());
    for &attacker in &TYPE_ORDER {
        for &defender in &TYPE_ORDER {
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
            part.extend([c::type_to_byte(attacker), c::type_to_byte(defender), value]);
        }
    }
    let mut data = main;
    data.extend([c::TYPE_CHART_FORESIGHT, c::TYPE_CHART_FORESIGHT, 0]);
    data.extend(ghost);
    data.extend([c::TYPE_CHART_END, c::TYPE_CHART_END, 0]);
    let at = rom.entry.need("TypeEffectivenessOffset")?;
    let overlay = rom.files.overlay(rom.entry.need("BattleOvlNumber")?)?;
    files::put(overlay, at, &data)
}

impl GameHandler for Gen4Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    /// Tutors from Platinum on, starter held items in Diamond, Pearl and Platinum, time-based encounters in
    /// `HeartGold` and `SoulSilver`; move names in English games (a game code ending in `E`).
    fn capabilities(&self) -> Capabilities {
        let family = self.rom.family();
        Capabilities {
            intro_pokemon: true,
            starter_held_items: family.is_dppt(),
            move_tutors: family != Family::DiamondPearl,
            map_indices: true,
            encounter_locations: true,
            shop_sizes: true,
            time_based_encounters: family == Family::HeartGoldSoulSilver,
            physical_special_split: true,
            functional_formes: family != Family::DiamondPearl,
            english: self.loaded.identity.code.ends_with('E'),
            ..Capabilities::default()
        }
    }

    /// The shops share the room they had in the ARM9 and the bytes it was extended by: the progressive shop takes 4
    /// bytes an item, every other shop 2 an item and 2 for its end; wild sets store levels as their kind does.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        let costs: Vec<_> = (0..self.loaded.shops.len())
            .map(|i| {
                if i == 0 {
                    ppr_model::ShopCost { base: 0, per_item: 4 }
                } else {
                    ppr_model::ShopCost { base: 2, per_item: 2 }
                }
            })
            .collect();
        let loaded: usize = self
            .loaded
            .shops
            .iter()
            .zip(&costs)
            .map(|(s, c)| c.of(s.items.len()))
            .sum();
        rules.shops.size = ppr_model::ShopSize::Shared {
            bytes: loaded + self.rom.entry.int("Arm9RepointExtensionSize"),
            costs,
            only_changed: false,
            max: usize::from(u8::MAX),
        };
        rules.set_wild_levels(self.encounters.levels().into_iter().enumerate());
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        self.write_species(data)?;
        if data.type_chart != self.loaded.type_chart {
            write_type_chart(&mut self.rom, &data.type_chart)?;
        }
        if data.moves != self.loaded.moves {
            moves::write(&mut self.rom, &self.loaded.moves, &data.moves)?;
        }
        if data.evolutions != self.loaded.evolutions {
            evolutions::write(&mut self.rom, &self.loaded.evolutions, &data.evolutions)?;
        }
        if data.easier_happiness_evolutions && !self.loaded.easier_happiness_evolutions {
            evolutions::ease_happiness(&mut self.rom);
        }
        if data.learnsets != self.loaded.learnsets {
            learnsets::write(&mut self.rom, &self.loaded.learnsets, &data.learnsets)?;
        }
        if data.egg_moves != self.loaded.egg_moves {
            learnsets::write_egg_moves(&mut self.rom, &data.egg_moves)?;
        }
        if data.palettes != self.loaded.palettes {
            palettes::write(&mut self.rom, &self.loaded.palettes, &data.palettes)?;
        }
        tms::write(&mut self.rom, &self.loaded, data)?;
        starters::write(&mut self.rom, &self.loaded, data)?;
        encounters::write(
            &mut self.rom,
            &self.encounters,
            &self.loaded.encounters,
            &data.encounters,
        )?;
        trainers::write(&mut self.rom, &self.teams, &self.loaded, data)?;
        self.write_items(data)?;
        for &tweak in data.applied_tweaks.difference(&self.loaded.applied_tweaks) {
            tweaks::apply(&mut self.rom, tweak)?;
        }
        if data.guaranteed_catching && !self.loaded.guaranteed_catching {
            tweaks::guaranteed_catching(&mut self.rom)?;
        }
        if data.correct_static_music && !self.loaded.correct_static_music {
            statics::correct_music(
                &mut self.rom,
                &self.loaded.special_music_statics,
                &self.loaded.statics,
                &data.statics,
            )?;
        }
        statics::write(&mut self.rom, &self.statics, &self.loaded, data)?;
        trades::write(&mut self.rom, &self.loaded, data)?;
        if let Some(t) = data.catching_tutorial {
            intro::write_tutorial(&mut self.rom, t)?;
        }
        // After the catching tutorial, as the intro Pokémon is the tutorial's catcher.
        intro::write(&mut self.rom, data)?;
        self.rom.files.flush()
    }

    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        out.write_all(&self.rom.files.save()?)?;
        Ok(())
    }
}
