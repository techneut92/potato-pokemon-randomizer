//! The randomization log: a text log of everything a run changed, section by section, in a fixed layout and
//! wording.
//!
//! Deliberate choices:
//! - The log is made from the game data before and after the run, so a section appears when its data changed, not
//!   merely because its randomizer ran. The update sections of base stats, moves and
//!   type effectiveness are part of the species, move data and type effectiveness sections.
//! - Evolution improvements are listed only when evolutions were not randomized too, since the evolutions before
//!   the improvements are not kept.
//! - The head has this app's name, version and issue page, and the settings as JSON rather than a
//!   settings string. The statistics have the time, not RNG call counts, and trainers have no ROM offsets.
//! - Wild areas are in the game's order, not sorted by location. Level-cap ties keep the trainers' order.
//! - The width of the ability names is measured over every ability.
//! - The diagnostics name a loaded game update by its file name and the version its title screen shows, instead
//!   of checksums of its executable and file system.

mod table;

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::time::Duration;

use ppr_model::{
    DexNumber, Effectiveness, EvolutionType, GameData, ItemId, MiscTweak, Move, MoveCategory, MoveId, Species,
    StaticEncounter, Trainer, TrainerPokemon, Type,
};
use ppr_random::Seed;
use ppr_settings::{BattleStyleMode, CompatMode, MovesetMode, Settings, ShopItemMode, StarterMode};

use self::table::TextTable;

/// What a log is made from.
#[derive(Debug)]
pub(crate) struct LogInput<'a> {
    /// The game as loaded.
    pub(crate) before: &'a GameData,
    /// The game as randomized.
    pub(crate) after: &'a GameData,
    pub(crate) settings: &'a Settings,
    pub(crate) seed: Seed,
    /// The loaded ROM's file name.
    pub(crate) file_name: &'a str,
    /// The loaded ROM's CRC32.
    pub(crate) crc32: u32,
    /// The game update laid over the ROM, if any.
    pub(crate) update: Option<UpdateInfo>,
    pub(crate) elapsed: Duration,
}

/// A game update in the log's diagnostics.
#[derive(Debug)]
pub(crate) struct UpdateInfo {
    /// The update's file name.
    pub(crate) file_name: String,
    /// What the game's title screen shows with it, e.g. `Ver. 1.5`.
    pub(crate) version: String,
}

/// A section's title and its search shortcut.
type Section = (&'static str, &'static str);

const TOC: Section = ("Table of Contents", "TABL");
const OVERVIEW: Section = ("Overview of Randomization", "OVRD");
const STATISTICS: Section = ("Randomization Statistics", "STAT");
const DIAGNOSTICS: Section = ("Randomization/ROM Diagnostics", "DIAG");
const SPECIES_TRAITS: Section = ("Pokemon Base Statistics / Types / Abilities", "PKST");
const EVOLUTIONS: Section = ("Pokemon Evolutions", "PKEV");
const EVOLUTION_IMPROVEMENTS: Section = ("Pokemon Evolution Improvements", "PKEI");
const STARTERS: Section = ("Starter Pokemon", "SRPK");
const STATICS: Section = ("Static Pokemon", "STPK");
const TRADES: Section = ("In-Game Trades", "IGTR");
const MOVE_DATA: Section = ("Move Data", "MVDT");
const MOVESETS: Section = ("Pokemon Movesets", "PKMV");
const LEVEL_CAPS: Section = ("Level Caps", "LVLC");
const TRAINERS: Section = ("Trainer Pokemon", "TRPK");
const TOTEMS: Section = ("Totem Pokemon", "TOPK");
const WILD: Section = ("Wild Pokemon", "WDPK");
const TMS: Section = ("TM Moves", "TMMV");
const TM_COMPAT: Section = ("TM/HM Compatibility", "TMCB");
const TUTORS: Section = ("Move Tutor Moves", "MTMV");
const TUTOR_COMPAT: Section = ("Move Tutor Compatibility", "MTCB");
const SHOPS: Section = ("Shop Items", "SHMS");
const PICKUP: Section = ("Pickup Items", "PUMS");
const TYPE_CHART: Section = ("Type Effectiveness", "TPEF");

const SEPARATOR: &str = "\n==========================================================\n\n";
/// Species and moves listed per line in the compatibility sections.
const COMPAT_ROW_WIDTH: usize = 5;
/// Width of the type column: two type names and a `/`.
const TYPE_COLUMN: usize = 8 * 2 + 1;
const EFFECTIVENESS_SYMBOLS: [&str; 4] = ["Imm", "nve", "---", "SE!"];
const EFFECTIVENESS_NAMES: [&str; 4] = ["No Effect/Immune", "Not Very Effective", "Neutral", "Super Effective"];
const ISSUES_URL: &str = "https://github.com/techneut92/potato-pokemon-randomizer/issues";

/// What changed, compared once.
#[derive(Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per part of the game")]
struct Changes {
    base_stats: bool,
    types: bool,
    abilities: bool,
    wild_held_items: bool,
    evolutions: bool,
    starters: bool,
    statics: bool,
    totems: bool,
    trades: bool,
    move_data: bool,
    movesets: bool,
    trainer_pokemon: bool,
    trainer_movesets: bool,
    trainer_names: bool,
    wild: bool,
    tms: bool,
    tm_hm_compat: bool,
    tutors: bool,
    tutor_compat: bool,
    field_items: bool,
    shops: bool,
    pickup: bool,
    type_chart: bool,
    palettes: bool,
}

impl Changes {
    fn of(b: &GameData, a: &GameData) -> Self {
        let species = |f: fn(&Species, &Species) -> bool| b.species.iter().zip(&a.species).any(|(x, y)| f(x, y));
        let teams = |f: fn(&TrainerPokemon, &TrainerPokemon) -> bool| {
            b.trainers.len() != a.trainers.len()
                || b.trainers.iter().zip(&a.trainers).any(|(x, y)| {
                    x.pokemon.len() != y.pokemon.len() || x.pokemon.iter().zip(&y.pokemon).any(|(p, q)| f(p, q))
                })
        };
        let moves = |m: &Move| (m.name.clone(), m.power, m.pp, m.accuracy, m.move_type, m.category);
        Self {
            base_stats: species(|x, y| x.stats != y.stats),
            types: species(|x, y| (x.primary_type, x.secondary_type) != (y.primary_type, y.secondary_type)),
            abilities: species(|x, y| x.abilities != y.abilities),
            wild_held_items: b.wild_held_items != a.wild_held_items,
            evolutions: b.evolutions != a.evolutions,
            starters: b.starters != a.starters || b.starter_held_items != a.starter_held_items,
            statics: b.statics != a.statics,
            totems: b.totems != a.totems,
            trades: b.trades != a.trades,
            move_data: b.moves.iter().map(moves).ne(a.moves.iter().map(moves)),
            movesets: b.learnsets != a.learnsets || b.egg_moves != a.egg_moves,
            trainer_pokemon: teams(|p, q| (p.species, p.level, p.held_item) != (q.species, q.level, q.held_item))
                || b.trainers
                    .iter()
                    .zip(&a.trainers)
                    .any(|(x, y)| x.battle_style != y.battle_style),
            trainer_movesets: teams(|p, q| p.moves != q.moves),
            trainer_names: b.trainers.iter().zip(&a.trainers).any(|(x, y)| x.name != y.name)
                || b.trainer_class_names != a.trainer_class_names,
            wild: b.encounters != a.encounters,
            tms: b.tm_moves != a.tm_moves,
            tm_hm_compat: b.tm_hm_compat != a.tm_hm_compat,
            tutors: b.tutor_moves != a.tutor_moves,
            tutor_compat: b.tutor_compat != a.tutor_compat,
            field_items: b.field_items != a.field_items,
            shops: b.shops != a.shops,
            pickup: b.pickup_items != a.pickup_items,
            type_chart: b.type_chart != a.type_chart,
            palettes: b.palettes != a.palettes,
        }
    }
}

/// The whole log.
pub(crate) fn write(input: &LogInput<'_>) -> String {
    let mut log = Logger {
        input,
        changes: Changes::of(input.before, input.after),
        out: String::new(),
    };
    log.head();
    log.table_of_contents();
    log.overview();
    log.optional_sections();
    log.statistics();
    log.diagnostics();
    log.out
}

struct Logger<'a> {
    input: &'a LogInput<'a>,
    changes: Changes,
    out: String,
}

/// `write!` into the log; writing to a `String` cannot fail.
macro_rules! w {
    ($log:expr, $($arg:tt)*) => {{
        let _ = write!($log.out, $($arg)*);
    }};
}

/// The names of the misc tweaks, as the log shows them.
fn tweak_name(t: MiscTweak) -> &'static str {
    match t {
        MiscTweak::BwExpPatch => "B/W Exp Patch",
        MiscTweak::NerfXAccuracy => "Nerf X Accuracy",
        MiscTweak::FixCritRate => "\"Fix\" Crit Rate",
        MiscTweak::FastestText => "Fastest Text",
        MiscTweak::RunningShoesIndoors => "Running Shoes Indoors",
        MiscTweak::RandomizePcPotion => "Randomize PC Potion",
        MiscTweak::AllowPikachuEvolution => "Allow Pikachu Evolution",
        MiscTweak::NationalDexAtStart => "Give National Dex at Start",
        MiscTweak::FastEggHatching => "Fast Egg Hatching",
        MiscTweak::ForceChallengeMode => "Force Challenge Mode",
        MiscTweak::LowerCaseNames => "Lower Case Pokemon Names",
        MiscTweak::RandomizeCatchingTutorial => "Randomize Catching Tutorial",
        MiscTweak::BanLuckyEgg => "Ban Lucky Egg",
        MiscTweak::NoFreeLuckyEgg => "No Free Lucky Egg",
        MiscTweak::BanBigManiacItems => "Ban Big Money Maniac Items",
        MiscTweak::SosBattlesForAll => "All Wild Pokemon Can Call Allies",
        other => tweak_name_rest(other),
    }
}

/// The tweaks of later games; split off to keep [`tweak_name`] short.
fn tweak_name_rest(t: MiscTweak) -> &'static str {
    match t {
        MiscTweak::BalanceStaticLevels => "Balance Static Pokemon Levels",
        MiscTweak::RetainAltFormes => "Don't Revert Temporary Alt Formes",
        MiscTweak::RunWithoutRunningShoes => "Run Without Running Shoes",
        MiscTweak::FasterHpAndExpBars => "Faster HP and EXP Bars",
        MiscTweak::FastDistortionWorld => "Fast Distortion World",
        MiscTweak::UpdateRotomFormeTyping => "Update Rotom Appliance Typings",
        MiscTweak::DisableLowHpMusic => "Disable Low HP Music",
        MiscTweak::ReusableTms => "Reusable TMs",
        MiscTweak::ForgettableHms => "Forgettable HMs",
        MiscTweak::NoEvYields => "No EVs From Pokemon",
        _ => "",
    }
}

/// A type's name as the log prints it: in capitals.
fn type_name(t: Type) -> String {
    t.to_string().to_uppercase()
}

/// A species' full name: an alternate forme's with its suffix.
fn species_name(g: &GameData, n: DexNumber) -> String {
    g.species(n).map_or_else(|| format!("#{n}"), Species::full_name)
}

fn item_name(g: &GameData, id: ItemId) -> String {
    g.items
        .iter()
        .find(|i| i.id == id)
        .map_or_else(|| format!("item {id}"), |i| i.name.clone())
}

fn find_move(g: &GameData, id: MoveId) -> Option<&Move> {
    g.moves.iter().find(|m| m.number == id)
}

fn move_name(g: &GameData, id: MoveId) -> String {
    find_move(g, id).map_or_else(|| format!("move {id}"), |m| m.name.clone())
}

fn ability_name(g: &GameData, id: u16) -> String {
    g.ability_names.get(usize::from(id)).cloned().unwrap_or_default()
}

/// A trainer's full name: the class and the name.
fn trainer_name(g: &GameData, t: &Trainer) -> String {
    let class = g
        .trainer_class_names
        .get(usize::from(t.class))
        .filter(|c| !c.is_empty());
    match (class, t.name.is_empty()) {
        (Some(c), false) => format!("{c} {}", t.name),
        (Some(c), true) => c.clone(),
        (None, _) => t.name.clone(),
    }
}

/// A trainer's Pokémon: species, held item and level.
fn trainer_pokemon(g: &GameData, p: &TrainerPokemon) -> String {
    let item = p.held_item.map(|i| format!("@{}", item_name(g, i))).unwrap_or_default();
    format!("{}{item} Lv{}", species_name(g, p.species), p.level)
}

/// A static encounter, with its level when `print_level` is set.
fn static_encounter(g: &GameData, s: &StaticEncounter, print_level: bool) -> String {
    let mut out = species_name(g, s.species);
    if s.egg {
        out.push_str(" (egg)");
    } else if print_level {
        let _ = write!(out, " Lv{}", s.level);
        if s.linked_levels.iter().any(|&l| l != s.level) {
            for l in &s.linked_levels {
                let _ = write!(out, " / Lv{l}");
            }
        }
    }
    out
}

/// The ability a trainer's Pokémon has.
fn trainer_ability(g: &GameData, p: &TrainerPokemon) -> u16 {
    if p.ability_slot == 0 {
        return 0;
    }
    g.trainer_pokemon_abilities(p.species)
        .get(usize::from(p.ability_slot) - 1)
        .copied()
        .unwrap_or(0)
}

/// The text for an evolution method, with its item, move or species.
fn evolution_kind(g: &GameData, kind: EvolutionType, parameter: u16) -> String {
    use EvolutionType as E;
    let item = || item_name(g, parameter);
    match kind {
        E::Level | E::LevelCreateExtra | E::LevelGameThis => "Level-up".into(),
        E::Stone => format!("Use {}", item()),
        E::Trade => "Trade".into(),
        E::TradeItem => format!("Trade, holding {}", item()),
        E::Happiness => "Level-up with high happiness".into(),
        E::HappinessDay => "Level-up with high happiness during the day".into(),
        E::HappinessNight => "Level-up with high happiness at night".into(),
        E::LevelAttackHigher => "Level-up with higher Atk".into(),
        E::LevelDefenseHigher => "Level-up with higher Def".into(),
        E::LevelAtkDefSame => "Level-up with equal Atk/Def".into(),
        E::LevelLowPv => "Level-up with low PV".into(),
        E::LevelHighPv => "Level-up with high PV".into(),
        E::LevelIsExtra => "Level-up with an empty party slot and a Poké Ball in the bag".into(),
        E::HighBeauty => "Level-up with high beauty".into(),
        E::StoneMaleOnly => format!("Use {}, male only", item()),
        E::StoneFemaleOnly => format!("Use {}, female only", item()),
        E::ItemDay => format!("Level-up, holding {} during the day", item()),
        E::ItemNight => format!("Level-up, holding {} at night", item()),
        E::WithMove => format!("Level-up, knowing {}", move_name(g, parameter)),
        E::WithOther => format!("Level-up with {} in the party", species_name(g, parameter)),
        E::LevelMaleOnly => "Level-up, male only".into(),
        E::LevelFemaleOnly => "Level-up, female only".into(),
        // No Gen 3 game has the places these need, so the place is "Nowhere".
        E::MagneticField | E::Snowy => "Level-up at Nowhere".into(),
        E::MossRock => "Level-up near the moss rock in Nowhere".into(),
        E::IceRock => "Level-up near the ice rock in Nowhere".into(),
        other => evolution_kind_rest(g, other, parameter),
    }
}

/// The evolution methods of later games; split off to keep [`evolution_kind`] short.
fn evolution_kind_rest(g: &GameData, kind: EvolutionType, parameter: u16) -> String {
    use EvolutionType as E;
    match kind {
        E::TradeSpecial => "Trade with Karrablast/Shelmet".into(),
        E::FairyAffection => "Level-up, knowing a Fairy-type move and having high affection".into(),
        E::LevelWithDark => "Level-up with a Dark-type Pokémon in the party".into(),
        E::LevelUpsideDown => "Level-up with the console held upside down".into(),
        E::LevelRain => "Level-up while it is raining in the overworld".into(),
        E::LevelDay | E::LevelGameThisDay => "Level-up during the day".into(),
        E::LevelNight | E::LevelGameThisNight => "Level-up at night".into(),
        E::LevelGameOther | E::LevelGameOtherDay | E::LevelGameOtherNight => "None".into(),
        E::LevelDusk => "Level-up at dusk".into(),
        E::LevelUltra => "Level-up in Ultra Space".into(),
        E::StoneUltra => format!("Use {} in Ultra Space", item_name(g, parameter)),
        E::Item => format!("Level-up, holding {}", item_name(g, parameter)),
        _ => String::new(),
    }
}

/// An evolution method, with the estimated level where it has one.
fn evolution_method(g: &GameData, kind: EvolutionType, parameter: u16, estimated_level: u16) -> String {
    let mut out = evolution_kind(g, kind, parameter);
    if kind.uses_level_threshold() {
        let _ = write!(out, ", lvl. {parameter}+");
    } else {
        let _ = write!(out, " (estimated evo lvl. {estimated_level})");
    }
    out
}

/// The Gen 3 Pickup chances: percent per item and per range of 10 levels, by position in the table.
fn pickup_chances(family: &str, count: usize) -> Vec<[u8; 10]> {
    let mut chances = vec![[0; 10]; count];
    let mut set = |i: usize, range: usize, chance: u8| {
        if let Some(c) = chances.get_mut(i) {
            c[range] = chance;
        }
    };
    for range in 0..10 {
        match family {
            "Ruby" | "Sapp" => {
                set(0, range, 30);
                (1..7).for_each(|i| set(i, range, 10));
                set(7, range, 5);
                set(8, range, 4);
                set(9, range, 1);
            }
            "FRLG" => {
                set(0, range, 15);
                (1..7).for_each(|i| set(i, range, 10));
                (7..11).for_each(|i| set(i, range, 5));
                (11..16).for_each(|i| set(i, range, 1));
            }
            _ => {
                set(range, range, 30);
                (1..7).for_each(|i| set(range + i, range, 10));
                set(range + 7, range, 4);
                set(range + 8, range, 4);
                set(18 + range, range, 1);
                set(18 + range + 1, range, 1);
            }
        }
    }
    chances
}

impl<'a> Logger<'a> {
    fn before(&self) -> &'a GameData {
        self.input.before
    }

    fn after(&self) -> &'a GameData {
        self.input.after
    }

    fn settings(&self) -> &'a Settings {
        self.input.settings
    }

    fn metronome(&self) -> bool {
        self.settings().movesets.mode == MovesetMode::MetronomeOnly
    }

    fn abilities_per_species(&self) -> usize {
        if self.after().ability_names.is_empty() {
            0
        } else {
            self.after()
                .species
                .iter()
                .map(|s| s.abilities.len())
                .max()
                .unwrap_or(0)
        }
    }

    fn section_title(&mut self, (title, shortcut): Section) {
        w!(self, " ( {title} {{{shortcut}}} )\n\n");
    }

    fn separator(&mut self) {
        self.out.push_str(SEPARATOR);
    }

    // The optional sections, in groups; each group ends with a blank line in the table of contents.
    fn groups(&self) -> Vec<Vec<(Section, bool)>> {
        let c = &self.changes;
        let s = self.settings();
        let trainers = c.trainer_pokemon || c.trainer_movesets || c.trainer_names;
        let improvements = s.evolutions.mode == ppr_settings::EvolutionMode::Unchanged
            && (s.evolutions.change_impossible
                || s.evolutions.easier_max_level.is_some()
                || s.evolutions.remove_time_based)
            && c.evolutions;
        vec![
            vec![
                (
                    SPECIES_TRAITS,
                    c.base_stats || c.types || c.abilities || c.wild_held_items,
                ),
                (EVOLUTIONS, c.evolutions && !improvements),
                (EVOLUTION_IMPROVEMENTS, improvements),
            ],
            vec![(STARTERS, c.starters), (STATICS, c.statics), (TRADES, c.trades)],
            vec![(MOVE_DATA, c.move_data), (MOVESETS, c.movesets || self.metronome())],
            vec![(LEVEL_CAPS, trainers), (TRAINERS, trainers), (TOTEMS, c.totems)],
            vec![(WILD, c.wild)],
            vec![
                (TMS, c.tms || self.metronome()),
                (TM_COMPAT, c.tm_hm_compat),
                (
                    TUTORS,
                    !self.after().tutor_moves.is_empty() && (c.tutors || self.metronome()),
                ),
                (TUTOR_COMPAT, !self.after().tutor_moves.is_empty() && c.tutor_compat),
            ],
            vec![(SHOPS, c.shops), (PICKUP, c.pickup)],
            vec![(TYPE_CHART, c.type_chart)],
        ]
    }

    /// The head: the app, its version and the settings.
    fn head(&mut self) {
        self.out
            .push_str("==========================================================\n");
        self.out
            .push_str("|               Potato Pokémon Randomizer                |\n");
        self.out
            .push_str("==========================================================\n");
        self.out.push_str(" [ Log for Randomized Game ]\n\n");
        w!(self, "Base game: {}\n", self.after().identity.name);
        w!(
            self,
            "Randomizer version: Potato Pokémon Randomizer {}\n",
            env!("CARGO_PKG_VERSION")
        );
        w!(self, "Random seed: {}\n", self.input.seed);
        let settings = serde_json::to_string(self.settings()).unwrap_or_default();
        w!(self, "Settings: {settings}\n\n");
        self.out.push_str(
            "If you are having problems using the Potato Pokémon Randomizer,\nplease report them with this log.\n\n",
        );
        w!(self, "GitHub issues page link: {ISSUES_URL}\n");
        self.separator();
    }

    fn contents_row(&mut self, (title, shortcut): Section) {
        let dashes = "-".repeat(49usize.saturating_sub(title.chars().count()));
        w!(self, " {title}{dashes}{{{shortcut}}}\n");
    }

    /// The table of contents.
    fn table_of_contents(&mut self) {
        self.section_title(TOC);
        self.out
            .push_str("Search (ctrl-f) for the strings in curly brackets {}\nto find the below sections quickly.\n\n");
        self.contents_row(TOC);
        self.contents_row(OVERVIEW);
        self.out.push('\n');
        for group in self.groups() {
            let mut any = false;
            for (section, logged) in group {
                if logged {
                    self.contents_row(section);
                    any = true;
                }
            }
            if any {
                self.out.push('\n');
            }
        }
        self.contents_row(STATISTICS);
        self.contents_row(DIAGNOSTICS);
        self.separator();
    }

    fn overview_line(&mut self, title: &str, changed: bool, relevant: bool) {
        if relevant {
            let state = if changed { "Randomized/Changed" } else { "Unchanged" };
            w!(self, "{title}: {state}\n");
        }
    }

    /// The overview.
    fn overview(&mut self) {
        self.section_title(OVERVIEW);
        let c = std::mem::take(&mut self.changes);
        let a = self.after();
        let abilities = self.abilities_per_species() != 0;
        let (statics, totems, tutors, shops, pickup, palettes) = (
            !a.statics.is_empty(),
            !a.totems.is_empty(),
            !a.tutor_moves.is_empty(),
            !a.shops.is_empty(),
            !a.pickup_items.is_empty(),
            !a.palettes.is_empty(),
        );
        self.overview_line("Pokemon Base Statistics", c.base_stats, true);
        self.overview_line("Pokemon Types", c.types, true);
        self.overview_line("Pokemon Abilities", c.abilities, abilities);
        self.overview_line("Pokemon Evolutions", c.evolutions, true);
        self.overview_line("Starter Pokemon", c.starters, true);
        self.overview_line("Static Pokemon", c.statics, statics);
        self.overview_line("In-Game Trades", c.trades, true);
        self.overview_line("Move Data", c.move_data, true);
        self.overview_line("Pokemon Movesets", c.movesets, true);
        self.overview_line("Trainer Pokemon", c.trainer_pokemon, true);
        self.overview_line("Trainer Movesets", c.trainer_movesets, true);
        self.overview_line("Trainer Names", c.trainer_names, true);
        self.overview_line("Totem Pokemon", c.totems, totems);
        self.overview_line("Wild Pokemon", c.wild, true);
        self.overview_line("TM Moves", c.tms, true);
        self.overview_line("TM/HM Compatibility", c.tm_hm_compat, true);
        self.overview_line("Move Tutor Moves", c.tutors, tutors);
        self.overview_line("Move Tutor Compatibility", c.tutor_compat, tutors);
        self.overview_line("Field Items", c.field_items, true);
        self.overview_line("Shop Items", c.shops, shops);
        self.overview_line("Pickup Items", c.pickup, pickup);
        self.overview_line("Type Effectiveness", c.type_chart, true);
        self.overview_line("Pokemon Palettes", c.palettes, palettes);
        self.changes = c;
        self.out.push('\n');
        let tweaks: Vec<MiscTweak> = self.after().applied_tweaks.iter().copied().collect();
        if tweaks.is_empty() {
            self.out.push_str("No Misc. Tweaks were applied.\n");
        } else {
            self.out.push_str("The following Misc. Tweaks were applied:\n");
            for t in tweaks {
                w!(self, "{}\n", tweak_name(t));
            }
        }
        self.out.push('\n');
        if let Some(n) = self.after().intro_pokemon
            && self.before().intro_pokemon != Some(n)
        {
            w!(self, "Intro Pokemon: {}\n", species_name(self.after(), n));
        }
        self.separator();
    }

    /// The sections whose data changed.
    fn optional_sections(&mut self) {
        for (section, logged) in self.groups().into_iter().flatten() {
            if !logged {
                continue;
            }
            match section {
                SPECIES_TRAITS => self.species_traits(),
                EVOLUTIONS => self.evolutions(),
                EVOLUTION_IMPROVEMENTS => self.evolution_improvements(),
                STARTERS => self.starters(),
                STATICS => self.statics(),
                TRADES => self.trades(),
                MOVE_DATA => self.move_data(),
                MOVESETS => self.movesets(),
                LEVEL_CAPS => self.level_caps(),
                TRAINERS => self.trainers(),
                TOTEMS => self.totems(),
                WILD => self.wild(),
                TMS => self.tms(),
                TM_COMPAT => self.tm_compat(),
                TUTORS => self.tutors(),
                TUTOR_COMPAT => self.tutor_compat(),
                SHOPS => self.shops(),
                PICKUP => self.pickup(),
                _ => self.type_chart(),
            }
        }
    }

    /// The statistics: the time only.
    fn statistics(&mut self) {
        self.section_title(STATISTICS);
        w!(self, "Time elapsed: {}ms\n", self.input.elapsed.as_millis());
        self.separator();
    }

    /// The diagnostics: the file name and the CRC-32 of the original ROM, and the game update laid over it.
    fn diagnostics(&mut self) {
        self.section_title(DIAGNOSTICS);
        w!(self, "File name: {}\n", self.input.file_name);
        w!(self, "Original ROM CRC32: {:08X}\n", self.input.crc32);
        if let Some(update) = &self.input.update {
            w!(self, "Game update: {} ({})\n", update.file_name, update.version);
        }
    }

    fn name_width(&self) -> usize {
        self.after()
            .species
            .iter()
            .map(|s| s.full_name().chars().count())
            .max()
            .unwrap_or(0)
    }

    /// The species' stats, types and abilities.
    fn species_traits(&mut self) {
        self.section_title(SPECIES_TRAITS);
        let g = self.input.after;
        // Each base species with its alternate formes after it, but cosmetic ones.
        let listed: Vec<&Species> = g
            .base_species()
            .flat_map(|b| std::iter::once(b).chain(g.formes_of(b.number).filter(|f| !f.forme.cosmetic)))
            .collect();
        let num = listed.len().to_string().len();
        let name = self.name_width();
        let per_species = self.abilities_per_species();
        let ability = g.ability_names.iter().map(|n| n.chars().count()).max().unwrap_or(0);
        w!(
            self,
            "{:>num$}|{:<name$}|{:<TYPE_COLUMN$}|{:>4}",
            "NUM",
            "NAME",
            "TYPE",
            "BST"
        );
        w!(
            self,
            "|{:>4}|{:>4}|{:>4}|{:>4}|{:>4}|{:>4}",
            "HP",
            "ATK",
            "DEF",
            "SATK",
            "SDEF",
            "SPD"
        );
        for header in ["ABILITY 1", "ABILITY 2", "H. ABILITY"].into_iter().take(per_species) {
            w!(self, "|{header:<ability$}");
        }
        self.out.push_str("|HELD ITEM\n");
        for s in listed {
            let types = s.secondary_type.map_or_else(
                || type_name(s.primary_type),
                |t2| format!("{}/{}", type_name(s.primary_type), type_name(t2)),
            );
            let st = &s.stats;
            let number = if s.is_base_forme() {
                s.number.to_string()
            } else {
                String::new()
            };
            w!(
                self,
                "{number:>num$}|{:<name$}|{types:<TYPE_COLUMN$}|{:>4}",
                s.full_name(),
                st.total()
            );
            w!(
                self,
                "|{:>4}|{:>4}|{:>4}|{:>4}|{:>4}|{:>4}",
                st.hp,
                st.attack,
                st.defense,
                st.sp_attack,
                st.sp_defense,
                st.speed
            );
            let first = s.abilities.first().copied().unwrap_or(0);
            for i in 0..per_species {
                let id = s.abilities.get(i).copied().unwrap_or(0);
                let text = if i > 0 && id == first {
                    "--".to_owned()
                } else {
                    ability_name(g, id)
                };
                w!(self, "|{text:<ability$}");
            }
            self.out.push('|');
            let mut items = Vec::new();
            if let Some(h) = g.wild_held_items.get(&s.number) {
                if let Some(i) = h.guaranteed {
                    items.push(format!("{} (100%)", item_name(g, i)));
                } else {
                    for (item, what) in [
                        (h.common, "common"),
                        (h.rare, "rare"),
                        (h.dark_grass, "dark grass only"),
                    ] {
                        if let Some(i) = item {
                            items.push(format!("{} ({what})", item_name(g, i)));
                        }
                    }
                }
            }
            w!(self, "{}\n", items.join(", "));
        }
        self.separator();
    }

    /// The evolutions.
    fn evolutions(&mut self) {
        self.section_title(EVOLUTIONS);
        let g = self.input.after;
        let name = self.name_width();
        w!(self, "{:<name$}|{:<name$}|METHOD\n", "FROM", "TO");
        for s in &g.species {
            for (i, e) in g.evolutions.iter().filter(|e| e.from == s.number).enumerate() {
                let from = if i == 0 { s.name.as_str() } else { "" };
                w!(
                    self,
                    "{from:<name$}|{:<name$}|{}\n",
                    species_name(g, e.to),
                    evolution_method(g, e.kind, e.parameter, e.estimated_level)
                );
            }
        }
        self.separator();
    }

    /// The evolution improvements, comparing the evolutions before and after.
    fn evolution_improvements(&mut self) {
        self.section_title(EVOLUTION_IMPROVEMENTS);
        let (b, g) = (self.input.before, self.input.after);
        let s = &self.input.settings.evolutions;
        self.out
            .push_str("Some evolutions have been changed through the option(s):\n");
        if s.change_impossible {
            self.out.push_str("- Change Impossible Evolutions\n");
        }
        if s.easier_max_level.is_some() {
            self.out.push_str("- Make Evolutions Easier\n");
        }
        if s.remove_time_based {
            self.out.push_str("- Remove Time-Based Evolutions\n");
        }
        self.out.push('\n');
        if s.easier_max_level.is_some() && g.identity.generation.number() != 1 {
            self.out
                .push_str("Friendship evolutions now take 160 happiness (was 220).\n");
        }
        self.out.push('\n');
        let mut table = TextTable::new(4);
        table.add_row(["FROM", "TO", "OLD METHOD", "NEW METHOD"]);
        let of = |game: &GameData, n: DexNumber| -> Vec<ppr_model::Evolution> {
            game.evolutions.iter().filter(|e| e.from == n).copied().collect()
        };
        for sp in &g.species {
            let (old, new) = (of(b, sp.number), of(g, sp.number));
            if old == new {
                continue;
            }
            table.add_cell(0, sp.name.clone());
            let mut targets: Vec<DexNumber> = Vec::new();
            for e in &new {
                if !targets.contains(&e.to) {
                    targets.push(e.to);
                }
            }
            for to in targets {
                table.add_cell(1, species_name(g, to));
                let old_evos: Vec<_> = old.iter().filter(|e| e.to == to).collect();
                if old_evos.is_empty() {
                    table.add_cell(2, "None");
                }
                for (i, e) in new.iter().filter(|e| e.to == to).enumerate() {
                    table.add_cell(3, evolution_method(g, e.kind, e.parameter, e.estimated_level));
                    if let Some(o) = old_evos.get(i) {
                        table.add_cell(2, evolution_method(b, o.kind, o.parameter, o.estimated_level));
                    }
                }
                table.even_out();
            }
            table.even_out();
        }
        w!(self, "{table}");
        self.separator();
    }

    /// The starters.
    fn starters(&mut self) {
        self.section_title(STARTERS);
        let g = self.input.after;
        let mode = match self.input.settings.starters.mode {
            StarterMode::Unchanged => {
                "Unchanged (!! This should never be visible. If you see this, please make a bug report. !!)"
            }
            StarterMode::Custom => "Custom",
            StarterMode::Random => "Random (completely)",
            StarterMode::RandomBasic => "Random (any basic Pokemon)",
            StarterMode::RandomWithTwoEvolutions => "Random (basic Pokemon with 2 evolutions)",
        };
        w!(self, "Mode: {mode}\n");
        let items = &g.starter_held_items;
        for (i, &n) in g.starters.iter().enumerate() {
            let item = match items.as_slice() {
                [Some(one)] => Some(*one),
                _ if items.len() == g.starters.len() => items[i],
                _ => None,
            };
            match item {
                Some(it) => w!(
                    self,
                    "Set starter {} to {}, holding {}\n",
                    i + 1,
                    species_name(g, n),
                    item_name(g, it)
                ),
                None => w!(self, "Set starter {} to {}\n", i + 1, species_name(g, n)),
            }
        }
        self.separator();
    }

    /// The static Pokémon.
    fn statics(&mut self) {
        self.section_title(STATICS);
        let (b, g) = (self.input.before, self.input.after);
        let levels = self.input.settings.statics.level_modifier != 0;
        let mut seen: HashMap<String, usize> = HashMap::new();
        for (old, new) in b.statics.iter().zip(&g.statics) {
            let old_text = static_encounter(b, old, levels);
            self.out.push_str(&old_text);
            let count = seen.entry(old_text).or_insert(0);
            *count += 1;
            if *count > 1 {
                w!(self, "({count})");
            }
            w!(self, " => {}\n", static_encounter(g, new, levels));
        }
        self.separator();
    }

    /// The Totem Pokémon: each old species, then the new totem with its held item, level, aura and allies.
    fn totems(&mut self) {
        self.section_title(TOTEMS);
        let (b, g) = (self.input.before, self.input.after);
        for (old, new) in b.totems.iter().zip(&g.totems) {
            w!(self, "{} =>\n", species_name(b, old.species));
            let item = new
                .held_item
                .map(|i| format!("@{}", item_name(g, i)))
                .unwrap_or_default();
            w!(
                self,
                "{}{item} Lv{}\n\tAura: {}",
                species_name(g, new.species),
                new.level,
                new.aura
            );
            for (i, a) in new.allies.iter().enumerate() {
                w!(self, "\n\tAlly {}: {} Lv{}", i + 1, species_name(g, a.species), a.level);
            }
            self.out.push('\n');
        }
        self.separator();
    }

    /// The in-game trades.
    fn trades(&mut self) {
        self.section_title(TRADES);
        let (b, g) = (self.input.before, self.input.after);
        let mut table = TextTable::new(6);
        table.add_row([
            "OLD REQUESTED",
            "OLD GIVEN",
            "OLD NICKNAME",
            "NEW REQUESTED",
            "NEW GIVEN",
            "NEW NICKNAME",
        ]);
        let requested = |game: &GameData, r: Option<DexNumber>| {
            r.map_or_else(|| "Any Pokemon".to_owned(), |n| species_name(game, n))
        };
        for (old, new) in b.trades.iter().zip(&g.trades) {
            table.add_row([
                requested(b, old.requested),
                species_name(b, old.given),
                old.nickname.clone(),
                requested(g, new.requested),
                species_name(g, new.given),
                new.nickname.clone(),
            ]);
        }
        w!(self, "{table}");
        self.separator();
    }

    fn category_name(g: &GameData, m: &Move) -> &'static str {
        if m.category == MoveCategory::Status {
            "STATUS"
        } else if g.is_physical(m) {
            "PHYSICAL"
        } else {
            "SPECIAL"
        }
    }

    /// The move data.
    fn move_data(&mut self) {
        self.section_title(MOVE_DATA);
        let (b, g) = (self.input.before, self.input.after);
        let split = g.identity.generation.number() >= 4;
        let renamed = b.moves.iter().map(|m| &m.name).ne(g.moves.iter().map(|m| &m.name));
        let mut head = vec!["NUM", "NAME", "TYPE", "POWER", "ACC.", "PP"];
        if renamed {
            head.insert(1, "OLD NAME");
        }
        if split {
            head.push("CATEGORY");
        }
        let mut table = TextTable::new(head.len());
        table.add_row(head);
        for m in g.moves.iter().filter(|m| m.number != 0) {
            let mut row = vec![m.number.to_string()];
            if renamed {
                row.push(find_move(b, m.number).map_or_else(|| "???".to_owned(), |o| o.name.clone()));
            }
            row.extend([
                m.name.clone(),
                m.move_type.map_or_else(|| "???".to_owned(), type_name),
                m.power.to_string(),
                format!("{}%", m.accuracy),
                m.pp.to_string(),
            ]);
            if split {
                row.push(Self::category_name(g, m).to_owned());
            }
            table.add_row(row);
        }
        w!(self, "{table}");
        self.separator();
    }

    /// One move of a moveset: type, category, power, PP and accuracy, marked STAB when the learner shares its type.
    fn moveset_move(g: &GameData, m: &Move, learner: &Species) -> String {
        let category = Self::category_name(g, m);
        let power = if m.category == MoveCategory::Status {
            "--".to_owned()
        } else {
            format!("{:>3}", m.power)
        };
        let move_type = m.move_type.map_or_else(|| "???".to_owned(), type_name);
        let mut out = format!(
            "{:<w$}| {move_type:<8} | {category:<8} | POW={power:>3} | PP={:>2} | ACC={:>3}%",
            m.name,
            m.pp,
            m.accuracy,
            w = g.max_move_name_length
        );
        if m.category != MoveCategory::Status && m.move_type.is_some_and(|t| learner.has_type(t)) {
            out.push_str(" (STAB)");
        }
        out
    }

    /// The movesets.
    fn movesets(&mut self) {
        self.section_title(MOVESETS);
        if self.metronome() {
            self.out
                .push_str("Metronome only mode - every Pokémon learns only Metronome.\n");
            self.separator();
            return;
        }
        let g = self.input.after;
        for s in &g.species {
            w!(self, "{:03} {} -> ", s.number, s.name);
            let evolved = g.evolved_species(s.number);
            if evolved.is_empty() {
                self.out.push_str(" (no evolution)\n");
            } else {
                let names: Vec<String> = evolved.iter().map(|&n| species_name(g, n)).collect();
                w!(self, "{}\n", names.join(", "));
            }
            let st = &s.stats;
            w!(
                self,
                "  HP| ATK| DEF|SATK|SDEF| SPD \n{:>4}|{:>4}|{:>4}|{:>4}|{:>4}|{:>4} \n",
                st.hp,
                st.attack,
                st.defense,
                st.sp_attack,
                st.sp_defense,
                st.speed
            );
            for lm in g.learnsets.get(&s.number).map_or(&[][..], Vec::as_slice) {
                let Some(m) = find_move(g, lm.move_id) else {
                    w!(self, "invalid move at level {:<2} \n", lm.level);
                    continue;
                };
                if lm.level == 0 {
                    self.out.push_str("Learnt upon evolution: ");
                } else {
                    w!(self, "Level {:<2}: ", lm.level);
                }
                w!(self, "{}\n", Self::moveset_move(g, m, s));
            }
            if let Some(eggs) = g.egg_moves.get(&s.number).filter(|e| !e.is_empty()) {
                self.out.push_str("Egg Moves:\n");
                for m in eggs.iter().filter_map(|&id| find_move(g, id)) {
                    w!(self, " - {}\n", Self::moveset_move(g, m, s));
                }
            }
            self.out.push('\n');
        }
        self.separator();
    }

    /// The TM moves.
    fn tms(&mut self) {
        self.section_title(TMS);
        if self.metronome() {
            self.out
                .push_str("Metronome only mode - every TM contains Metronome.\n");
        } else {
            let g = self.input.after;
            for (i, &m) in g.tm_moves.iter().enumerate() {
                w!(self, "TM{:02} {}\n", i + 1, move_name(g, m));
            }
        }
        self.separator();
    }

    /// A compatibility table: by species, then by move.
    fn compatibility(&mut self, compat: &BTreeMap<DexNumber, Vec<bool>>, moves: &[String], by_move: &str) {
        let g = self.input.after;
        let species = |n: DexNumber| format!("#{n:03} {}", species_name(g, n));
        self.out.push_str("--By Species:--\n");
        for (&n, flags) in compat {
            self.out.push_str(&species(n));
            let mut j = 0;
            for (i, m) in moves.iter().enumerate() {
                if flags.get(i).copied().unwrap_or(false) {
                    if j != 0 {
                        self.out.push_str(", ");
                    }
                    if j % COMPAT_ROW_WIDTH == 0 {
                        self.out.push_str("\n\t");
                    }
                    self.out.push_str(m);
                    j += 1;
                }
            }
            self.out.push('\n');
        }
        self.out.push('\n');
        self.out.push_str(by_move);
        for (i, m) in moves.iter().enumerate() {
            self.out.push_str(m);
            let mut j = 0;
            for (&n, flags) in compat {
                if flags.get(i).copied().unwrap_or(false) {
                    if j != 0 {
                        self.out.push_str(", ");
                    }
                    if j % COMPAT_ROW_WIDTH == 0 {
                        self.out.push_str("\n\t");
                    }
                    self.out.push_str(&species(n));
                    j += 1;
                }
            }
            self.out.push('\n');
        }
    }

    /// The TM and HM compatibility.
    fn tm_compat(&mut self) {
        self.section_title(TM_COMPAT);
        let g = self.input.after;
        let s = &self.input.settings.tm_hm_compat;
        if s.full_hm {
            self.out.push_str("All Pokémon can learn all HMs.\n");
        }
        if s.mode == CompatMode::Full {
            self.out.push_str("All Pokémon can learn all TMs/HMs.\n");
        } else if s.mode != CompatMode::Unchanged {
            let mut moves: Vec<String> = g
                .tm_moves
                .iter()
                .enumerate()
                .map(|(i, &m)| format!("TM{:02} {}", i + 1, move_name(g, m)))
                .collect();
            if !s.full_hm {
                moves.extend(
                    g.hm_moves
                        .iter()
                        .enumerate()
                        .map(|(i, &m)| format!("HM{:02} {}", i + 1, move_name(g, m))),
                );
            }
            self.compatibility(&g.tm_hm_compat, &moves, "--By TM/HM:--\n");
        }
        self.separator();
    }

    /// The move tutor moves.
    fn tutors(&mut self) {
        self.section_title(TUTORS);
        if self.metronome() {
            self.out
                .push_str("Metronome only mode - every Move Tutor teaches Metronome.\n");
        } else {
            let (b, g) = (self.input.before, self.input.after);
            for (&old, &new) in b.tutor_moves.iter().zip(&g.tutor_moves) {
                w!(self, "{:<10} -> {:<10}\n", move_name(b, old), move_name(g, new));
            }
        }
        self.separator();
    }

    /// The move tutor compatibility.
    fn tutor_compat(&mut self) {
        self.section_title(TUTOR_COMPAT);
        let g = self.input.after;
        if self.input.settings.tutor_compat.mode == CompatMode::Full {
            self.out.push_str("All Pokémon can learn all Move Tutor moves.\n");
        } else {
            let moves: Vec<String> = g.tutor_moves.iter().map(|&m| move_name(g, m)).collect();
            self.compatibility(&g.tutor_compat, &moves, "--By Tutor Move:--\n");
        }
        self.separator();
    }

    /// Whether a trainer is a boss for the level caps, in the generations ported.
    fn is_level_cap_boss(g: &GameData, t: &Trainer) -> bool {
        let tag = t.tag.as_deref().unwrap_or_default();
        let leader = (tag.starts_with("GYM") && tag.ends_with("-LEADER"))
            || tag.starts_with("ELITE")
            || tag.starts_with("CHAMPION");
        let rival = matches!(g.identity.generation.number(), 1 | 3)
            && (g.identity.generation.number() == 1 || g.identity.family == "FRLG")
            && tag == "RIVAL8-0";
        leader || rival
    }

    /// The trainers' level caps.
    fn level_caps(&mut self) {
        self.section_title(LEVEL_CAPS);
        let g = self.input.after;
        let max_level = |t: &Trainer| t.pokemon.iter().map(|p| p.level).max().unwrap_or(0);
        let mut bosses: Vec<(String, &Trainer)> = Vec::new();
        for t in g.trainers.iter().filter(|t| Self::is_level_cap_boss(g, t)) {
            let name = trainer_name(g, t);
            match bosses.iter_mut().find(|(n, _)| *n == name) {
                Some(entry) => {
                    if max_level(entry.1) > max_level(t) {
                        entry.1 = t;
                    }
                }
                None => bosses.push((name, t)),
            }
        }
        bosses.sort_by_key(|(_, t)| max_level(t));
        for (name, t) in bosses {
            w!(self, "{name} - Level Cap: {}\n", max_level(t));
        }
        self.separator();
    }

    /// The trainers.
    fn trainers(&mut self) {
        self.section_title(TRAINERS);
        let (b, g) = (self.input.before, self.input.after);
        let names_changed = self.changes.trainer_names;
        let styles = self.input.settings.battle_style.mode != BattleStyleMode::Unchanged;
        let abilities = self.abilities_per_species() != 0;
        let mut previous_custom = false;
        for t in &g.trainers {
            let custom = t.pokemon_have_custom_moves();
            if custom && !previous_custom {
                self.out.push('\n');
            }
            previous_custom = custom;
            w!(self, "#{} ", t.index);
            let current = trainer_name(g, t);
            if !current.is_empty() {
                if names_changed {
                    let original = b
                        .trainers
                        .iter()
                        .find(|o| o.index == t.index)
                        .map(|o| trainer_name(b, o))
                        .unwrap_or_default();
                    w!(self, "({original} => {current})");
                } else {
                    w!(self, "({current})");
                }
            }
            if custom {
                self.out.push('\n');
                for p in &t.pokemon {
                    self.out.push_str(&trainer_pokemon(g, p));
                    if abilities {
                        w!(self, ", Ability: {}", ability_name(g, trainer_ability(g, p)));
                    }
                    let moves: Vec<String> = p.moves.iter().filter(|&&m| m != 0).map(|&m| move_name(g, m)).collect();
                    w!(self, " - {}\n", moves.join(", "));
                }
            } else {
                let team: Vec<String> = t.pokemon.iter().map(|p| trainer_pokemon(g, p)).collect();
                w!(self, " - {}", team.join(", "));
            }
            if styles {
                w!(self, " (Battle Style: {:?})", t.battle_style);
            }
            self.out.push('\n');
        }
        self.separator();
    }

    /// The wild Pokémon.
    fn wild(&mut self) {
        self.section_title(WILD);
        let g = self.input.after;
        let areas = g
            .encounters
            .iter()
            .filter(|a| a.kind != ppr_model::EncounterKind::Unused);
        for (i, area) in areas.enumerate() {
            if area.name.is_empty() {
                w!(self, "Area #{} (rate={})\n", i + 1, area.rate);
            } else {
                w!(self, "Area #{} - {} (rate={})\n", i + 1, area.name, area.rate);
            }
            for e in &area.encounters {
                let name = species_name(g, e.species);
                if e.max_level > 0 && e.max_level != e.min_level {
                    w!(self, "{name} Lvs{}-{}\n", e.min_level, e.max_level);
                } else {
                    w!(self, "{name} Lv{}\n", e.min_level);
                }
            }
            self.out.push('\n');
        }
        self.separator();
    }

    /// The shop items.
    fn shops(&mut self) {
        self.section_title(SHOPS);
        let g = self.input.after;
        let s = &self.input.settings.shops;
        if s.add_cheap_rare_candies {
            self.out.push_str("Added cheap Rare Candies to most shops.\n\n");
        }
        if s.mode != ShopItemMode::Unchanged {
            self.out.push_str("--Special Shops:--\n\n");
            for shop in g.shops.iter().filter(|s| s.special) {
                w!(self, "{}\n", shop.name);
                for &item in &shop.items {
                    w!(self, "- {:>5}\n", item_name(g, item));
                }
                self.out.push('\n');
            }
        }
        self.separator();
    }

    /// The Pickup items.
    fn pickup(&mut self) {
        self.section_title(PICKUP);
        let g = self.input.after;
        let chances = pickup_chances(&g.identity.family, g.pickup_items.len());
        for range in 0..10 {
            w!(self, "Level {}-{}\n", range * 10 + 1, (range + 1) * 10);
            let mut by_chance: BTreeMap<u8, Vec<String>> = BTreeMap::new();
            for (&item, chance) in g.pickup_items.iter().zip(&chances) {
                if chance[range] > 0 {
                    by_chance.entry(chance[range]).or_default().push(item_name(g, item));
                }
            }
            for (chance, items) in by_chance.iter().rev() {
                w!(self, "{chance}%: {}\n", items.join(", "));
            }
            self.out.push('\n');
        }
        self.separator();
    }

    /// The type effectiveness chart.
    fn type_chart(&mut self) {
        self.section_title(TYPE_CHART);
        let chart = &self.input.after.type_chart;
        let names: Vec<(Type, String)> = chart.types().iter().map(|&t| (t, type_name(t))).collect();
        let width = names.iter().map(|(_, n)| n.len()).max().unwrap_or(0);
        self.out.push_str(&" ".repeat(width + 1));
        for (_, n) in &names {
            w!(self, "{:<3} ", n.chars().take(3).collect::<String>());
        }
        self.out.push('\n');
        for (attacker, name) in &names {
            w!(self, "{name:<width$}");
            for (defender, _) in &names {
                let e = chart.get(*attacker, *defender).unwrap_or_default();
                let i = match e {
                    Effectiveness::Zero => 0,
                    Effectiveness::Half => 1,
                    Effectiveness::Neutral => 2,
                    Effectiveness::Double => 3,
                };
                w!(self, "|{}", EFFECTIVENESS_SYMBOLS[i]);
            }
            self.out.push_str("|\n");
        }
        self.out.push_str("Attacker (left), Defender (top)\n");
        for i in [2, 3, 1, 0] {
            w!(self, "{} = {}\n", EFFECTIVENESS_SYMBOLS[i], EFFECTIVENESS_NAMES[i]);
        }
        self.separator();
    }
}

#[cfg(test)]
mod tests;
