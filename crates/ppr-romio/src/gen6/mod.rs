//! Gen 6: X, Y, Omega Ruby and Alpha Sapphire, 3DS games whose data lies in GARC archives of the RomFS, in code
//! modules (`.cro`) and in the executable.
//!
//! The image is read from its file as needed, not loaded whole. The handler reads everything into [`GameData`] on
//! loading and writes back only what changed, through a [`rom::Rom`] that keeps every file it read and hands the
//! image only the files that differ from the source. Saving writes the game partition as a CXI, which is a byte copy
//! of the source's when nothing changed, or a LayeredFS folder with only the changed files. A changed game's title
//! says it was randomized, and with which seed.
//!
//! A game update (a decrypted CIA) is laid over the game by the shared 3DS layer: the executable and every file the
//! update has are read from it, the game data is read again, and the changes are saved as a LayeredFS folder only.
//!
//! Deliberate choices:
//! - Alternate formes with data of their own are species of the game data from 722, in the order of their files;
//!   their stats, held items, level-up moves and compatibility are read and written like a species'. Trainer,
//!   wild and static Pokémon of such a forme have its number. Mega Evolutions are formes marked as such, with
//!   their Mega Stone; the Mega Evolution table is not written.
//! - Only what changed is written: species, moves, learnsets, encounter sets, trainers, trades, prices, and the
//!   names of moves and trainers, so an unchanged game saves byte for byte.
//! - The TM, HM and tutor bits past the moves the game data holds stay as they are.
//! - The female Meowstic forme is made female only and Meowstic male only when the species or evolutions are
//!   written, as Espurr's evolution then picks the forme.
//! - The baby table is rebuilt from the evolutions when they change for a restricted species pool.
//! - The title names the seed only when the game changed, so an unchanged game still saves byte for byte.
//! - An update of another version than the ROM entry describes is loaded with a warning in the log, not refused.
//! - The Luminous Moss counts among the items worth holding, as it is among the consumable ones.
//! - Shops and Pickup items are found by their first items; a game whose first ones changed has none.
//!
//! Not supported yet: Pokémon graphics and palettes.

mod amx;
mod constants;
mod encounters;
mod evolutions;
mod items;
mod moves;
mod rom;
mod species;
mod statics;
mod tms;
mod trades;
mod trainers;
mod tweaks;

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use ppr_model::{Capabilities, DexNumber, GameData, Generation, ItemId, MiscTweak, RomIdentity, Trainer};

use crate::RomError;
use crate::entries::{RomEntry, catalog};
use crate::handler::GameHandler;
use constants::{self as c, Family};
#[cfg(test)]
use rom::Bytes;
pub(crate) use rom::{BoxedSource, Source};
use rom::{BytesMut, Rom, file_path, need};

const MAX_TRAINER_NAME_LENGTH: usize = 10;
const MAX_TRAINER_CLASS_NAME_LENGTH: usize = 15;
const PUBLISHER: &str = "Potato Pokémon Randomizer";

/// The files of the ROM entry.
#[derive(Debug, Clone)]
struct Paths {
    text: String,
    story: String,
    wild: String,
    scripts: String,
    trainer_data: String,
    trainer_pokemon: String,
    dex_area: String,
    moves: String,
    egg_moves: String,
    movesets: String,
    evolutions: String,
    stats: String,
    baby: String,
    items: String,
    battle: String,
    field: String,
    intro: String,
    starter_display: String,
    evolution_module: String,
    megas: String,
}

impl Paths {
    fn read(entry: &RomEntry) -> Result<Self, RomError> {
        let f = |key: &str| file_path(entry, key);
        Ok(Self {
            text: f("TextStrings")?,
            story: f("StoryText")?,
            wild: f("WildPokemon")?,
            scripts: f("Scripts")?,
            trainer_data: f("TrainerData")?,
            trainer_pokemon: f("TrainerPokemon")?,
            dex_area: f("PokedexAreaData")?,
            moves: f("MoveData")?,
            egg_moves: f("EggMoves")?,
            movesets: f("PokemonMovesets")?,
            evolutions: f("PokemonEvolutions")?,
            stats: f("PokemonStats")?,
            baby: f("BabyPokemon")?,
            items: f("ItemData")?,
            battle: f("Battle")?,
            field: f("StaticPokemon")?,
            intro: f("Intro")?,
            starter_display: f("StarterDisplay")?,
            evolution_module: f("Evolution")?,
            megas: f("MegaEvolutions")?,
        })
    }
}

/// A loaded Gen 6 game.
#[derive(Debug)]
pub(crate) struct Gen6Handler {
    /// Behind a lock as saving reads from the source. `None` only after loading an update failed half-way.
    rom: Mutex<Option<Rom>>,
    /// What the title screen of the loaded update shows, e.g. `Ver. 1.5`.
    update_version: Option<String>,
    entry: RomEntry,
    family: Family,
    paths: Paths,
    loaded: GameData,
    formes: species::FormeTable,
    evolution_files: evolutions::EvolutionFiles,
    statics: statics::Statics,
    starter_slots: Vec<statics::Slot>,
    encounter_places: encounters::Places,
    trainer_extras: trainers::Extras,
    field_item_ids: Vec<ItemId>,
    shop_table: Option<items::ShopTable>,
    pickup_table: Option<usize>,
    trade_table: Option<trades::Table>,
}

fn numbers(entry: &RomEntry, key: &str) -> Vec<u16> {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| u16::try_from(v).ok())
        .collect()
}

fn lock(rom: &Mutex<Option<Rom>>) -> MutexGuard<'_, Option<Rom>> {
    rom.lock().unwrap_or_else(PoisonError::into_inner)
}

fn unloaded() -> RomError {
    RomError::Malformed("the game is no longer loaded, as loading an update failed".into())
}

impl Gen6Handler {
    /// Opens the game in `src`, read as needed.
    pub(crate) fn open(src: Box<dyn Source>, identity: RomIdentity) -> Result<Self, RomError> {
        let image = crate::ctr::open_game(BoxedSource(src), None)?;
        Self::load(Rom::new(image)?, identity)
    }

    /// Lays the update in `src` over the game and reads the game data again from the updated files.
    ///
    /// # Errors
    /// [`RomError::Unsupported`] for an encrypted update or one of another game, which leaves the game as it was;
    /// other errors when the updated game cannot be read, which leaves no game loaded.
    pub(crate) fn load_update(&mut self, src: Box<dyn Source>) -> Result<(), RomError> {
        let identity = self.loaded.identity.clone();
        let slot = self.rom.get_mut().unwrap_or_else(PoisonError::into_inner);
        let base = slot.as_ref().ok_or_else(unloaded)?.image().base();
        let update = crate::ctr::open_update(base, BoxedSource(src))?;
        crate::ctr::check_update_version(&update, &self.entry);
        let mut image = slot.take().ok_or_else(unloaded)?.into_image();
        image.set_update(Some(update));
        let mut reloaded = Self::load(Rom::new(image)?, identity)?;
        let text = reloaded.paths.text.clone();
        let rom = reloaded.rom.get_mut().unwrap_or_else(PoisonError::into_inner);
        let rom = rom.as_mut().ok_or_else(unloaded)?;
        reloaded.update_version = Some(crate::ctr::title_screen_version(&reloaded.entry, |file| {
            rom.texts(&text, file)
        })?);
        *self = reloaded;
        Ok(())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "a line for each field of GameData, which grows with every randomizer"
    )]
    fn load(mut rom: Rom, identity: RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(6).ok_or_else(|| RomError::Malformed("generation 6".into()))?;
        let entry = catalog(generation)
            .entries
            .iter()
            .find(|e| e.name == identity.name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {}", identity.name)))?;
        let family = Family::parse(&entry.family)
            .ok_or_else(|| RomError::Malformed(format!("unknown Gen 6 family {}", entry.family)))?;
        let paths = Paths::read(&entry)?;
        let text =
            |rom: &mut Rom, key: &str| -> Result<Vec<String>, RomError> { rom.texts(&paths.text, need(&entry, key)?) };

        let species_names = species::names(&text(&mut rom, "PokemonNamesTextOffset")?)?;
        let formes = species::read_formes(&mut rom, &paths.stats)?;
        let megas = species::read_megas(&mut rom, &paths.megas)?;
        let mut species_list = Vec::with_capacity(species::entry_count(family));
        let mut wild_held_items = BTreeMap::new();
        for n in 1..=species::entry_count(family) {
            let data = rom.garc_file(&paths.stats, false, n)?;
            let number = DexNumber::try_from(n).map_err(|_| RomError::Malformed("species number".into()))?;
            let name = species_names
                .get(usize::from(formes.base(number)))
                .ok_or_else(|| RomError::Malformed(format!("no name for species {n}")))?;
            let mut sp = species::read_species(number, name, &data)?;
            sp.forme = species::forme_of(number, &formes, family, &megas);
            species_list.push(sp);
            let held = species::read_held_items(&data)?;
            if !held.is_empty() {
                wild_held_items.insert(number, held);
            }
        }
        let (tm_hm_compat, tutor_compat) = tms::read_all_compat(&mut rom, &paths.stats, family)?;
        let (evolution_list, evolution_files) = evolutions::read(
            &mut rom,
            &paths.evolutions,
            &paths.stats,
            &paths.evolution_module,
            family,
            &formes.formes,
        )?;

        let move_names = text(&mut rom, "MoveNamesTextOffset")?;
        let move_list = moves::read(&mut rom, &paths.moves, family, &move_names)?;
        let learnsets = moves::read_learnsets(&mut rom, &paths.movesets, family)?;
        let egg_moves = moves::read_egg_moves(&mut rom, &paths.egg_moves)?;

        let tables = statics::Tables::new(&entry, family)?;
        let field = rom.file(&paths.field)?.to_vec();
        let (starters, starter_held_items, starter_slots) =
            statics::read_starters(&field, &tables, &formes, &entry, family)?;
        let (static_encounters, statics) = statics::read(&field, rom.code(), &tables, &formes, &entry, family)?;

        let map_names = text(&mut rom, "MapNamesTextOffset")?;
        let encounter_files = encounters::Files {
            wild: &paths.wild,
            field: Some(&paths.field),
            dex_area: &paths.dex_area,
            family,
            formes: &formes,
        };
        let (encounter_sets, encounter_places) = encounters::read(
            &mut rom,
            &encounter_files,
            need(&entry, "MapTableFileOffset")?,
            &map_names,
        )?;

        let trainer_names = text(&mut rom, "TrainerNamesTextOffset")?;
        let trainer_files = trainers::Files {
            data: &paths.trainer_data,
            pokemon: &paths.trainer_pokemon,
            family,
            formes: &formes,
        };
        let (trainer_list, trainer_extras) =
            trainers::read(&mut rom, &trainer_files, trainer_names.get(1..).unwrap_or_default())?;

        let item_names = text(&mut rom, "ItemNamesTextOffset")?;
        let item_list = items::read(&item_names, family);
        let field_files = items::FieldFiles::new(&paths.scripts, &entry, family)?;
        let field_item_ids = items::read_field_ids(&mut rom, &field_files)?;
        let shop_table = items::ShopTable::find(rom.code(), &entry, family)?;
        let pickup_table = items::pickup_table(rom.code());
        let trade_table = trades::Table::find(rom.code(), &entry, family)?;
        let trade_list = match trade_table {
            Some(table) => trades::read(&mut rom, &paths.text, table)?,
            None => Vec::new(),
        };

        let mut ability_names = text(&mut rom, "AbilityNamesTextOffset")?;
        ability_names.truncate(family.highest_ability() + 1);

        let mut loaded = GameData {
            formes: ppr_model::FormeRules {
                trainers: true,
                wild: true,
                statics: true,
                starters: true,
                evolutions: false,
                banned_for_trainers: Vec::new(),
                trainer_base_forme_abilities: false,
                force_swap_static_megas: family == Family::Xy,
            },
            type_chart: tweaks::read_type_chart(&mut rom, &paths.battle, need(&entry, "TypeEffectivenessOffset")?)?,
            types: c::TYPES.to_vec(),
            species: species_list,
            evolutions: evolution_list,
            evolution_level_samples: Vec::new(),
            restricted_species: None,
            starters,
            starter_held_items,
            starter_type_triangle: None,
            wild_held_items,
            // Gen 6 wild Pokémon may always hold an item, and has no dark grass.
            guaranteed_wild_held_items: true,
            dark_grass_held_items: false,
            palettes: BTreeMap::new(),
            palette_files: None,
            player_graphics_packs: Vec::new(),
            player_graphics: None,
            encounters: encounter_sets,
            guaranteed_catching: false,
            statics: static_encounters,
            main_game_legendaries: numbers(&entry, "MainGameLegendaries"),
            special_music_statics: Vec::new(),
            correct_static_music: false,
            totems: Vec::new(),
            easier_happiness_evolutions: false,
            trades: trade_list,
            max_trade_nickname_length: trades::MAX_NICKNAME_LENGTH,
            max_trade_ot_name_length: trades::MAX_OT_NAME_LENGTH,
            wild_banned: Vec::new(),
            ability_names,
            ability_variations: c::ABILITY_VARIATIONS.iter().map(|&(a, v)| (a, v.to_vec())).collect(),
            useless_abilities: c::USELESS_ABILITIES.to_vec(),
            moves: move_list,
            max_move_name_length: moves::MAX_NAME_LENGTH,
            learnsets,
            egg_moves,
            hm_moves: tms::read_hms(rom.code(), family)?,
            game_breaking_moves: c::GAME_BREAKING_MOVES.to_vec(),
            banned_level_up_moves: vec![c::HYPERSPACE_FURY],
            four_starting_moves: true,
            tm_moves: tms::read_tms(rom.code(), family)?,
            tutor_moves: tms::read_tutors(rom.code(), family)?,
            tm_hm_compat,
            tutor_compat,
            early_required_hm_moves: Vec::new(),
            field_moves: match family {
                Family::Xy => c::FIELD_MOVES_XY.to_vec(),
                Family::Oras => c::FIELD_MOVES_ORAS.to_vec(),
            },
            field_items: items::allowed_field_items(&field_item_ids, &item_list),
            required_field_tms: match family {
                Family::Xy => c::REQUIRED_FIELD_TMS_XY.to_vec(),
                Family::Oras => c::REQUIRED_FIELD_TMS_ORAS.to_vec(),
            },
            shops: items::read_shops(rom.code(), shop_table.as_ref(), &entry, family)?,
            item_prices: items::read_prices(&mut rom, &paths.items)?,
            balanced_item_prices: c::BALANCED_PRICES.into_iter().collect(),
            op_shop_items: c::OP_SHOP_ITEMS.to_vec(),
            mega_stones: crate::formes::mega_stones(),
            evolution_items: c::EVOLUTION_ITEMS.to_vec(),
            pickup_items: items::read_pickup(rom.code(), pickup_table)?,
            // Gen 6 TMs are not used up.
            pickup_tms: false,
            items: item_list,
            trainers: trainer_list,
            trainer_class_names: text(&mut rom, "TrainerClassesTextOffset")?,
            elite_four_trainers: numbers(&entry, "EliteFourIndices"),
            gym_and_elite_themes: c::themes(family).iter().map(|&(tag, t)| (tag.to_owned(), t)).collect(),
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
            intro_candidates: match family {
                // Any species or alternate forme, Mega Evolutions included: the intro loads a forme's model.
                Family::Xy => (1..=DexNumber::try_from(species::entry_count(family)).unwrap_or_default()).collect(),
                Family::Oras => Vec::new(),
            },
            intro_pokemon: None,
            held_item_pools: c::held_item_pools(),
            trainer_pokemon_additions: [true; 3],
            trainer_custom_movesets: [true; 3],
            trainer_held_item_additions: [true; 3],
            available_tweaks: tweaks::available(family),
            applied_tweaks: BTreeSet::new(),
            pc_potion: None,
            catching_tutorial_opponents: Vec::new(),
            catching_tutorial_players: Vec::new(),
            catching_tutorial: None,
            fossils: Vec::new(),
            identity,
        };
        loaded.estimate_evolution_levels();
        Ok(Self {
            rom: Mutex::new(Some(rom)),
            update_version: None,
            entry,
            family,
            paths,
            loaded,
            formes,
            evolution_files,
            statics,
            starter_slots,
            encounter_places,
            trainer_extras,
            field_item_ids,
            shop_table,
            pickup_table,
            trade_table,
        })
    }

    fn text_index(&self, key: &str) -> Result<usize, RomError> {
        need(&self.entry, key)
    }

    fn write_starters(&self, rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
        let tables = statics::Tables::new(&self.entry, self.family)?;
        if data.starters != self.loaded.starters {
            statics::write_starters(
                rom,
                &self.paths.field,
                &self.paths.starter_display,
                &self.paths.text,
                &self.entry,
                &tables,
                &self.formes,
                &self.starter_slots,
                &data.starters,
            )?;
        }
        if data.starter_held_items != self.loaded.starter_held_items {
            statics::write_starter_items(
                rom,
                &self.paths.field,
                &tables,
                &self.starter_slots,
                &data.starter_held_items,
            )?;
        }
        if let Some(intro) = data.intro_pokemon
            && self.loaded.intro_pokemon != Some(intro)
            && self.family == Family::Xy
        {
            let (base, forme) = data
                .species(intro)
                .map_or((intro, 0), |sp| (sp.base_number(), sp.forme.number));
            tweaks::write_intro(rom, &self.paths.intro, base, forme)?;
        }
        Ok(())
    }

    fn write_encounters_and_statics(&self, rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
        if data.encounters != self.loaded.encounters {
            let files = encounters::Files {
                wild: &self.paths.wild,
                field: Some(&self.paths.field),
                dex_area: &self.paths.dex_area,
                family: self.family,
                formes: &self.formes,
            };
            encounters::write(
                rom,
                &files,
                &self.encounter_places,
                &self.loaded.encounters,
                &data.encounters,
            )?;
        }
        if data.statics != self.loaded.statics {
            let files = statics::Files {
                field: &self.paths.field,
                wild: &self.paths.wild,
                scripts: &self.paths.scripts,
                entry: &self.entry,
                family: self.family,
                tables: statics::Tables::new(&self.entry, self.family)?,
                formes: &self.formes,
            };
            statics::write(rom, &files, &self.statics, &data.statics)?;
        }
        if data.trades != self.loaded.trades
            && let Some(table) = self.trade_table
        {
            trades::write(
                rom,
                &self.paths.text,
                &self.paths.story,
                table,
                self.family,
                &data.species,
                &self.loaded.trades,
                &data.trades,
            )?;
        }
        Ok(())
    }

    /// Writes what lives in the personal data; true when any of it may have changed.
    fn write_species(&self, rom: &mut Rom, data: &GameData) -> Result<bool, RomError> {
        let old = &self.loaded;
        let path = &self.paths.stats;
        let mut changed = false;
        for (sp, before) in data.species.iter().zip(&old.species) {
            let n = sp.number;
            let held = data.wild_held_items.get(&n).copied().unwrap_or_default();
            let held_before = old.wild_held_items.get(&n).copied().unwrap_or_default();
            let tm = data.tm_hm_compat.get(&n);
            let tutor = data.tutor_compat.get(&n);
            let same_compat = tm == old.tm_hm_compat.get(&n) && tutor == old.tutor_compat.get(&n);
            if sp == before && held == held_before && same_compat {
                continue;
            }
            changed = true;
            let mut file = rom.garc_file(path, false, usize::from(n))?;
            if sp != before {
                species::write_species(sp, &mut file)?;
            }
            if held != held_before {
                species::write_held_items(&held, &mut file)?;
            }
            if let Some(flags) = tm {
                tms::write_tm_compat(&mut file, flags)?;
            }
            if let Some(flags) = tutor
                && self.family == Family::Oras
            {
                tms::write_tutor_compat(&mut file, flags)?;
            }
            rom.set_garc_file(path, false, usize::from(n), file)?;
        }
        if data.species != old.species && data.species.iter().zip(&old.species).any(|(a, b)| a.name != b.name) {
            let file = self.text_index("PokemonNamesTextOffset")?;
            let mut names = rom.texts(&self.paths.text, file)?;
            for sp in data.base_species() {
                if let Some(line) = names.get_mut(usize::from(sp.number)) {
                    line.clone_from(&sp.name);
                }
            }
            rom.set_texts(&self.paths.text, file, &names)?;
        }
        moves::write_learnsets(rom, &self.paths.movesets, self.family, &old.learnsets, &data.learnsets)?;
        moves::write_egg_moves(rom, &self.paths.egg_moves, &old.egg_moves, &data.egg_moves)?;
        if data.evolutions != old.evolutions {
            evolutions::write(
                rom,
                &self.paths.evolutions,
                &self.paths.evolution_module,
                self.family,
                &self.evolution_files,
                &data.evolutions,
            )?;
            if data.restricted_species.is_some() {
                evolutions::write_baby_table(rom, &self.paths.baby, &data.evolutions)?;
            }
        }
        if changed || data.evolutions != old.evolutions {
            // Espurr's evolution picks the Meowstic forme by gender, so each forme has one.
            let mut male = rom.garc_file(path, false, usize::from(c::MEOWSTIC))?;
            male.set_u8(c::BS_GENDER, 0)?;
            rom.set_garc_file(path, false, usize::from(c::MEOWSTIC), male)?;
            let mut female = rom.garc_file(path, false, usize::from(c::MEOWSTIC_F))?;
            female.set_u8(c::BS_GENDER, species::MEOWSTIC_F_GENDER)?;
            rom.set_garc_file(path, false, usize::from(c::MEOWSTIC_F), female)?;
            changed = true;
        }
        Ok(changed)
    }

    fn write_moves(&self, rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded;
        if data.moves != old.moves {
            moves::write(
                rom,
                &self.paths.moves,
                &self.paths.text,
                self.text_index("MoveNamesTextOffset")?,
                self.family,
                &old.moves,
                &data.moves,
            )?;
        }
        if data.tm_moves != old.tm_moves {
            let texts = tms::TmTexts {
                path: &self.paths.text,
                item_descriptions: self.text_index("ItemDescriptionsTextOffset")?,
                move_descriptions: self.text_index("MoveDescriptionsTextOffset")?,
            };
            tms::write_tms(rom, self.family, &data.tm_moves, &data.moves, texts)?;
        }
        if data.tutor_moves != old.tutor_moves && self.family == Family::Oras {
            tms::write_tutors(rom, &data.tutor_moves)?;
        }
        Ok(())
    }

    fn write_trainers(&self, rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded;
        if data.trainers != old.trainers {
            let files = trainers::Files {
                data: &self.paths.trainer_data,
                pokemon: &self.paths.trainer_pokemon,
                family: self.family,
                formes: &self.formes,
            };
            trainers::write(
                rom,
                &files,
                &self.trainer_extras,
                &old.trainers,
                &data.trainers,
                &data.learnsets,
            )?;
        }
        let names = |t: &[Trainer]| t.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        if names(&data.trainers) != names(&old.trainers) {
            let file = self.text_index("TrainerNamesTextOffset")?;
            let mut lines = rom.texts(&self.paths.text, file)?;
            lines.truncate(1);
            lines.extend(names(&data.trainers));
            self.write_all_languages(rom, file, &lines)?;
        }
        if data.trainer_class_names != old.trainer_class_names {
            let file = self.text_index("TrainerClassesTextOffset")?;
            self.write_all_languages(rom, file, &data.trainer_class_names)?;
        }
        Ok(())
    }

    /// Writes a text file of every language.
    fn write_all_languages(&self, rom: &mut Rom, file: usize, lines: &[String]) -> Result<(), RomError> {
        rom.set_texts(&self.paths.text, file, lines)?;
        for language in trainers::OTHER_LANGUAGES {
            let path = file_path(&self.entry, &format!("TextStrings{language}"))?;
            rom.set_texts(&path, file, lines)?;
        }
        Ok(())
    }

    fn write_items(&self, rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
        let old = &self.loaded;
        if data.field_items != old.field_items {
            let files = items::FieldFiles::new(&self.paths.scripts, &self.entry, self.family)?;
            items::write_field_items(rom, &files, &self.field_item_ids, &old.items, &data.field_items)?;
        }
        if data.shops != old.shops
            && let Some(table) = &self.shop_table
        {
            items::write_shops(rom, table, &data.shops)?;
        }
        items::write_prices(rom, &self.paths.items, &old.item_prices, &data.item_prices)?;
        if data.pickup_items != old.pickup_items {
            items::write_pickup(rom, self.pickup_table, &data.pickup_items)?;
        }
        Ok(())
    }

    /// Applies the new tweaks; true when one changed the personal data.
    fn write_tweaks(&self, rom: &mut Rom, data: &GameData) -> Result<bool, RomError> {
        let files = tweaks::Files {
            family: self.family,
            stats: &self.paths.stats,
            battle: &self.paths.battle,
        };
        let mut personal = false;
        let new: Vec<MiscTweak> = data
            .applied_tweaks
            .difference(&self.loaded.applied_tweaks)
            .copied()
            .collect();
        for tweak in new {
            personal |= matches!(tweak, MiscTweak::FastEggHatching | MiscTweak::NoEvYields);
            tweaks::apply(rom, files, tweak)?;
        }
        if data.guaranteed_catching && !self.loaded.guaranteed_catching {
            tweaks::guaranteed_catching(rom, &self.paths.battle)?;
        }
        if data.easier_happiness_evolutions && !self.loaded.easier_happiness_evolutions {
            tweaks::ease_happiness(rom)?;
        }
        Ok(personal)
    }

    /// The title of a randomized game says so, and with which seed when it is known.
    fn mark_title(entry: &RomEntry, rom: &mut Rom, seed: Option<u64>) {
        let acronym = entry.value("Acronym").unwrap_or(&entry.name).to_owned();
        let description = match seed {
            Some(seed) => format!("{acronym} randomizer seed: {seed}"),
            None => format!("{acronym} randomizer"),
        };
        if let Some(smdh) = rom.image_mut().smdh_mut() {
            smdh.set_all_descriptions(&description);
            smdh.set_all_publishers(PUBLISHER);
        }
    }
}

impl GameHandler for Gen6Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            intro_pokemon: self.family == Family::Xy,
            starter_held_items: true,
            move_tutors: self.family == Family::Oras,
            map_indices: self.family == Family::Oras,
            encounter_locations: true,
            shop_sizes: false,
            time_based_encounters: false,
            mega_evolutions: true,
            physical_special_split: true,
            wild_alt_formes: true,
            static_alt_formes: true,
            starter_alt_formes: true,
            functional_formes: true,
            totem_pokemon: false,
            english: true,
        }
    }

    /// Shops keep their sizes; the falling and rustling spots of X and Y keep one level per slot.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        rules.shops.size = ppr_model::ShopSize::Fixed;
        rules.set_wild_levels(encounters::levels(&self.encounter_places).into_iter().enumerate());
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        let mut rom = lock(&self.rom);
        self.apply_to(rom.as_mut().ok_or_else(unloaded)?, data)
    }

    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        let mut rom = lock(&self.rom);
        let image = rom.as_mut().ok_or_else(unloaded)?.image_mut();
        if image.update().is_some() {
            return Err(RomError::Unsupported(
                "with a game update loaded, the game can only be saved as a LayeredFS folder".into(),
            ));
        }
        image.write_cxi(out)?;
        Ok(())
    }

    fn set_seed(&mut self, seed: u64) {
        let slot = self.rom.get_mut().unwrap_or_else(PoisonError::into_inner);
        if let Some(rom) = slot.as_mut()
            && rom.image().is_modified()
        {
            Self::mark_title(&self.entry, rom, Some(seed));
        }
    }

    fn save_layered_fs(&self, dir: &Path) -> Result<PathBuf, RomError> {
        let mut rom = lock(&self.rom);
        Ok(rom.as_mut().ok_or_else(unloaded)?.image_mut().write_layered_fs(dir)?)
    }

    fn load_game_update(&mut self, path: &Path) -> Result<(), RomError> {
        let file = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(path)?);
        self.load_update(Box::new(file))
    }

    fn game_update_version(&self) -> Option<String> {
        self.update_version.clone()
    }
}

impl Gen6Handler {
    fn apply_to(&self, rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
        self.write_starters(rom, data)?;
        self.write_encounters_and_statics(rom, data)?;
        if data.type_chart != self.loaded.type_chart {
            tweaks::write_type_chart(
                rom,
                &self.paths.battle,
                need(&self.entry, "TypeEffectivenessOffset")?,
                &data.type_chart,
            )?;
        }
        let mut personal = self.write_species(rom, data)?;
        self.write_moves(rom, data)?;
        self.write_trainers(rom, data)?;
        self.write_items(rom, data)?;
        personal |= self.write_tweaks(rom, data)?;
        if personal {
            species::update_aggregate(rom, &self.paths.stats, self.family)?;
        }
        rom.flush()?;
        if rom.image().is_modified() {
            Self::mark_title(&self.entry, rom, None);
        }
        Ok(())
    }
}
