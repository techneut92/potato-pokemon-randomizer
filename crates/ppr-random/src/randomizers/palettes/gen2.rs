//! Gen 2: a species' sprite has four colors, but white and black are fixed, so its normal and shiny palettes have
//! two colors each: a bright one and a dark one. The normal palette gets random colors, or colors of the species'
//! types (the bright one of the primary type, the dark one of the secondary type, or the primary when it has
//! one); evolutions keep their pre-evolution's colors, a new one only for a type that changed.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ppr_model::{Color, DexNumber, GameData, Type};
use ppr_settings::PaletteSettings;
use rand_core::Rng;

use super::base_colors::{from_hsv, parse_type_colors};
use crate::choice::{below, unit};
use crate::randomizers::copy_up::{self, CopyUp};

/// The bright colors of `Gen2BrightTypeColors.txt`.
static BRIGHT_COLORS: LazyLock<BTreeMap<Type, Vec<Color>>> =
    LazyLock::new(|| parse_type_colors(include_str!("../../../data/palettes/Gen2BrightTypeColors.txt")));
/// The dark colors of `Gen2DarkTypeColors.txt`.
static DARK_COLORS: LazyLock<BTreeMap<Type, Vec<Color>>> =
    LazyLock::new(|| parse_type_colors(include_str!("../../../data/palettes/Gen2DarkTypeColors.txt")));

/// The bright color of a type without colors.
const DEFAULT_BRIGHT: Color = Color::hex(0xC0_C0_C0);
/// The dark color of a type without colors.
const DEFAULT_DARK: Color = Color::hex(0x80_80_80);

fn type_color(rng: &mut impl Rng, colors: &BTreeMap<Type, Vec<Color>>, t: Type, default: Color) -> Color {
    colors
        .get(&t)
        .filter(|c| !c.is_empty())
        .map_or(default, |c| c[below(rng, c.len())])
}

fn bright_type_color(rng: &mut impl Rng, t: Type) -> Color {
    type_color(rng, &BRIGHT_COLORS, t, DEFAULT_BRIGHT)
}

fn dark_type_color(rng: &mut impl Rng, t: Type) -> Color {
    type_color(rng, &DARK_COLORS, t, DEFAULT_DARK)
}

/// A random color by rejection sampling: hue, saturation and value uniform between `low` and `high`, kept with a
/// chance of `weight` over `max_weight`, its exact highest value. Tries 201 times, then takes the last.
fn random_color(
    rng: &mut impl Rng,
    low: [f64; 3],
    high: [f64; 3],
    weight: fn(&[f64; 3]) -> f64,
    max_weight: f64,
) -> Color {
    const ATTEMPTS: usize = 201;
    let mut hsv = [0.0; 3];
    for _ in 0..ATTEMPTS {
        for (i, c) in hsv.iter_mut().enumerate() {
            *c = unit(rng) * (high[i] - low[i]) + low[i];
        }
        if unit(rng) * max_weight < weight(&hsv) {
            break;
        }
    }
    from_hsv(hsv[0], hsv[1], hsv[2])
}

/// A random bright color: value 0.6 to 1, weighted by saturation plus half the value, three times as likely for
/// hues 20 to 70.
fn random_bright(rng: &mut impl Rng) -> Color {
    random_color(
        rng,
        [0.0, 0.0, 0.6],
        [360.0, 1.0, 1.0],
        |hsv| {
            let w = hsv[1] + hsv[2] * 0.5;
            if (20.0..=70.0).contains(&hsv[0]) { w * 3.0 } else { w }
        },
        4.5,
    )
}

/// A random dark color: value 0.5 to 0.8, weighted by saturation.
fn random_dark(rng: &mut impl Rng) -> Color {
    random_color(rng, [0.0, 0.0, 0.5], [360.0, 1.0, 0.8], |hsv| hsv[1] / 2.0, 0.5)
}

/// New normal palettes for every species with palettes; with `shiny_from_normal` the shiny palette becomes the old
/// normal one.
pub(super) fn randomize(rng: &mut impl Rng, game: &mut GameData, settings: &PaletteSettings) {
    let options = copy_up::Options {
        evolution_sanity: settings.follow_evolutions,
        copy_split_evos: true,
    };
    let set = game.palettes.keys().copied().collect();
    let types = |g: &GameData, n: DexNumber| g.species(n).map(|s| (s.primary_type, s.secondary_type));
    for step in copy_up::plan(game, &set, options) {
        let (n, normal) = match step {
            // Gen 1 and 2 have no alternate formes.
            CopyUp::AltForme { .. } | CopyUp::Cosmetic { .. } => continue,
            CopyUp::Basic(n) => {
                let Some((primary, secondary)) = types(game, n) else {
                    continue;
                };
                let normal = if settings.follow_types {
                    let bright = bright_type_color(rng, primary);
                    vec![bright, dark_type_color(rng, secondary.unwrap_or(primary))]
                } else {
                    let bright = random_bright(rng);
                    vec![bright, random_dark(rng)]
                };
                (n, normal)
            }
            CopyUp::Evolved { from, to, .. } => {
                let (Some(from_types), Some(to_types), Some(mut normal)) = (
                    types(game, from),
                    types(game, to),
                    game.palettes.get(&from).map(|p| p.normal.clone()),
                ) else {
                    continue;
                };
                if settings.follow_types {
                    if to_types.0 != from_types.0 {
                        if let Some(c) = normal.first_mut() {
                            *c = bright_type_color(rng, to_types.0);
                        }
                    } else if to_types.1 != from_types.1
                        && let Some(c) = normal.get_mut(1)
                    {
                        *c = dark_type_color(rng, to_types.1.unwrap_or(to_types.0));
                    }
                }
                (to, normal)
            }
        };
        if let Some(p) = game.palettes.get_mut(&n) {
            if settings.shiny_from_normal {
                p.shiny.clone_from(&p.normal);
            }
            p.normal = normal;
        }
    }
}

#[cfg(test)]
mod tests {
    use ppr_model::{Evolution, EvolutionType, SpeciesPalettes};
    use rand_core::SeedableRng;

    use super::*;
    use crate::randomizers::testing::{emerald, species};

    const OLD: [Color; 2] = [Color::hex(0x11_22_33), Color::hex(0x44_55_66)];

    fn game() -> GameData {
        let mut g = emerald(vec![
            species(1, Type::Grass, Some(Type::Poison), 45),
            species(2, Type::Grass, Some(Type::Poison), 60),
            species(3, Type::Grass, None, 80),
            species(4, Type::Fire, None, 39),
            species(5, Type::Water, None, 58),
        ]);
        g.evolutions = vec![
            Evolution::new(1, 2, EvolutionType::Level, 16),
            Evolution::new(2, 3, EvolutionType::Level, 32),
            Evolution::new(4, 5, EvolutionType::Level, 16),
        ];
        for n in 1..=5 {
            g.palettes.insert(
                n,
                SpeciesPalettes {
                    normal: OLD.to_vec(),
                    shiny: vec![Color::WHITE; 2],
                    id: None,
                },
            );
        }
        g
    }

    fn run(settings: &PaletteSettings, seed: u64) -> GameData {
        let mut g = game();
        randomize(&mut rand_chacha::ChaCha8Rng::seed_from_u64(seed), &mut g, settings);
        g
    }

    #[test]
    fn type_colors_are_read_from_the_data_files() {
        assert_eq!(BRIGHT_COLORS.len(), 18);
        assert_eq!(DARK_COLORS.len(), 18);
        assert_eq!(BRIGHT_COLORS[&Type::Dragon][0], Color::hex(0xE7_D6_39));
        assert_eq!(DARK_COLORS[&Type::Fighting][0], Color::hex(0x8C_39_29));
    }

    #[test]
    fn colors_follow_types_and_evolutions() {
        let settings = PaletteSettings {
            random: true,
            follow_types: true,
            follow_evolutions: true,
            shiny_from_normal: true,
        };
        for seed in 0..10 {
            let g = run(&settings, seed);
            let normal = |n: DexNumber| g.palettes[&n].normal.clone();
            assert!(BRIGHT_COLORS[&Type::Grass].contains(&normal(1)[0]));
            assert!(DARK_COLORS[&Type::Poison].contains(&normal(1)[1]));
            assert_eq!(normal(2), normal(1), "same types: the same colors");
            // The secondary type is gone: a dark color of the primary type.
            assert_eq!(normal(3)[0], normal(1)[0]);
            assert!(DARK_COLORS[&Type::Grass].contains(&normal(3)[1]));
            // A new primary type: a new bright color, the dark one kept.
            assert!(BRIGHT_COLORS[&Type::Water].contains(&normal(5)[0]));
            assert_eq!(normal(5)[1], normal(4)[1]);
            assert!(
                g.palettes.values().all(|p| p.shiny == OLD),
                "shiny from the old normal palette"
            );
        }
    }

    #[test]
    fn random_colors_are_bright_and_dark() {
        let settings = PaletteSettings {
            random: true,
            ..PaletteSettings::default()
        };
        let g = run(&settings, 3);
        let value = |c: Color| c.r.max(c.g).max(c.b);
        for p in g.palettes.values() {
            assert!(value(p.normal[0]) >= 152, "{:?}", p.normal);
            assert!((126..=204).contains(&value(p.normal[1])), "{:?}", p.normal);
            assert_eq!(p.shiny, vec![Color::WHITE; 2]);
        }
        assert_ne!(g.palettes[&1].normal, g.palettes[&2].normal);
    }
}
