//! Settings strings and `.rnqs` settings files, in a format shared with other randomizers, so settings travel
//! between them and this app.

use std::path::{Path, PathBuf};

use ppr_settings::{Settings, SettingsExtras};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::ServiceError;

/// The extension of settings files.
pub const SETTINGS_EXTENSION: &str = "rnqs";

/// Settings as a settings string or file holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedSettings {
    /// The options.
    pub settings: Settings,
    /// The game they were made for and Race Mode.
    pub extras: SettingsExtras,
}

/// The settings string of `shared`.
#[must_use]
pub fn settings_string(shared: &SharedSettings) -> String {
    ppr_settings::to_settings_string(&shared.settings, &shared.extras)
}

/// Reads a settings string, from this app or another randomizer that writes the format.
pub fn read_settings_string(text: &str) -> Result<SharedSettings, ServiceError> {
    let (settings, extras) = ppr_settings::from_settings_string(text)?;
    Ok(SharedSettings { settings, extras })
}

/// `path` with the `.rnqs` extension, added when missing.
#[must_use]
pub fn settings_file_path(path: &Path) -> PathBuf {
    with_extension(path, SETTINGS_EXTENSION)
}

/// `path` with `extension` added when it has another one or none.
pub(crate) fn with_extension(path: &Path, extension: &str) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(extension)) {
        path.to_path_buf()
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".");
        name.push(extension);
        PathBuf::from(name)
    }
}

/// Saves `shared` as a `.rnqs` file at `path` (extension added when missing) and returns where.
pub fn save_settings_file(path: &Path, shared: &SharedSettings) -> Result<PathBuf, ServiceError> {
    let path = settings_file_path(path);
    let bytes = ppr_settings::to_settings_file(&shared.settings, &shared.extras);
    std::fs::write(&path, bytes).map_err(|source| ServiceError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Reads a `.rnqs` settings file, from this app or another randomizer that writes the format.
pub fn load_settings_file(path: &Path) -> Result<SharedSettings, ServiceError> {
    let bytes = std::fs::read(path).map_err(|source| ServiceError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let (settings, extras) = ppr_settings::from_settings_file(&bytes)?;
    Ok(SharedSettings { settings, extras })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> SharedSettings {
        let mut s = SharedSettings::default();
        s.extras.rom_name = "Pokemon Emerald (U)".into();
        s.settings.starters.mode = ppr_settings::StarterMode::Random;
        s
    }

    #[test]
    fn strings_and_files_carry_the_settings() {
        let s = shared();
        let text = settings_string(&s);
        assert!(text.starts_with("428"));
        assert_eq!(read_settings_string(&text).expect("reads"), s);

        let dir = tempfile::tempdir().expect("temp dir");
        let saved = save_settings_file(&dir.path().join("mine"), &s).expect("saves");
        assert_eq!(saved, dir.path().join("mine.rnqs"));
        assert_eq!(load_settings_file(&saved).expect("loads"), s);
    }

    #[test]
    fn the_extension_is_added_only_when_missing() {
        assert_eq!(settings_file_path(Path::new("a/b.RNQS")), PathBuf::from("a/b.RNQS"));
        assert_eq!(settings_file_path(Path::new("a/b.txt")), PathBuf::from("a/b.txt.rnqs"));
    }

    #[test]
    fn bad_input_gets_its_own_codes() {
        let e = read_settings_string("nonsense").expect_err("refused");
        assert_eq!(e.code(), "settings_invalid");
        let e = read_settings_string(&format!("427{}", &settings_string(&shared())[3..])).expect_err("refused");
        assert_eq!(e.code(), "settings_too_old");
        let e = load_settings_file(Path::new("/nonexistent/x.rnqs")).expect_err("refused");
        assert_eq!(e.code(), "open_failed");
    }
}
