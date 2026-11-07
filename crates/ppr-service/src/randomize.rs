use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::path::{Path, PathBuf};
use std::time::Instant;

use std::collections::BTreeMap;

use ppr_model::{DexNumber, Edit, EditList, GameData, Species, TypeChart};
use ppr_random::{CancelToken, DONE, Progress, ProgressSink, RunData, Seed, standard_pipeline};
use ppr_settings::Settings;
use serde::Serialize;
use ts_rs::TS;

use crate::{EditSeverity, ServiceError, applying_edits_step, validate_edits};

/// One randomization of one ROM.
#[derive(Debug, Clone)]
pub struct RandomizeRequest {
    /// The ROM to read. It is never modified.
    pub input: PathBuf,
    /// A decrypted game update of a 3DS game to lay over it first. The randomized game is then a LayeredFS folder.
    pub update: Option<PathBuf>,
    /// Save a 3DS game as a LayeredFS folder instead of a ROM, also without an update. Implied by an update; other
    /// games refuse it.
    pub layered_fs: bool,
    /// Where to write the randomized ROM; for a LayeredFS folder, the folder it goes into.
    pub output: PathBuf,
    /// The settings.
    pub settings: Settings,
    /// The seed; a random one when `None`.
    pub seed: Option<Seed>,
    /// Make a log of the run ([`RunReport::log`]). Leave it off in Race Mode.
    pub log: bool,
    /// Manual edits to apply after the randomizers (ADR 0008); `None` or an empty list for none.
    pub edits: Option<EditList>,
}

impl RandomizeRequest {
    /// Whether the game is saved as a LayeredFS folder: when asked for, and always with a game update.
    #[must_use]
    pub fn saves_layered_fs(&self) -> bool {
        self.layered_fs || self.update.is_some()
    }
}

/// A starter slot, or a starter's held item, before and after.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct StarterChange {
    /// Name before.
    pub before: String,
    /// Name after.
    pub after: String,
}

/// What a run did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RunReport {
    /// The seed, to reproduce the run; serialized as a decimal string.
    #[ts(type = "string")]
    pub seed: Seed,
    /// What was written: the ROM, or the LayeredFS folder named after the game's title ID.
    #[ts(type = "string")]
    pub output: PathBuf,
    /// Starters, per slot.
    pub starters: Vec<StarterChange>,
    /// Items the starters hold, per held-item slot (Gen 3 has one for all); "None" for no item.
    pub starter_held_items: Vec<StarterChange>,
    /// Species in the game.
    pub species: u32,
    /// Species with different base stats.
    pub base_stats_changed: u32,
    /// Species with different types.
    pub types_changed: u32,
    /// Species with different abilities.
    pub abilities_changed: u32,
    /// Species with a different EXP curve.
    pub exp_curves_changed: u32,
    /// Species that evolve differently.
    pub evolutions_changed: u32,
    /// Pairs of attacking and defending types in the type chart.
    pub type_pairs: u32,
    /// Pairs whose effectiveness changed.
    pub type_pairs_changed: u32,
    /// Moves in the game.
    pub moves: u32,
    /// Moves with different data.
    pub moves_changed: u32,
    /// Species that learn different moves by level or as eggs.
    pub movesets_changed: u32,
    /// TMs in the game.
    pub tms: u32,
    /// TMs that teach a different move.
    pub tms_changed: u32,
    /// Species that can learn different TMs or HMs.
    pub tm_hm_compat_changed: u32,
    /// Move tutors in the game.
    pub tutors: u32,
    /// Move tutors that teach a different move.
    pub tutors_changed: u32,
    /// Species that can learn different move tutor moves.
    pub tutor_compat_changed: u32,
    /// Wild encounter sets in the game.
    pub wild_sets: u32,
    /// Wild encounter sets with a different species in at least one slot.
    pub wild_sets_changed: u32,
    /// Trainer battles.
    pub trainers: u32,
    /// Trainer battles with a different team or style.
    pub trainers_changed: u32,
    /// The check value of the randomized game, which Race Mode shows instead of a log.
    pub check_value: u32,
    /// Static encounters in the game.
    pub statics: u32,
    /// Static encounters with a different species.
    pub statics_changed: u32,
    /// Totem Pokémon in the game.
    pub totems: u32,
    /// Totem Pokémon with any change, their allies included.
    pub totems_changed: u32,
    /// In-game trades in the game.
    pub trades: u32,
    /// In-game trades with any change.
    pub trades_changed: u32,
    /// Items on the ground and hidden items that may change.
    pub field_items: u32,
    /// Field items that are a different item.
    pub field_items_changed: u32,
    /// Shops in the game.
    pub shops: u32,
    /// Shops that sell different items.
    pub shops_changed: u32,
    /// Items Pickup can find.
    pub pickup_items: u32,
    /// Pickup items that are a different item.
    pub pickup_items_changed: u32,
    /// Manual edits applied after the randomizers.
    pub manual_edits: u32,
    /// The log of the run, when [`RandomizeRequest::log`] asked for one; see [`save_log`].
    #[serde(skip)]
    #[ts(skip)]
    pub log: Option<String>,
}

/// [`Progress::step`] while the output is written.
pub const WRITING: &str = "Writing ROM";
/// Share of the progress bar for the randomization steps; applying manual edits and writing take the rest.
const STEPS_SHARE: u8 = 90;
/// Where the progress bar is when writing starts after manual edits were applied.
const EDITS_DONE: u8 = 95;

/// Every step a run with `edits` manual edits can report, in order, ending with [`WRITING`]. Steps that do not apply
/// are skipped. Applying the edits ([`applying_edits_step`]) comes before writing, when there are any.
pub fn randomization_steps(edits: usize) -> Vec<String> {
    let mut steps: Vec<String> = standard_pipeline().labels().into_iter().map(str::to_owned).collect();
    if edits > 0 {
        steps.push(applying_edits_step(edits));
    }
    steps.push(WRITING.to_owned());
    steps
}

/// Scales the pipeline's progress into the steps' share and leaves "done" to the caller, who still has to write.
struct StepsProgress<'a>(&'a mut dyn ProgressSink);

impl ProgressSink for StepsProgress<'_> {
    fn report(&mut self, progress: Progress) {
        if progress.step != DONE {
            self.0.report(Progress {
                step: progress.step.clone(),
                // In u16: a u8 overflows from 3% on.
                percent: u8::try_from(u16::from(progress.percent.min(100)) * u16::from(STEPS_SHARE) / 100)
                    .unwrap_or(STEPS_SHARE),
            });
        }
    }
}

/// A seed from the OS's randomness, through std's randomly keyed hasher (no extra dependency).
fn fresh_seed() -> Seed {
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(0);
    Seed(hasher.finish())
}

fn item_name(game: &GameData, item: Option<u16>) -> String {
    item.map_or_else(
        || "None".to_owned(),
        |id| {
            game.items
                .iter()
                .find(|i| i.id == id)
                .map_or_else(|| format!("item {id}"), |i| i.name.clone())
        },
    )
}

fn name(game: &GameData, number: u16) -> String {
    game.species(number)
        .map_or_else(|| format!("#{number}"), |s| s.name.clone())
}

/// Loads the ROM, runs every ported randomizer, applies the manual edits and writes the result. The output appears
/// only when the whole run succeeded: it is written to a temporary file next to it and then renamed (ADR 0006).
///
/// The manual edits are checked against the game as loaded (in the editor's view, without time of day) before
/// anything is randomized, and applied after the last randomizer, all or none; with time of day, an edit of a wild
/// set goes into the day and the night set of its table. When one doesn't fit, the run fails with
/// [`ServiceError::EditsRejected`] and nothing is written.
pub fn randomize_file(
    request: &RandomizeRequest,
    progress: &mut dyn ProgressSink,
    cancel: &CancelToken,
) -> Result<RunReport, ServiceError> {
    let started = Instant::now();
    request.settings.validate()?;
    let seed = request.seed.unwrap_or_else(fresh_seed);
    let mut handler = open_rom(&request.input, request.update.as_deref())?;
    let layered_fs = request.saves_layered_fs();
    // Refused before the run, not after it.
    if layered_fs && handler.game_data().identity.platform != ppr_model::Platform::Nintendo3ds {
        return Err(
            ppr_romio::RomError::Unsupported("only 3DS games can be saved as a LayeredFS folder".into()).into(),
        );
    }
    let update_version = handler.game_update_version();
    // The edits are made in the editor's view, without time of day, and checked against it.
    let edits = request.edits.as_ref().filter(|e| !e.edits.is_empty());
    if let Some(edits) = edits {
        let issues = validate_edits(handler.game_data(), &handler.edit_rules(), edits)?;
        if issues.iter().any(|i| i.severity == EditSeverity::Error) {
            return Err(ServiceError::EditsRejected(issues));
        }
    }
    // The wild encounters by time of day are their own sets, for every step that sees them.
    handler.use_time_of_day(request.settings.wild.time_of_day)?;
    let origins = handler.encounter_set_origins();
    let original = handler.game_data().clone();
    let mut data = RunData {
        game: original.clone(),
        settings: request.settings.clone(),
    };
    standard_pipeline().run(&mut data, seed, cancel, &mut StepsProgress(progress))?;
    if let Some(edits) = edits {
        progress.report(Progress {
            step: applying_edits_step(edits.edits.len()).into(),
            percent: STEPS_SHARE,
        });
        apply_edits(edits, &origins, &mut data.game, &original)?;
    }
    progress.report(Progress {
        step: WRITING.into(),
        percent: if edits.is_some() { EDITS_DONE } else { STEPS_SHARE },
    });
    handler.apply(&data.game)?;
    handler.set_seed(seed.0);
    let output = if layered_fs {
        write_layered_fs(&request.output, handler.as_ref())?
    } else {
        write_atomically(&request.output, |out| handler.save(out).map_err(ServiceError::from))?;
        request.output.clone()
    };
    progress.report(Progress {
        step: DONE.into(),
        percent: 100,
    });
    let mut report = report(seed, output, &original, &data.game);
    report.manual_edits = edits.map_or(0, |e| u32::try_from(e.edits.len()).unwrap_or(u32::MAX));
    if request.log {
        // Read in pieces: a 3DS game takes gigabytes.
        let crc32 = std::fs::File::open(&request.input)
            .and_then(|f| ppr_formats::crc32_of(&mut std::io::BufReader::new(f)))
            .map_err(|source| ServiceError::Open {
                path: request.input.clone(),
                source,
            })?;
        report.log = Some(crate::log::write(&crate::log::LogInput {
            before: &original,
            after: &data.game,
            settings: &request.settings,
            seed,
            file_name: &request.input.file_name().unwrap_or_default().to_string_lossy(),
            crc32,
            update: request.update.as_ref().map(|path| crate::log::UpdateInfo {
                file_name: path.file_name().unwrap_or_default().to_string_lossy().into_owned(),
                version: update_version.clone().unwrap_or_default(),
            }),
            elapsed: started.elapsed(),
        }));
    }
    Ok(report)
}

/// Opens the ROM at `input` and lays the game update at `update` over it, when given.
pub(crate) fn open_rom(input: &Path, update: Option<&Path>) -> Result<Box<dyn ppr_romio::GameHandler>, ServiceError> {
    let mut handler = ppr_romio::open(input)?;
    if let Some(update) = update {
        // A missing update is the user's to fix, like a missing ROM.
        std::fs::metadata(update).map_err(|source| ServiceError::Open {
            path: update.to_owned(),
            source,
        })?;
        handler.load_game_update(update)?;
    }
    Ok(handler)
}

/// Writes the changes as a LayeredFS folder below `dir` and returns it. The folder appears only when it was
/// written completely: it is written into a temporary folder in `dir` and then moved into place, replacing an
/// earlier one.
fn write_layered_fs(dir: &Path, handler: &dyn ppr_romio::GameHandler) -> Result<PathBuf, ServiceError> {
    let fail = |source| ServiceError::Write {
        path: dir.to_owned(),
        source,
    };
    std::fs::create_dir_all(dir).map_err(fail)?;
    let tmp = tempfile::Builder::new()
        .prefix(".ppr-layeredfs-")
        .tempdir_in(dir)
        .map_err(fail)?;
    let written = handler.save_layered_fs(tmp.path())?;
    let name = written
        .file_name()
        .ok_or_else(|| fail(std::io::ErrorKind::InvalidData.into()))?;
    let target = dir.join(name);
    if target.exists() {
        std::fs::remove_dir_all(&target).map_err(fail)?;
    }
    std::fs::rename(&written, &target).map_err(fail)?;
    Ok(target)
}

/// Applies `edits` to the randomized `game`, all or none; `original` names the objects of their errors. `origins`
/// says which set without time of day each of the game's wild sets comes from (see
/// [`ppr_romio::GameHandler::encounter_set_origins`]): an edit of a wild set goes into every set that comes from it, so
/// by time of day an edited table has the same Pokémon at every time.
fn apply_edits(
    edits: &EditList,
    origins: &[usize],
    game: &mut GameData,
    original: &GameData,
) -> Result<(), ServiceError> {
    let in_view = |index: usize| origins.iter().position(|&o| o == index).unwrap_or(usize::MAX);
    let mut view = edits.clone();
    for edit in &mut view.edits {
        if let Edit::WildSet(e) = edit {
            e.index = in_view(e.index);
        }
    }
    if let Err(errors) = view.apply(game) {
        return Err(ServiceError::EditsRejected(crate::edits::issues(errors, original)?));
    }
    for edit in &view.edits {
        if let Edit::WildSet(e) = edit
            && let Some(&origin) = origins.get(e.index)
        {
            let encounters = game.encounters[e.index].encounters.clone();
            for (set, _) in game
                .encounters
                .iter_mut()
                .zip(origins)
                .skip(e.index + 1)
                .filter(|&(_, &o)| o == origin)
            {
                set.encounters.clone_from(&encounters);
            }
        }
    }
    Ok(())
}

/// Where the log of a randomized ROM goes: next to it, with the extension `.log`.
pub fn log_path(output: &Path) -> PathBuf {
    output.with_extension("log")
}

/// Writes `log` next to `output` (see [`log_path`]) and returns where.
pub fn save_log(output: &Path, log: &str) -> Result<PathBuf, ServiceError> {
    save_log_as(output, "log", log)
}

/// Writes `log` next to `output`, with `ending` for its extension, and returns where.
pub(crate) fn save_log_as(output: &Path, ending: &str, log: &str) -> Result<PathBuf, ServiceError> {
    let path = output.with_extension(ending);
    write_atomically(&path, |out| {
        out.write_all(log.as_bytes()).map_err(|source| ServiceError::Write {
            path: path.clone(),
            source,
        })
    })?;
    Ok(path)
}

/// Counts of `before` and `after` pairs that `differ`.
fn changed<T>(before: &[T], after: &[T], differ: impl Fn(&T, &T) -> bool) -> u32 {
    let n = before.iter().zip(after).filter(|(b, a)| differ(b, a)).count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Pairs of types whose effectiveness differs.
fn type_pairs_changed(before: &TypeChart, after: &TypeChart) -> u32 {
    let types = before.types();
    let n = types
        .iter()
        .flat_map(|&a| types.iter().map(move |&d| (a, d)))
        .filter(|&(a, d)| before.get(a, d) != after.get(a, d))
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Species whose compatibility flags differ.
fn compat_changed(before: &BTreeMap<DexNumber, Vec<bool>>, after: &BTreeMap<DexNumber, Vec<bool>>) -> u32 {
    let n = after.iter().filter(|(n, flags)| before.get(n) != Some(flags)).count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Species whose evolutions differ in method, parameter or target.
fn evolutions_changed(original: &GameData, game: &GameData) -> u32 {
    let of = |g: &GameData, n| {
        g.evolutions
            .iter()
            .filter(|e| e.from == n)
            .map(|e| (e.to, e.kind, e.parameter))
            .collect::<Vec<_>>()
    };
    let n = original
        .species
        .iter()
        .filter(|sp| of(original, sp.number) != of(game, sp.number))
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// Species whose learnset or egg moves differ.
fn movesets_changed(original: &GameData, game: &GameData) -> u32 {
    let n = original
        .species
        .iter()
        .filter(|sp| {
            original.learnsets.get(&sp.number) != game.learnsets.get(&sp.number)
                || original.egg_moves.get(&sp.number) != game.egg_moves.get(&sp.number)
        })
        .count();
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// What a run changed from `original` to `game`.
fn report(seed: Seed, output: PathBuf, original: &GameData, game: &GameData) -> RunReport {
    let starters = original
        .starters
        .iter()
        .zip(&game.starters)
        .map(|(&b, &a)| StarterChange {
            before: name(original, b),
            after: name(game, a),
        })
        .collect();
    let starter_held_items = original
        .starter_held_items
        .iter()
        .zip(&game.starter_held_items)
        .map(|(&b, &a)| StarterChange {
            before: item_name(original, b),
            after: item_name(game, a),
        })
        .collect();
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    let species = |differ: fn(&Species, &Species) -> bool| changed(&original.species, &game.species, differ);
    RunReport {
        seed,
        output,
        starters,
        starter_held_items,
        species: count(original.species.len()),
        base_stats_changed: species(|b, a| b.stats != a.stats),
        types_changed: species(|b, a| (b.primary_type, b.secondary_type) != (a.primary_type, a.secondary_type)),
        abilities_changed: species(|b, a| b.abilities != a.abilities),
        exp_curves_changed: species(|b, a| b.growth_curve != a.growth_curve),
        evolutions_changed: evolutions_changed(original, game),
        moves: count(original.moves.len()),
        moves_changed: changed(&original.moves, &game.moves, |b, a| b != a),
        movesets_changed: movesets_changed(original, game),
        tms: count(original.tm_moves.len()),
        tms_changed: changed(&original.tm_moves, &game.tm_moves, |b, a| b != a),
        tm_hm_compat_changed: compat_changed(&original.tm_hm_compat, &game.tm_hm_compat),
        type_pairs: count(original.type_chart.types().len().pow(2)),
        type_pairs_changed: type_pairs_changed(&original.type_chart, &game.type_chart),
        tutors: count(original.tutor_moves.len()),
        tutors_changed: changed(&original.tutor_moves, &game.tutor_moves, |b, a| b != a),
        tutor_compat_changed: compat_changed(&original.tutor_compat, &game.tutor_compat),
        wild_sets: count(original.encounters.len()),
        wild_sets_changed: changed(&original.encounters, &game.encounters, |b, a| {
            b.encounters
                .iter()
                .zip(&a.encounters)
                .any(|(x, y)| x.species != y.species)
        }),
        trainers: count(original.trainers.len()),
        trainers_changed: changed(&original.trainers, &game.trainers, |b, a| {
            b.pokemon != a.pokemon || b.battle_style != a.battle_style
        }),
        check_value: crate::check_value(game),
        statics: count(original.statics.len()),
        statics_changed: changed(&original.statics, &game.statics, |b, a| b.species != a.species),
        totems: count(original.totems.len()),
        totems_changed: changed(&original.totems, &game.totems, |b, a| b != a),
        trades: count(original.trades.len()),
        trades_changed: changed(&original.trades, &game.trades, |b, a| b != a),
        field_items: count(original.field_items.len()),
        field_items_changed: changed(&original.field_items, &game.field_items, |b, a| b != a),
        shops: count(original.shops.len()),
        shops_changed: changed(&original.shops, &game.shops, |b, a| b.items != a.items),
        pickup_items: count(original.pickup_items.len()),
        pickup_items_changed: changed(&original.pickup_items, &game.pickup_items, |b, a| b != a),
        manual_edits: 0,
        log: None,
    }
}

fn write_atomically(
    path: &Path,
    write: impl FnOnce(&mut dyn std::io::Write) -> Result<(), ServiceError>,
) -> Result<(), ServiceError> {
    let fail = |source| ServiceError::Write {
        path: path.to_owned(),
        source,
    };
    let dir = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut tmp = tempfile::NamedTempFile::new_in(dir).map_err(fail)?;
    write(tmp.as_file_mut())?;
    tmp.as_file().sync_all().map_err(fail)?;
    tmp.persist(path).map_err(|e| fail(e.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use ppr_model::ObjectRef;

    use super::*;

    #[test]
    fn fresh_seeds_differ() {
        assert_ne!(fresh_seed(), fresh_seed());
    }

    #[test]
    fn failed_writes_leave_no_output() {
        let dir = tempfile::tempdir().expect("temp dir");
        let out = dir.path().join("out.gba");
        let result = write_atomically(&out, |_| {
            Err(ServiceError::Randomize(ppr_random::RandomizeError::Cancelled))
        });
        assert!(result.is_err());
        assert!(!out.exists());
        assert_eq!(
            std::fs::read_dir(dir.path()).expect("readable").count(),
            0,
            "temporary file removed"
        );
    }

    #[test]
    fn unknown_files_are_rejected_before_writing() {
        let dir = tempfile::tempdir().expect("temp dir");
        let input = dir.path().join("in.gba");
        std::fs::write(&input, [0u8; 64]).expect("write");
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input,
            output: dir.path().join("out.gba"),
            settings: Settings::default(),
            seed: Some(Seed(1)),
            log: false,
            edits: None,
        };
        let err = randomize_file(&request, &mut |_: ppr_random::Progress| {}, &CancelToken::default())
            .expect_err("not a ROM");
        assert_eq!(err.code(), "rom_unsupported");
        assert!(!request.output.exists());
    }

    /// A synthetic Emerald in a temporary directory, and a request to randomize it into the same directory.
    fn emerald_request(settings: Settings, seed: u64) -> (tempfile::TempDir, RandomizeRequest) {
        let dir = tempfile::tempdir().expect("temp dir");
        let input = dir.path().join("emerald.gba");
        std::fs::write(&input, ppr_romio::testing::emerald()).expect("write");
        let output = dir.path().join("out.gba");
        let request = RandomizeRequest {
            update: None,
            layered_fs: false,
            input,
            output,
            settings,
            seed: Some(Seed(seed)),
            log: false,
            edits: None,
        };
        (dir, request)
    }

    fn random_starters() -> Settings {
        let mut settings = Settings::default();
        settings.starters.mode = ppr_settings::StarterMode::Random;
        settings
    }

    fn run(request: &RandomizeRequest) -> Result<RunReport, ServiceError> {
        randomize_file(request, &mut |_: ppr_random::Progress| {}, &CancelToken::default())
    }

    #[test]
    fn an_update_implies_layered_fs() {
        let (_dir, mut request) = emerald_request(Settings::default(), 1);
        assert!(!request.saves_layered_fs());
        request.layered_fs = true;
        assert!(request.saves_layered_fs());
        request.layered_fs = false;
        request.update = Some("update.cia".into());
        assert!(request.saves_layered_fs());
    }

    #[test]
    fn only_3ds_games_are_saved_as_layered_fs() {
        let (dir, mut request) = emerald_request(random_starters(), 1);
        request.layered_fs = true;
        request.output = dir.path().join("out");
        let err = run(&request).expect_err("not a 3DS game");
        assert_eq!(err.code(), "rom_unsupported");
        assert!(err.to_string().contains("only 3DS games"), "{err}");
        assert!(!request.output.exists(), "nothing written");
    }

    #[test]
    fn randomized_starters_are_saved_and_reported() {
        let (_dir, request) = emerald_request(random_starters(), 42);
        let original = std::fs::read(&request.input).expect("readable");
        let report = run(&request).expect("randomizes");
        assert_eq!(report.seed, Seed(42));
        assert_eq!(
            std::fs::read(&request.input).expect("readable"),
            original,
            "input untouched"
        );

        let saved = ppr_romio::open(&request.output).expect("output loads");
        let starters = &saved.game_data().starters;
        assert_eq!(starters.len(), 3);
        assert_ne!(starters, &[252, 255, 258], "changed");
        let names: Vec<_> = starters.iter().map(|&n| name(saved.game_data(), n)).collect();
        let after: Vec<_> = report.starters.iter().map(|s| s.after.clone()).collect();
        assert_eq!(names, after, "the report matches the saved ROM");
        assert!(report.starters.iter().all(|s| s.before.starts_with("MON2")));
    }

    #[test]
    fn progress_ends_with_writing_and_done() {
        let (_dir, request) = emerald_request(random_starters(), 3);
        let mut seen = Vec::new();
        randomize_file(
            &request,
            &mut |p: Progress| seen.push((p.step.into_owned(), p.percent)),
            &CancelToken::default(),
        )
        .expect("randomizes");
        assert_eq!(
            seen,
            steps(&[("Starter Pokémon", 0), (WRITING, 90), (DONE, 100)]),
            "steps that don't apply are skipped"
        );
        assert_eq!(
            randomization_steps(0),
            [
                "Limit Pokémon",
                "Type Effectiveness Updates",
                "Move Updates",
                "Base Stat Updates",
                "Type Effectiveness",
                "Move Data",
                "Move Names",
                "Misc. Tweaks",
                "EXP Curves",
                "Evolutions",
                "Base Stat Totals",
                "Evolution Levels",
                "Types",
                "Wild Held Items",
                "Base Statistics",
                "Abilities",
                "Evolution Improvements",
                "Starter Pokémon",
                "Pokémon Movesets",
                "TM Moves",
                "TM/HM Compatibility",
                "Move Tutor Moves",
                "Move Tutor Compatibility",
                "Wild Pokémon",
                "Trainer Levels",
                "Added Trainer Pokémon",
                "Battle Style",
                "Trainer Pokémon",
                "Trainer Movesets",
                "Trainer Held Items",
                "Trainer Names",
                "Metronome Only",
                "Static Pokémon",
                "Totem Pokémon",
                "In-Game Trades",
                "Field Items",
                "Shop Items",
                "Pickup Items",
                "Pokémon Palettes",
                "Intro Pokémon",
                "Custom Player Graphics",
                WRITING
            ]
        );

        let mut settings = random_starters();
        settings.wild.mode = ppr_settings::WildMode::Random;
        let (_dir, request) = emerald_request(settings, 3);
        seen.clear();
        randomize_file(
            &request,
            &mut |p: Progress| seen.push((p.step.into_owned(), p.percent)),
            &CancelToken::default(),
        )
        .expect("randomizes");
        assert_eq!(
            seen,
            steps(&[("Starter Pokémon", 0), ("Wild Pokémon", 45), (WRITING, 90), (DONE, 100)])
        );
    }

    fn steps(expected: &[(&str, u8)]) -> Vec<(String, u8)> {
        expected.iter().map(|&(s, p)| (s.to_owned(), p)).collect()
    }

    /// An edit list for the synthetic Emerald: the first starter becomes Pikachu, the first trainer's first Pokémon
    /// is level 77, and the first shop sells Master Balls only.
    fn emerald_edits(request: &RandomizeRequest) -> EditList {
        let handler = ppr_romio::open(&request.input).expect("loads");
        let game = handler.game_data().clone();
        let view = crate::editable_game(&game, &handler.edit_rules());
        let mut list = EditList::new(&game.identity);
        list.set(ppr_model::Edit::Starter(ppr_model::StarterEdit {
            slot: 0,
            species: 25,
        }));
        let mut trainer = view.trainers[0].value.clone();
        trainer.pokemon[0].level = 77;
        list.set(ppr_model::Edit::Trainer(trainer));
        list.set(ppr_model::Edit::Shop(ppr_model::ShopEdit {
            index: 0,
            items: vec![1],
        }));
        list
    }

    #[test]
    fn edits_are_applied_after_the_randomizers_and_saved() {
        let (_dir, mut request) = emerald_request(random_starters(), 42);
        let edits = emerald_edits(&request);
        request.edits = Some(edits.clone());
        let mut seen = Vec::new();
        let report = randomize_file(
            &request,
            &mut |p: Progress| seen.push((p.step.into_owned(), p.percent)),
            &CancelToken::default(),
        )
        .expect("randomizes");
        assert_eq!(
            seen,
            steps(&[
                ("Starter Pokémon", 0),
                ("Applying 3 manual edits", 90),
                (WRITING, 95),
                (DONE, 100)
            ])
        );
        assert_eq!(report.manual_edits, 3);
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let game = saved.game_data();
        assert_eq!(game.starters[0], 25, "the edit wins over the random starter");
        assert_eq!(
            report.starters[0].after,
            name(game, 25),
            "the report includes the edits"
        );
        assert_ne!(&game.starters[1..], &[255, 258], "the other starters are still random");
        let ObjectRef::Trainer { index } = edits.edits[1].target() else {
            panic!("a trainer edit");
        };
        let trainer = game.trainers.iter().find(|t| t.index == index).expect("trainer");
        assert_eq!(trainer.pokemon[0].level, 77);
        assert_eq!(game.shops[0].items, [1]);
        assert_eq!(
            randomization_steps(3)[randomization_steps(3).len() - 2..],
            ["Applying 3 manual edits", WRITING]
        );
    }

    #[test]
    fn a_wild_set_edit_goes_into_every_set_of_its_table() {
        let (_dir, request) = emerald_request(Settings::default(), 1);
        let mut game = ppr_romio::open(&request.input).expect("loads").game_data().clone();
        // As by time of day: sets 0 and 1 come from table 0, sets 2 and 3 from table 1.
        game.encounters.truncate(2);
        let tables = game.encounters.clone();
        game.encounters = tables.iter().flat_map(|s| [s.clone(), s.clone()]).collect();
        game.encounters[1].encounters[0].species = 1;
        let slots: Vec<ppr_model::WildSlotEdit> = tables[1]
            .encounters
            .iter()
            .map(|_| ppr_model::WildSlotEdit {
                species: 25,
                min_level: 7,
                max_level: 9,
            })
            .collect();
        let mut edits = EditList::new(&game.identity);
        edits.set(Edit::WildSet(ppr_model::WildSetEdit { index: 1, slots }));
        let original = game.clone();
        let before = game.encounters.clone();
        apply_edits(&edits, &[0, 0, 1, 1], &mut game, &original).expect("applies");
        assert_eq!(game.encounters[..2], before[..2], "table 0 is not edited");
        for set in &game.encounters[2..] {
            assert!(
                set.encounters
                    .iter()
                    .all(|e| (e.species, e.min_level, e.max_level) == (25, 7, 9))
            );
        }
        // Without time of day, each set is its own.
        let mut plain = ppr_romio::open(&request.input).expect("loads").game_data().clone();
        let untouched = plain.encounters[2].clone();
        let plain_original = plain.clone();
        let origins: Vec<usize> = (0..plain.encounters.len()).collect();
        apply_edits(&edits, &origins, &mut plain, &plain_original).expect("applies");
        assert_eq!(plain.encounters[1].encounters, game.encounters[2].encounters);
        assert_eq!(plain.encounters[2], untouched);
    }

    #[test]
    fn with_every_option_off_only_the_edits_change_the_rom() {
        let (_dir, mut request) = emerald_request(Settings::default(), 1);
        let mut list = EditList::new(&ppr_romio::open(&request.input).expect("loads").game_data().identity);
        list.set(ppr_model::Edit::Starter(ppr_model::StarterEdit { slot: 2, species: 1 }));
        request.edits = Some(list);
        run(&request).expect("randomizes");
        let saved = ppr_romio::open(&request.output).expect("output loads");
        assert_eq!(saved.game_data().starters, [252, 255, 1]);
        let mut empty = request.clone();
        empty.edits = Some(EditList::new(&saved.game_data().identity));
        empty.output = request.output.with_file_name("same.gba");
        let report = run(&empty).expect("randomizes");
        assert_eq!(report.manual_edits, 0, "an empty list is no edits");
        assert_eq!(
            std::fs::read(&empty.input).expect("readable"),
            std::fs::read(&empty.output).expect("readable")
        );
    }

    #[test]
    fn edits_that_dont_fit_write_nothing() {
        let (_dir, mut request) = emerald_request(random_starters(), 1);
        let mut edits = emerald_edits(&request);
        edits.set(ppr_model::Edit::Starter(ppr_model::StarterEdit {
            slot: 1,
            species: 9999,
        }));
        edits.set(ppr_model::Edit::FieldItem(ppr_model::FieldItemEdit {
            index: 99_999,
            item: 1,
        }));
        request.edits = Some(edits);
        let mut seen = Vec::new();
        let err = randomize_file(
            &request,
            &mut |p: Progress| seen.push(p.step.into_owned()),
            &CancelToken::default(),
        )
        .expect_err("rejected");
        assert_eq!(err.code(), "edits_rejected");
        let ServiceError::EditsRejected(issues) = &err else {
            panic!("{err}");
        };
        assert_eq!(issues.len(), 2, "every issue is reported: {issues:?}");
        assert!(
            err.to_string().starts_with("2 of the manual edits can't be applied: "),
            "{err}"
        );
        assert!(seen.is_empty(), "rejected before randomizing: {seen:?}");
        assert!(!request.output.exists());

        let mut other = emerald_edits(&request);
        other.game.name = "Ruby (U)".into();
        request.edits = Some(other);
        assert_eq!(run(&request).expect_err("another game").code(), "edits_wrong_game");
        assert!(!request.output.exists());
    }

    #[test]
    fn applying_edits_after_the_randomizers_is_all_or_nothing() {
        let (_dir, request) = emerald_request(Settings::default(), 1);
        let mut game = ppr_romio::open(&request.input).expect("loads").game_data().clone();
        let mut edits = emerald_edits(&request);
        // Randomizers never remove an object, but should one be gone, no edit is applied.
        game.shops.clear();
        let before = game.clone();
        let origins: Vec<usize> = (0..game.encounters.len()).collect();
        let err = apply_edits(&edits, &origins, &mut game, &before).expect_err("the shop is gone");
        assert_eq!(err.code(), "edits_rejected");
        assert_eq!(game, before, "the starter and trainer edits were not applied either");
        edits.edits.pop();
        apply_edits(&edits, &origins, &mut game, &before).expect("applies");
        assert_eq!(game.starters[0], 25);
    }

    #[test]
    fn wild_pokemon_are_saved_and_reported() {
        let mut settings = Settings::default();
        settings.wild.mode = ppr_settings::WildMode::Random;
        settings.wild.min_catch_rate = 4;
        let (_dir, request) = emerald_request(settings, 11);
        let report = run(&request).expect("randomizes");
        assert_eq!(report.wild_sets, 209, "as many as the real Emerald");
        assert_eq!(report.wild_sets_changed, report.wild_sets);
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let land = &saved.game_data().encounters[0];
        assert!(land.encounters.iter().any(|e| e.species != 10), "changed");
        assert!(saved.game_data().species.iter().all(|s| s.catch_rate == 255));
    }

    #[test]
    fn static_pokemon_are_saved_and_reported() {
        let mut settings = Settings::default();
        settings.statics.mode = ppr_settings::StaticMode::RandomMatching;
        settings.statics.fix_music = true;
        settings.statics.level_modifier = 50;
        let (_dir, request) = emerald_request(settings, 5);
        let original = ppr_romio::open(&request.input)
            .expect("input loads")
            .game_data()
            .clone();
        let report = run(&request).expect("randomizes");
        assert_eq!(report.statics, 26, "as many as the real Emerald");
        assert!(report.statics_changed > 0);
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let statics = &saved.game_data().statics;
        assert_eq!(statics.len(), 26);
        for (old, new) in original.statics.iter().zip(statics) {
            assert_eq!(
                ppr_model::is_legendary(old.species),
                ppr_model::is_legendary(new.species)
            );
            if !old.egg {
                assert!(new.level > old.level, "{old:?} -> {new:?}");
            }
        }
    }

    #[test]
    fn trades_are_saved_and_reported() {
        let mut settings = Settings::default();
        settings.trades.mode = ppr_settings::TradeMode::GivenAndRequested;
        settings.trades.random_nicknames = true;
        let (_dir, request) = emerald_request(settings, 9);
        let report = run(&request).expect("randomizes");
        assert_eq!(
            (report.trades, report.trades_changed),
            (4, 4),
            "as many as the real Emerald"
        );
        let saved = ppr_romio::open(&request.output).expect("output loads");
        assert!(saved.game_data().trades.iter().all(|t| !t.nickname.starts_with("NICK")));
    }

    #[test]
    fn base_stats_are_saved_and_reported() {
        let mut settings = Settings::default();
        settings.base_stats.mode = ppr_settings::BaseStatMode::Random;
        settings.base_stats.follow_evolutions = true;
        let (_dir, request) = emerald_request(settings, 9);
        let original = ppr_romio::open(&request.input).expect("input loads");
        let report = run(&request).expect("randomizes");
        assert_eq!(report.species, 386);
        assert!(report.base_stats_changed > 300, "{}", report.base_stats_changed);
        let saved = ppr_romio::open(&request.output).expect("output loads");
        let changed = original
            .game_data()
            .species
            .iter()
            .zip(&saved.game_data().species)
            .filter(|(b, a)| b.stats != a.stats)
            .count();
        assert_eq!(u32::try_from(changed).expect("small"), report.base_stats_changed);
        for (b, a) in original.game_data().species.iter().zip(&saved.game_data().species) {
            assert_eq!(b.stats.total(), a.stats.total(), "{}", b.name);
        }
    }

    #[test]
    fn the_same_seed_writes_the_same_rom() {
        let (_a, first) = emerald_request(random_starters(), 7);
        let (_b, second) = emerald_request(random_starters(), 7);
        run(&first).expect("randomizes");
        run(&second).expect("randomizes");
        assert_eq!(
            std::fs::read(&first.output).expect("readable"),
            std::fs::read(&second.output).expect("readable")
        );
    }

    #[test]
    fn unchanged_settings_write_an_identical_rom() {
        let (_dir, request) = emerald_request(Settings::default(), 1);
        let report = run(&request).expect("randomizes");
        assert!(report.starters.iter().all(|s| s.before == s.after));
        let same =
            std::fs::read(&request.input).expect("readable") == std::fs::read(&request.output).expect("readable");
        assert!(same, "no byte changed");
    }

    #[test]
    fn a_cancelled_run_writes_nothing() {
        let (_dir, request) = emerald_request(random_starters(), 1);
        let cancel = CancelToken::default();
        cancel.cancel();
        let err = randomize_file(&request, &mut |_: ppr_random::Progress| {}, &cancel).expect_err("cancelled");
        assert_eq!(err.code(), "cancelled");
        assert!(!request.output.exists());
    }

    #[test]
    fn a_log_is_made_only_when_asked_and_saved_next_to_the_output() {
        let (_dir, mut request) = emerald_request(random_starters(), 42);
        assert_eq!(run(&request).expect("randomizes").log, None);
        request.log = true;
        let report = run(&request).expect("randomizes");
        let log = report.log.as_deref().expect("a log");
        assert!(log.contains("Random seed: 42\n"));
        assert!(log.contains(&format!("Set starter 1 to {}\n", report.starters[0].after)));
        assert!(log.contains("File name: emerald.gba\n"));
        let path = save_log(&request.output, log).expect("saved");
        assert_eq!(path, request.output.with_file_name("out.log"));
        assert_eq!(std::fs::read_to_string(&path).expect("readable"), log);
    }

    #[test]
    fn the_report_serializes_the_seed_as_a_string() {
        let report = RunReport {
            seed: Seed(u64::MAX),
            output: PathBuf::from("out.gba"),
            starters: vec![StarterChange {
                before: "A".into(),
                after: "B".into(),
            }],
            starter_held_items: Vec::new(),
            species: 5,
            base_stats_changed: 1,
            types_changed: 2,
            abilities_changed: 3,
            exp_curves_changed: 4,
            evolutions_changed: 5,
            type_pairs: 289,
            type_pairs_changed: 14,
            moves: 6,
            moves_changed: 7,
            movesets_changed: 8,
            tms: 50,
            tms_changed: 9,
            tm_hm_compat_changed: 11,
            tutors: 30,
            tutors_changed: 10,
            tutor_compat_changed: 12,
            wild_sets: 2,
            wild_sets_changed: 1,
            trainers: 850,
            trainers_changed: 15,
            check_value: 0x285C_CECB,
            statics: 3,
            statics_changed: 0,
            totems: 18,
            totems_changed: 2,
            trades: 4,
            trades_changed: 4,
            field_items: 268,
            field_items_changed: 13,
            shops: 27,
            shops_changed: 3,
            pickup_items: 29,
            pickup_items_changed: 20,
            manual_edits: 2,
            log: Some("not serialized".into()),
        };
        let json = serde_json::to_string(&report).expect("serializes");
        assert_eq!(
            json,
            r#"{"seed":"18446744073709551615","output":"out.gba","starters":[{"before":"A","after":"B"}],"starterHeldItems":[],"species":5,"baseStatsChanged":1,"typesChanged":2,"abilitiesChanged":3,"expCurvesChanged":4,"evolutionsChanged":5,"typePairs":289,"typePairsChanged":14,"moves":6,"movesChanged":7,"movesetsChanged":8,"tms":50,"tmsChanged":9,"tmHmCompatChanged":11,"tutors":30,"tutorsChanged":10,"tutorCompatChanged":12,"wildSets":2,"wildSetsChanged":1,"trainers":850,"trainersChanged":15,"checkValue":677170891,"statics":3,"staticsChanged":0,"totems":18,"totemsChanged":2,"trades":4,"tradesChanged":4,"fieldItems":268,"fieldItemsChanged":13,"shops":27,"shopsChanged":3,"pickupItems":29,"pickupItemsChanged":20,"manualEdits":2}"#
        );
    }
}
