//! Type colors, base color maps and base color lists: which base color each part of a species' palette gets.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use ppr_model::{Color, Type};
use rand_core::Rng;

use super::description::LightDark;
use crate::choice::{below, unit};

/// A color with the type it stands for; `None` for a random color. Two type colors are the same when their RGB
/// values are.
#[derive(Debug, Clone, Copy)]
pub(super) struct TypeColor {
    pub(super) color: Color,
    ty: Option<Type>,
}

impl PartialEq for TypeColor {
    fn eq(&self, other: &Self) -> bool {
        self.color == other.color
    }
}

/// The color of types without colors.
const DEFAULT_COLOR: Color = Color::hex(0xC0_C0_C0);

/// The type colors of `Gen3To5TypeColors.txt`: `[TYPE]` followed by `(RRGGBB)` colors.
static TYPE_COLORS: LazyLock<BTreeMap<Type, Vec<Color>>> =
    LazyLock::new(|| parse_type_colors(include_str!("../../../data/palettes/Gen3To5TypeColors.txt")));

pub(super) fn parse_type_colors(text: &str) -> BTreeMap<Type, Vec<Color>> {
    let mut map: BTreeMap<Type, Vec<Color>> = BTreeMap::new();
    let mut current = None;
    let mut rest = text;
    while let Some(start) = rest.find(['[', '(']) {
        let close = if rest[start..].starts_with('[') { ']' } else { ')' };
        let Some(len) = rest[start..].find(close) else { break };
        let token = &rest[start + 1..start + len];
        if close == ']' {
            current = token.parse::<Type>().ok();
        } else if let (Some(t), Ok(hex)) = (current, u32::from_str_radix(token, 16)) {
            map.entry(t).or_default().push(Color::hex(hex));
        }
        rest = &rest[start + len + 1..];
    }
    map
}

/// A random color of type `ty`.
fn random_type_color(rng: &mut impl Rng, ty: Type) -> TypeColor {
    let color = TYPE_COLORS
        .get(&ty)
        .filter(|c| !c.is_empty())
        .map_or(DEFAULT_COLOR, |colors| colors[below(rng, colors.len())]);
    TypeColor { color, ty: Some(ty) }
}

/// A random color: a random hue, saturation and
/// value (0.6 to 1) by rejection sampling, weighted by saturation and three times as likely for hues 20 to 70.
fn random_color(rng: &mut impl Rng) -> TypeColor {
    /// Tries before the last draw is taken as it is.
    const ATTEMPTS: usize = 201;
    /// The largest weight, exactly.
    const MAX_WEIGHT: f64 = 3.0;
    let mut hsv = [0.0; 3];
    for _ in 0..ATTEMPTS {
        hsv = [unit(rng) * 360.0, unit(rng), unit(rng) * 0.4 + 0.6];
        let value = unit(rng) * MAX_WEIGHT;
        let weight = if (20.0..=70.0).contains(&hsv[0]) {
            hsv[1] * 3.0
        } else {
            hsv[1]
        };
        if value < weight {
            break;
        }
    }
    TypeColor {
        color: from_hsv(hsv[0], hsv[1], hsv[2]),
        ty: None,
    }
}

/// A color from hue, saturation and value: hue 0 to 360, saturation and value 0 to 1.
pub(super) fn from_hsv(hue: f64, saturation: f64, value: f64) -> Color {
    let chroma = value * saturation;
    let second = chroma * (1.0 - ((hue / 60.0) % 2.0 - 1.0).abs());
    let m = value - chroma;
    let bases = [
        [chroma, second, 0.0],
        [second, chroma, 0.0],
        [0.0, chroma, second],
        [0.0, second, chroma],
        [second, 0.0, chroma],
        [chroma, 0.0, second],
    ];
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "truncated, 0 to 5"
    )]
    let base = bases[((hue / 60.0) as usize).min(5)];
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "truncated, 0 to 255"
    )]
    let comp = |i: usize| ((base[i] + m) * 255.0).clamp(0.0, 255.0) as u8;
    Color::rgb(comp(0), comp(1), comp(2))
}

/// The tweaked base color and light/dark mode of each type color, shared by an
/// evolution line so its members get the same colors.
#[derive(Debug, Default)]
pub(super) struct BaseColorMap {
    colors: BTreeMap<Color, (Color, LightDark)>,
}

impl BaseColorMap {
    /// The base color and light/dark mode of `key`: made the first time a type color is asked for.
    pub(super) fn get(&mut self, rng: &mut impl Rng, key: TypeColor) -> (Color, LightDark) {
        *self
            .colors
            .entry(key.color)
            .or_insert_with(|| (tweak(rng, key.color), light_dark(rng)))
    }
}

/// A randomly tweaked color (as in Artemis251's Emerald Randomizer): each component a few percent up or down.
fn tweak(rng: &mut impl Rng, color: Color) -> Color {
    let mut out = Color::WHITE;
    for i in 0..3 {
        let c = f64::from(color.comp(i));
        #[expect(clippy::cast_possible_truncation, reason = "truncating casts")]
        let (min, max) = ((-0.032 * c + 7.1) as i32, (-0.042 * c + 17.19) as i32);
        let span = usize::try_from(max - min).unwrap_or(1).max(1);
        let change =
            i32::try_from(below(rng, span) + below(rng, usize::try_from(max).unwrap_or(1).max(1))).unwrap_or_default();
        let sign = if below(rng, 2) == 1 { -1.0 } else { 1.0 };
        #[expect(clippy::cast_possible_truncation, reason = "truncating cast")]
        let delta = (c * f64::from(change) * 0.01 * sign) as i32;
        let value = (i32::from(color.comp(i)) + delta).clamp(0, 255);
        out.set_comp(i, u8::try_from(value).unwrap_or(u8::MAX));
    }
    out
}

/// A random light/dark mode: light or dark one time in five.
fn light_dark(rng: &mut impl Rng) -> LightDark {
    if unit(rng) < 0.2 {
        if below(rng, 2) == 1 {
            LightDark::Light
        } else {
            LightDark::Dark
        }
    } else {
        LightDark::Default
    }
}

/// The type colors of a species' palette parts, in order, and which
/// [`BaseColorMap`] of the run gives their base colors.
#[derive(Debug, Clone)]
pub(super) struct TypeBaseColorList {
    pub(super) colors: Vec<TypeColor>,
    pub(super) map: usize,
}

/// How many base colors a species gets.
const MIN_COLORS: usize = 16;
/// Tries at an unused color.
const TRIES: usize = 100;
/// The chance that an evolution swaps its first two colors (without type sanity, or with unchanged types).
const SWAP_CHANCE: f64 = 0.3;

/// The species' types, the second `None` for a single type.
pub(super) type Types = (Type, Option<Type>);

impl TypeBaseColorList {
    /// The colors of a species without a pre-evolution to copy from: with type sanity, colors of the species'
    /// types first; then random colors up to 16.
    pub(super) fn basic(rng: &mut impl Rng, types: Types, type_sanity: bool, map: usize) -> Self {
        let mut list = Self {
            colors: Vec::new(),
            map,
        };
        if type_sanity {
            let (primary, secondary) = types;
            list.move_first_of_type(rng, 0, primary);
            list.move_first_of_type(rng, 1, secondary.unwrap_or(primary));
            list.move_first_of_type(rng, 2, primary);
            if let Some(secondary) = secondary {
                list.move_first_of_type(rng, 3, secondary);
            }
        }
        while list.colors.len() < MIN_COLORS {
            let color = list.unused(rng, random_color);
            list.colors.push(color);
        }
        list
    }

    /// The colors of an evolution: the pre-evolution's colors; the first two swapped now and then,
    /// or with type sanity the colors of changed types moved to the front.
    pub(super) fn evolved(
        rng: &mut impl Rng,
        prevo: &Self,
        prevo_types: Types,
        types: Types,
        type_sanity: bool,
    ) -> Self {
        let mut list = prevo.clone();
        if !type_sanity || prevo_types == types {
            if unit(rng) < SWAP_CHANCE && list.colors.len() >= 2 {
                list.colors.swap(0, 1);
            }
        } else {
            if types.0 != prevo_types.0 {
                list.move_first_of_type(rng, 0, types.0);
            }
            if types.1 != prevo_types.1 {
                list.move_first_of_type(rng, 1, types.1.unwrap_or(types.0));
            }
        }
        list
    }

    /// The first color of `ty` from `at` on moves to `at`, or a new one of that type
    /// goes there.
    fn move_first_of_type(&mut self, rng: &mut impl Rng, at: usize, ty: Type) {
        let found = self.colors.iter().skip(at).position(|c| c.ty == Some(ty));
        let color = match found {
            Some(i) => self.colors.remove(at + i),
            None => self.unused(rng, |rng: &mut _| random_type_color(rng, ty)),
        };
        self.colors.insert(at.min(self.colors.len()), color);
    }

    /// A color not in the list yet, from 100 tries; the last try when all were
    /// taken.
    fn unused<R: Rng>(&self, rng: &mut R, mut draw: impl FnMut(&mut R) -> TypeColor) -> TypeColor {
        let mut color = draw(rng);
        for _ in 1..TRIES {
            if !self.colors.contains(&color) {
                break;
            }
            color = draw(rng);
        }
        color
    }
}

#[cfg(test)]
mod tests {
    use rand_core::SeedableRng;

    use super::*;

    #[test]
    fn type_colors_are_read_from_the_data_file() {
        assert_eq!(TYPE_COLORS.len(), 18);
        assert_eq!(
            TYPE_COLORS[&Type::Normal],
            [Color::hex(0xFF_9D_E7), Color::hex(0xCB_95_FD), Color::hex(0xC5_9A_8B)]
        );
        assert_eq!(from_hsv(0.0, 1.0, 1.0), Color::rgb(255, 0, 0));
        assert_eq!(from_hsv(120.0, 1.0, 1.0), Color::rgb(0, 255, 0));
    }

    #[test]
    fn lists_put_type_colors_first_and_evolutions_keep_them() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(3);
        let fire = TYPE_COLORS[&Type::Fire].clone();
        let list = TypeBaseColorList::basic(&mut rng, (Type::Fire, Some(Type::Flying)), true, 0);
        assert_eq!(list.colors.len(), 16);
        assert!(fire.contains(&list.colors[0].color) && fire.contains(&list.colors[2].color));
        assert!(TYPE_COLORS[&Type::Flying].contains(&list.colors[1].color));
        let evolved = TypeBaseColorList::evolved(
            &mut rng,
            &list,
            (Type::Fire, Some(Type::Flying)),
            (Type::Water, Some(Type::Flying)),
            true,
        );
        assert!(TYPE_COLORS[&Type::Water].contains(&evolved.colors[0].color));
        // The new color is inserted: the others move one place back.
        assert_eq!(evolved.colors[1], list.colors[0]);
        let mut map = BaseColorMap::default();
        let a = map.get(&mut rng, list.colors[0]);
        assert_eq!(
            map.get(&mut rng, list.colors[0]),
            a,
            "the same type color keeps its base color"
        );
    }
}
