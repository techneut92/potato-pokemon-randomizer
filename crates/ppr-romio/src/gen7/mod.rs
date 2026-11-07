//! Generation 7 (Sun, Moon, Ultra Sun, Ultra Moon): the decrypted game partition of a 3DS image, with an optional
//! game update laid over it ([`files`]).
//!
//! Ported: species with their stats, types, abilities and wild held items ([`personal`]), evolutions
//! ([`evolutions`]), moves, learnsets and egg moves ([`moves`]), starters with their held items and texts, gifts,
//! static encounters, the assembled Zygarde and in-game trades ([`statics`]), Totem Pokémon with their allies and
//! auras ([`totems`]), wild encounters and field items ([`encounters`]), trainers with their names and classes
//! ([`trainers`]), items, TMs with their descriptions and palettes, move tutors (Ultra Sun and Ultra Moon), shops,
//! prices, Pickup, the type chart and the code patches ([`misc`]). Writing produces a full game partition (a CXI) or
//! a LayeredFS folder.
//!
//! Deliberate choices:
//! - Alternate formes with personal entries of their own are species of the game data, numbered by their entry
//!   (from 803 in Sun and Moon, 808 in Ultra Sun and Ultra Moon); Mega Evolutions come from their table. Wild
//!   encounters, trainer Pokémon, starters, gifts, static encounters, Totem Pokémon and their allies of such a forme
//!   have its number as their species; a slot keeps its stored forme while its species and forme stay the same.
//!   Trades keep the species they are stored as, with their formes, and a new species gets forme 0. An evolution
//!   written back unchanged keeps its entry, so "keep the forme" entries stay as they are.
//! - Only what changed is written, so an unchanged game saves byte for byte: Nincada's Shedinja patch, Beast
//!   Lusamine's aura boosts and the Route 1 static encounter are written when what they follow changed.
//! - Nincada's extra evolution is read from the executable: the species it creates, while Nincada evolves into
//!   the species the executable checks for. A game randomized before reads back as it was written, and its
//!   Shedinja patch is not applied a second time.
//! - Wild encounters are read without time of day until [`GameHandler::use_time_of_day`] asks for it: then each
//!   table is a day set and a night set ([`encounters`]). An update loaded afterwards keeps the view.
//! - Trade trainer IDs are 32 bits; the game data holds the low 16, and a trade keeps its full ID while those
//!   stay. IVs of -1 (random) are 255.
//! - Egg moves keep each species' number of egg moves.
//! - Trainer Pokémon added by the randomizer copy the nature, EVs, IVs and gender of the team's first Pokémon.

mod amx;
mod constants;
mod encounters;
mod evolutions;
mod files;
mod misc;
mod moves;
mod personal;
mod statics;
#[cfg(test)]
mod tests;
mod totems;
mod trainers;

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use ppr_formats::ctr::Decompression;
use ppr_model::{Capabilities, DexNumber, GameData, Generation, ItemId, MiscTweak, PriceFormat, RomIdentity, Type};

use self::constants::{self as c, Family};
pub(crate) use self::files::Source;
use self::files::{Files, Image};
use crate::entries::{RomEntry, catalog};
use crate::{GameHandler, RomError};

/// The name the randomized game's title shows as its publisher.
const PUBLISHER: &str = "Potato Pokémon Randomizer";

pub(super) fn word(d: &[u8], at: usize) -> Result<u16, RomError> {
    d.get(at..at + 2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .ok_or_else(|| RomError::Malformed(format!("data cut off at {at:#x}")))
}

pub(super) fn put_u16(d: &mut [u8], at: usize, v: u16) {
    if let Some(b) = d.get_mut(at..at + 2) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

pub(super) fn put_u32(d: &mut [u8], at: usize, v: u32) {
    if let Some(b) = d.get_mut(at..at + 4) {
        b.copy_from_slice(&v.to_le_bytes());
    }
}

/// Where the only match of the hex string `pattern` in `data` is; a match at 0 or more than one counts as none.
pub(super) fn find_at(data: &[u8], pattern: &str) -> Result<Option<usize>, RomError> {
    let needle = crate::gba::hex(pattern)?;
    if needle.is_empty() || needle.len() > data.len() {
        return Ok(None);
    }
    let mut found = None;
    let mut i = 0;
    while i + needle.len() <= data.len() {
        if data[i..i + needle.len()] == needle[..] {
            if found.is_some() {
                return Ok(None);
            }
            found = Some(i);
            // Matches don't overlap.
            i += needle.len();
        } else {
            i += 1;
        }
    }
    Ok(found.filter(|&at| at > 0))
}

/// Where the data after the only match of the prefix `pattern` starts.
pub(super) fn find(data: &[u8], pattern: &str) -> Result<Option<usize>, RomError> {
    Ok(find_at(data, pattern)?.map(|at| at + pattern.len() / 2))
}

/// The paths and values of a game's ROM entry.
#[derive(Debug, Clone)]
struct Entry {
    rom: RomEntry,
    family: Family,
    sunny: bool,
}

impl Entry {
    fn find(identity: &RomIdentity) -> Result<Self, RomError> {
        let generation = Generation::new(7).ok_or_else(|| RomError::Malformed("generation 7".into()))?;
        let entry = catalog(generation)
            .entries
            .iter()
            .find(|e| e.name == identity.name)
            .cloned()
            .ok_or_else(|| RomError::Malformed(format!("no ROM entry named {}", identity.name)))?;
        Ok(Self {
            family: Family::parse(&entry.family),
            sunny: entry.int("IsSunny").is_some_and(|v| v != 0),
            rom: entry,
        })
    }

    /// The RomFS path of `File<key>`: `<a/0/1/1, [CRC, CRC]>`.
    fn path(&self, key: &str) -> Result<String, RomError> {
        let raw = self
            .rom
            .value(&format!("File<{key}>"))
            .ok_or_else(|| RomError::Malformed(format!("ROM entry has no file {key}")))?;
        let inner = raw.trim().trim_start_matches('<');
        Ok(inner.split(',').next().unwrap_or_default().trim().to_owned())
    }

    fn int(&self, key: &str) -> Result<usize, RomError> {
        self.rom
            .int(key)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| RomError::Malformed(format!("ROM entry has no {key}")))
    }

    fn array(&self, key: &str) -> Vec<usize> {
        self.rom
            .array(key)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| usize::try_from(v).ok())
            .collect()
    }
}

/// The loaded parts the game data leaves out.
#[derive(Debug)]
struct Layout {
    personal: personal::Personal,
    evolutions: evolutions::Evolutions,
    encounters: encounters::Encounters,
    teams: trainers::Teams,
    statics: statics::Statics,
    trades: statics::Trades,
    totems: totems::Totems,
    starter_formes: Vec<u8>,
}

/// Loaded Gen 7 game.
#[derive(Debug)]
pub(crate) struct Gen7Handler {
    files: RefCell<Files>,
    entry: Entry,
    loaded: GameData,
    layout: Layout,
    /// What the title screen of the loaded update shows, e.g. `Ver. 1.2`.
    update_version: Option<String>,
}

/// The texts of a text archive file.
fn texts(files: &mut Files, path: &str, index: usize) -> Result<Vec<String>, RomError> {
    let file = files.garc(path, &Decompression::None)?.file(index)?;
    crate::ctr_text::read_texts(&file, true)
}

fn set_texts(files: &mut Files, path: &str, index: usize, lines: &[String]) -> Result<(), RomError> {
    let garc = files.garc_mut(path, &Decompression::None)?;
    let old = garc.file(index)?;
    let new = crate::ctr_text::write_texts(Some(&old), lines, true)?;
    garc.set_file(index, new)?;
    Ok(())
}

impl Gen7Handler {
    /// Loads the game from its partition, with an update when one is given.
    pub(crate) fn load(base: Image, update: Option<Image>, identity: RomIdentity) -> Result<Self, RomError> {
        let entry = Entry::find(&identity)?;
        let mut files = Files::new(base, update)?;
        let (loaded, layout) = read(&mut files, &entry, identity, false)?;
        Ok(Self {
            files: RefCell::new(files),
            entry,
            loaded,
            layout,
            update_version: None,
        })
    }

    /// Opens a game from any readable image.
    pub(crate) fn open(src: Box<dyn Source>, identity: RomIdentity) -> Result<Self, RomError> {
        let base = crate::ctr::open(src)?;
        Self::load(base, None, identity)
    }

    /// Loads a decrypted update of this game and reads the game data again with the update's files.
    pub(crate) fn load_update(&mut self, src: Box<dyn Source>) -> Result<(), RomError> {
        let files = self.files.get_mut();
        let update = crate::ctr::open_update(files.game.base(), src)?;
        crate::ctr::check_update_version(&update, &self.entry.rom);
        files.set_update(Some(update))?;
        let timed = self.layout.encounters.timed();
        let (loaded, layout) = read(files, &self.entry, self.loaded.identity.clone(), timed)?;
        self.loaded = loaded;
        self.layout = layout;
        let text = self.entry.path("TextStrings")?;
        self.update_version = Some(crate::ctr::title_screen_version(&self.entry.rom, |file| {
            texts(files, &text, file)
        })?);
        Ok(())
    }

    #[expect(clippy::too_many_lines, reason = "one block per part of the game data")]
    fn write(&mut self, data: &GameData) -> Result<(), RomError> {
        let family = self.entry.family;
        let loaded = &self.loaded;
        let files = self.files.get_mut();
        let text = self.entry.path("TextStrings")?;
        let story = self.entry.path("StoryText")?;
        let layout = &mut self.layout;

        // Personal data.
        let mut personal = layout.personal.clone();
        for (old, sp) in loaded.species.iter().zip(&data.species) {
            if old != sp {
                personal::write_species(&mut personal, sp);
            }
        }
        if data.wild_held_items != loaded.wild_held_items {
            for sp in &data.species {
                let held = data.wild_held_items.get(&sp.number).copied().unwrap_or_default();
                if loaded.wild_held_items.get(&sp.number).copied().unwrap_or_default() != held {
                    personal::write_wild_held_items(&mut personal, sp.number, held);
                }
            }
        }
        for (n, f) in &data.tm_hm_compat {
            if loaded.tm_hm_compat.get(n) != Some(f) {
                personal::write_tm_compat(&mut personal, *n, f);
            }
        }
        if family == Family::UltraSunMoon {
            for (n, f) in &data.tutor_compat {
                if loaded.tutor_compat.get(n) != Some(f) {
                    personal::write_tutor_compat(&mut personal, *n, f);
                }
            }
        }
        let new_tweaks: BTreeSet<MiscTweak> = data
            .applied_tweaks
            .difference(&loaded.applied_tweaks)
            .copied()
            .collect();
        if new_tweaks.contains(&MiscTweak::SosBattlesForAll) {
            personal::sos_battles_for_all(&mut personal);
        }
        if new_tweaks.contains(&MiscTweak::FastEggHatching) {
            personal::fast_egg_hatching(&mut personal);
        }
        if new_tweaks.contains(&MiscTweak::NoEvYields) {
            personal::no_ev_yields(&mut personal);
        }
        if personal.entries != layout.personal.entries {
            let path = self.entry.path("PokemonStats")?;
            personal.write(files.garc_mut(&path, &Decompression::None)?)?;
            layout.personal = personal;
        }

        // Species names.
        if data.species.iter().zip(&loaded.species).any(|(a, b)| a.name != b.name) {
            let index = self.entry.int("PokemonNamesTextOffset")?;
            let mut lines = texts(files, &text, index)?;
            for sp in data.base_species() {
                if let Some(l) = lines.get_mut(usize::from(sp.number)) {
                    l.clone_from(&sp.name);
                }
            }
            set_texts(files, &text, index, &lines)?;
        }

        if data.evolutions != loaded.evolutions {
            let path = self.entry.path("PokemonEvolutions")?;
            layout
                .evolutions
                .write(files, &path, &loaded.evolutions, &data.evolutions)?;
        }

        if data.moves != loaded.moves {
            let path = self.entry.path("MoveData")?;
            moves::write(files, &path, &loaded.moves, &data.moves)?;
            if data.moves.iter().zip(&loaded.moves).any(|(a, b)| a.name != b.name) {
                let index = self.entry.int("MoveNamesTextOffset")?;
                let mut lines = texts(files, &text, index)?;
                for m in &data.moves {
                    if let Some(l) = lines.get_mut(usize::from(m.number)) {
                        l.clone_from(&m.name);
                    }
                }
                set_texts(files, &text, index, &lines)?;
            }
        }
        moves::write_learnsets(
            files,
            &self.entry.path("PokemonMovesets")?,
            &loaded.learnsets,
            &data.learnsets,
        )?;
        moves::write_egg_moves(
            files,
            &self.entry.path("EggMoves")?,
            &layout.personal,
            &loaded.egg_moves,
            &data.egg_moves,
        )?;

        // Starters, statics and trades.
        let static_path = self.entry.path("StaticPokemon")?;
        if data.starters != loaded.starters || data.starter_held_items != loaded.starter_held_items {
            let garc = files.garc_mut(&static_path, &Decompression::None)?;
            let mut gifts = garc.file(c::GIFTS_FILE)?;
            statics::write_starters(
                &mut gifts,
                &loaded.starters,
                &layout.starter_formes,
                &data.starters,
                &data.starter_held_items,
                &layout.personal.forme_index(),
            );
            garc.set_file(c::GIFTS_FILE, gifts)?;
        }
        if data.starters != loaded.starters {
            let index = self.entry.int("StarterTextOffset")?;
            let mut lines = texts(files, &story, index)?;
            statics::starter_text(family, &mut lines, data);
            set_texts(files, &story, index, &lines)?;
        }
        if data.statics != loaded.statics {
            layout.statics.write(
                files,
                [&static_path, &self.entry.path("Scripts")?],
                &loaded.statics,
                &data.statics,
            )?;
        }
        if data.totems != loaded.totems {
            layout.totems.write(files, &static_path, &loaded.totems, &data.totems)?;
        }
        if data.trades != loaded.trades {
            let index = self.entry.int("IngameTradesTextOffset")?;
            let mut lines = texts(files, &story, index)?;
            let garc = files.garc_mut(&static_path, &Decompression::None)?;
            let mut file = garc.file(c::TRADES_FILE)?;
            layout.trades.write(family, &mut file, &mut lines, loaded, data)?;
            garc.set_file(c::TRADES_FILE, file)?;
            set_texts(files, &story, index, &lines)?;
        }

        // Wild encounters.
        let wild_path = self.entry.path("WildPokemon")?;
        if data.encounters != loaded.encounters {
            layout
                .encounters
                .write(files, &wild_path, &loaded.encounters, &data.encounters)?;
            encounters::patch_minior(&mut files.code)?;
            statics::write_route_1(files, &static_path, data, &layout.personal)?;
        }
        if data.field_items != loaded.field_items {
            let allowed: BTreeSet<ItemId> = loaded.items.iter().filter(|i| i.allowed).map(|i| i.id).collect();
            encounters::write_field_items(files, &wild_path, family, |id| allowed.contains(&id), &data.field_items)?;
        }

        // Trainers.
        let trainer_paths = [self.entry.path("TrainerData")?, self.entry.path("TrainerPokemon")?];
        let [data_path, team_path] = &trainer_paths;
        if data.trainers != loaded.trainers {
            layout
                .teams
                .write(files, [data_path, team_path], data, &layout.personal)?;
            if data
                .trainers
                .iter()
                .zip(&loaded.trainers)
                .any(|(a, b)| a.name != b.name)
            {
                let index = self.entry.int("TrainerNamesTextOffset")?;
                let mut lines = texts(files, &text, index)?;
                let mut names = vec![lines.first().cloned().unwrap_or_default()];
                names.extend(data.trainers.iter().map(|t| t.name.clone()));
                lines = names;
                set_texts(files, &text, index, &lines)?;
                write_all_languages(files, &self.entry, index, &lines)?;
            }
        }
        if family == Family::SunMoon && (data.trainers != loaded.trainers || data.species != loaded.species) {
            let path = self.entry.path("Battle")?;
            let mut battle = files.read(&path)?;
            trainers::write_beast_lusamine(&mut battle, data)?;
            files.write(&path, battle)?;
        }
        if data.trainer_class_names != loaded.trainer_class_names {
            let index = self.entry.int("TrainerClassesTextOffset")?;
            set_texts(files, &text, index, &data.trainer_class_names)?;
            write_all_languages(files, &self.entry, index, &data.trainer_class_names)?;
        }

        // Items, TMs, tutors, shops.
        if data.items.iter().zip(&loaded.items).any(|(a, b)| a.name != b.name) {
            let index = self.entry.int("ItemNamesTextOffset")?;
            let mut lines = texts(files, &text, index)?;
            for i in &data.items {
                if let Some(l) = lines.get_mut(usize::from(i.id)) {
                    l.clone_from(&i.name);
                }
            }
            set_texts(files, &text, index, &lines)?;
        }
        if data.tm_moves != loaded.tm_moves {
            let item_index = self.entry.int("ItemDescriptionsTextOffset")?;
            let move_index = self.entry.int("MoveDescriptionsTextOffset")?;
            let mut item_descriptions = texts(files, &text, item_index)?;
            let move_descriptions = texts(files, &text, move_index)?;
            misc::write_tms(
                &mut files.code,
                family,
                &data.tm_moves,
                &data.moves,
                &mut item_descriptions,
                &move_descriptions,
            )?;
            set_texts(files, &text, item_index, &item_descriptions)?;
        }
        let shop_path = self.entry.path("ShopsAndTutors")?;
        if family == Family::UltraSunMoon && data.tutor_moves != loaded.tutor_moves {
            let mut shop = files.read(&shop_path)?;
            misc::write_tutors(&mut files.code, &mut shop, &data.tutor_moves)?;
            files.write(&shop_path, shop)?;
        }
        if data.shops != loaded.shops {
            let mut shop = files.read(&shop_path)?;
            misc::write_shops(&mut shop, family, &self.entry.array("ShopItemSizes"), &data.shops)?;
            files.write(&shop_path, shop)?;
        }
        if data.item_prices != loaded.item_prices {
            let garc = files.garc_mut(&self.entry.path("ItemData")?, &Decompression::None)?;
            for i in 1..garc.len() {
                let id = ItemId::try_from(i).unwrap_or_default();
                let (old, new) = (loaded.item_prices.get(&id), data.item_prices.get(&id));
                if old != new {
                    let mut file = garc.file(i)?;
                    put_u16(&mut file, 0, new.map_or(0, |&p| PriceFormat::TENS.stored_u16(p)));
                    garc.set_file(i, file)?;
                }
            }
        }
        if data.pickup_items != loaded.pickup_items {
            let garc = files.garc_mut(&self.entry.path("PickupData")?, &Decompression::All)?;
            let mut file = garc.file(0)?;
            misc::write_pickup(&mut file, &data.pickup_items);
            garc.set_file(0, file)?;
        }

        // The baby table follows the evolutions when the species are limited.
        if data.restricted_species.is_some() && data.evolutions != loaded.evolutions {
            write_babies(files, &self.entry, family, data)?;
        }

        // Code patches.
        if data.type_chart != loaded.type_chart {
            misc::write_type_chart(&mut files.code, self.entry.int("TypeTableOffset")?, &data.type_chart);
        }
        let battle_path = self.entry.path("Battle")?;
        if data.guaranteed_catching && !loaded.guaranteed_catching {
            let mut battle = files.read(&battle_path)?;
            misc::guaranteed_catching(&mut battle)?;
            files.write(&battle_path, battle)?;
        }
        if data.easier_happiness_evolutions && !loaded.easier_happiness_evolutions {
            misc::easier_happiness(&mut files.code)?;
        }
        if new_tweaks.contains(&MiscTweak::FastestText) {
            misc::fastest_text(&mut files.code)?;
        }
        if new_tweaks.contains(&MiscTweak::RetainAltFormes) {
            let mut battle = files.read(&battle_path)?;
            misc::retain_alt_formes(&mut files.code, &mut battle, family)?;
            files.write(&battle_path, battle)?;
        }
        files.flush()
    }
}

/// Trainer names and class names go into every language's texts.
fn write_all_languages(files: &mut Files, entry: &Entry, index: usize, lines: &[String]) -> Result<(), RomError> {
    for key in c::OTHER_LANGUAGE_TEXTS {
        let path = entry.path(key)?;
        set_texts(files, &path, index, lines)?;
    }
    Ok(())
}

/// The baby table: each species' first pre-evolution as far back as it goes, in a file per species and in the
/// master file after them.
fn write_babies(files: &mut Files, entry: &Entry, family: Family, data: &GameData) -> Result<(), RomError> {
    let count = family.pokemon_count();
    let garc = files.garc_mut(&entry.path("BabyPokemon")?, &Decompression::None)?;
    let mut master = garc.file(count + 1)?;
    for n in 1..=count {
        let mut baby = DexNumber::try_from(n).unwrap_or_default();
        let mut seen = BTreeSet::new();
        while seen.insert(baby) {
            match data.evolutions.iter().find(|e| e.to == baby) {
                Some(e) => baby = e.from,
                None => break,
            }
        }
        let mut file = garc.file(n)?;
        put_u16(&mut file, 0, baby);
        put_u16(&mut master, n * 2, baby);
        garc.set_file(n, file)?;
    }
    garc.set_file(count + 1, master)?;
    Ok(())
}

/// Every part of the game data, from the files.
#[expect(clippy::too_many_lines, reason = "a line for each field of GameData")]
fn read(files: &mut Files, entry: &Entry, identity: RomIdentity, timed: bool) -> Result<(GameData, Layout), RomError> {
    let family = entry.family;
    let generation = identity.generation;
    let text = entry.path("TextStrings")?;
    let story = entry.path("StoryText")?;
    let text_of = |files: &mut Files, key: &str| -> Result<Vec<String>, RomError> {
        let index = entry.int(key)?;
        texts(files, &text, index)
    };

    let item_names = text_of(files, "ItemNamesTextOffset")?;
    let items = misc::items(&item_names, family);
    let allowed: BTreeSet<ItemId> = items.iter().filter(|i| i.allowed).map(|i| i.id).collect();

    let personal_garc = files.garc(&entry.path("PokemonStats")?, &Decompression::None)?;
    let personal = personal::Personal::read(personal_garc, family)?;
    let names = text_of(files, "PokemonNamesTextOffset")?;
    let megas = {
        let garc = files.garc(&entry.path("MegaEvolutions")?, &Decompression::None)?;
        let mut megas = personal::Megas::new();
        for n in 1..=family.pokemon_count() {
            let base = DexNumber::try_from(n).unwrap_or_default();
            for (forme, item) in crate::formes::parse_megas(&garc.file(n)?, 2) {
                megas.insert((base, forme), item);
            }
        }
        megas
    };
    let species = personal::species(&personal, &names, &megas)?;
    let wild_held_items = personal::wild_held_items(&personal)?;
    let tm_hm_compat = personal::tm_compat(&personal);
    let tutor_compat = if family == Family::UltraSunMoon {
        personal::tutor_compat(&personal)
    } else {
        BTreeMap::new()
    };
    let (evolution_layout, evolutions) =
        evolutions::Evolutions::read(files, &entry.path("PokemonEvolutions")?, &personal, entry.sunny)?;

    let move_names = text_of(files, "MoveNamesTextOffset")?;
    let moves = moves::read(files, &entry.path("MoveData")?, family, &move_names)?;
    let learnsets = moves::read_learnsets(files, &entry.path("PokemonMovesets")?, family)?;
    let egg_moves = moves::read_egg_moves(files, &entry.path("EggMoves")?, &personal)?;

    let static_path = entry.path("StaticPokemon")?;
    let (gifts, static_file, trade_file) = {
        let garc = files.garc(&static_path, &Decompression::None)?;
        (
            garc.file(c::GIFTS_FILE)?,
            garc.file(c::STATICS_FILE)?,
            garc.file(c::TRADES_FILE)?,
        )
    };
    let (starters, starter_held_items, starter_formes) = statics::read_starters(&gifts, &personal.forme_index())?;
    let script = files
        .garc(&entry.path("Scripts")?, &Decompression::None)?
        .file(c::ZYGARDE_ASSEMBLY_SCRIPT_FILE)?;
    let totem_indices = entry.array("TotemPokemonIndices");
    let (totem_layout, totems) = totems::read(&static_file, &totem_indices, personal.forme_index())?;
    let mut skip = totem_indices;
    skip.extend(entry.array("AllyPokemonIndices"));
    skip.push(c::ROUTE_1_PIKIPEK_STATIC);
    let links = statics::parse_links(entry.rom.value("LinkedStaticEncounterOffsets").unwrap_or_default());
    let code = files.code.clone();
    let (static_layout, static_encounters) = statics::read(
        &gifts,
        &static_file,
        &script,
        &code,
        family,
        &skip,
        links,
        entry.array("ZygardeScriptLevelOffsets"),
        personal.forme_index(),
    )?;
    let trade_index = entry.int("IngameTradesTextOffset")?;
    let trade_texts = texts(files, &story, trade_index)?;
    let (trade_layout, trades) = statics::read_trades(&trade_file, &trade_texts)?;

    let map_names = text_of(files, "MapNamesTextOffset")?;
    let wild_path = entry.path("WildPokemon")?;
    let (encounter_layout, encounter_sets) = encounters::Encounters::read(
        files,
        [&wild_path, &entry.path("ZoneData")?, &entry.path("WorldData")?],
        family,
        &map_names,
        timed,
        &personal,
    )?;
    let field_items = encounters::read_field_items(files, &wild_path, family, |id| allowed.contains(&id))?;

    let trainer_class_names = text_of(files, "TrainerClassesTextOffset")?;
    let trainer_names: Vec<String> = text_of(files, "TrainerNamesTextOffset")?.into_iter().skip(1).collect();
    let (teams, trainers) = trainers::read(
        files,
        [&entry.path("TrainerData")?, &entry.path("TrainerPokemon")?],
        family,
        &trainer_names,
        &personal,
    )?;

    let ability_names: Vec<String> = text_of(files, "AbilityNamesTextOffset")?
        .into_iter()
        .take(family.highest_ability() + 1)
        .collect();
    let shop = files.read(&entry.path("ShopsAndTutors")?)?;
    let shops = misc::read_shops(
        &shop,
        family,
        &entry.array("ShopItemSizes"),
        &entry.array("TMShops"),
        &entry.array("RegularShops"),
    )?;
    let item_prices = misc::read_prices(files.garc(&entry.path("ItemData")?, &Decompression::None)?)?;
    let pickup = files.garc(&entry.path("PickupData")?, &Decompression::All)?.file(0)?;
    let pickup_items = misc::read_pickup(&pickup)?;
    let type_chart = misc::read_type_chart(&code, entry.int("TypeTableOffset")?)?;
    let tm_moves = misc::read_tms(&code, family)?;
    let tutor_moves = if family == Family::UltraSunMoon {
        misc::read_tutors(&code)?
    } else {
        Vec::new()
    };

    let mut loaded = GameData {
        identity,
        types: Type::in_generation(generation),
        formes: ppr_model::FormeRules {
            trainers: true,
            wild: true,
            statics: true,
            starters: true,
            evolutions: true,
            banned_for_trainers: Vec::new(),
            trainer_base_forme_abilities: false,
            force_swap_static_megas: false,
        },
        type_chart,
        species,
        evolutions,
        evolution_level_samples: Vec::new(),
        restricted_species: None,
        starters,
        starter_held_items,
        starter_type_triangle: None,
        items,
        wild_held_items,
        palettes: BTreeMap::new(),
        palette_files: None,
        player_graphics_packs: Vec::new(),
        player_graphics: None,
        // Wild Pokémon may always hold an item; Gen 7 has no dark grass.
        guaranteed_wild_held_items: true,
        dark_grass_held_items: false,
        encounters: encounter_sets,
        guaranteed_catching: false,
        statics: static_encounters,
        main_game_legendaries: entry
            .array("MainGameLegendaries")
            .into_iter()
            .filter_map(|n| DexNumber::try_from(n).ok())
            .collect(),
        special_music_statics: Vec::new(),
        correct_static_music: false,
        totems,
        easier_happiness_evolutions: false,
        trades,
        max_trade_nickname_length: c::MAX_TRADE_NICKNAME_LENGTH,
        max_trade_ot_name_length: c::MAX_TRADE_OT_NAME_LENGTH,
        moves,
        max_move_name_length: c::MAX_MOVE_NAME_LENGTH,
        learnsets,
        egg_moves,
        hm_moves: Vec::new(),
        game_breaking_moves: c::GAME_BREAKING_MOVES.to_vec(),
        banned_level_up_moves: c::BANNED_LEVEL_UP_MOVES.to_vec(),
        four_starting_moves: true,
        tm_moves,
        tutor_moves,
        tm_hm_compat,
        tutor_compat,
        early_required_hm_moves: Vec::new(),
        field_moves: Vec::new(),
        field_items,
        required_field_tms: match family {
            Family::SunMoon => c::REQUIRED_FIELD_TMS_SM.to_vec(),
            Family::UltraSunMoon => c::REQUIRED_FIELD_TMS_USUM.to_vec(),
        },
        shops,
        item_prices,
        balanced_item_prices: c::BALANCED_PRICES.into_iter().collect(),
        op_shop_items: c::OP_SHOP_ITEMS.to_vec(),
        mega_stones: crate::formes::mega_stones(),
        evolution_items: c::EVOLUTION_ITEMS.to_vec(),
        pickup_items,
        // TMs can't be held, so Pickup doesn't find them.
        pickup_tms: false,
        ability_names,
        ability_variations: c::ABILITY_VARIATIONS.iter().map(|&(a, v)| (a, v.to_vec())).collect(),
        useless_abilities: c::USELESS_ABILITIES.to_vec(),
        wild_banned: Vec::new(),
        trainers,
        trainer_class_names,
        elite_four_trainers: entry
            .array("EliteFourIndices")
            .into_iter()
            .filter_map(|i| u16::try_from(i).ok())
            .collect(),
        gym_and_elite_themes: c::gym_and_elite_themes(family)
            .into_iter()
            .map(|(k, t)| (k.to_owned(), t))
            .collect(),
        original_types: BTreeMap::new(),
        max_trainer_name_length: c::MAX_TRAINER_NAME_LENGTH,
        trainer_name_shares_class_line: false,
        max_trainer_names_total: None,
        max_trainer_class_name_length: c::MAX_TRAINER_CLASS_NAME_LENGTH,
        doubles_trainer_classes: entry
            .array("DoublesTrainerClasses")
            .into_iter()
            .filter_map(|i| u8::try_from(i).ok())
            .collect(),
        personal_trainer_classes: Vec::new(),
        fixed_trainer_text: false,
        fixed_trainer_name_length: false,
        fixed_trainer_class_name_length: false,
        text_tokens: Vec::new(),
        intro_candidates: Vec::new(),
        intro_pokemon: None,
        held_item_pools: c::held_item_pools(),
        trainer_pokemon_additions: [true; 3],
        trainer_custom_movesets: [true; 3],
        trainer_held_item_additions: [true; 3],
        available_tweaks: vec![
            MiscTweak::FastestText,
            MiscTweak::BanLuckyEgg,
            MiscTweak::SosBattlesForAll,
            MiscTweak::RetainAltFormes,
            MiscTweak::FastEggHatching,
            MiscTweak::NoEvYields,
        ],
        applied_tweaks: BTreeSet::new(),
        pc_potion: None,
        catching_tutorial_opponents: Vec::new(),
        catching_tutorial_players: Vec::new(),
        catching_tutorial: None,
        fossils: Vec::new(),
    };
    // The evolution level estimates are taken from the game as loaded.
    loaded.estimate_evolution_levels();
    Ok((
        loaded,
        Layout {
            personal,
            evolutions: evolution_layout,
            encounters: encounter_layout,
            teams,
            statics: static_layout,
            trades: trade_layout,
            totems: totem_layout,
            starter_formes,
        },
    ))
}

impl GameHandler for Gen7Handler {
    fn game_data(&self) -> &GameData {
        &self.loaded
    }

    /// Move tutors in Ultra Sun and Ultra Moon only. Wild Pokémon, static encounters and starters can be alternate
    /// formes (and so can Totem Pokémon, which every Gen 7 game has).
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            starter_held_items: true,
            move_tutors: self.entry.family == Family::UltraSunMoon,
            encounter_locations: true,
            time_based_encounters: true,
            physical_special_split: true,
            totem_pokemon: true,
            english: true,
            mega_evolutions: true,
            wild_alt_formes: true,
            static_alt_formes: true,
            starter_alt_formes: true,
            functional_formes: true,
            ..Capabilities::default()
        }
    }

    /// Shops keep their sizes; every wild table has one level range for all its slots.
    fn edit_rules(&self) -> ppr_model::EditRules {
        let mut rules = ppr_model::EditRules::from_game(&self.loaded);
        rules.shops.size = ppr_model::ShopSize::Fixed;
        rules.set_wild_levels((0..self.loaded.encounters.len()).map(|i| (i, ppr_model::WildLevels::Shared)));
        rules
    }

    fn apply(&mut self, data: &GameData) -> Result<(), RomError> {
        self.write(data)
    }

    fn use_time_of_day(&mut self, on: bool) -> Result<(), RomError> {
        self.loaded.encounters = self.layout.encounters.set_timed(on)?;
        Ok(())
    }

    fn encounter_set_origins(&self) -> Vec<usize> {
        self.layout.encounters.origins()
    }

    /// Writes the game partition as a decrypted CXI. With a game update loaded, the changes only make sense on top
    /// of the update, so they can only be saved as a LayeredFS folder.
    fn save(&self, out: &mut dyn std::io::Write) -> Result<(), RomError> {
        let mut files = self.files.borrow_mut();
        if files.game.update().is_some() {
            return Err(RomError::Unsupported(
                "with a game update loaded, the game can only be saved as a LayeredFS folder".into(),
            ));
        }
        files.game.write_cxi(out)?;
        Ok(())
    }

    fn set_seed(&mut self, seed: u64) {
        let acronym = self.entry.rom.value("Acronym").unwrap_or_default().to_owned();
        if let Some(smdh) = self.files.get_mut().game.smdh_mut() {
            smdh.set_all_descriptions(&format!("{acronym} randomizer seed: {seed}"));
            smdh.set_all_publishers(PUBLISHER);
        }
    }

    fn save_layered_fs(&self, dir: &Path) -> Result<PathBuf, RomError> {
        Ok(self.files.borrow_mut().game.write_layered_fs(dir)?)
    }

    /// Loads a decrypted update of this game; the game data is read again with the update's files.
    fn load_game_update(&mut self, path: &Path) -> Result<(), RomError> {
        let file = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(path)?);
        self.load_update(Box::new(file))
    }

    fn game_update_version(&self) -> Option<String> {
        self.update_version.clone()
    }
}
