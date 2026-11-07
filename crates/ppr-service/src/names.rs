//! The Custom Names Editor's lists on disk: five UTF-8 text files with one name per line (`TrainerNames.txt` and so
//! on), so an existing folder of name lists in that layout can be used as it is.

use std::path::Path;

use ppr_settings::CustomNames;

use crate::ServiceError;

/// The file names, in the order of [`CustomNames`]' lists.
const FILES: [&str; 5] = [
    "TrainerNames.txt",
    "TrainerClasses.txt",
    "DoublesTrainerNames.txt",
    "DoublesTrainerClasses.txt",
    "PokemonNicknames.txt",
];

fn lists(names: &mut CustomNames) -> [&mut Vec<String>; 5] {
    [
        &mut names.trainer_names,
        &mut names.trainer_classes,
        &mut names.doubles_trainer_names,
        &mut names.doubles_trainer_classes,
        &mut names.pokemon_nicknames,
    ]
}

/// The built-in default custom names.
#[must_use]
pub fn default_custom_names() -> CustomNames {
    ppr_random::default_custom_names()
}

/// Reads the lists in `dir`; a list without its file keeps the defaults. Lines are trimmed.
pub fn load_custom_names(dir: &Path) -> Result<CustomNames, ServiceError> {
    let mut names = default_custom_names();
    for (file, list) in FILES.iter().zip(lists(&mut names)) {
        let path = dir.join(file);
        match std::fs::read_to_string(&path) {
            Ok(text) => *list = text.lines().map(|l| l.trim().to_owned()).collect(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(ServiceError::Open { path, source }),
        }
    }
    Ok(names)
}

/// Writes the lists to `dir`, creating it if needed.
pub fn save_custom_names(dir: &Path, names: &CustomNames) -> Result<(), ServiceError> {
    let write_error = |path: &Path, source| ServiceError::Write {
        path: path.to_path_buf(),
        source,
    };
    std::fs::create_dir_all(dir).map_err(|e| write_error(dir, e))?;
    let mut names = names.clone();
    for (file, list) in FILES.iter().zip(lists(&mut names)) {
        let path = dir.join(file);
        let mut text = list.join("\n");
        text.push('\n');
        std::fs::write(&path, text).map_err(|e| write_error(&path, e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_round_trip_and_missing_files_keep_the_defaults() {
        let dir = tempfile::tempdir().expect("temp dir");
        assert_eq!(load_custom_names(dir.path()).expect("loads"), default_custom_names());
        let mut names = default_custom_names();
        names.pokemon_nicknames = vec!["Pebble".into(), "Comet".into()];
        let sub = dir.path().join("names");
        save_custom_names(&sub, &names).expect("saves");
        assert_eq!(load_custom_names(&sub).expect("loads"), names);
        std::fs::remove_file(sub.join("TrainerNames.txt")).expect("removes");
        std::fs::write(sub.join("PokemonNicknames.txt"), " Mochi \r\nNova\n").expect("writes");
        let loaded = load_custom_names(&sub).expect("loads");
        assert_eq!(loaded.trainer_names, default_custom_names().trainer_names);
        assert_eq!(loaded.pokemon_nicknames, ["Mochi", "Nova"]);
    }
}
