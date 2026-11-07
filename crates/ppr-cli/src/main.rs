//! `ppr`: command-line interface of Potato Pokémon Randomizer.

use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use ppr_service::{
    AbilityMode, AbilitySettings, AuraMode, BaseStatMode, BaseStatSettings, BatchProgress, BatchRequest, BattleStyle,
    BattleStyleMode, BattleStyleSettings, BstMode, BstSettings, CancelToken, CompatMode, EditList, EvolutionMode,
    EvolutionSettings, ExpCurve, ExpCurveScope, ExpCurveSettings, FieldItemMode, FieldItemSettings, MoveDataSettings,
    MovesetMode, MovesetSettings, PaletteSettings, PickupItemSettings, PlayerCharacter, PlayerGraphicsSettings,
    Progress, RandomizeRequest, RomReport, RomSession, RunReport, Seed, Settings, ShopItemMode, ShopItemSettings,
    StarterMode, StarterSettings, StarterTypes, StaticMode, StaticSettings, TmHmCompatSettings, TmSettings, TotemMode,
    TotemSettings, TradeMode, TradeSettings, TrainerMode, TrainerSettings, TutorCompatSettings, TutorSettings, Type,
    TypeEffectivenessMode, TypeEffectivenessSettings, TypeMode, TypeSettings, WildEvolution, WildMode, WildSettings,
    WildTypes, WildZones, identify_file, load_custom_names, load_premade_seed, load_rom, load_settings_file,
    randomize_batch, randomize_file, read_settings_string, save_log, supported_games,
};

#[derive(Parser)]
#[command(name = "ppr", version, about = "Potato Pokémon Randomizer")]
struct Cli {
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Identify ROM files.
    Identify {
        /// ROM files to check.
        #[arg(required = true)]
        files: Vec<PathBuf>,
    },
    /// List the supported games.
    Games,
    /// Randomize a ROM (Gen 1 to 7).
    Randomize(Box<RandomizeArgs>),
    /// Randomize a ROM many times, each with its own seed.
    Batch(BatchArgs),
    /// List the custom player graphics packs a ROM can use, for `randomize --player-graphics`.
    PlayerGraphics {
        /// The ROM.
        rom: PathBuf,
    },
    /// Print the objects of a ROM that manual edits can change, as JSON, for writing a `randomize --edits` file.
    EditView {
        /// The ROM.
        rom: PathBuf,
        /// Lay this decrypted 3DS game update (`.cia`) over the game first, as `randomize --update` does.
        #[arg(long, value_name = "FILE")]
        update: Option<PathBuf>,
    },
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("settings").required(true).args(["settings_string", "settings_file"])))]
struct BatchArgs {
    /// The ROM to randomize; it is not modified.
    rom: PathBuf,
    /// The folder the ROMs go to; it must exist.
    #[arg(short, long)]
    dir: PathBuf,
    /// Lay this decrypted 3DS game update (`.cia`) over the game first; each game is then a `<prefix><index>`
    /// folder with a LayeredFS folder.
    #[arg(short, long, value_name = "CIA")]
    update: Option<PathBuf>,
    /// Save each 3DS game as a `<prefix><index>` folder with a LayeredFS folder instead of a ROM, also without
    /// `--update`.
    #[arg(long)]
    layered_fs: bool,
    /// How many ROMs to make.
    #[arg(short = 'n', long, value_parser = clap::value_parser!(u32).range(1..))]
    count: u32,
    /// The index of the first ROM.
    #[arg(long, default_value_t = 0)]
    start: u32,
    /// The start of every file name: `<prefix><index>.<extension>`.
    #[arg(long, default_value = "random")]
    prefix: String,
    /// Save a log next to each ROM, with this extension.
    #[arg(long, num_args = 0..=1, default_missing_value = "log", value_name = "ENDING")]
    logs: Option<String>,
    /// Pick trainer names, class names and nicknames from the lists in this folder, as `randomize` does.
    #[arg(long, value_name = "DIR")]
    custom_names: Option<PathBuf>,
    #[arg(long, help = "The settings, as a UPR FVX settings string.")]
    settings_string: Option<String>,
    #[arg(long, help = "The settings, from a UPR FVX `.rnqs` settings file.")]
    settings_file: Option<PathBuf>,
}

#[derive(Args)]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct RandomizeArgs {
    /// The ROM to randomize; it is not modified.
    rom: PathBuf,
    /// Where to write the randomized ROM; with `--layered-fs` or `--update`, the folder the LayeredFS folder goes
    /// into.
    #[arg(short, long)]
    output: PathBuf,
    /// Save the 3DS game as a LayeredFS folder instead of a ROM: only the changed files, in a folder named after the
    /// game's title ID, to copy to the console's or emulator's mod folder.
    #[arg(short = 'd', long)]
    layered_fs: bool,
    /// Lay this decrypted 3DS game update (`.cia`) over the game first. The randomized game is then always a
    /// LayeredFS folder, as with `--layered-fs`.
    #[arg(short, long, value_name = "CIA")]
    update: Option<PathBuf>,
    /// Seed, to reproduce a run. Random when left out.
    #[arg(long)]
    seed: Option<Seed>,
    /// Also save a log of everything that changed next to the output, with the extension `.log`. Leave it off
    /// when racing.
    #[arg(long)]
    log: bool,
    /// Pick trainer names, class names and nicknames from the lists in this folder
    /// (`TrainerNames.txt` and so on); the built-in lists otherwise.
    #[arg(long, value_name = "DIR")]
    custom_names: Option<PathBuf>,
    /// Apply the manual edits in this JSON file after the randomizers. They must be for this ROM's game; the run
    /// fails, writing nothing, when one doesn't fit.
    #[arg(long, value_name = "FILE")]
    edits: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with_all = ["seed", "settings_string", "settings_file"],
        help = "Use the seed and settings of a UPR FVX premade seed file (`.rndp`) instead of `--seed` and the option flags."
    )]
    premade_seed: Option<PathBuf>,
    #[arg(
        long,
        conflicts_with = "settings_file",
        help = "Use the settings of a UPR FVX settings string instead of the option flags."
    )]
    settings_string: Option<String>,
    #[arg(
        long,
        help = "Use the settings of a UPR FVX `.rnqs` settings file instead of the option flags."
    )]
    settings_file: Option<PathBuf>,
    /// The game's intro shows a random Pokémon (on by default in the app).
    #[arg(long)]
    random_intro_pokemon: bool,
    /// Wild and trainer Pokémon are never evolved beyond what their level allows.
    #[arg(long)]
    no_premature_evolutions: bool,
    /// Alternate formes that only last in battle or under some condition are never chosen.
    #[arg(long)]
    no_irregular_alt_formes: bool,
    /// Limit Pokémon: only these generations (1–7) appear, e.g. `1,2`.
    #[arg(long, value_delimiter = ',', value_parser = clap::value_parser!(u8).range(1..=7))]
    limit_generations: Vec<u8>,
    /// With `--limit-generations`, leave out the evolutionary relatives of other generations.
    #[arg(long, requires = "limit_generations")]
    no_evolutionary_relatives: bool,
    /// Misc tweaks by ID, applied where the game supports them, e.g. `fastestText,reusableTMs`.
    #[arg(long, value_delimiter = ',', value_parser = parse_tweak)]
    tweaks: Vec<ppr_service::MiscTweak>,
    /// How to choose starters.
    #[arg(long, value_enum, default_value_t = StarterModeArg::Unchanged)]
    starters: StarterModeArg,
    /// Custom starters by national number, 0 for random, e.g. `--custom 133,0,0`. Implies `--starters custom`.
    #[arg(long, value_delimiter = ',')]
    custom: Vec<u16>,
    /// Type rule for random starters.
    #[arg(long, value_enum, default_value_t = StarterTypesArg::None)]
    starter_types: StarterTypesArg,
    /// The type for `--starter-types single-type`, e.g. `fire`. Random when left out.
    #[arg(long)]
    starter_type: Option<Type>,
    /// No legendary starters.
    #[arg(long)]
    no_legendaries: bool,
    /// No dual-type starters.
    #[arg(long)]
    no_dual_types: bool,
    /// Minimum base stat total of random starters.
    #[arg(long, default_value_t = 0)]
    bst_min: u16,
    /// Maximum base stat total of random starters (0 for none).
    #[arg(long, default_value_t = 0)]
    bst_max: u16,
    /// Give the starters a random held item.
    #[arg(long)]
    starter_held_items: bool,
    /// With `--starter-held-items`, leave out items that are useless to hold.
    #[arg(long, requires = "starter_held_items")]
    starter_ban_bad_items: bool,
    /// Starters may be alternate formes, where the game's starters have a forme.
    #[arg(long)]
    starter_alt_formes: bool,
    #[command(flatten)]
    base_stats: BaseStatArgs,
    #[command(flatten)]
    bsts: BstArgs,
    #[command(flatten)]
    types: TypeArgs,
    #[command(flatten)]
    abilities: AbilityArgs,
    #[command(flatten)]
    exp_curves: ExpCurveArgs,
    #[command(flatten)]
    evolutions: EvolutionArgs,
    #[command(flatten)]
    move_data: MoveDataArgs,
    #[command(flatten)]
    movesets: MovesetArgs,
    #[command(flatten)]
    type_effectiveness: TypeEffectivenessArgs,
    #[command(flatten)]
    tms: TmArgs,
    #[command(flatten)]
    tm_hm_compat: TmHmCompatArgs,
    #[command(flatten)]
    tutors: TutorArgs,
    #[command(flatten)]
    tutor_compat: TutorCompatArgs,
    #[command(flatten)]
    wild: WildArgs,
    #[command(flatten)]
    trainers: TrainerArgs,
    #[command(flatten)]
    battle_style: BattleStyleArgs,
    #[command(flatten)]
    statics: StaticArgs,
    #[command(flatten)]
    totems: TotemArgs,
    #[command(flatten)]
    trades: TradeArgs,
    #[command(flatten)]
    field_items: FieldItemArgs,
    #[command(flatten)]
    shops: ShopArgs,
    #[command(flatten)]
    pickup: PickupArgs,
    #[command(flatten)]
    palettes: PaletteArgs,
    #[command(flatten)]
    player_graphics: PlayerGraphicsArgs,
}

#[derive(Args)]
#[command(next_help_heading = "Evolutions")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct EvolutionArgs {
    /// Randomize evolutions: each leads to a random species, or every species evolves at every level.
    #[arg(long, value_enum, default_value = "unchanged")]
    evolutions: EvolutionModeArg,
    /// Random evolutions lead to species about as strong as the old ones.
    #[arg(long)]
    evolutions_similar_strength: bool,
    /// Random evolutions share a type with the species that evolves.
    #[arg(long)]
    evolutions_same_typing: bool,
    /// Random evolution lines have at most three stages.
    #[arg(long)]
    evolutions_max_three_stages: bool,
    /// Random evolutions never lead to an old one.
    #[arg(long)]
    evolutions_force_change: bool,
    /// Random evolutions have a higher base stat total.
    #[arg(long)]
    evolutions_force_growth: bool,
    /// No two species evolve into the same one.
    #[arg(long)]
    evolutions_no_convergence: bool,
    /// Evolutions at a level happen at a level fitting the new base stat totals.
    #[arg(long)]
    adjust_evolution_levels: bool,
    /// Evolutions that can't happen in the game alone, like trade evolutions, happen by level or stone.
    #[arg(long)]
    change_impossible_evolutions: bool,
    /// Evolutions happen at this level at the latest, and need less happiness.
    #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(u8).range(30..=55))]
    easier_evolutions: Option<u8>,
    /// Evolutions changed into ones at a level use an estimated level instead of a fixed one.
    #[arg(long)]
    estimated_evolution_levels: bool,
    /// Evolutions by time of day happen at any time.
    #[arg(long)]
    remove_time_based_evolutions: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum EvolutionModeArg {
    Unchanged,
    Random,
    RandomEveryLevel,
}

impl EvolutionArgs {
    fn settings(&self) -> Result<EvolutionSettings> {
        let mode = match self.evolutions {
            EvolutionModeArg::Unchanged => EvolutionMode::Unchanged,
            EvolutionModeArg::Random => EvolutionMode::Random,
            EvolutionModeArg::RandomEveryLevel => EvolutionMode::RandomEveryLevel,
        };
        let random = mode != EvolutionMode::Unchanged;
        let only_random = [
            self.evolutions_similar_strength,
            self.evolutions_max_three_stages,
            self.evolutions_force_growth,
        ];
        if only_random.iter().any(|&on| on) && mode != EvolutionMode::Random {
            anyhow::bail!(
                "--evolutions-similar-strength, --evolutions-max-three-stages and --evolutions-force-growth need \
                 --evolutions random"
            );
        }
        let random_options = [
            self.evolutions_same_typing,
            self.evolutions_force_change,
            self.evolutions_no_convergence,
        ];
        if random_options.iter().any(|&on| on) && !random {
            anyhow::bail!(
                "--evolutions-same-typing, --evolutions-force-change and --evolutions-no-convergence need --evolutions"
            );
        }
        Ok(EvolutionSettings {
            mode,
            similar_strength: self.evolutions_similar_strength,
            same_typing: self.evolutions_same_typing,
            limit_three_stages: self.evolutions_max_three_stages,
            force_change: self.evolutions_force_change,
            force_growth: self.evolutions_force_growth,
            no_convergence: self.evolutions_no_convergence,
            // Only decides whether Follow Mega Evolutions can be used with every level, which the app offers.
            allow_alt_formes: false,
            adjust_levels: self.adjust_evolution_levels,
            change_impossible: self.change_impossible_evolutions,
            easier_max_level: self.easier_evolutions,
            use_estimated_levels: self.estimated_evolution_levels,
            remove_time_based: self.remove_time_based_evolutions,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "TM moves")]
struct TmArgs {
    /// TMs teach random moves.
    #[arg(long)]
    random_tms: bool,
    /// With `--random-tms`, TMs of moves used outside battle, like Dig, keep them.
    #[arg(long, requires = "random_tms")]
    tms_keep_field_moves: bool,
    /// With `--random-tms`, no Sonic Boom or Dragon Rage.
    #[arg(long, requires = "random_tms")]
    tms_no_game_breaking_moves: bool,
    /// With `--random-tms`, at least this share of the TMs, in percent, teach good damaging moves.
    #[arg(long, value_name = "PERCENT", requires = "random_tms", value_parser = clap::value_parser!(u8).range(0..=100))]
    tms_good_damaging_moves: Option<u8>,
}

impl TmArgs {
    fn settings(&self) -> TmSettings {
        TmSettings {
            random: self.random_tms,
            keep_field_moves: self.tms_keep_field_moves,
            no_game_breaking: self.tms_no_game_breaking_moves,
            good_damaging_percent: self.tms_good_damaging_moves,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "TM/HM compatibility")]
struct TmHmCompatArgs {
    /// Change which species can learn the TMs and HMs: at random preferring their own types, at random, or all.
    #[arg(long, value_enum, default_value_t = CompatModeArg::Unchanged)]
    tm_compat: CompatModeArg,
    /// Species can learn the TMs of the moves they learn by level.
    #[arg(long)]
    tm_level_up_sanity: bool,
    /// Evolutions can learn the TMs their pre-evolutions can; random compatibility mostly carries over.
    #[arg(long)]
    tm_follow_evolutions: bool,
    /// Every species can learn every HM.
    #[arg(long)]
    full_hm_compat: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum CompatModeArg {
    Unchanged,
    RandomSameType,
    Random,
    Full,
}

impl CompatModeArg {
    fn mode(self) -> CompatMode {
        match self {
            Self::Unchanged => CompatMode::Unchanged,
            Self::RandomSameType => CompatMode::RandomPreferSameType,
            Self::Random => CompatMode::RandomCompletely,
            Self::Full => CompatMode::Full,
        }
    }
}

impl TmHmCompatArgs {
    fn settings(&self) -> Result<TmHmCompatSettings> {
        if self.tm_follow_evolutions && self.tm_compat == CompatModeArg::Unchanged && !self.tm_level_up_sanity {
            anyhow::bail!("--tm-follow-evolutions needs --tm-compat or --tm-level-up-sanity");
        }
        if self.full_hm_compat && self.tm_compat == CompatModeArg::Full {
            anyhow::bail!("--full-hm-compat is part of --tm-compat full");
        }
        Ok(TmHmCompatSettings {
            mode: self.tm_compat.mode(),
            level_up_sanity: self.tm_level_up_sanity,
            follow_evolutions: self.tm_follow_evolutions,
            full_hm: self.full_hm_compat,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Type effectiveness")]
struct TypeEffectivenessArgs {
    /// Change the type chart: random, random with no type stronger or weaker than before, keeping each type's
    /// strengths and weaknesses overall, or inverted.
    #[arg(long, value_enum, default_value_t = TypeEffectivenessArg::Unchanged)]
    type_effectiveness: TypeEffectivenessArg,
    /// With `--type-effectiveness inverse`, some former weaknesses become immunities.
    #[arg(long)]
    inverse_random_immunities: bool,
    /// Give the type chart the changes of Gen 6 first.
    #[arg(long)]
    update_type_effectiveness: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TypeEffectivenessArg {
    Unchanged,
    Random,
    RandomBalanced,
    KeepIdentities,
    Inverse,
}

impl TypeEffectivenessArgs {
    fn settings(&self) -> Result<TypeEffectivenessSettings> {
        if self.inverse_random_immunities && self.type_effectiveness != TypeEffectivenessArg::Inverse {
            anyhow::bail!("--inverse-random-immunities needs --type-effectiveness inverse");
        }
        Ok(TypeEffectivenessSettings {
            mode: match self.type_effectiveness {
                TypeEffectivenessArg::Unchanged => TypeEffectivenessMode::Unchanged,
                TypeEffectivenessArg::Random => TypeEffectivenessMode::Random,
                TypeEffectivenessArg::RandomBalanced => TypeEffectivenessMode::RandomBalanced,
                TypeEffectivenessArg::KeepIdentities => TypeEffectivenessMode::KeepIdentities,
                TypeEffectivenessArg::Inverse => TypeEffectivenessMode::Inverse,
            },
            random_immunities: self.inverse_random_immunities,
            update: self.update_type_effectiveness,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Move tutor moves")]
struct TutorArgs {
    /// Move tutors teach random moves that no TM teaches.
    #[arg(long)]
    random_tutors: bool,
    /// With `--random-tutors`, tutors of moves used outside battle keep them.
    #[arg(long, requires = "random_tutors")]
    tutors_keep_field_moves: bool,
    /// With `--random-tutors`, no Sonic Boom or Dragon Rage.
    #[arg(long, requires = "random_tutors")]
    tutors_no_game_breaking_moves: bool,
    /// With `--random-tutors`, at least this share of the tutors, in percent, teach good damaging moves.
    #[arg(long, value_name = "PERCENT", requires = "random_tutors", value_parser = clap::value_parser!(u8).range(0..=100))]
    tutors_good_damaging_moves: Option<u8>,
}

impl TutorArgs {
    fn settings(&self) -> TutorSettings {
        TutorSettings {
            random: self.random_tutors,
            keep_field_moves: self.tutors_keep_field_moves,
            no_game_breaking: self.tutors_no_game_breaking_moves,
            good_damaging_percent: self.tutors_good_damaging_moves,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Move tutor compatibility")]
#[expect(clippy::struct_field_names, reason = "the field names are the flags")]
struct TutorCompatArgs {
    /// Change which species can learn the move tutor moves: at random preferring their own types, at random, or
    /// all.
    #[arg(long, value_enum, default_value_t = CompatModeArg::Unchanged)]
    tutor_compat: CompatModeArg,
    /// Species can learn the tutor moves they learn by level.
    #[arg(long)]
    tutor_level_up_sanity: bool,
    /// Evolutions can learn the tutor moves their pre-evolutions can; random compatibility mostly carries over.
    #[arg(long)]
    tutor_follow_evolutions: bool,
}

impl TutorCompatArgs {
    fn settings(&self) -> Result<TutorCompatSettings> {
        if self.tutor_follow_evolutions && self.tutor_compat == CompatModeArg::Unchanged && !self.tutor_level_up_sanity
        {
            anyhow::bail!("--tutor-follow-evolutions needs --tutor-compat or --tutor-level-up-sanity");
        }
        Ok(TutorCompatSettings {
            mode: self.tutor_compat.mode(),
            level_up_sanity: self.tutor_level_up_sanity,
            follow_evolutions: self.tutor_follow_evolutions,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Movesets")]
struct MovesetArgs {
    /// Randomize the moves species learn by level and as eggs: preferring their own types, or any.
    #[arg(long, value_enum, default_value_t = MovesetModeArg::Unchanged)]
    movesets: MovesetModeArg,
    /// Random movesets start with this many moves at level 1.
    #[arg(long, value_name = "COUNT", value_parser = clap::value_parser!(u8).range(2..=4))]
    guaranteed_level_one_moves: Option<u8>,
    /// Random movesets learn their damaging moves from weakest to strongest.
    #[arg(long)]
    reorder_damaging_moves: bool,
    /// Random movesets leave out Sonic Boom and Dragon Rage.
    #[arg(long)]
    no_game_breaking_moves: bool,
    /// At least this share of random movesets, in percent, are good damaging moves.
    #[arg(long, value_name = "PERCENT", value_parser = clap::value_parser!(u8).range(0..=100))]
    good_damaging_moves: Option<u8>,
}

#[derive(Clone, Copy, ValueEnum)]
enum MovesetModeArg {
    Unchanged,
    RandomSameType,
    Random,
    /// Every species learns only Metronome, and every TM and tutor teaches it.
    MetronomeOnly,
}

impl MovesetArgs {
    fn settings(&self) -> Result<MovesetSettings> {
        let mode = match self.movesets {
            MovesetModeArg::Unchanged => MovesetMode::Unchanged,
            MovesetModeArg::RandomSameType => MovesetMode::RandomPreferSameType,
            MovesetModeArg::Random => MovesetMode::RandomCompletely,
            MovesetModeArg::MetronomeOnly => MovesetMode::MetronomeOnly,
        };
        let options = self.guaranteed_level_one_moves.is_some()
            || self.reorder_damaging_moves
            || self.no_game_breaking_moves
            || self.good_damaging_moves.is_some();
        if options && mode == MovesetMode::Unchanged {
            anyhow::bail!(
                "--guaranteed-level-one-moves, --reorder-damaging-moves, --no-game-breaking-moves and \
                 --good-damaging-moves need --movesets"
            );
        }
        Ok(MovesetSettings {
            mode,
            guaranteed_level_one_moves: self.guaranteed_level_one_moves,
            reorder_damaging: self.reorder_damaging_moves,
            no_game_breaking: self.no_game_breaking_moves,
            good_damaging_percent: self.good_damaging_moves,
            evolution_moves_for_all: false,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Move data")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct MoveDataArgs {
    /// Damaging moves get a random power.
    #[arg(long)]
    random_move_power: bool,
    /// Moves that can miss get a random accuracy; very inaccurate ones stay inaccurate.
    #[arg(long)]
    random_move_accuracy: bool,
    /// Moves get random PP.
    #[arg(long)]
    random_move_pp: bool,
    /// Moves get a random type.
    #[arg(long)]
    random_move_types: bool,
    /// Physical and special moves swap at random, in games with the physical/special split.
    #[arg(long)]
    random_move_categories: bool,
    /// Moves get random names, made of a word for their type and one for what they do.
    #[arg(long)]
    random_move_names: bool,
    /// Moves get their power, accuracy, PP, type and category of this later generation.
    #[arg(long, value_name = "GENERATION", value_parser = clap::value_parser!(u8).range(2..=9))]
    update_moves: Option<u8>,
}

impl MoveDataArgs {
    fn settings(&self) -> MoveDataSettings {
        MoveDataSettings {
            power: self.random_move_power,
            accuracy: self.random_move_accuracy,
            pp: self.random_move_pp,
            types: self.random_move_types,
            category: self.random_move_categories,
            names: self.random_move_names,
            update_to_generation: self.update_moves,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "EXP curves")]
struct ExpCurveArgs {
    /// Give species this EXP curve.
    #[arg(long, value_enum)]
    exp_curve: Option<ExpCurveArg>,
    /// Which species get it; the others of the legendaries get Slow.
    #[arg(long, value_enum, default_value = "legendaries-slow", requires = "exp_curve")]
    exp_curve_for: ExpCurveScopeArg,
}

#[derive(Clone, Copy, ValueEnum)]
enum ExpCurveArg {
    MediumFast,
    Erratic,
    Fluctuating,
    MediumSlow,
    Fast,
    Slow,
}

#[derive(Clone, Copy, ValueEnum)]
enum ExpCurveScopeArg {
    /// All but the legendaries, which get Slow.
    LegendariesSlow,
    /// All but the strongest legendaries, which get Slow.
    StrongLegendariesSlow,
    /// Every species.
    All,
}

impl ExpCurveArgs {
    fn settings(&self) -> ExpCurveSettings {
        ExpCurveSettings {
            standardize: self.exp_curve.map(|c| match c {
                ExpCurveArg::MediumFast => ExpCurve::MediumFast,
                ExpCurveArg::Erratic => ExpCurve::Erratic,
                ExpCurveArg::Fluctuating => ExpCurve::Fluctuating,
                ExpCurveArg::MediumSlow => ExpCurve::MediumSlow,
                ExpCurveArg::Fast => ExpCurve::Fast,
                ExpCurveArg::Slow => ExpCurve::Slow,
            }),
            apply_to: match self.exp_curve_for {
                ExpCurveScopeArg::LegendariesSlow => ExpCurveScope::Legendaries,
                ExpCurveScopeArg::StrongLegendariesSlow => ExpCurveScope::StrongLegendaries,
                ExpCurveScopeArg::All => ExpCurveScope::All,
            },
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Abilities")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct AbilityArgs {
    /// Give species random abilities.
    #[arg(long)]
    abilities: bool,
    /// Wonder Guard may be given out.
    #[arg(long, requires = "abilities")]
    allow_wonder_guard: bool,
    /// Evolutions keep their pre-evolution's abilities.
    #[arg(long, requires = "abilities")]
    abilities_follow_evolutions: bool,
    /// With `--abilities-follow-evolutions`, Mega Evolutions keep their base species' abilities.
    #[arg(long, requires = "abilities")]
    abilities_follow_megas: bool,
    /// Abilities that do the same count as one.
    #[arg(long, requires = "abilities")]
    combine_duplicate_abilities: bool,
    /// Every species gets two abilities.
    #[arg(long, requires = "abilities")]
    ensure_two_abilities: bool,
    /// Leave out trapping abilities.
    #[arg(long, requires = "abilities")]
    ban_trapping_abilities: bool,
    /// Leave out abilities that hurt their holder.
    #[arg(long, requires = "abilities")]
    ban_negative_abilities: bool,
    /// Leave out abilities that hardly do anything.
    #[arg(long, requires = "abilities")]
    ban_bad_abilities: bool,
}

impl AbilityArgs {
    fn settings(&self) -> AbilitySettings {
        AbilitySettings {
            mode: if self.abilities {
                AbilityMode::Random
            } else {
                AbilityMode::Unchanged
            },
            allow_wonder_guard: self.allow_wonder_guard,
            follow_evolutions: self.abilities_follow_evolutions,
            follow_megas: self.abilities_follow_megas,
            combine_duplicates: self.combine_duplicate_abilities,
            ensure_two: self.ensure_two_abilities,
            ban_trapping: self.ban_trapping_abilities,
            ban_negative: self.ban_negative_abilities,
            ban_bad: self.ban_bad_abilities,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Types")]
struct TypeArgs {
    /// Randomize species' types: evolutions keeping theirs, or each species on its own.
    #[arg(long, value_enum)]
    types: Option<TypeModeArg>,
    /// Give every species two types.
    #[arg(long, requires = "types")]
    types_force_dual: bool,
    /// With `--types follow-evolutions`, Mega Evolutions keep their base species' types.
    #[arg(long, requires = "types")]
    types_follow_megas: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum TypeModeArg {
    /// Evolutions keep their pre-evolution's types, sometimes adding a second.
    FollowEvolutions,
    /// Every species on its own.
    Random,
}

impl TypeArgs {
    fn settings(&self) -> TypeSettings {
        TypeSettings {
            mode: match self.types {
                None => TypeMode::Unchanged,
                Some(TypeModeArg::FollowEvolutions) => TypeMode::FollowEvolutions,
                Some(TypeModeArg::Random) => TypeMode::Random,
            },
            force_dual: self.types_force_dual,
            follow_megas: self.types_follow_megas,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Base stat totals")]
struct BstArgs {
    /// Randomize base stat totals; the stats keep their proportions.
    #[arg(long, value_enum)]
    bst: Option<BstModeArg>,
    /// With `--bst buff-nerf`, the largest change in percent.
    #[arg(long, default_value_t = 0, value_parser = clap::value_parser!(u8).range(0..=50))]
    bst_buff_nerf: u8,
    /// With `--bst buff-nerf` or `shuffle`, evolution lines change together.
    #[arg(long, requires = "bst")]
    bst_follow_evolutions: bool,
    /// With `--bst shuffle`, legendaries swap totals only among themselves.
    #[arg(long, requires = "bst")]
    bst_separate_legendaries: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum BstModeArg {
    /// Raise or lower each total by up to `--bst-buff-nerf` percent.
    BuffNerf,
    /// Give each species another's total.
    Shuffle,
    /// Give each species a random total.
    Random,
}

impl BstArgs {
    fn settings(&self) -> BstSettings {
        let mode = match self.bst {
            None => BstMode::Unchanged,
            Some(BstModeArg::BuffNerf) => BstMode::BuffNerf,
            Some(BstModeArg::Shuffle) => BstMode::Shuffle,
            Some(BstModeArg::Random) => BstMode::Random,
        };
        BstSettings {
            mode,
            max_buff_nerf_percent: self.bst_buff_nerf,
            follow_evolutions: self.bst_follow_evolutions && matches!(mode, BstMode::BuffNerf | BstMode::Shuffle),
            separate_legendaries: self.bst_separate_legendaries && mode == BstMode::Shuffle,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Base statistics")]
struct BaseStatArgs {
    /// Randomize base stats: shuffle each species' stats, or spread its total at random.
    #[arg(long, value_enum)]
    base_stats: Option<BaseStatModeArg>,
    /// Evolutions follow their pre-evolution's stat order or spread.
    #[arg(long, requires = "base_stats")]
    base_stats_follow_evolutions: bool,
    /// With `--base-stats-follow-evolutions`, Mega Evolutions follow their base species' stat order or spread.
    #[arg(long, requires = "base_stats")]
    base_stats_follow_megas: bool,
    /// With `--base-stats random`, spread the stats an evolution adds at random.
    #[arg(long, requires = "base_stats_follow_evolutions")]
    base_stats_random_evolution_stats: bool,
    /// First give species the base stats of a later generation, where it changed them.
    #[arg(long, value_name = "GEN", value_parser = clap::value_parser!(u8).range(6..=9))]
    update_base_stats: Option<u8>,
}

#[derive(Clone, Copy, ValueEnum)]
enum BaseStatModeArg {
    /// Shuffle each species' stats; the total stays.
    Shuffle,
    /// Spread each species' total at random.
    Random,
}

impl BaseStatArgs {
    fn settings(&self) -> BaseStatSettings {
        BaseStatSettings {
            mode: match self.base_stats {
                None => BaseStatMode::Unchanged,
                Some(BaseStatModeArg::Shuffle) => BaseStatMode::Shuffle,
                Some(BaseStatModeArg::Random) => BaseStatMode::Random,
            },
            follow_evolutions: self.base_stats_follow_evolutions,
            follow_megas: self.base_stats_follow_megas,
            random_evolution_stats: self.base_stats_random_evolution_stats,
            update_to_generation: self.update_base_stats,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Field items")]
struct FieldItemArgs {
    /// Change the items on the ground and hidden items: shuffled, random, or random with every item about as often.
    #[arg(long, value_enum, default_value_t = FieldItemModeArg::Unchanged)]
    field_items: FieldItemModeArg,
    /// Random field items are never bad items.
    #[arg(long)]
    field_items_ban_bad: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum FieldItemModeArg {
    Unchanged,
    Shuffle,
    Random,
    RandomEven,
}

impl FieldItemArgs {
    fn settings(&self) -> Result<FieldItemSettings> {
        let mode = match self.field_items {
            FieldItemModeArg::Unchanged => FieldItemMode::Unchanged,
            FieldItemModeArg::Shuffle => FieldItemMode::Shuffle,
            FieldItemModeArg::Random => FieldItemMode::Random,
            FieldItemModeArg::RandomEven => FieldItemMode::RandomEven,
        };
        if self.field_items_ban_bad && !matches!(mode, FieldItemMode::Random | FieldItemMode::RandomEven) {
            anyhow::bail!("--field-items-ban-bad needs --field-items random or random-even");
        }
        Ok(FieldItemSettings {
            mode,
            ban_bad: self.field_items_ban_bad,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Shop items")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct ShopArgs {
    /// Change what special shops sell: their items shuffled among them, or random items.
    #[arg(long, value_enum, default_value_t = ShopItemModeArg::Unchanged)]
    shop_items: ShopItemModeArg,
    /// Random shops sell no Rare Candies, Lucky Eggs or items that sell for a lot.
    #[arg(long)]
    shops_ban_overpowered: bool,
    /// Random shops sell no bad items.
    #[arg(long)]
    shops_ban_bad: bool,
    /// Random shops sell none of the usual Poké Mart goods.
    #[arg(long)]
    shops_ban_regular: bool,
    /// Random shops sell every evolution item.
    #[arg(long)]
    shops_guarantee_evolution_items: bool,
    /// Random shops sell every X item, Guard Spec. and Dire Hit.
    #[arg(long)]
    shops_guarantee_x_items: bool,
    /// Items whose price does not fit their use get a better one.
    #[arg(long)]
    balance_shop_prices: bool,
    /// Every shop sells Rare Candies, for 10 each.
    #[arg(long)]
    cheap_rare_candies: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ShopItemModeArg {
    Unchanged,
    Shuffle,
    Random,
}

impl ShopArgs {
    fn settings(&self) -> Result<ShopItemSettings> {
        let random_options = self.shops_ban_overpowered
            || self.shops_ban_bad
            || self.shops_ban_regular
            || self.shops_guarantee_evolution_items
            || self.shops_guarantee_x_items;
        if random_options && self.shop_items != ShopItemModeArg::Random {
            anyhow::bail!("the --shops-ban-* and --shops-guarantee-* flags need --shop-items random");
        }
        Ok(ShopItemSettings {
            mode: match self.shop_items {
                ShopItemModeArg::Unchanged => ShopItemMode::Unchanged,
                ShopItemModeArg::Shuffle => ShopItemMode::Shuffle,
                ShopItemModeArg::Random => ShopItemMode::Random,
            },
            ban_overpowered: self.shops_ban_overpowered,
            ban_bad: self.shops_ban_bad,
            ban_regular: self.shops_ban_regular,
            guarantee_evolution_items: self.shops_guarantee_evolution_items,
            guarantee_x_items: self.shops_guarantee_x_items,
            balance_prices: self.balance_shop_prices,
            add_cheap_rare_candies: self.cheap_rare_candies,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Trainer Pokémon")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct TrainerArgs {
    /// How trainers' Pokémon are chosen.
    #[arg(long, value_enum, default_value_t = TrainerModeArg::Unchanged)]
    trainers: TrainerModeArg,
    /// Rivals and friends use the starter the player didn't pick, evolving through the game.
    #[arg(long)]
    rival_carries_starter: bool,
    /// Replacements are about as strong as what they replace.
    #[arg(long)]
    trainers_similar_strength: bool,
    /// Replacements may be alternate formes, where trainers' Pokémon have a forme.
    #[arg(long)]
    trainers_alt_formes: bool,
    /// Trainers' Pokémon that hold their Mega Stone become species that can Mega Evolve, holding a Mega Stone of
    /// theirs (Gen 6 and 7).
    #[arg(long)]
    trainers_swap_mega_evos: bool,
    /// A trainer has no species twice while others are left.
    #[arg(long)]
    trainers_avoid_duplicates: bool,
    /// With `--trainers type-themed`, types are picked by how many species have them.
    #[arg(long)]
    trainers_weight_types: bool,
    /// No legendaries for trainers.
    #[arg(long)]
    trainers_no_legendaries: bool,
    /// No Wonder Guard below level 20.
    #[arg(long)]
    trainers_no_early_wonder_guard: bool,
    /// Only species met in the wild in the main game, and their families.
    #[arg(long)]
    trainers_local_pokemon: bool,
    /// Each chosen trainer Pokémon is shiny with a chance of 1 in 256 (Gen 7).
    #[arg(long)]
    random_shiny_trainer_pokemon: bool,
    /// The Elite Four's and Champion's strongest 1 or 2 Pokémon are unique to them.
    #[arg(long)]
    elite_four_unique: Option<u8>,
    /// Pokémon added to the teams of boss, important and regular trainers, each 0 to 5.
    #[arg(long, num_args = 3, value_names = ["BOSS", "IMPORTANT", "REGULAR"])]
    additional_trainer_pokemon: Option<Vec<u8>>,
    /// No two Pokémon of a team share a type, for boss, important and regular trainers (`true` or `false` each).
    #[arg(long, num_args = 3, value_names = ["BOSS", "IMPORTANT", "REGULAR"])]
    diverse_trainer_types: Option<Vec<bool>>,
    /// Pokémon of boss, important and regular trainers hold random items (`true` or `false` each).
    #[arg(long, num_args = 3, value_names = ["BOSS", "IMPORTANT", "REGULAR"])]
    trainer_held_items: Option<Vec<bool>>,
    /// Trainer Pokémon only get items used up in battle, like berries.
    #[arg(long)]
    consumable_trainer_items: bool,
    /// Trainer Pokémon only get items that suit them and their moves.
    #[arg(long)]
    sensible_trainer_items: bool,
    /// Only each team's highest-level Pokémon gets an item.
    #[arg(long)]
    highest_level_gets_item: bool,
    /// Pokémon of boss, important and regular trainers get better movesets (`true` or `false` each).
    #[arg(long, num_args = 3, value_names = ["BOSS", "IMPORTANT", "REGULAR"])]
    better_trainer_movesets: Option<Vec<bool>>,
    /// Trainers get random names.
    #[arg(long)]
    random_trainer_names: bool,
    /// Trainer classes get random names.
    #[arg(long)]
    random_trainer_class_names: bool,
    /// Change trainer Pokémon levels by this percentage, -100 to 155.
    #[arg(long, allow_hyphen_values = true)]
    trainer_level_modifier: Option<i16>,
    /// Trainers' Pokémon evolve as far as their level allows, at evolution levels changed by this percentage
    /// (0 for the plain levels).
    #[arg(long, allow_hyphen_values = true)]
    trainers_evolve: Option<i16>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum TrainerModeArg {
    Unchanged,
    Random,
    RandomEvenDistribution,
    RandomEvenDistributionMainGame,
    TypeThemed,
    TypeThemedEliteFourGyms,
    KeepThemed,
    KeepThemesOrPrimary,
}

impl TrainerArgs {
    fn settings(&self) -> TrainerSettings {
        let three = |v: &Option<Vec<u8>>| v.as_deref().map_or([0; 3], |v| [v[0], v[1], v[2]]);
        let flags = |v: &Option<Vec<bool>>| v.as_deref().map_or([false; 3], |v| [v[0], v[1], v[2]]);
        TrainerSettings {
            mode: match self.trainers {
                TrainerModeArg::Unchanged => TrainerMode::Unchanged,
                TrainerModeArg::Random => TrainerMode::Random,
                TrainerModeArg::RandomEvenDistribution => TrainerMode::RandomEvenDistribution,
                TrainerModeArg::RandomEvenDistributionMainGame => TrainerMode::RandomEvenDistributionMainGame,
                TrainerModeArg::TypeThemed => TrainerMode::TypeThemed,
                TrainerModeArg::TypeThemedEliteFourGyms => TrainerMode::TypeThemedEliteFourGyms,
                TrainerModeArg::KeepThemed => TrainerMode::KeepThemed,
                TrainerModeArg::KeepThemesOrPrimary => TrainerMode::KeepThemesOrPrimary,
            },
            level_modifier: self.trainer_level_modifier,
            evolve: self.trainers_evolve,
            rival_carries_starter: self.rival_carries_starter,
            similar_strength: self.trainers_similar_strength,
            allow_alt_formes: self.trainers_alt_formes,
            swap_mega_evos: self.trainers_swap_mega_evos,
            avoid_duplicates: self.trainers_avoid_duplicates,
            weight_types: self.trainers_weight_types,
            no_legendaries: self.trainers_no_legendaries,
            no_early_wonder_guard: self.trainers_no_early_wonder_guard,
            local_pokemon: self.trainers_local_pokemon,
            random_shiny: self.random_shiny_trainer_pokemon,
            elite_four_unique: self.elite_four_unique,
            additional_pokemon: three(&self.additional_trainer_pokemon),
            diverse_types: flags(&self.diverse_trainer_types),
            held_items: flags(&self.trainer_held_items),
            consumable_items_only: self.consumable_trainer_items,
            sensible_items_only: self.sensible_trainer_items,
            highest_level_gets_item: self.highest_level_gets_item,
            better_movesets: flags(&self.better_trainer_movesets),
            random_names: self.random_trainer_names,
            random_class_names: self.random_trainer_class_names,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Battle style")]
struct BattleStyleArgs {
    /// Change trainers' battle styles: a random style each, or `--battle-style` for all.
    #[arg(long, value_enum, default_value_t = BattleStyleModeArg::Unchanged)]
    battle_styles: BattleStyleModeArg,
    /// With `--battle-styles single-style`, the style every trainer battles in.
    #[arg(long, value_enum)]
    battle_style: Option<BattleStyleArg>,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BattleStyleModeArg {
    Unchanged,
    Random,
    SingleStyle,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum BattleStyleArg {
    Single,
    Double,
    Triple,
    Rotation,
}

impl BattleStyleArgs {
    fn settings(&self) -> Result<BattleStyleSettings> {
        if self.battle_style.is_some() != (self.battle_styles == BattleStyleModeArg::SingleStyle) {
            anyhow::bail!("--battle-style goes with --battle-styles single-style, and only with it");
        }
        Ok(BattleStyleSettings {
            mode: match self.battle_styles {
                BattleStyleModeArg::Unchanged => BattleStyleMode::Unchanged,
                BattleStyleModeArg::Random => BattleStyleMode::Random,
                BattleStyleModeArg::SingleStyle => BattleStyleMode::SingleStyle,
            },
            style: match self.battle_style {
                None | Some(BattleStyleArg::Single) => BattleStyle::Single,
                Some(BattleStyleArg::Double) => BattleStyle::Double,
                Some(BattleStyleArg::Triple) => BattleStyle::Triple,
                Some(BattleStyleArg::Rotation) => BattleStyle::Rotation,
            },
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Pickup items")]
struct PickupArgs {
    /// Pickup finds random items, each as likely as the item it replaces.
    #[arg(long)]
    random_pickup_items: bool,
    /// With `--random-pickup-items`, no bad items.
    #[arg(long, requires = "random_pickup_items")]
    pickup_items_ban_bad: bool,
}

#[derive(Args)]
#[command(next_help_heading = "Pokémon palettes")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per palette option")]
struct PaletteArgs {
    /// Pokémon get random colors.
    #[arg(long)]
    random_palettes: bool,
    /// With `--random-palettes`, colors follow the species' types.
    #[arg(long, requires = "random_palettes")]
    palettes_follow_types: bool,
    /// With `--random-palettes`, evolutions keep their pre-evolution's colors.
    #[arg(long, requires = "random_palettes")]
    palettes_follow_evolutions: bool,
    /// With `--random-palettes`, the shiny palette becomes the original normal one.
    #[arg(long, requires = "random_palettes")]
    shiny_from_normal: bool,
}

#[derive(Args)]
#[command(next_help_heading = "Custom player graphics")]
struct PlayerGraphicsArgs {
    /// Puts this graphics pack in the game (its folder name; `ppr player-graphics <rom>` lists them).
    #[arg(long, value_name = "PACK")]
    player_graphics: Option<String>,
    /// With `--player-graphics`, the pack replaces the second player character (May, Leaf, Kris) instead of the first.
    #[arg(long, requires = "player_graphics")]
    replace_second_player: bool,
}

impl PlayerGraphicsArgs {
    fn settings(&self) -> PlayerGraphicsSettings {
        PlayerGraphicsSettings {
            pack: self.player_graphics.clone(),
            replace: if self.replace_second_player {
                PlayerCharacter::Pc2
            } else {
                PlayerCharacter::Pc1
            },
        }
    }
}

impl PaletteArgs {
    fn settings(&self) -> PaletteSettings {
        PaletteSettings {
            random: self.random_palettes,
            follow_types: self.palettes_follow_types,
            follow_evolutions: self.palettes_follow_evolutions,
            shiny_from_normal: self.shiny_from_normal,
        }
    }
}

impl PickupArgs {
    fn settings(&self) -> PickupItemSettings {
        PickupItemSettings {
            random: self.random_pickup_items,
            ban_bad: self.pickup_items_ban_bad,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "In-game trades")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct TradeArgs {
    /// Randomize in-game trades: the Pokémon given, or also the ones asked for.
    #[arg(long, value_enum)]
    trades: Option<TradeModeArg>,
    /// Give traded Pokémon random nicknames.
    #[arg(long, requires = "trades")]
    trade_nicknames: bool,
    /// Give traded Pokémon random original trainers.
    #[arg(long, requires = "trades")]
    trade_ots: bool,
    /// Give traded Pokémon random IVs.
    #[arg(long, requires = "trades")]
    trade_ivs: bool,
    /// Give traded Pokémon random held items.
    #[arg(long, requires = "trades")]
    trade_items: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum TradeModeArg {
    /// Only the Pokémon given.
    Given,
    /// The Pokémon given and the ones asked for.
    Both,
}

impl TradeArgs {
    fn settings(&self) -> TradeSettings {
        TradeSettings {
            mode: match self.trades {
                None => TradeMode::Unchanged,
                Some(TradeModeArg::Given) => TradeMode::Given,
                Some(TradeModeArg::Both) => TradeMode::GivenAndRequested,
            },
            random_nicknames: self.trade_nicknames,
            random_ots: self.trade_ots,
            random_ivs: self.trade_ivs,
            random_items: self.trade_items,
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Static Pokémon")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct StaticArgs {
    /// Replace static Pokémon (legendaries, gifts, fossils and other one-time encounters).
    #[arg(long, value_enum)]
    statics: Option<StaticModeArg>,
    /// With `--statics similar-strength`: the legendaries the story makes you face get no stronger replacement.
    #[arg(long)]
    statics_limit_main_game_legendaries: bool,
    /// With `--statics similar-strength`: statics with a base stat total of 600 or more become any species.
    #[arg(long)]
    statics_randomize_600_bst: bool,
    /// Special battle music follows replaced legendaries (`FireRed`, `LeafGreen`, Emerald).
    #[arg(long, requires = "statics")]
    statics_fix_music: bool,
    /// Replacements may be alternate formes, where static Pokémon have a forme.
    #[arg(long, requires = "statics")]
    statics_alt_formes: bool,
    /// Statics that hold their Mega Stone become species that can Mega Evolve, holding a Mega Stone of theirs (Gen 6).
    #[arg(long, requires = "statics")]
    statics_swap_mega_evos: bool,
    /// Change static levels by this percentage, -100 to 155.
    #[arg(long, allow_hyphen_values = true, value_parser = clap::value_parser!(i16).range(-100..=155))]
    static_levels: Option<i16>,
}

#[derive(Clone, Copy, ValueEnum)]
enum StaticModeArg {
    /// Legendaries become legendaries, the others non-legendaries.
    Matching,
    /// Any species.
    Random,
    /// A species of similar base stat total.
    SimilarStrength,
}

impl StaticArgs {
    fn settings(&self) -> Result<StaticSettings> {
        let mode = match self.statics {
            None => StaticMode::Unchanged,
            Some(StaticModeArg::Matching) => StaticMode::RandomMatching,
            Some(StaticModeArg::Random) => StaticMode::Random,
            Some(StaticModeArg::SimilarStrength) => StaticMode::SimilarStrength,
        };
        if (self.statics_limit_main_game_legendaries || self.statics_randomize_600_bst)
            && mode != StaticMode::SimilarStrength
        {
            anyhow::bail!(
                "--statics-limit-main-game-legendaries and --statics-randomize-600-bst need --statics similar-strength"
            );
        }
        Ok(StaticSettings {
            mode,
            limit_main_game_legendaries: self.statics_limit_main_game_legendaries,
            randomize_600_bst: self.statics_randomize_600_bst,
            fix_music: self.statics_fix_music,
            level_modifier: self.static_levels.unwrap_or(0),
            allow_alt_formes: self.statics_alt_formes,
            swap_mega_evos: self.statics_swap_mega_evos,
        })
    }
}

#[derive(Args)]
#[command(next_help_heading = "Totem Pokémon (Sun, Moon, Ultra Sun, Ultra Moon)")]
struct TotemArgs {
    /// Replace the Totem Pokémon.
    #[arg(long, value_enum)]
    totems: Option<TotemModeArg>,
    /// Replace the allies Totem Pokémon call.
    #[arg(long, value_enum)]
    totem_allies: Option<TotemModeArg>,
    /// Change the auras of Totem Pokémon: any stat by 1 to 3 stages, or another stat by as many stages.
    #[arg(long, value_enum)]
    totem_auras: Option<AuraModeArg>,
    /// Totem Pokémon that hold an item get a random consumable one.
    #[arg(long)]
    totem_held_items: bool,
    /// New Totem Pokémon and allies may be alternate formes.
    #[arg(long)]
    totem_alt_formes: bool,
    /// Change the levels of Totem Pokémon and their allies by this percentage, -100 to 155.
    #[arg(long, allow_hyphen_values = true, value_parser = clap::value_parser!(i16).range(-100..=155))]
    totem_levels: Option<i16>,
}

#[derive(Clone, Copy, ValueEnum)]
enum TotemModeArg {
    /// Any species.
    Random,
    /// A species of similar base stat total.
    SimilarStrength,
}

#[derive(Clone, Copy, ValueEnum)]
enum AuraModeArg {
    /// Any stat, or every stat, by 1 to 3 stages.
    Random,
    /// Another single stat by as many stages; auras that raise every stat stay.
    SameStrength,
}

impl TotemArgs {
    fn settings(&self) -> TotemSettings {
        let mode = |arg: Option<TotemModeArg>| match arg {
            None => TotemMode::Unchanged,
            Some(TotemModeArg::Random) => TotemMode::Random,
            Some(TotemModeArg::SimilarStrength) => TotemMode::SimilarStrength,
        };
        TotemSettings {
            mode: mode(self.totems),
            ally_mode: mode(self.totem_allies),
            aura_mode: match self.totem_auras {
                None => AuraMode::Unchanged,
                Some(AuraModeArg::Random) => AuraMode::Random,
                Some(AuraModeArg::SameStrength) => AuraMode::SameStrength,
            },
            randomize_held_items: self.totem_held_items,
            allow_alt_formes: self.totem_alt_formes,
            level_modifier: self.totem_levels.unwrap_or(0),
        }
    }
}

#[derive(Args)]
#[command(next_help_heading = "Wild Pokémon")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per option")]
struct WildArgs {
    /// Replace wild Pokémon, consistently within these zones.
    #[arg(long, value_enum)]
    wild: Option<WildZonesArg>,
    /// Split zones by encounter type (grass, water, rock smash, fishing).
    #[arg(long, requires = "wild")]
    wild_split: bool,
    /// Type rule for wild Pokémon.
    #[arg(long, value_enum, default_value_t = WildTypesArg::None, requires = "wild")]
    wild_types: WildTypesArg,
    /// A zone whose Pokémon all share a type keeps that type.
    #[arg(long, requires = "wild")]
    wild_keep_themes: bool,
    /// Evolution rule for wild Pokémon.
    #[arg(long, value_enum, default_value_t = WildEvolutionArg::None, requires = "wild")]
    wild_evolution: WildEvolutionArg,
    /// Replace members of one evolution line in a zone by members of one new line.
    #[arg(long, requires = "wild")]
    wild_keep_relations: bool,
    /// Replacements have a similar base stat total.
    #[arg(long, requires = "wild")]
    wild_similar_strength: bool,
    /// With --wild-similar-strength: no strong Pokémon at low levels.
    #[arg(long, requires = "wild_similar_strength")]
    wild_balance_low_levels: bool,
    /// Use every species once before any species appears twice.
    #[arg(long, requires = "wild")]
    wild_catch_em_all: bool,
    /// No legendary wild Pokémon.
    #[arg(long, requires = "wild")]
    wild_no_legendaries: bool,
    /// Minimum catch rate level: 1–4 raise catch rates, 5 makes every ball catch.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=5))]
    min_catch_rate: Option<u8>,
    /// Change wild levels by this percentage, -100 to 155.
    #[arg(long, allow_hyphen_values = true, value_parser = clap::value_parser!(i16).range(-100..=155))]
    wild_levels: Option<i16>,
    /// Randomize the items wild Pokémon hold.
    #[arg(long)]
    wild_held_items: bool,
    /// With random wild held items: no bad items.
    #[arg(long, requires = "wild_held_items")]
    wild_held_items_ban_bad: bool,
    /// Wild encounters by time of day, where the game has them: each time of day is randomized on its own.
    #[arg(long)]
    wild_time_of_day: bool,
    /// Replacements may be alternate formes, where wild Pokémon have a forme.
    #[arg(long, requires = "wild")]
    wild_alt_formes: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum WildZonesArg {
    /// Every slot on its own.
    Slot,
    /// One encounter set.
    Set,
    /// One map (games that number their maps).
    Map,
    /// One named location.
    Location,
    /// The whole game.
    Game,
}

#[derive(Clone, Copy, ValueEnum)]
enum WildTypesArg {
    None,
    ZoneThemes,
    KeepPrimary,
}

#[derive(Clone, Copy, ValueEnum)]
enum WildEvolutionArg {
    None,
    BasicOnly,
    SameStage,
}

impl WildArgs {
    fn settings(&self) -> WildSettings {
        WildSettings {
            mode: if self.wild.is_some() {
                WildMode::Random
            } else {
                WildMode::Unchanged
            },
            zones: match self.wild {
                Some(WildZonesArg::Slot) => WildZones::Slot,
                Some(WildZonesArg::Set) => WildZones::Set,
                Some(WildZonesArg::Map) => WildZones::Map,
                Some(WildZonesArg::Location) => WildZones::Location,
                Some(WildZonesArg::Game) | None => WildZones::Game,
            },
            split_by_kind: self.wild_split,
            types: match self.wild_types {
                WildTypesArg::None => WildTypes::None,
                WildTypesArg::ZoneThemes => WildTypes::ZoneThemes,
                WildTypesArg::KeepPrimary => WildTypes::KeepPrimary,
            },
            keep_themes: self.wild_keep_themes,
            evolution: match self.wild_evolution {
                WildEvolutionArg::None => WildEvolution::None,
                WildEvolutionArg::BasicOnly => WildEvolution::BasicOnly,
                WildEvolutionArg::SameStage => WildEvolution::SameStage,
            },
            keep_relations: self.wild_keep_relations,
            similar_strength: self.wild_similar_strength,
            balance_low_levels: self.wild_balance_low_levels,
            catch_em_all: self.wild_catch_em_all,
            no_legendaries: self.wild_no_legendaries,
            min_catch_rate: self.min_catch_rate.unwrap_or(0),
            level_modifier: self.wild_levels.unwrap_or(0),
            random_held_items: self.wild_held_items,
            ban_bad_held_items: self.wild_held_items_ban_bad,
            time_of_day: self.wild_time_of_day,
            allow_alt_formes: self.wild_alt_formes,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum StarterModeArg {
    Unchanged,
    Custom,
    Random,
    RandomTwoEvolutions,
    RandomBasic,
}

#[derive(Clone, Copy, ValueEnum)]
enum StarterTypesArg {
    None,
    FireWaterGrass,
    TypeTriangle,
    Unique,
    SingleType,
}

impl RandomizeArgs {
    /// The settings of `--settings-string` or `--settings-file`, or else of the option flags.
    fn chosen_settings(&self) -> Result<Settings> {
        if let Some(text) = &self.settings_string {
            return Ok(read_settings_string(text)
                .context("reading --settings-string")?
                .settings);
        }
        if let Some(path) = &self.settings_file {
            return Ok(load_settings_file(path)
                .with_context(|| format!("reading {}", path.display()))?
                .settings);
        }
        self.settings()
    }

    fn settings(&self) -> Result<Settings> {
        if self.starter_type.is_some() && !matches!(self.starter_types, StarterTypesArg::SingleType) {
            anyhow::bail!("--starter-type needs --starter-types single-type");
        }
        let mode = match (self.custom.is_empty(), self.starters) {
            (false, _) | (true, StarterModeArg::Custom) => StarterMode::Custom,
            (true, StarterModeArg::Unchanged) => StarterMode::Unchanged,
            (true, StarterModeArg::Random) => StarterMode::Random,
            (true, StarterModeArg::RandomTwoEvolutions) => StarterMode::RandomWithTwoEvolutions,
            (true, StarterModeArg::RandomBasic) => StarterMode::RandomBasic,
        };
        let types = match self.starter_types {
            StarterTypesArg::None => StarterTypes::None,
            StarterTypesArg::FireWaterGrass => StarterTypes::FireWaterGrass,
            StarterTypesArg::TypeTriangle => StarterTypes::TypeTriangle,
            StarterTypesArg::Unique => StarterTypes::Unique,
            StarterTypesArg::SingleType => StarterTypes::SingleType(self.starter_type),
        };
        Ok(Settings {
            custom_names: None,
            starters: StarterSettings {
                mode,
                custom: self.custom.iter().map(|&n| (n != 0).then_some(n)).collect(),
                types,
                no_legendaries: self.no_legendaries,
                no_dual_types: self.no_dual_types,
                bst_min: self.bst_min,
                bst_max: self.bst_max,
                random_held_items: self.starter_held_items,
                ban_bad_items: self.starter_ban_bad_items,
                allow_alt_formes: self.starter_alt_formes,
            },
            base_stats: self.base_stats.settings(),
            bsts: self.bsts.settings(),
            general: ppr_service::GeneralSettings {
                random_intro_pokemon: self.random_intro_pokemon,
                no_premature_evolutions: self.no_premature_evolutions,
                no_irregular_alt_formes: self.no_irregular_alt_formes,
                limit_pokemon: (!self.limit_generations.is_empty()).then(|| ppr_service::GenerationLimit {
                    generations: std::array::from_fn(|i| {
                        self.limit_generations.iter().any(|&g| usize::from(g) == i + 1)
                    }),
                    allow_evolutionary_relatives: !self.no_evolutionary_relatives,
                }),
            },
            types: self.types.settings(),
            abilities: self.abilities.settings(),
            exp_curves: self.exp_curves.settings(),
            evolutions: self.evolutions.settings()?,
            move_data: self.move_data.settings(),
            movesets: self.movesets.settings()?,
            tms: self.tms.settings(),
            tm_hm_compat: self.tm_hm_compat.settings()?,
            tutors: self.tutors.settings(),
            tutor_compat: self.tutor_compat.settings()?,
            wild: self.wild.settings(),
            misc_tweaks: self.tweaks.clone(),
            statics: self.statics.settings()?,
            totems: self.totems.settings(),
            trades: self.trades.settings(),
            field_items: self.field_items.settings()?,
            shops: self.shops.settings()?,
            pickup: self.pickup.settings(),
            palettes: self.palettes.settings(),
            player_graphics: self.player_graphics.settings(),
            battle_style: self.battle_style.settings()?,
            trainers: self.trainers.settings(),
            type_effectiveness: self.type_effectiveness.settings()?,
        })
    }
}

/// A misc tweak by its ID, like `fastestText`.
fn parse_tweak(id: &str) -> std::result::Result<ppr_service::MiscTweak, String> {
    serde_json::from_value(serde_json::Value::String(id.to_owned())).map_err(|_| format!("no misc tweak {id:?}"))
}

fn main() -> Result<()> {
    match run() {
        // `ppr games | head` closes stdout early; that is not an error.
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe) =>
        {
            Ok(())
        }
        other => other,
    }
}

fn run() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .init();
    let cli = Cli::parse();
    let mut out = io::stdout().lock();
    match cli.command {
        Command::Identify { files } => {
            for file in files {
                let report = identify_file(&file)?;
                if cli.json {
                    writeln!(out, "{}", serde_json::to_string(&report)?)?;
                } else {
                    let text = match report {
                        RomReport::Supported { identity } => format!("{} ({})", identity.name, identity.summary()),
                        RomReport::Unsupported { reason } => reason,
                    };
                    writeln!(out, "{}: {text}", file.display())?;
                }
            }
        }
        Command::Games => {
            let games = supported_games();
            if cli.json {
                writeln!(out, "{}", serde_json::to_string_pretty(&games)?)?;
            } else {
                for g in games {
                    writeln!(
                        out,
                        "{:<6} {:<17} {}",
                        g.generation.to_string(),
                        g.platform.display_name(),
                        g.name
                    )?;
                }
            }
        }
        Command::PlayerGraphics { rom } => {
            let loaded = load_rom(&rom).with_context(|| format!("loading {}", rom.display()))?;
            let packs: Vec<_> = loaded.player_graphics_packs.into_iter().map(|o| o.pack).collect();
            if cli.json {
                writeln!(out, "{}", serde_json::to_string_pretty(&packs)?)?;
            } else {
                for p in packs {
                    writeln!(out, "{:<12} {} ({})", p.id, p.name, p.from)?;
                }
            }
        }
        Command::EditView { rom, update } => {
            let (_, session) =
                RomSession::open(&rom, update.as_deref()).with_context(|| format!("loading {}", rom.display()))?;
            let session = session.with_context(|| format!("{} can't be randomized yet", rom.display()))?;
            writeln!(out, "{}", serde_json::to_string_pretty(&session.editable())?)?;
        }
        Command::Batch(args) => batch(&args, cli.json, &mut out)?,
        Command::Randomize(args) => {
            let (settings, seed) = match &args.premade_seed {
                Some(path) => {
                    let premade = load_premade_seed(path).with_context(|| format!("reading {}", path.display()))?;
                    (premade.shared.settings, Some(premade.seed))
                }
                None => (args.chosen_settings()?, args.seed),
            };
            let request = RandomizeRequest {
                input: args.rom.clone(),
                update: args.update.clone(),
                layered_fs: args.layered_fs,
                output: args.output.clone(),
                settings: with_names(settings, args.custom_names.as_deref())?,
                seed,
                log: args.log,
                edits: args.edits.as_deref().map(read_edits).transpose()?,
            };
            let mut progress = |p: Progress| tracing::info!("{:>3}% {}", p.percent, p.step);
            let report = randomize_file(&request, &mut progress, &CancelToken::default())
                .with_context(|| format!("randomizing {}", args.rom.display()))?;
            let log = report
                .log
                .as_deref()
                .map(|log| save_log(&report.output, log))
                .transpose()
                .context("saving the log")?;
            if cli.json {
                writeln!(out, "{}", serde_json::to_string(&report)?)?;
            } else {
                print_report(&mut out, &report)?;
                writeln!(out, "Saved to {}", report.output.display())?;
                if let Some(log) = log {
                    writeln!(out, "Log saved to {}", log.display())?;
                }
            }
        }
    }
    Ok(())
}

/// `settings` with the custom names in `dir`, if any.
fn with_names(mut settings: Settings, dir: Option<&std::path::Path>) -> Result<Settings> {
    if let Some(dir) = dir {
        settings.custom_names =
            Some(load_custom_names(dir).with_context(|| format!("reading the names in {}", dir.display()))?);
    }
    Ok(settings)
}

/// `ppr batch`: many randomized ROMs, each with its own seed, with the settings of a string or file.
fn batch(args: &BatchArgs, json: bool, out: &mut impl Write) -> Result<()> {
    let settings = match (&args.settings_string, &args.settings_file) {
        (Some(text), _) => {
            read_settings_string(text)
                .context("reading --settings-string")?
                .settings
        }
        (None, Some(path)) => {
            load_settings_file(path)
                .with_context(|| format!("reading {}", path.display()))?
                .settings
        }
        (None, None) => unreachable!("clap requires one of them"),
    };
    let request = BatchRequest {
        input: args.rom.clone(),
        update: args.update.clone(),
        layered_fs: args.layered_fs,
        dir: args.dir.clone(),
        prefix: args.prefix.clone(),
        start: args.start,
        count: args.count,
        settings: with_names(settings, args.custom_names.as_deref())?,
        log_ending: args.logs.clone(),
        edits: None,
    };
    let mut progress = |p: BatchProgress| tracing::info!("{:>3}% ROM {} of {}: {}", p.percent, p.rom, p.count, p.step);
    let report = randomize_batch(&request, &mut progress, &CancelToken::default())
        .with_context(|| format!("randomizing {}", args.rom.display()))?;
    if json {
        writeln!(out, "{}", serde_json::to_string(&report)?)?;
    } else {
        for rom in &report.roms {
            writeln!(out, "{} (seed {})", rom.output.display(), rom.seed)?;
        }
    }
    Ok(())
}

/// The run report as text, one line per part that changed.
/// Reads an edit list from a JSON file.
fn read_edits(path: &std::path::Path) -> Result<EditList> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("reading the manual edits in {}", path.display()))
}

fn print_report(out: &mut impl Write, report: &RunReport) -> Result<()> {
    writeln!(out, "Seed: {}", report.seed)?;
    writeln!(out, "Check value: {:08X}", report.check_value)?;
    for (i, s) in report.starters.iter().enumerate() {
        writeln!(out, "Starter {}: {} -> {}", i + 1, s.before, s.after)?;
    }
    for s in report.starter_held_items.iter().filter(|s| s.before != s.after) {
        writeln!(out, "Starter held item: {} -> {}", s.before, s.after)?;
    }
    let species = (report.species, " species");
    let lines = [
        ("Base stats", report.base_stats_changed, species),
        ("Types", report.types_changed, species),
        ("Abilities", report.abilities_changed, species),
        ("EXP curves", report.exp_curves_changed, species),
        ("Evolutions", report.evolutions_changed, species),
        (
            "Type effectiveness",
            report.type_pairs_changed,
            (report.type_pairs, " pairs"),
        ),
        ("Moves", report.moves_changed, (report.moves, "")),
        ("TMs", report.tms_changed, (report.tms, "")),
        ("TM/HM compatibility", report.tm_hm_compat_changed, species),
        ("Move tutors", report.tutors_changed, (report.tutors, "")),
        ("Move tutor compatibility", report.tutor_compat_changed, species),
        ("Movesets", report.movesets_changed, species),
        (
            "Wild Pokémon",
            report.wild_sets_changed,
            (report.wild_sets, " encounter sets"),
        ),
        ("Trainers", report.trainers_changed, (report.trainers, "")),
        ("Static Pokémon", report.statics_changed, (report.statics, "")),
        ("Totem Pokémon", report.totems_changed, (report.totems, "")),
        ("In-game trades", report.trades_changed, (report.trades, "")),
        ("Field items", report.field_items_changed, (report.field_items, "")),
        ("Shops", report.shops_changed, (report.shops, "")),
        ("Pickup items", report.pickup_items_changed, (report.pickup_items, "")),
    ];
    for (label, changed, (total, unit)) in lines.into_iter().filter(|&(_, changed, _)| changed > 0) {
        writeln!(out, "{label}: {changed} of {total}{unit} changed")?;
    }
    if report.manual_edits > 0 {
        writeln!(out, "Manual edits: {}", report.manual_edits)?;
    }
    Ok(())
}
