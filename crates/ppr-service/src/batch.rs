//! Batch randomization: one ROM randomized many times, each with its own seed, into `<prefix><index>.<extension>`
//! files.
//!
//! Deliberate choices:
//! - A batch can be cancelled; the ROMs written before stay, and the report says where to continue.
//! - The extension is the input's own, since every handler of this app writes the format it read.
//! - A log replaces the ROM's extension with the log ending (`random3.log`), like a single run's log here.
//! - Saved as LayeredFS (always with a game update), each game is a folder without extension (`random3/`) holding
//!   its LayeredFS folder, as in the original.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use ppr_model::EditList;
use ppr_random::{CancelToken, Progress, RandomizeError, Seed};
use ppr_settings::Settings;
use serde::Serialize;
use ts_rs::TS;

use crate::{RandomizeRequest, ServiceError, randomize_file};

/// A batch of randomizations of one ROM.
#[derive(Debug, Clone)]
pub struct BatchRequest {
    /// The ROM to read. It is never modified.
    pub input: PathBuf,
    /// A decrypted game update of a 3DS game to lay over it first; each game is then a LayeredFS folder.
    pub update: Option<PathBuf>,
    /// Save each 3DS game as a LayeredFS folder instead of a ROM, also without an update; see
    /// [`RandomizeRequest::layered_fs`].
    pub layered_fs: bool,
    /// The folder the ROMs go to.
    pub dir: PathBuf,
    /// The start of every file name.
    pub prefix: String,
    /// The index of the first ROM.
    pub start: u32,
    /// How many ROMs to make.
    pub count: u32,
    /// The settings, the same for every ROM.
    pub settings: Settings,
    /// Save a log next to each ROM, with this extension; `None` for no logs.
    pub log_ending: Option<String>,
    /// Manual edits to apply to every ROM after the randomizers; `None` or an empty list for none.
    pub edits: Option<EditList>,
}

/// How far a batch is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchProgress {
    /// The ROM being made, from 1.
    pub rom: u32,
    /// How many ROMs the batch makes.
    pub count: u32,
    /// Of the whole batch, 0 to 100.
    pub percent: u8,
    /// The step of the current ROM.
    pub step: Cow<'static, str>,
}

/// One ROM of a batch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchRom {
    /// Where it was written.
    pub output: PathBuf,
    /// Its seed, as a decimal string.
    #[ts(type = "string")]
    pub seed: Seed,
    /// Its log, when the batch saves logs.
    pub log: Option<PathBuf>,
}

/// What a batch did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BatchReport {
    /// The ROMs written, in order.
    pub roms: Vec<BatchRom>,
    /// The index after the last ROM written, where the next batch continues.
    pub next_start: u32,
    /// Whether the batch was cancelled before the last ROM.
    pub cancelled: bool,
}

/// Where ROM `index` of a batch goes: `<dir>/<prefix><index>.<extension of input>`, or the folder
/// `<dir>/<prefix><index>` for a game saved as LayeredFS (`folder`), which gets its LayeredFS folder.
#[must_use]
pub fn batch_output(dir: &Path, prefix: &str, index: u32, input: &Path, folder: bool) -> PathBuf {
    let mut name = format!("{prefix}{index}");
    if let Some(ext) = input.extension().filter(|_| !folder) {
        name.push('.');
        name.push_str(&ext.to_string_lossy());
    }
    dir.join(name)
}

/// Randomizes `request.input` `request.count` times. Stops at the first failure; a cancel stops the batch and
/// reports the ROMs written so far.
pub fn randomize_batch(
    request: &BatchRequest,
    progress: &mut dyn FnMut(BatchProgress),
    cancel: &CancelToken,
) -> Result<BatchReport, ServiceError> {
    request.settings.validate()?;
    let layered_fs = request.layered_fs || request.update.is_some();
    let mut roms = Vec::new();
    for (done, index) in (request.start..request.start.saturating_add(request.count)).enumerate() {
        let done = u32::try_from(done).unwrap_or(u32::MAX);
        let output = batch_output(&request.dir, &request.prefix, index, &request.input, layered_fs);
        let run = RandomizeRequest {
            input: request.input.clone(),
            update: request.update.clone(),
            layered_fs,
            output: output.clone(),
            settings: request.settings.clone(),
            seed: None,
            log: request.log_ending.is_some(),
            edits: request.edits.clone(),
        };
        let mut sink = |p: Progress| {
            let whole = (u64::from(done) * 100 + u64::from(p.percent)) / u64::from(request.count.max(1));
            progress(BatchProgress {
                rom: done + 1,
                count: request.count,
                percent: u8::try_from(whole.min(100)).unwrap_or(100),
                step: p.step,
            });
        };
        let report = match randomize_file(&run, &mut sink, cancel) {
            Ok(report) => report,
            Err(ServiceError::Randomize(RandomizeError::Cancelled)) => {
                return Ok(BatchReport {
                    roms,
                    next_start: index,
                    cancelled: true,
                });
            }
            Err(e) => return Err(e),
        };
        let log = match (&request.log_ending, &report.log) {
            (Some(ending), Some(text)) => Some(crate::randomize::save_log_as(&output, ending, text)?),
            _ => None,
        };
        roms.push(BatchRom {
            output,
            seed: report.seed,
            log,
        });
    }
    Ok(BatchReport {
        roms,
        next_start: request.start.saturating_add(request.count),
        cancelled: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn games_saved_as_layered_fs_get_folders() {
        let (dir, input) = (Path::new("out"), Path::new("x.3ds"));
        assert_eq!(batch_output(dir, "random", 3, input, false), dir.join("random3.3ds"));
        assert_eq!(batch_output(dir, "random", 3, input, true), dir.join("random3"));
    }

    fn request(dir: &Path, count: u32, logs: bool) -> BatchRequest {
        let input = dir.join("emerald.gba");
        std::fs::write(&input, ppr_romio::testing::emerald()).expect("write");
        let mut settings = Settings::default();
        settings.starters.mode = ppr_settings::StarterMode::Random;
        BatchRequest {
            input,
            update: None,
            layered_fs: false,
            dir: dir.join("out"),
            prefix: "random".into(),
            start: 3,
            count,
            settings,
            log_ending: logs.then(|| "txt".into()),
            edits: None,
        }
    }

    #[test]
    fn writes_numbered_roms_with_their_own_seeds_and_logs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let req = request(dir.path(), 3, true);
        std::fs::create_dir(&req.dir).expect("out dir");
        let mut seen = Vec::new();
        let report = randomize_batch(&req, &mut |p| seen.push(p), &CancelToken::default()).expect("runs");
        let names: Vec<_> = report
            .roms
            .iter()
            .map(|r| r.output.file_name().expect("a name").to_owned())
            .collect();
        assert_eq!(names, ["random3.gba", "random4.gba", "random5.gba"]);
        assert!(report.roms.iter().all(|r| r.output.exists()));
        assert!(report.roms[0].seed != report.roms[1].seed, "fresh seeds");
        assert_eq!(
            report.roms[2].log.as_deref(),
            Some(req.dir.join("random5.txt").as_path())
        );
        assert!(req.dir.join("random5.txt").exists());
        assert_eq!((report.next_start, report.cancelled), (6, false));
        assert_eq!(seen.first().map(|p| (p.rom, p.count)), Some((1, 3)));
        assert_eq!(seen.last().map(|p| (p.rom, p.percent)), Some((3, 100)));
        assert!(
            seen.windows(2).all(|w| w[0].percent <= w[1].percent),
            "progress only grows"
        );
    }

    #[test]
    fn only_3ds_games_are_saved_as_layered_fs() {
        let dir = tempfile::tempdir().expect("temp dir");
        let mut req = request(dir.path(), 2, false);
        req.layered_fs = true;
        std::fs::create_dir(&req.dir).expect("out dir");
        let err = randomize_batch(&req, &mut |_| {}, &CancelToken::default()).expect_err("not a 3DS game");
        assert_eq!(err.code(), "rom_unsupported");
        assert_eq!(
            std::fs::read_dir(&req.dir).expect("readable").count(),
            0,
            "nothing written"
        );
    }

    #[test]
    fn a_cancelled_batch_keeps_what_it_wrote() {
        let dir = tempfile::tempdir().expect("temp dir");
        let req = request(dir.path(), 4, false);
        std::fs::create_dir(&req.dir).expect("out dir");
        let cancel = CancelToken::default();
        let report = randomize_batch(
            &req,
            &mut |p| {
                if p.rom == 2 {
                    cancel.cancel();
                }
            },
            &cancel,
        )
        .expect("stops cleanly");
        assert!(report.cancelled);
        assert_eq!(report.roms.len(), 1);
        assert_eq!(report.next_start, 4);
        assert!(!req.dir.join("random4.gba").exists());
        assert!(report.roms[0].log.is_none());
    }
}
