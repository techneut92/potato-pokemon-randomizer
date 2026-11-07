//! Gen 5: Black, White, Black 2 and White 2, NDS ROMs whose data lies in NARC archives, overlays and the ARM9.
//!
//! The handler reads everything into [`GameData`] on loading and writes back only what changed, through a
//! [`store::Store`] that keeps every part it read and saves only the parts that differ from the ROM.
//!
//! Alternate formes with data of their own are species from 650 (see [`species`]); trainer, wild and static Pokémon
//! use them as their species, with their forme number, and are written as their base species and forme.
//!
//! Deliberate choices:
//! - The ARM9 is extended (as the patches and repointed shops need) right before it first changes, not on loading,
//!   so a ROM whose data doesn't change saves unchanged.
//! - The Shedinja patch is applied when the evolutions change, and the roamer patch of Black and White when the
//!   statics change.

mod constants;
mod encounters;
mod entry;
mod items;
mod moves;
mod species;
mod starters;
mod statics;
mod store;
mod tms;
mod trainers;
mod tweaks;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use ppr_formats::nds::NdsRom;
use ppr_model::{
    Capabilities, DexNumber, FormeRules, GameData, Generation, HeldItemPools, MiscTweak, RomIdentity, Trainer, Type,
};

use crate::RomError;
use crate::entries::{RomEntry, catalog};
use crate::handler::GameHandler;
use constants as c;
use entry::{Family, file_path, int_or_zero, need, optional_file_path};
use store::{Store, Text};

const NINCADA: DexNumber = 290;
const MAX_TRAINER_NAME_LENGTH: usize = 10;
const MAX_TRAINER_CLASS_NAME_LENGTH: usize = 12;

/// The files of the ROM entry.
#[derive(Debug, Clone)]
pub(super) struct Paths {
    pub(super) stats: String,
    pub(super) movesets: String,
    pub(super) evolutions: String,
    pub(super) moves: String,
    pub(super) items: String,
    pub(super) scripts: String,
    pub(super) egg_moves: String,
    pub(super) graphics: String,
    pub(super) trades: String,
    pub(super) map_table: String,
    pub(super) wild: String,
    pub(super) area_data: String,
    pub(super) trainer_data: String,
    pub(super) trainer_pokemon: String,
    pub(super) trainer_text_boxes: Option<String>,
    pub(super) driftveil: Option<String>,
}

impl Paths {
    fn read(entry: &RomEntry, family: Family) -> Result<Self, RomError> {
        Ok(Self {
            stats: file_path(entry, "PokemonStats")?,
            movesets: file_path(entry, "PokemonMovesets")?,
            evolutions: file_path(entry, "PokemonEvolutions")?,
            moves: file_path(entry, "MoveData")?,
            items: file_path(entry, "ItemData")?,
            scripts: file_path(entry, "Scripts")?,
            egg_moves: file_path(entry, "EggMoves")?,
            graphics: file_path(entry, "PokemonGraphics")?,
            trades: file_path(entry, "InGameTrades")?,
            map_table: file_path(entry, "MapTableFile")?,
            wild: file_path(entry, "WildPokemon")?,
            area_data: file_path(entry, "PokedexAreaData")?,
            trainer_data: file_path(entry, "TrainerData")?,
            trainer_pokemon: file_path(entry, "TrainerPokemon")?,
            trainer_text_boxes: optional_file_path(entry, "TrainerTextBoxes"),
            driftveil: match family {
                Family::Bw1 => None,
                Family::Bw2 => optional_file_path(entry, "DriftveilPokemon"),
            },
        })
    }
}

/// A loaded Gen 5 ROM.
#[derive(Debug)]
pub(crate) struct Gen5Handler {
    store: Store,
    entry: RomEntry,
    family: Family,
    paths: Paths,
    loaded: GameData,
    trainers: trainers::Trainers,
    encounter_formes: encounters::Formes,
    statics: statics::Statics,
    field_item_places: Vec<(usize, usize)>,
    pickup_table: Option<usize>,
    /// The species of the special music table, alternate formes by their own numbers.
    music_statics: Vec<u16>,
    /// Whether double battles against a single trainer were made possible.
    doubles_allowed: bool,
}

fn numbers(entry: &RomEntry, key: &str) -> Vec<u16> {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| u16::try_from(v).ok())
        .collect()
}

/// The base species of a species number, alternate formes (from 650) included.
fn base_species(n: u16) -> DexNumber {
    c::FORME_NUMBERS
        .iter()
        .find(|&&(_, _, number)| number == n)
        .map_or(n, |&(base, _, _)| base)
}

/// The species number of forme `forme` of `base`: an alternate forme with data of its own counts from 650, other
/// formes (cosmetic ones, and forme 0) are `base`.
fn forme_species(family: Family, base: DexNumber, forme: u8) -> DexNumber {
    let last = species::SPECIES_COUNT + family.forme_count();
    c::FORME_NUMBERS
        .iter()
        .find(|&&(b, f, number)| b == base && f == forme && forme != 0 && number <= last)
        .map_or(base, |&(_, _, number)| number)
}

fn held_item_pools() -> HeldItemPools {
    HeldItemPools {
        all: c::ALL_HELD_ITEMS.to_vec(),
        consumable: c::CONSUMABLE_HELD_ITEMS.to_vec(),
        general_consumable: c::GENERAL_PURPOSE_CONSUMABLE_ITEMS.to_vec(),
        general: c::GENERAL_PURPOSE_ITEMS.to_vec(),
        // Liechi Berry, Petaya Berry, Choice Band and Muscle Band.
        physical_consumable: vec![201],
        special_consumable: vec![204],
        physical: vec![220, 266],
        type_boosting: c::TYPE_BOOSTING_ITEMS
            .iter()
            .map(|&(t, items)| (t, items.to_vec()))
            .collect(),
        species_boosting: c::SPECIES_BOOSTING_ITEMS
            .iter()
            .map(|&(n, items)| (n, items.to_vec()))
            .collect(),
    }
}

impl Gen5Handler {
    #[expect(
        clippy::too_many_lines,
        reason = "a line for each field of GameData, which grows with every randomizer"
    )]
    pub(crate) fn load(bytes: Vec<u8>, identity: RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(5).ok_or_else(|| RomError::Malformed("generation 5".into()))?;
        let entry = catalog(generation)
            .entries
            .iter()
            .find(|e| e.name == identity.name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {}", identity.name)))?;
        let family = Family::parse(&entry)?;
        let paths = Paths::read(&entry, family)?;
        let rom = NdsRom::from_bytes(bytes)?;
        let mut store = Store::new(rom, file_path(&entry, "TextStrings")?, file_path(&entry, "TextStory")?);
        let patch = int_or_zero(&entry, "Arm9PatchExtensionSize");
        let repoint = int_or_zero(&entry, "Arm9RepointExtensionSize");
        if patch + repoint > 0 {
            store.plan_arm9_extension(patch, repoint, entry.value("TCMCopyingPrefix").unwrap_or_default());
        }

        let species_names = store
            .strings(Text::Strings, need(&entry, "PokemonNamesTextOffset")?)?
            .to_vec();
        let species = species::read(&mut store, &paths.stats, &species_names, family)?;
        let items = items::read(&mut store, &entry, family)?;
        let field_item_places = items::field_item_places(&mut store, &entry, &paths.scripts)?;
        let field_items = items::read_field_items(&mut store, &paths.scripts, &field_item_places, &items)?;
        let pickup_table = items::pickup_table(&mut store, &entry)?;
        let (encounter_sets, encounter_formes) = encounters::read(&mut store, &entry, family, &paths)?;
        let (static_encounters, statics) = statics::read(&mut store, &entry, family, &paths.scripts)?;
        let (trainer_list, trainers) = trainers::read(&mut store, &entry, family, &paths)?;
        let (tm_moves, hm_moves) = tms::read(&mut store)?;
        let types = Type::in_generation(generation);
        let music_statics = if entry.value("NewIndexToMusicTweak").is_some() {
            numbers(&entry, "SpecialMusicStatics")
        } else {
            Vec::new()
        };
        let mut special_music_statics: Vec<DexNumber> = music_statics.iter().map(|&n| base_species(n)).collect();
        special_music_statics.dedup();

        let mut loaded = GameData {
            formes: FormeRules {
                trainers: true,
                wild: true,
                statics: true,
                starters: false,
                evolutions: false,
                banned_for_trainers: Vec::new(),
                trainer_base_forme_abilities: true,
                force_swap_static_megas: false,
            },
            identity,
            type_chart: tweaks::read_type_chart(&mut store, &entry, &types)?,
            types,
            evolutions: species::read_evolutions(&mut store, &paths.evolutions)?,
            evolution_level_samples: Vec::new(),
            restricted_species: None,
            starters: starters::read(&mut store, &entry, &paths.scripts)?,
            starter_type_triangle: None,
            starter_held_items: Vec::new(),
            wild_held_items: species::read_held_items(&mut store, &paths.stats, family)?,
            // Gen 5 wild Pokémon may always hold an item, a third one in dark grass.
            guaranteed_wild_held_items: true,
            dark_grass_held_items: true,
            palettes: species::read_palettes(&mut store, &paths.graphics)?,
            palette_files: Some("BW".to_owned()),
            player_graphics_packs: Vec::new(),
            player_graphics: None,
            encounters: encounter_sets,
            guaranteed_catching: false,
            statics: static_encounters,
            main_game_legendaries: numbers(&entry, "MainGameLegendaries"),
            special_music_statics,
            correct_static_music: false,
            totems: Vec::new(),
            easier_happiness_evolutions: false,
            trades: items::read_trades(&mut store, &entry, &paths.trades)?,
            max_trade_nickname_length: items::MAX_NICKNAME_LENGTH,
            max_trade_ot_name_length: items::MAX_OT_NAME_LENGTH,
            wild_banned: Vec::new(),
            ability_names: store
                .strings(Text::Strings, need(&entry, "AbilityNamesTextOffset")?)?
                .to_vec(),
            ability_variations: c::ABILITY_VARIATIONS.iter().map(|&(a, v)| (a, v.to_vec())).collect(),
            useless_abilities: c::USELESS_ABILITIES.to_vec(),
            moves: moves::read(&mut store, &paths.moves, need(&entry, "MoveNamesTextOffset")?)?,
            max_move_name_length: moves::MAX_NAME_LENGTH,
            learnsets: species::read_learnsets(&mut store, &paths.movesets, family)?,
            egg_moves: species::read_egg_moves(&mut store, &paths.egg_moves)?,
            hm_moves,
            // Sonic Boom and Dragon Rage.
            game_breaking_moves: vec![49, 82],
            banned_level_up_moves: Vec::new(),
            four_starting_moves: true,
            tm_moves,
            tutor_moves: tms::read_tutors(&mut store, &entry, family)?,
            tm_hm_compat: species::read_tm_compat(&mut store, &paths.stats, family)?,
            tutor_compat: if family == Family::Bw2 {
                species::read_tutor_compat(&mut store, &paths.stats, family)?
            } else {
                BTreeMap::new()
            },
            // Cut in Black and White.
            early_required_hm_moves: family.pick(vec![15], Vec::new()),
            field_moves: c::FIELD_MOVES.to_vec(),
            field_items,
            required_field_tms: family
                .pick(&c::BW1_REQUIRED_FIELD_TMS[..], &c::BW2_REQUIRED_FIELD_TMS[..])
                .to_vec(),
            shops: items::read_shops(&mut store, &entry, family)?,
            item_prices: items::read_prices(&mut store, &paths.items)?,
            balanced_item_prices: c::BALANCED_ITEM_PRICES.into_iter().collect(),
            op_shop_items: c::OP_SHOP_ITEMS.to_vec(),
            mega_stones: Vec::new(),
            evolution_items: c::EVOLUTION_ITEMS.to_vec(),
            pickup_items: items::read_pickup(&mut store, &entry, pickup_table)?,
            // Gen 5 TMs are not used up.
            pickup_tms: false,
            items,
            species,
            trainers: trainer_list,
            trainer_class_names: trainers::class_names(&mut store, &entry, family)?,
            elite_four_trainers: numbers(&entry, "EliteFourIndices"),
            gym_and_elite_themes: family
                .pick(&c::BW1_THEMES[..], &c::BW2_THEMES[..])
                .iter()
                .map(|&(tag, t)| (tag.to_owned(), t))
                .collect(),
            original_types: BTreeMap::new(),
            max_trainer_name_length: MAX_TRAINER_NAME_LENGTH,
            trainer_name_shares_class_line: false,
            max_trainer_names_total: None,
            max_trainer_class_name_length: MAX_TRAINER_CLASS_NAME_LENGTH,
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
            intro_candidates: (1..=species::SPECIES_COUNT).collect(),
            intro_pokemon: None,
            held_item_pools: held_item_pools(),
            trainer_pokemon_additions: [true; 3],
            trainer_custom_movesets: [true; 3],
            trainer_held_item_additions: [true; 3],
            available_tweaks: tweaks::available(&entry, family),
            applied_tweaks: BTreeSet::new(),
            pc_potion: None,
            catching_tutorial_opponents: Vec::new(),
            catching_tutorial_players: Vec::new(),
            catching_tutorial: None,
            fossils: Vec::new(),
        };
        loaded.estimate_evolution_levels();
        store.forget_unchanged();
        Ok(Self {
            store,
            entry,
            family,
            paths,
            loaded,
            trainers,
            encounter_formes,
            statics,
            field_item_places,
            pickup_table,
            music_statics,
            doubles_allowed: false,
        })
    }

    fn write_species(&mut self, data: &GameData) -> Result<(), RomError> {
        let (store, paths, old) = (&mut self.store, &self.paths, &self.loaded);
        species::write(
            store,
            &paths.stats,
            self.family,
            &old.species,
            &data.species,
            &old.wild_held_items,
            &data.wild_held_items,
        )?;
        if data.species.iter().zip(&old.species).any(|(a, b)| a.name != b.name) {
            let names = store.strings_mut(Text::Strings, need(&self.entry, "PokemonNamesTextOffset")?)?;
            for s in data.base_species() {
                if let Some(line) = names.get_mut(usize::from(s.number)) {
                    line.clone_from(&s.name);
                }
            }
        }
        species::write_palettes(store, &paths.graphics, &old.palettes, &data.palettes)?;
        species::write_tm_compat(store, &paths.stats, self.family, &old.tm_hm_compat, &data.tm_hm_compat)?;
        if self.family == Family::Bw2 {
            species::write_tutor_compat(store, &paths.stats, self.family, &old.tutor_compat, &data.tutor_compat)?;
        }
        species::write_learnsets(store, &paths.movesets, self.family, &old.learnsets, &data.learnsets)?;
        species::write_egg_moves(store, &paths.egg_moves, &old.egg_moves, &data.egg_moves)?;
        if data.evolutions != old.evolutions {
            species::write_evolutions(store, &paths.evolutions, &old.evolutions, &data.evolutions)?;
            self.write_shedinja(data)?;
        }
        Ok(())
    }

    /// Nincada's second evolution comes with the first, as Shedinja; the patched game reads its species from a
    /// constant.
    fn write_shedinja(&mut self, data: &GameData) -> Result<(), RomError> {
        let (Some(arm9_patch), Some(overlay_patch)) = (
            self.entry.value("ShedinjaEvolutionTweak"),
            self.entry.value("ShedinjaEvolutionOvlTweak"),
        ) else {
            return Ok(());
        };
        let from_nincada: Vec<_> = data.evolutions.iter().filter(|e| e.from == NINCADA).collect();
        let Some(second) = from_nincada.get(1) else {
            return Ok(());
        };
        let evolution = need(&self.entry, "EvolutionOvlNumber")?;
        crate::ips::named(overlay_patch)?.apply(self.store.overlay_mut(evolution)?)?;
        crate::ips::named(arm9_patch)?.apply(self.store.arm9_mut()?)?;
        let at = int_or_zero(&self.entry, "ShedinjaSpeciesOffset");
        if at > 0 {
            store::put_long(self.store.arm9_mut()?, at, u32::from(second.to))?;
        }
        Ok(())
    }

    fn write_statics(&mut self, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded.statics;
        if data.correct_static_music && !self.loaded.correct_static_music && !self.music_statics.is_empty() {
            // The music fix knows alternate formes by their own numbers, as the statics' species are.
            let mut changes: BTreeMap<u16, u16> = BTreeMap::new();
            for (o, n) in old.iter().zip(&data.statics) {
                if self.music_statics.contains(&o.species) && o.species != n.species {
                    changes.insert(o.species, n.species);
                }
            }
            if !changes.is_empty() {
                let changes: Vec<(u16, u16)> = changes.into_iter().collect();
                statics::correct_music(&mut self.store, &self.entry, self.family, &changes, &self.music_statics)?;
            }
        }
        if data.statics != *old {
            statics::write(
                &mut self.store,
                &self.entry,
                self.family,
                &self.paths.scripts,
                &self.paths.stats,
                &self.statics,
                old,
                &data.statics,
            )?;
        }
        Ok(())
    }

    fn write_trainers(&mut self, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded;
        if data.trainers != old.trainers {
            trainers::write(
                &mut self.store,
                &self.entry,
                self.family,
                &self.paths,
                &self.trainers,
                &old.trainers,
                &data.trainers,
                |p| data.moves_at_level(p.species, p.level),
            )?;
            let forced = |t: &[Trainer]| t.iter().any(|t| t.forced_battle_style);
            if forced(&data.trainers) && !forced(&old.trainers) && !self.doubles_allowed {
                trainers::allow_double_battles(&mut self.store, &self.entry, &self.paths, &self.trainers)?;
                self.doubles_allowed = true;
            }
        }
        if data.trainer_class_names != old.trainer_class_names {
            trainers::write_class_names(&mut self.store, &self.entry, self.family, &data.trainer_class_names)?;
        }
        Ok(())
    }

    fn write_items(&mut self, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded;
        let store = &mut self.store;
        items::write_field_items(
            store,
            &self.paths.scripts,
            &self.field_item_places,
            &old.items,
            &old.field_items,
            &data.field_items,
        )?;
        items::write_shops(store, &self.entry, self.family, &old.shops, &data.shops)?;
        items::write_prices(store, &self.paths.items, &old.item_prices, &data.item_prices)?;
        items::write_pickup(
            store,
            &self.entry,
            self.pickup_table,
            &old.pickup_items,
            &data.pickup_items,
        )?;
        let names: Vec<String> = std::iter::once(String::new())
            .chain(data.base_species().map(|s| s.name.clone()))
            .collect();
        items::write_trades(
            store,
            &self.entry,
            &self.paths.trades,
            &self.paths.scripts,
            &names,
            &old.trades,
            &data.trades,
        )
    }

    fn write_moves(&mut self, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded;
        if data.moves != old.moves {
            moves::write(
                &mut self.store,
                &self.paths.moves,
                need(&self.entry, "MoveNamesTextOffset")?,
                need(&self.entry, "BattleMoveNamesTextOffset")?,
                &old.moves,
                &data.moves,
            )?;
        }
        if data.tm_moves != old.tm_moves {
            tms::write(&mut self.store, &self.entry, self.family, &data.moves, &data.tm_moves)?;
        }
        if data.tutor_moves != old.tutor_moves && self.family == Family::Bw2 {
            tms::write_tutors(&mut self.store, &self.entry, &data.tutor_moves)?;
        }
        Ok(())
    }

    fn write_tweaks(&mut self, data: &GameData) -> Result<(), RomError> {
        let files = tweaks::species_files(self.family);
        let new: Vec<MiscTweak> = data
            .applied_tweaks
            .difference(&self.loaded.applied_tweaks)
            .copied()
            .collect();
        for tweak in new {
            tweaks::apply(
                &mut self.store,
                &self.entry,
                &self.paths.scripts,
                &self.paths.stats,
                &files,
                tweak,
            )?;
        }
        if data.guaranteed_catching && !self.loaded.guaranteed_catching {
            tweaks::enable_guaranteed_catching(&mut self.store, &self.entry)?;
        }
        if data.easier_happiness_evolutions && !self.loaded.easier_happiness_evolutions {
            tweaks::ease_happiness(&mut self.store)?;
        }
        Ok(())
    }
}

impl GameHandler for Gen5Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            intro_pokemon: true,
            starter_held_items: false,
            move_tutors: self.family == Family::Bw2,
            map_indices: true,
            encounter_locations: true,
            shop_sizes: true,
            time_based_encounters: true,
            mega_evolutions: false,
            physical_special_split: true,
            wild_alt_formes: true,
            static_alt_formes: false,
            starter_alt_formes: false,
            functional_formes: true,
            totem_pokemon: false,
            english: self.entry.code.as_bytes().get(3) == Some(&b'O'),
        }
    }

    /// Black and White move a changed shop into the room the ARM9 was extended by; Black 2 and White 2 keep each
    /// shop in a file of its own. A shop holds at most 255 items. The Driftveil tournament keeps its team sizes and
    /// sets its levels.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        rules.shops.size = match self.family {
            Family::Bw1 => ppr_model::ShopSize::Shared {
                bytes: int_or_zero(&self.entry, "Arm9RepointExtensionSize"),
                costs: vec![ppr_model::ShopCost { base: 0, per_item: 2 }; self.loaded.shops.len()],
                only_changed: true,
                max: usize::from(u8::MAX),
            },
            Family::Bw2 => ppr_model::ShopSize::Grow {
                max: Some(usize::from(u8::MAX)),
            },
        };
        let tournament = self.loaded.trainers.iter().skip(self.trainers.regular());
        for t in tournament {
            if let Some(limits) = rules.trainers.trainers.iter_mut().find(|l| l.index == t.index) {
                limits.min_pokemon = t.pokemon.len();
                limits.max_pokemon = t.pokemon.len();
                limits.levels = false;
            }
        }
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        if data.starters != self.loaded.starters {
            starters::write(
                &mut self.store,
                &self.entry,
                self.family,
                &self.paths.scripts,
                &self.paths.graphics,
                &data.species,
                &data.starters,
            )?;
        }
        if let Some(intro) = data.intro_pokemon
            && self.loaded.intro_pokemon != Some(intro)
        {
            starters::write_intro(&mut self.store, &self.entry, self.family, intro)?;
        }
        if data.encounters != self.loaded.encounters {
            encounters::write(
                &mut self.store,
                &self.entry,
                self.family,
                &self.paths,
                &self.encounter_formes,
                &self.loaded.encounters,
                &data.encounters,
            )?;
        }
        self.write_statics(data)?;
        if data.type_chart != self.loaded.type_chart {
            tweaks::write_type_chart(&mut self.store, &self.entry, &data.type_chart, &data.types)?;
        }
        self.write_species(data)?;
        self.write_moves(data)?;
        self.write_trainers(data)?;
        self.write_items(data)?;
        self.write_tweaks(data)?;
        self.store.flush()
    }

    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        out.write_all(&self.store.save()?)?;
        Ok(())
    }
}
