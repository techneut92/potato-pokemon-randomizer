//! Tauri commands. Each one only adapts `ppr-service` to IPC.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use ppr_service::{
    ApiError, BatchReport, BatchRequest, CancelToken, CustomNames, EditGame, EditIssue, EditList, EditableGame,
    LoadedRom, PremadeSeed, Progress, RandomizeRequest, RomReport, RomSession, RunReport, Seed, Settings,
    SharedSettings, SupportedGame, randomize_file, save_log,
};
use serde::Serialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

/// Runs blocking service work off the async runtime and flattens both failure kinds into an [`ApiError`].
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, ppr_service::ServiceError> + Send + 'static,
) -> Result<T, ApiError> {
    tauri::async_runtime::spawn_blocking(move || work().map_err(|e| ApiError::from(&e)))
        .await
        .map_err(|e| ApiError {
            code: "internal",
            message: e.to_string(),
        })?
}

/// Identifies the ROM at `path` (chosen by the user through the dialog plugin).
#[tauri::command]
pub(crate) async fn identify_rom(path: PathBuf) -> Result<RomReport, ApiError> {
    blocking(move || ppr_service::identify_file(&path)).await
}

/// The ROM open in the app, with its data as loaded and its manual edits; `None` before one is opened or when its
/// generation isn't ported.
#[derive(Debug, Default)]
pub(crate) struct OpenRom(Mutex<Option<RomSession>>);

impl OpenRom {
    fn lock(&self) -> MutexGuard<'_, Option<RomSession>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `f` on the open ROM when it is the one at `path`; fails with `no_rom` otherwise.
    fn with<T>(&self, path: &Path, f: impl FnOnce(&mut RomSession) -> Result<T, ApiError>) -> Result<T, ApiError> {
        match self.lock().as_mut().filter(|s| s.path() == path) {
            Some(session) => f(session),
            None => Err(ApiError {
                code: "no_rom",
                message: format!("{} is not open, or can't be edited", path.display()),
            }),
        }
    }
}

/// Opens the ROM at `path` for the main screen, with the decrypted game update at `update` laid over it (3DS games).
/// The editor and validation see the game with its update. Its manual edits start empty, unless it is the ROM that
/// was open.
#[tauri::command]
pub(crate) async fn open_rom(
    path: PathBuf,
    update: Option<PathBuf>,
    open: State<'_, OpenRom>,
) -> Result<LoadedRom, ApiError> {
    let (rom, mut session) = blocking(move || RomSession::open(&path, update.as_deref())).await?;
    let mut current = open.lock();
    if let Some(session) = session.as_mut() {
        session.keep_edits_of(current.take());
    }
    *current = session;
    Ok(rom)
}

/// What the manual edits editor shows of the open ROM at `path`: its objects as loaded, with their names.
#[tauri::command]
pub(crate) async fn editable_game(path: PathBuf, open: State<'_, OpenRom>) -> Result<EditableGame, ApiError> {
    open.with(&path, |s| Ok(s.editable()))
}

/// Checks `edits` against the open ROM at `path` without keeping them. Fails with `edits_wrong_game` or
/// `edits_unsupported_version` for a list that isn't for this game or this version.
#[tauri::command]
pub(crate) async fn validate_edits(
    path: PathBuf,
    edits: EditList,
    open: State<'_, OpenRom>,
) -> Result<Vec<EditIssue>, ApiError> {
    open.with(&path, |s| s.validate(&edits).map_err(|e| ApiError::from(&e)))
}

/// Keeps `edits` as the manual edits of the open ROM at `path` and returns their issues. Fails like
/// [`validate_edits`], keeping the edits as they were.
#[tauri::command]
pub(crate) async fn set_edits(
    path: PathBuf,
    edits: EditList,
    open: State<'_, OpenRom>,
) -> Result<Vec<EditIssue>, ApiError> {
    open.with(&path, |s| s.set_edits(edits).map_err(|e| ApiError::from(&e)))
}

/// Every supported ROM.
#[tauri::command]
pub(crate) fn supported_games() -> Vec<SupportedGame> {
    ppr_service::supported_games()
}

/// Every step a run without manual edits can report, for the progress dialog.
#[tauri::command]
pub(crate) fn randomization_steps() -> Vec<String> {
    ppr_service::randomization_steps(0)
}

/// Progress of a run, as sent to the window.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProgressEvent {
    percent: u8,
    step: Cow<'static, str>,
}

/// The cancel token of the run in progress, if any. The UI runs one randomization at a time.
#[derive(Debug, Default)]
pub(crate) struct CurrentRun(Mutex<Option<CancelToken>>);

impl CurrentRun {
    fn set(&self, token: Option<CancelToken>) {
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = token;
    }

    fn cancel(&self) {
        if let Some(token) = self.0.lock().unwrap_or_else(PoisonError::into_inner).as_ref() {
            token.cancel();
        }
    }
}

/// The log of the last run and the ROM it belongs to, kept until the user decides whether to save it.
#[derive(Debug, Default)]
pub(crate) struct LastLog(Mutex<Option<(PathBuf, String)>>);

/// Randomizes `input` into `output`, reporting progress on `on_progress`, and applies `edits` after the randomizers,
/// all or none. With `layered_fs` (a 3DS game) or `update`, which lays the game update over the game first, the game
/// is saved as a LayeredFS folder and `output` is the folder it goes into. With `log` (off in Race Mode) the run's log
/// is kept for [`save_run_log`].
#[tauri::command]
#[expect(clippy::too_many_arguments, reason = "one per IPC argument plus the managed state")]
pub(crate) async fn randomize(
    input: PathBuf,
    update: Option<PathBuf>,
    layered_fs: bool,
    output: PathBuf,
    settings: Settings,
    seed: Option<Seed>,
    log: bool,
    edits: Option<EditList>,
    on_progress: Channel<ProgressEvent>,
    run: State<'_, CurrentRun>,
    last_log: State<'_, LastLog>,
) -> Result<RunReport, ApiError> {
    let cancel = CancelToken::default();
    run.set(Some(cancel.clone()));
    *last_log.0.lock().unwrap_or_else(PoisonError::into_inner) = None;
    let request = RandomizeRequest {
        input,
        update,
        layered_fs,
        output,
        settings,
        seed,
        log,
        edits,
    };
    let result = blocking(move || {
        let mut sink = |p: Progress| {
            // A closed window is no reason to stop the run; the result is still written.
            let _ = on_progress.send(ProgressEvent {
                percent: p.percent,
                step: p.step,
            });
        };
        randomize_file(&request, &mut sink, &cancel)
    })
    .await;
    run.set(None);
    let mut report = result?;
    if let Some(text) = report.log.take() {
        *last_log.0.lock().unwrap_or_else(PoisonError::into_inner) = Some((report.output.clone(), text));
    }
    Ok(report)
}

/// Progress of a batch, as sent to the window.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct BatchProgressEvent {
    percent: u8,
    step: Cow<'static, str>,
    rom: u32,
    count: u32,
}

/// Randomizes `input` `count` times into `dir` (`<prefix><index>.<ext>`, or a `<prefix><index>` folder with a
/// LayeredFS folder with `layered_fs` or when `update` is laid over the game), each with its own seed, reporting progress on
/// `on_progress`, and applies `edits` to each after the randomizers. With `log_ending` each ROM gets a log with that
/// extension.
#[tauri::command]
#[expect(clippy::too_many_arguments, reason = "one per IPC argument plus the managed state")]
pub(crate) async fn randomize_batch(
    input: PathBuf,
    update: Option<PathBuf>,
    layered_fs: bool,
    dir: PathBuf,
    prefix: String,
    start: u32,
    count: u32,
    settings: Settings,
    log_ending: Option<String>,
    edits: Option<EditList>,
    on_progress: Channel<BatchProgressEvent>,
    run: State<'_, CurrentRun>,
) -> Result<BatchReport, ApiError> {
    let cancel = CancelToken::default();
    run.set(Some(cancel.clone()));
    let request = BatchRequest {
        input,
        update,
        layered_fs,
        dir,
        prefix,
        start,
        count,
        settings,
        log_ending,
        edits,
    };
    let result = blocking(move || {
        let mut sink = |p: ppr_service::BatchProgress| {
            // A closed window is no reason to stop the batch.
            let _ = on_progress.send(BatchProgressEvent {
                percent: p.percent,
                step: p.step,
                rom: p.rom,
                count: p.count,
            });
        };
        ppr_service::randomize_batch(&request, &mut sink, &cancel)
    })
    .await;
    run.set(None);
    result
}

/// Saves the log of the last run next to its ROM and returns where. Fails when the last run made no log.
#[tauri::command]
pub(crate) async fn save_run_log(last_log: State<'_, LastLog>) -> Result<PathBuf, ApiError> {
    let Some((output, text)) = last_log.0.lock().unwrap_or_else(PoisonError::into_inner).take() else {
        return Err(ApiError {
            code: "no_log",
            message: "the last randomization made no log".into(),
        });
    };
    blocking(move || save_log(&output, &text)).await
}

/// The settings string of `shared`.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri deserializes IPC arguments by value")]
pub(crate) fn settings_string(shared: SharedSettings) -> String {
    ppr_service::settings_string(&shared)
}

/// Reads a settings string, from this app or an older compatible randomizer.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri deserializes IPC arguments by value")]
pub(crate) fn read_settings_string(text: String) -> Result<SharedSettings, ApiError> {
    ppr_service::read_settings_string(&text).map_err(|e| ApiError::from(&e))
}

/// Saves `shared` as a `.rnqs` settings file at `path` and returns where.
#[tauri::command]
pub(crate) async fn save_settings_file(path: PathBuf, shared: SharedSettings) -> Result<PathBuf, ApiError> {
    blocking(move || ppr_service::save_settings_file(&path, &shared)).await
}

/// Reads the `.rnqs` settings file at `path`.
#[tauri::command]
pub(crate) async fn load_settings_file(path: PathBuf) -> Result<SharedSettings, ApiError> {
    blocking(move || ppr_service::load_settings_file(&path)).await
}

/// Saves `premade` as a `.rndp` premade seed file at `path` and returns where.
#[tauri::command]
pub(crate) async fn save_premade_seed(path: PathBuf, premade: PremadeSeed) -> Result<PathBuf, ApiError> {
    blocking(move || ppr_service::save_premade_seed(&path, &premade)).await
}

/// Reads the `.rndp` premade seed file at `path`.
#[tauri::command]
pub(crate) async fn load_premade_seed(path: PathBuf) -> Result<PremadeSeed, ApiError> {
    blocking(move || ppr_service::load_premade_seed(&path)).await
}

/// Saves `list`, the loaded game's manual edits, as an edits file at `path` and returns where.
#[tauri::command]
pub(crate) async fn save_edits_file(path: PathBuf, list: EditList) -> Result<PathBuf, ApiError> {
    blocking(move || ppr_service::save_edits_file(&path, &list)).await
}

/// Reads the edits file at `path`, checked against `game`, the game the manual editor shows.
#[tauri::command]
pub(crate) async fn load_edits_file(path: PathBuf, game: EditGame) -> Result<EditList, ApiError> {
    blocking(move || ppr_service::load_edits_file(&path, &game)).await
}

/// Where the Custom Names Editor's lists are kept: five text files in the app's config folder.
fn names_dir(app: &AppHandle) -> Result<PathBuf, ApiError> {
    app.path()
        .app_config_dir()
        .map(|d| d.join("custom-names"))
        .map_err(|e| ApiError {
            code: "internal",
            message: e.to_string(),
        })
}

/// The Custom Names Editor's lists: the saved ones, or the built-in defaults.
#[tauri::command]
pub(crate) async fn custom_names(app: AppHandle) -> Result<CustomNames, ApiError> {
    let dir = names_dir(&app)?;
    blocking(move || ppr_service::load_custom_names(&dir)).await
}

/// Saves the Custom Names Editor's lists.
#[tauri::command]
pub(crate) async fn save_custom_names(app: AppHandle, names: CustomNames) -> Result<(), ApiError> {
    let dir = names_dir(&app)?;
    blocking(move || ppr_service::save_custom_names(&dir, &names)).await
}

/// Cancels the run in progress; nothing is written. Does nothing when no run is in progress.
#[tauri::command]
#[expect(clippy::needless_pass_by_value, reason = "Tauri passes managed state by value")]
pub(crate) fn cancel_randomize(run: State<'_, CurrentRun>) {
    run.cancel();
}

/// Opens the support page in the default browser, off the async runtime so waiting for the opener doesn't block the
/// window.
#[tauri::command]
pub(crate) async fn open_support_page() -> Result<(), ApiError> {
    blocking(ppr_service::open_support_page).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(name: &str, code: &str) -> EditGame {
        EditGame {
            name: name.into(),
            code: code.into(),
            version: Some(0),
        }
    }

    #[test]
    fn edits_files_round_trip_and_name_the_other_game() {
        let dir = std::env::temp_dir().join(format!("ppr-edits-command-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let list = EditList {
            version: 1,
            game: game("Ruby (U)", "AXVE"),
            edits: Vec::new(),
        };
        let saved = tauri::async_runtime::block_on(save_edits_file(dir.join("ruby"), list.clone())).expect("saves");
        assert_eq!(saved, dir.join("ruby.ppredits.json"));

        let loaded = tauri::async_runtime::block_on(load_edits_file(saved.clone(), game("Ruby (U)", "AXVE")));
        let refused = tauri::async_runtime::block_on(load_edits_file(saved, game("Emerald (U)", "BPEE")));
        std::fs::remove_dir_all(&dir).expect("cleans up");

        assert_eq!(loaded.expect("loads"), list);
        let e = refused.expect_err("refused");
        assert_eq!(e.code, "edits_wrong_game");
        assert_eq!(
            e.message,
            "This file is for Pokémon Ruby (U), but Pokémon Emerald (U) is loaded."
        );
    }
}
