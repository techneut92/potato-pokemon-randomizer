//! Premade seed files (`.rndp`): a seed and a settings string, so others can make the same randomization.
//!
//! A file holds the format version as a big-endian `u32`, the seed as a big-endian `i64` and the settings string with
//! its version, behind its length as a big-endian `u16`.
//!
//! The same seed and settings give the same game in this app, but not in other randomizers that read these files
//! (ADR 0005): the randomizers draw their numbers differently.

use std::path::{Path, PathBuf};

use ppr_random::Seed;
use ppr_settings::{SETTINGS_VERSION, SettingsStringError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{ServiceError, SharedSettings, read_settings_string, settings_string};

/// The extension of premade seed files.
pub const PREMADE_SEED_EXTENSION: &str = "rndp";

/// A seed and the settings to randomize with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PremadeSeed {
    /// The seed, as a decimal string.
    #[ts(type = "string")]
    pub seed: Seed,
    /// The settings.
    pub shared: SharedSettings,
}

/// The `.rndp` bytes for `premade`.
#[must_use]
pub fn premade_seed_bytes(premade: &PremadeSeed) -> Vec<u8> {
    let text = settings_string(&premade.shared);
    let mut out = Vec::with_capacity(14 + text.len());
    out.extend_from_slice(&SETTINGS_VERSION.to_be_bytes());
    out.extend_from_slice(&premade.seed.0.to_be_bytes());
    // A settings string is Base64 behind three digits, far below the 65535 bytes its length allows.
    out.extend_from_slice(&u16::try_from(text.len()).unwrap_or(u16::MAX).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    out
}

/// Reads `.rndp` bytes.
pub fn read_premade_seed(bytes: &[u8]) -> Result<PremadeSeed, ServiceError> {
    let malformed = || ServiceError::SettingsString(SettingsStringError::Malformed);
    let version = u32::from_be_bytes(
        bytes
            .get(..4)
            .ok_or_else(malformed)?
            .try_into()
            .map_err(|_| malformed())?,
    );
    match version {
        SETTINGS_VERSION => {}
        v if v > SETTINGS_VERSION => return Err(SettingsStringError::NewerVersion(v).into()),
        v => return Err(SettingsStringError::OldVersion(v).into()),
    }
    let seed = u64::from_be_bytes(
        bytes
            .get(4..12)
            .ok_or_else(malformed)?
            .try_into()
            .map_err(|_| malformed())?,
    );
    let length = usize::from(u16::from_be_bytes(
        bytes
            .get(12..14)
            .ok_or_else(malformed)?
            .try_into()
            .map_err(|_| malformed())?,
    ));
    let text = bytes.get(14..14 + length).ok_or_else(malformed)?;
    let text = std::str::from_utf8(text).map_err(|_| malformed())?;
    Ok(PremadeSeed {
        seed: Seed(seed),
        shared: read_settings_string(text)?,
    })
}

/// `path` with the `.rndp` extension, added when missing.
#[must_use]
pub fn premade_seed_path(path: &Path) -> PathBuf {
    crate::settings_files::with_extension(path, PREMADE_SEED_EXTENSION)
}

/// Saves `premade` as a `.rndp` file at `path` (extension added when missing) and returns where.
pub fn save_premade_seed(path: &Path, premade: &PremadeSeed) -> Result<PathBuf, ServiceError> {
    let path = premade_seed_path(path);
    std::fs::write(&path, premade_seed_bytes(premade)).map_err(|source| ServiceError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Reads the `.rndp` file at `path`.
pub fn load_premade_seed(path: &Path) -> Result<PremadeSeed, ServiceError> {
    let bytes = std::fs::read(path).map_err(|source| ServiceError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    read_premade_seed(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A reference file, for seed -4242424242424242.
    const REFERENCE_FILE: &[u8] = include_bytes!("reference-preset.rndp");

    #[test]
    fn reads_and_rewrites_reference_files_exactly() {
        let premade = read_premade_seed(REFERENCE_FILE).expect("reads");
        assert_eq!(premade.seed, "-4242424242424242".parse().expect("a seed"));
        assert_eq!(premade.shared.extras.rom_name, "Pokemon Emerald (U)");
        assert_eq!(premade_seed_bytes(&premade), REFERENCE_FILE);
    }

    #[test]
    fn files_round_trip_and_get_their_extension() {
        let premade = read_premade_seed(REFERENCE_FILE).expect("reads");
        let dir = tempfile::tempdir().expect("temp dir");
        let saved = save_premade_seed(&dir.path().join("race"), &premade).expect("saves");
        assert_eq!(saved, dir.path().join("race.rndp"));
        assert_eq!(load_premade_seed(&saved).expect("loads"), premade);
    }

    #[test]
    fn damaged_and_foreign_files_are_refused() {
        assert_eq!(
            read_premade_seed(&REFERENCE_FILE[..10]).expect_err("short").code(),
            "settings_invalid"
        );
        let mut newer = REFERENCE_FILE.to_vec();
        newer[3] += 1;
        assert_eq!(read_premade_seed(&newer).expect_err("newer").code(), "settings_too_new");
        let mut damaged = REFERENCE_FILE.to_vec();
        damaged[20] ^= 0x20;
        assert_eq!(
            read_premade_seed(&damaged).expect_err("damaged").code(),
            "settings_invalid"
        );
    }
}
