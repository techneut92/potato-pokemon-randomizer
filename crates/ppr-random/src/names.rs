//! The default custom names (`data/names`), and the lists a run picks names from.

use std::sync::LazyLock;

use ppr_settings::CustomNames;

fn lines(text: &'static str) -> Vec<&'static str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
}

/// Trainer names, also used as original trainers of traded Pokémon.
pub(crate) static TRAINER_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| lines(include_str!("../data/names/TrainerNames.txt")));

/// Trainer class names.
pub(crate) static TRAINER_CLASSES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| lines(include_str!("../data/names/TrainerClasses.txt")));

/// Names of pairs of trainers battling together, like twins.
pub(crate) static DOUBLES_TRAINER_NAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| lines(include_str!("../data/names/DoublesTrainerNames.txt")));

/// Class names of pairs of trainers battling together.
pub(crate) static DOUBLES_TRAINER_CLASSES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| lines(include_str!("../data/names/DoublesTrainerClasses.txt")));

/// Pokémon nicknames.
pub(crate) static POKEMON_NICKNAMES: LazyLock<Vec<&'static str>> =
    LazyLock::new(|| lines(include_str!("../data/names/PokemonNicknames.txt")));

/// The default custom names, as the Custom Names Editor starts out.
#[must_use]
pub fn default_custom_names() -> CustomNames {
    let owned = |list: &[&str]| list.iter().map(|&n| n.to_owned()).collect();
    CustomNames {
        trainer_names: owned(&TRAINER_NAMES),
        trainer_classes: owned(&TRAINER_CLASSES),
        doubles_trainer_names: owned(&DOUBLES_TRAINER_NAMES),
        doubles_trainer_classes: owned(&DOUBLES_TRAINER_CLASSES),
        pokemon_nicknames: owned(&POKEMON_NICKNAMES),
    }
}

/// Which list of [`CustomNames`].
#[derive(Debug, Clone, Copy)]
pub(crate) enum NameList {
    TrainerNames,
    TrainerClasses,
    DoublesTrainerNames,
    DoublesTrainerClasses,
    PokemonNicknames,
}

/// The names of `list` a run picks from: the custom ones, trimmed and without blanks, or else the defaults.
pub(crate) fn names(custom: Option<&CustomNames>, list: NameList) -> Vec<&str> {
    let Some(c) = custom else {
        let defaults: &[&'static str] = match list {
            NameList::TrainerNames => &TRAINER_NAMES,
            NameList::TrainerClasses => &TRAINER_CLASSES,
            NameList::DoublesTrainerNames => &DOUBLES_TRAINER_NAMES,
            NameList::DoublesTrainerClasses => &DOUBLES_TRAINER_CLASSES,
            NameList::PokemonNicknames => &POKEMON_NICKNAMES,
        };
        return defaults.to_vec();
    };
    let names = match list {
        NameList::TrainerNames => &c.trainer_names,
        NameList::TrainerClasses => &c.trainer_classes,
        NameList::DoublesTrainerNames => &c.doubles_trainer_names,
        NameList::DoublesTrainerClasses => &c.doubles_trainer_classes,
        NameList::PokemonNicknames => &c.pokemon_nicknames,
    };
    names.iter().map(|n| n.trim()).filter(|n| !n.is_empty()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_lists_replace_the_defaults() {
        assert_eq!(names(None, NameList::PokemonNicknames).first(), Some(&"Red"));
        let mut custom = default_custom_names();
        custom.pokemon_nicknames = vec![" Pebble ".into(), "  ".into(), "Comet".into()];
        assert_eq!(names(Some(&custom), NameList::PokemonNicknames), ["Pebble", "Comet"]);
        assert_eq!(names(Some(&custom), NameList::TrainerNames).len(), 60);
    }

    #[test]
    fn the_default_lists_load() {
        assert_eq!(TRAINER_NAMES.len(), 60);
        assert_eq!(POKEMON_NICKNAMES.first(), Some(&"Red"));
        assert_eq!(POKEMON_NICKNAMES.len(), 20);
        assert_eq!(
            [
                TRAINER_CLASSES.len(),
                DOUBLES_TRAINER_NAMES.len(),
                DOUBLES_TRAINER_CLASSES.len()
            ],
            [64, 24, 19]
        );
    }
}
