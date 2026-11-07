//! Trainer names: trainers and trainer classes get random names from the custom names (the defaults unless
//! edited), pairs battling together (`&` in the name, or a doubles class) from the lists for pairs. A name the game
//! uses more than once gets the same new name each time, except for the likes of `GRUNT`; names are in capitals
//! when the game's are.
//!
//! Deliberate choices:
//! - Random choices use the step's `ChaCha8` stream (ADR 0005).
//! - A name is picked from those that fit; when none does, the old name stays, rather than retrying random names a
//!   fixed number of times (or forever).
//! - Names are measured by the bytes they take in the game ([`GameData::text_length`]). In Gen 1, where a new name
//!   must take as many bytes as the old one, trainers have no names: the classes named after a person (the gym
//!   leaders, the Elite Four) count as trainer names instead. In Gen 2 a name shares its line with the trainer's
//!   class name, and all names together must fit in the room the game has for them: names are picked again, up
//!   to 10000 times, until they do, as the original does.

use std::collections::BTreeMap;

use ppr_model::GameData;

use crate::choice::below;
use ppr_settings::CustomNames;

use crate::names::{NameList, names};
use crate::{RandomizeError, RunData, Step, StepContext, StepRng, step_ids};

/// Custom trainer names longer than this are never used.
const MAX_CUSTOM_NAME_LENGTH: usize = 10;
/// Names shared by many trainers, each of whom gets a name of their own.
const REPEATED_NAMES: [&str; 6] = ["GRUNT", "EXECUTIVE", "SHADOW", "ADMIN", "GOON", "EMPLOYEE"];

/// Renames trainers and trainer classes by [`ppr_settings::TrainerSettings::random_names`] and
/// [`ppr_settings::TrainerSettings::random_class_names`].
#[derive(Debug, Clone, Copy, Default)]
pub struct TrainerNameRandomizer;

impl Step<RunData> for TrainerNameRandomizer {
    fn id(&self) -> u64 {
        step_ids::TRAINER_NAMES
    }

    fn label(&self) -> &'static str {
        "Trainer Names"
    }

    fn applies(&self, data: &RunData) -> bool {
        let s = &data.settings.trainers;
        let g = &data.game;
        !g.fixed_trainer_text
            && ((s.random_class_names && !g.trainer_class_names.is_empty())
                || (s.random_names && (!g.trainers.is_empty() || !g.personal_trainer_classes.is_empty())))
    }

    fn run(&self, ctx: StepContext<'_, RunData>) -> Result<(), RandomizeError> {
        let StepContext { data, mut rng, .. } = ctx;
        // Class names first.
        let custom = data.settings.custom_names.as_ref();
        if data.settings.trainers.random_class_names {
            randomize_class_names(&mut data.game, custom, &mut rng);
        }
        if data.settings.trainers.random_names {
            randomize_names(&mut data.game, custom, &mut rng).map_err(|message| RandomizeError::Impossible {
                step: self.label(),
                message,
            })?;
        }
        Ok(())
    }
}

/// Whether the names are in capitals: hardly any lower case letters.
fn upper_case(names: &[&str]) -> bool {
    let (mut upper, mut lower) = (0, 0);
    for c in names.iter().flat_map(|n| n.chars()) {
        if c.is_uppercase() {
            upper += 1;
        } else if c.is_lowercase() {
            lower += 1;
        }
    }
    lower == 0 || (upper > 0 && lower * 10 < upper)
}

/// A random name from `list` that `fits`, in capitals if `upper`; `None` when none fits.
fn pick(list: &[&str], fits: impl Fn(&str) -> bool, upper: bool, rng: &mut StepRng) -> Option<String> {
    let fitting: Vec<&str> = list.iter().copied().filter(|n| fits(n)).collect();
    if fitting.is_empty() {
        return None;
    }
    let name = fitting[below(rng, fitting.len())];
    Some(if upper { name.to_uppercase() } else { name.to_owned() })
}

/// Tries at picking trainer names whose lengths add up to no more than the game's room for them.
const MAX_TRIES: usize = 10_000;

/// Renames the trainers, and the classes named after a person; an error when no names fit in the game's room for them in [`MAX_TRIES`] tries.
fn randomize_names(game: &mut GameData, custom: Option<&CustomNames>, rng: &mut StepRng) -> Result<(), String> {
    let (singles, doubles) = (
        names(custom, NameList::TrainerNames),
        names(custom, NameList::DoublesTrainerNames),
    );
    // The trainers' names, then those of the classes named after a person.
    let personal: Vec<usize> = game.personal_trainer_classes.iter().map(|&c| usize::from(c)).collect();
    let old: Vec<String> = game
        .trainers
        .iter()
        .map(|t| t.name.clone())
        .chain(
            personal
                .iter()
                .filter_map(|&c| game.trainer_class_names.get(c).cloned()),
        )
        .collect();
    // The room the class name takes on each trainer's line.
    let class_lengths: Vec<usize> = game
        .trainers
        .iter()
        .map(|t| {
            game.trainer_class_names
                .get(usize::from(t.class))
                .filter(|_| game.trainer_name_shares_class_line)
                .map_or(0, |c| game.text_length(c))
        })
        .chain(personal.iter().map(|_| 0))
        .collect();
    let upper = upper_case(&old.iter().map(String::as_str).collect::<Vec<_>>());
    let custom_max = game.max_trainer_name_length.min(MAX_CUSTOM_NAME_LENGTH);
    let total_max = game.max_trainer_names_total.unwrap_or(usize::MAX);
    let mut new_names = Vec::with_capacity(old.len());
    let mut tries = 0;
    loop {
        let mut translation: BTreeMap<&str, String> = BTreeMap::new();
        let mut total = 0usize;
        new_names.clear();
        for (name, &class_length) in old.iter().zip(&class_lengths) {
            let repeated = REPEATED_NAMES.contains(&name.to_uppercase().as_str());
            if let Some(new) = translation.get(name.as_str()).filter(|_| !repeated) {
                total += game.text_length(new);
                new_names.push(new.clone());
            } else {
                let list = if name.contains('&') { &doubles } else { &singles };
                let length = game.text_length(name);
                let fits = |n: &str| {
                    let l = game.text_length(n);
                    l <= custom_max
                        && l + class_length <= game.max_trainer_name_length
                        && (!game.fixed_trainer_name_length || l == length)
                };
                let new = if length == 0 {
                    None
                } else {
                    pick(list, fits, upper, rng)
                }
                .unwrap_or_else(|| name.clone());
                total += game.text_length(&new);
                translation.insert(name, new.clone());
                new_names.push(new);
            }
            if total > total_max {
                break;
            }
        }
        if total <= total_max {
            break;
        }
        tries += 1;
        if tries == MAX_TRIES {
            return Err(format!(
                "could not fit the trainer names in the game's {total_max} characters in {MAX_TRIES} tries; add \
                 some shorter custom trainer names"
            ));
        }
    }
    let mut new_names = new_names.into_iter();
    for (t, new) in game.trainers.iter_mut().zip(new_names.by_ref()) {
        t.name = new;
    }
    for (&c, new) in personal.iter().zip(new_names) {
        if let Some(name) = game.trainer_class_names.get_mut(c) {
            *name = new;
        }
    }
    Ok(())
}

/// Gives trainer classes random names, doubles classes from the lists for pairs.
fn randomize_class_names(game: &mut GameData, custom: Option<&CustomNames>, rng: &mut StepRng) {
    let (singles, doubles) = (
        names(custom, NameList::TrainerClasses),
        names(custom, NameList::DoublesTrainerClasses),
    );
    // Classes named after a person are renamed as trainers.
    let old: Vec<(usize, String)> = game
        .trainer_class_names
        .iter()
        .cloned()
        .enumerate()
        .filter(|&(i, _)| !u8::try_from(i).is_ok_and(|i| game.personal_trainer_classes.contains(&i)))
        .collect();
    let upper = upper_case(&old.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>());
    let mut translation: BTreeMap<&str, String> = BTreeMap::new();
    for (i, name) in &old {
        let i = *i;
        let new = if let Some(new) = translation.get(name.as_str()) {
            new.clone()
        } else {
            let pair = u8::try_from(i).is_ok_and(|i| game.doubles_trainer_classes.contains(&i));
            let list = if pair { &doubles } else { &singles };
            let length = game.text_length(name);
            let fits = |n: &str| {
                n.chars().count() <= game.max_trainer_class_name_length
                    && (!game.fixed_trainer_class_name_length || game.text_length(n) == length)
            };
            let new = pick(list, fits, upper, rng).unwrap_or_else(|| name.clone());
            translation.insert(name, new.clone());
            new
        };
        game.trainer_class_names[i] = new;
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{Trainer, TrainerPokemon};
    use ppr_settings::{Settings, TrainerSettings};

    use super::*;
    use crate::names::{DOUBLES_TRAINER_CLASSES, DOUBLES_TRAINER_NAMES, TRAINER_CLASSES, TRAINER_NAMES};
    use crate::randomizers::testing::emerald;
    use crate::{CancelToken, Progress, Seed, standard_pipeline};

    fn game() -> GameData {
        let mut g = emerald(Vec::new());
        let names = ["BRENDAN", "ROXANNE", "BRENDAN", "", "ANNA & MEG"]
            .into_iter()
            .chain(["GRUNT"; 20]);
        g.trainers = names
            .enumerate()
            .map(|(i, n)| {
                Trainer::new(
                    u16::try_from(i + 1).expect("few"),
                    n,
                    0,
                    vec![TrainerPokemon::new(1, 5)],
                )
            })
            .collect();
        g.trainer_class_names = ["PKMN TRAINER", "LEADER", "TWINS", "PKMN TRAINER", "TEAM AQUA"]
            .map(String::from)
            .to_vec();
        g.doubles_trainer_classes = vec![2];
        g
    }

    fn run(game: GameData, names: bool, classes: bool, seed: u64) -> GameData {
        run_with(game, names, classes, seed, None)
    }

    fn run_with(game: GameData, names: bool, classes: bool, seed: u64, custom: Option<CustomNames>) -> GameData {
        let mut data = RunData {
            game,
            settings: Settings {
                trainers: TrainerSettings {
                    random_names: names,
                    random_class_names: classes,
                    ..TrainerSettings::default()
                },
                custom_names: custom,
                ..Settings::default()
            },
        };
        standard_pipeline()
            .run(&mut data, Seed(seed), &CancelToken::default(), &mut |_: Progress| {})
            .expect("runs");
        data.game
    }

    fn upper(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| n.to_uppercase()).collect()
    }

    #[test]
    fn trainer_names_are_translated() {
        let original = game();
        let g = run(original.clone(), true, false, 1);
        assert_eq!(g.trainer_class_names, original.trainer_class_names);
        let names: Vec<&str> = g.trainers.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names[0], names[2], "the same name, the same new name");
        assert_eq!(names[3], "", "no name stays none");
        assert!(
            upper(&DOUBLES_TRAINER_NAMES).iter().any(|n| n == names[4]),
            "{}",
            names[4]
        );
        let singles = upper(&TRAINER_NAMES);
        for n in [0, 1].into_iter().chain(5..25) {
            assert!(singles.iter().any(|s| s == names[n]), "{}", names[n]);
            assert!(names[n].chars().count() <= MAX_CUSTOM_NAME_LENGTH);
        }
        let grunts: std::collections::BTreeSet<_> = names[5..].iter().collect();
        assert!(grunts.len() > 5, "grunts get names of their own: {grunts:?}");
    }

    #[test]
    fn custom_names_are_used() {
        let custom = CustomNames {
            trainer_names: vec!["Robin".into(), " ".into()],
            doubles_trainer_names: vec!["Mo & Jo".into()],
            trainer_classes: vec!["Chef".into()],
            doubles_trainer_classes: vec!["Twins".into()],
            ..CustomNames::default()
        };
        let g = run_with(game(), true, true, 4, Some(custom));
        let names: Vec<&str> = g.trainers.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names[0], "ROBIN");
        assert_eq!(names[4], "MO & JO");
        assert_eq!(g.trainer_class_names, ["CHEF", "CHEF", "TWINS", "CHEF", "CHEF"]);
    }

    #[test]
    fn class_names_are_translated() {
        let mut original = game();
        original.max_trainer_class_name_length = 5;
        let g = run(original.clone(), false, true, 2);
        assert_eq!(g.trainers, original.trainers);
        let classes = &g.trainer_class_names;
        assert_eq!(classes[0], classes[3]);
        assert!(upper(&DOUBLES_TRAINER_CLASSES).contains(&classes[2]), "{}", classes[2]);
        for n in [0, 1, 4] {
            assert!(upper(&TRAINER_CLASSES).contains(&classes[n]), "{}", classes[n]);
        }
        assert!(classes.iter().all(|c| c.chars().count() <= 5), "{classes:?}");
        // No class name fits: the old ones stay.
        original.max_trainer_class_name_length = 1;
        assert_eq!(
            run(original.clone(), false, true, 2).trainer_class_names,
            original.trainer_class_names
        );
    }

    #[test]
    fn lower_case_games_keep_their_case() {
        let mut original = game();
        for t in &mut original.trainers {
            t.name = t.name.to_lowercase();
        }
        let g = run(original, true, false, 3);
        assert!(g.trainers.iter().any(|t| t.name.chars().any(char::is_lowercase)));
        assert!(
            upper_case(&["BRENDAN", "ROXANNE", "MAY", "Bo"]),
            "one in ten is lower case"
        );
        assert!(!upper_case(&["Brendan", "May"]));
    }

    /// Gen 1: nameless trainers; classes YOUNGSTER, BROCK (personal), HIKER, [PK][MN]X and MISTY (personal), in
    /// a text encoding where `[PK]` and `[MN]` take a byte each.
    fn gen1_game() -> GameData {
        let mut g = crate::randomizers::testing::gen1(Vec::new(), false);
        g.trainers = vec![Trainer::new(1, "", 0, vec![TrainerPokemon::new(1, 5)])];
        g.trainer_class_names = ["YOUNGSTER", "BROCK", "HIKER", "[PK][MN]X", "MISTY"]
            .map(String::from)
            .to_vec();
        g.personal_trainer_classes = vec![1, 4];
        g.fixed_trainer_name_length = true;
        g.fixed_trainer_class_name_length = true;
        g.max_trainer_name_length = usize::MAX;
        g.max_trainer_class_name_length = usize::MAX;
        let mut tokens: Vec<String> = ('A'..='Z').chain('a'..='z').map(String::from).collect();
        tokens.extend(["[PK]", "[MN]", " "].map(String::from));
        tokens.sort();
        g.text_tokens = tokens;
        g
    }

    fn gen1_custom() -> CustomNames {
        CustomNames {
            trainer_names: ["Al", "Bobby", "Clara", "Daniel"].map(String::from).to_vec(),
            trainer_classes: ["Ace", "Ninja", "Pilot", "Rockets"].map(String::from).to_vec(),
            ..CustomNames::default()
        }
    }

    #[test]
    fn gen1_names_keep_their_length_and_personal_classes_are_trainer_names() {
        for seed in 0..10 {
            let g = run_with(gen1_game(), true, true, seed, Some(gen1_custom()));
            let c = &g.trainer_class_names;
            assert_eq!(c[0], "YOUNGSTER", "no class name of 9 bytes");
            assert!(["NINJA", "PILOT"].contains(&c[2].as_str()), "{}", c[2]);
            assert_eq!(c[3], "ACE", "[PK][MN]X takes 3 bytes");
            for personal in [&c[1], &c[4]] {
                assert!(["BOBBY", "CLARA"].contains(&personal.as_str()), "{personal}");
            }
            assert_eq!(g.trainers[0].name, "");
        }
    }

    #[test]
    fn names_share_the_line_with_the_class_and_fit_in_the_total() {
        let mut original = game();
        original.trainer_name_shares_class_line = true;
        original.max_trainer_name_length = 17;
        // "PKMN TRAINER" leaves 5 characters on the line, "LEADER" 11.
        for t in &mut original.trainers {
            t.class = u16::from(t.index % 2 != 0);
        }
        for seed in 0..5 {
            let g = run(original.clone(), true, false, seed);
            for t in &g.trainers {
                let class = &g.trainer_class_names[usize::from(t.class)];
                assert!(
                    g.text_length(&t.name) + g.text_length(class) <= 17,
                    "{} {}",
                    class,
                    t.name
                );
            }
        }
        let total = |g: &GameData| g.trainers.iter().map(|t| g.text_length(&t.name)).sum::<usize>();
        original.max_trainer_names_total = Some(total(&original));
        let g = run(original.clone(), true, false, 1);
        assert!(total(&g) <= total(&original));
        // No names fit at all.
        original.max_trainer_names_total = Some(0);
        let mut data = RunData {
            game: original,
            settings: Settings {
                trainers: TrainerSettings {
                    random_names: true,
                    ..TrainerSettings::default()
                },
                ..Settings::default()
            },
        };
        assert!(
            standard_pipeline()
                .run(&mut data, Seed(1), &CancelToken::default(), &mut |_: Progress| {})
                .is_err()
        );
    }

    #[test]
    fn fixed_trainer_text_stays() {
        let mut original = gen1_game();
        original.fixed_trainer_text = true;
        let g = run_with(original.clone(), true, true, 1, Some(gen1_custom()));
        assert_eq!(g, original);
    }
}
