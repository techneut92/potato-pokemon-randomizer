//! Edits files (`.ppredits.json`): a game's manual edits, saved so they can be loaded again (ADR 0008).
//!
//! A file is the [`EditList`] as pretty-printed JSON. Loading checks the format version first, from the version
//! field alone, so a file from a newer build is reported as such even when its edits no longer parse here; then
//! that the file is for the loaded game.

use std::path::{Path, PathBuf};

use ppr_model::{EDIT_LIST_VERSION, EditError, EditGame, EditList};
use serde::Deserialize;

use crate::ServiceError;

/// The extension of edits files. They are JSON, so the name ends in `.json` and any JSON viewer opens them.
pub const EDITS_EXTENSION: &str = "ppredits.json";

/// Why an edits file can't be loaded for the loaded game.
#[derive(Debug, thiserror::Error)]
pub enum EditFileError {
    /// The file isn't JSON, or not an edit list.
    #[error("this file isn't a manual edits file: {0}")]
    Invalid(String),
    /// The file was written by a newer build, in a format this one can't read.
    #[error(
        "this file was saved by a newer version (edits format {found}; this version reads format {EDIT_LIST_VERSION})"
    )]
    TooNew {
        /// The file's format version.
        found: u32,
    },
    /// The file is in an older format this build no longer reads.
    #[error(
        "this file is in edits format {found}, which this version can't read (it reads format {EDIT_LIST_VERSION})"
    )]
    Unsupported {
        /// The file's format version.
        found: u32,
    },
    /// The file is for another game. Games are named the way the app shows them.
    #[error("This file is for Pokémon {found}, but Pokémon {loaded} is loaded.")]
    WrongGame {
        /// The game the file is for.
        found: String,
        /// The loaded game.
        loaded: String,
    },
}

impl From<EditError> for EditFileError {
    fn from(e: EditError) -> Self {
        match e {
            EditError::UnsupportedVersion { found } if found > EDIT_LIST_VERSION => Self::TooNew { found },
            EditError::UnsupportedVersion { found } => Self::Unsupported { found },
            EditError::WrongGame { expected, found } => Self::WrongGame {
                found,
                loaded: expected,
            },
            // `check_game` returns only the two above.
            other => Self::Invalid(other.to_string()),
        }
    }
}

impl EditFileError {
    /// Stable machine-readable code.
    pub(crate) fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "edits_invalid",
            Self::TooNew { .. } => "edits_too_new",
            Self::Unsupported { .. } => "edits_unsupported_version",
            Self::WrongGame { .. } => "edits_wrong_game",
        }
    }
}

/// `path` with the `.ppredits.json` extension, added when the name doesn't end in `.json` already.
#[must_use]
pub fn edits_file_path(path: &Path) -> PathBuf {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")) {
        path.to_path_buf()
    } else {
        let mut name = path.as_os_str().to_owned();
        name.push(".");
        name.push(EDITS_EXTENSION);
        PathBuf::from(name)
    }
}

/// The JSON of an edits file for `list`.
#[must_use]
pub fn edits_file_text(list: &EditList) -> String {
    // An edit list is plain data with string keys; serializing it can't fail.
    let mut text = serde_json::to_string_pretty(list).unwrap_or_default();
    text.push('\n');
    text
}

/// Reads the JSON of an edits file and checks that it is for `game`.
pub fn read_edits_file(text: &str, game: &EditGame) -> Result<EditList, ServiceError> {
    /// The field every format version has.
    #[derive(Deserialize)]
    struct Version {
        version: u32,
    }
    let invalid = |e: serde_json::Error| EditFileError::Invalid(e.to_string());
    let Version { version } = serde_json::from_str(text).map_err(invalid)?;
    if version > EDIT_LIST_VERSION {
        return Err(EditFileError::TooNew { found: version }.into());
    }
    let list: EditList = serde_json::from_str(text).map_err(invalid)?;
    list.check_game(game).map_err(EditFileError::from)?;
    Ok(list)
}

/// Saves `list` as an edits file at `path` (extension added when missing) and returns where.
pub fn save_edits_file(path: &Path, list: &EditList) -> Result<PathBuf, ServiceError> {
    let path = edits_file_path(path);
    std::fs::write(&path, edits_file_text(list)).map_err(|source| ServiceError::Write {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

/// Reads the edits file at `path` and checks that it is for `game`, the loaded game.
pub fn load_edits_file(path: &Path, game: &EditGame) -> Result<EditList, ServiceError> {
    let bytes = std::fs::read(path).map_err(|source| ServiceError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let text = String::from_utf8_lossy(&bytes);
    read_edits_file(text.trim_start_matches('\u{feff}'), game)
}

#[cfg(test)]
mod tests {
    use ppr_model::{Edit, StarterEdit};

    use super::*;
    use crate::ApiError;

    fn emerald() -> EditGame {
        EditGame {
            name: "Emerald (U)".into(),
            code: "BPEE".into(),
            version: Some(0),
        }
    }

    fn list() -> EditList {
        EditList {
            version: EDIT_LIST_VERSION,
            game: emerald(),
            edits: vec![Edit::Starter(StarterEdit { slot: 1, species: 258 })],
        }
    }

    fn refused(text: &str) -> ApiError {
        ApiError::from(&read_edits_file(text, &emerald()).expect_err("refused"))
    }

    #[test]
    fn a_saved_file_loads_again() {
        let dir = tempfile::tempdir().expect("temp dir");
        let saved = save_edits_file(&dir.path().join("emerald"), &list()).expect("saves");
        assert_eq!(saved, dir.path().join("emerald.ppredits.json"));
        assert_eq!(load_edits_file(&saved, &emerald()).expect("loads"), list());
    }

    #[test]
    fn the_file_is_readable_json() {
        let text = edits_file_text(&list());
        assert!(
            text.starts_with("{\n  \"version\": 1,\n  \"game\": {\n    \"name\": \"Emerald (U)\""),
            "{text}"
        );
        assert!(text.contains("\"kind\": \"starter\""), "{text}");
        assert!(text.ends_with("}\n"));
    }

    #[test]
    fn the_extension_is_added_only_when_the_name_is_not_json() {
        assert_eq!(
            edits_file_path(Path::new("a/b.ppredits.json")),
            PathBuf::from("a/b.ppredits.json")
        );
        assert_eq!(edits_file_path(Path::new("a/b.JSON")), PathBuf::from("a/b.JSON"));
        assert_eq!(edits_file_path(Path::new("a/b")), PathBuf::from("a/b.ppredits.json"));
        assert_eq!(
            edits_file_path(Path::new("a/b.txt")),
            PathBuf::from("a/b.txt.ppredits.json")
        );
    }

    #[test]
    fn a_file_for_another_game_names_both() {
        let mut ruby = list();
        ruby.game = EditGame {
            name: "Ruby (U)".into(),
            code: "AXVE".into(),
            version: Some(0),
        };
        let e = refused(&edits_file_text(&ruby));
        assert_eq!(e.code, "edits_wrong_game");
        assert_eq!(
            e.message,
            "This file is for Pokémon Ruby (U), but Pokémon Emerald (U) is loaded."
        );
    }

    #[test]
    fn another_revision_of_the_game_is_another_game() {
        let mut revision = list();
        revision.game.version = Some(1);
        assert_eq!(refused(&edits_file_text(&revision)).code, "edits_wrong_game");
    }

    #[test]
    fn a_newer_format_is_named_before_its_edits_are_read() {
        // A future format may hold edits this build doesn't know.
        let text = r#"{ "version": 2, "game": { "name": "Emerald (U)" }, "edits": [{ "kind": "evolution" }] }"#;
        let e = refused(text);
        assert_eq!(e.code, "edits_too_new");
        assert!(e.message.contains("newer version"), "{}", e.message);
    }

    #[test]
    fn an_older_format_is_refused() {
        let mut old = list();
        old.version = 0;
        assert_eq!(refused(&edits_file_text(&old)).code, "edits_unsupported_version");
    }

    #[test]
    fn invalid_json_and_other_files_are_refused() {
        for text in ["", "not json", "{ \"version\": 1 }", "[1, 2]", r#"{ "settings": {} }"#] {
            let e = refused(text);
            assert_eq!(e.code, "edits_invalid", "{text}");
            assert!(
                e.message.starts_with("this file isn't a manual edits file"),
                "{}",
                e.message
            );
        }
    }

    #[test]
    fn a_missing_file_is_an_open_failure() {
        let e = load_edits_file(Path::new("/nonexistent/x.ppredits.json"), &emerald()).expect_err("refused");
        assert_eq!(e.code(), "open_failed");
    }
}
