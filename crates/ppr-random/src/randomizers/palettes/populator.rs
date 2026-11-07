//! The shades of one palette part, from a base color (after Artemis251's Emerald Randomizer).

use ppr_model::Color;
use rand_core::Rng;

use super::description::{LightDark, PartDescription};
use crate::choice::below;

/// How much sibling shades vary.
const VARIA: usize = 10;
const VARIABASE: i32 = 3;

/// A double truncated towards zero, clamped to a color component.
fn clamp(value: f64) -> u8 {
    #[expect(clippy::cast_possible_truncation, reason = "truncated on purpose, then clamped")]
    let int = value as i32;
    u8::try_from(int.clamp(0, 255)).unwrap_or(u8::MAX)
}

/// Every component from `f(component)`, clamped.
fn map(color: Color, f: impl Fn(usize, f64) -> f64) -> Color {
    let mut out = color;
    for i in 0..3 {
        out.set_comp(i, clamp(f(i, f64::from(color.comp(i)))));
    }
    out
}

fn as_index(slot: i32) -> Option<usize> {
    usize::try_from(slot).ok()
}

/// A sum of `n` draws below `bound`.
fn draws(rng: &mut impl Rng, n: usize, bound: usize) -> i32 {
    (0..n)
        .map(|_| i32::try_from(below(rng, bound)).unwrap_or_default())
        .sum()
}

/// How much a shade changes per step towards light (`light`) or dark. The quotient is an integer division.
fn color_change(rng: &mut impl Rng, limit: usize, component: u8, light: bool) -> f64 {
    if limit == 0 {
        return 0.0;
    }
    let limit = i32::try_from(limit).unwrap_or(i32::MAX);
    let coeff = f64::from((draws(rng, 3, 5) + 153) / limit);
    let towards = if light { component } else { 255 - component };
    coeff - coeff / 255.0 * f64::from(towards) + f64::from(limit * limit)
}

/// The state while one part is filled.
pub(super) struct Populator<'a, R: Rng> {
    rng: &'a mut R,
    description: &'a PartDescription,
    base: Color,
    mode: LightDark,
    left: [f64; 3],
    right: [f64; 3],
}

impl<'a, R: Rng> Populator<'a, R> {
    /// Fills the part's slots from its base color.
    pub(super) fn populate(
        rng: &'a mut R,
        palette: &mut [Color],
        description: &'a PartDescription,
        base: Color,
        mode: LightDark,
    ) {
        let mut p = Populator {
            rng,
            description,
            base,
            mode: description.correct(mode),
            left: [0.0; 3],
            right: [0.0; 3],
        };
        if !description.slots.is_empty() {
            let shades = p.make_shades();
            fill(palette, &shades, &description.slots);
        }
        if let Some(shared) = description.shared_slot.and_then(as_index)
            && description.has_sibling()
            && let Some(&color) = palette.get(shared)
        {
            let shared_color = sibling_color(color, base);
            let shades = p.make_sibling_shades(shared_color, &description.sibling_slots, shared);
            fill(palette, &shades, &description.sibling_slots);
        }
    }

    fn len(&self) -> usize {
        self.description.slots.len()
    }

    fn even(&self) -> bool {
        self.len().is_multiple_of(2)
    }

    /// The index of the middle shade.
    fn middle(&self) -> usize {
        (self.len() - usize::from(!self.even())) / 2
    }

    fn changes(&mut self, light: bool) -> [f64; 3] {
        let (middle, base) = (self.middle(), self.base);
        [0, 1, 2].map(|i| color_change(self.rng, middle, base.comp(i), light))
    }

    /// The shades from brightest to darkest.
    fn make_shades(&mut self) -> Vec<Color> {
        let len = self.len();
        let mut sorted = vec![Color::WHITE; len];
        // The per-step changes towards light and dark.
        self.left = self.changes(true);
        self.right = self.changes(false);
        self.light_or_dark();
        self.middle_shades(&mut sorted);
        let left_bound = (len / 2).checked_sub(if self.even() { 2 } else { 1 });
        self.left_shades(&mut sorted, left_bound);
        self.right_shades(&mut sorted, len / 2 + 1);
        if self.description.end_darkened
            && let Some(last) = sorted.last_mut()
        {
            let right = self.right;
            *last = map(*last, |i, c| c - right[i]);
        }
        sorted
    }

    /// Moves the base color, and the changes per step, towards light or dark as the part's mode asks.
    fn light_or_dark(&mut self) {
        const MOD: f64 = 0.5;
        let steps = f64::from(u32::try_from(self.len() / 2 + 1).unwrap_or(u32::MAX));
        match self.mode {
            LightDark::Default => {}
            LightDark::Dark => {
                let right = self.right;
                self.base = map(self.base, |i, c| c - right[i] * steps * MOD);
                self.left = self.changes(false);
                self.right = self.changes(false);
            }
            LightDark::Light => {
                let left = self.left;
                self.base = map(self.base, |i, c| c + left[i] * steps * MOD);
                self.left = self.changes(true);
                self.right = self.changes(true);
                let coeff = light_darken_coeff(self.base);
                let extra = self.changes(true);
                for (r, e) in self.right.iter_mut().zip(extra) {
                    *r += coeff * e;
                }
            }
        }
    }

    /// The middle shade, or the two middle shades of an even number.
    fn middle_shades(&self, sorted: &mut [Color]) {
        let half = sorted.len() / 2;
        if self.even() {
            for p in 0..3 {
                let base = f64::from(self.base.comp(p));
                sorted[half - 1].set_comp(p, clamp(base + self.left[p] * 0.5));
                sorted[half].set_comp(p, clamp(base - self.right[p] * 0.5));
            }
        } else {
            sorted[half] = self.base;
        }
    }

    /// The bright shades: from `bound` down, each brighter than the one after it.
    fn left_shades(&self, sorted: &mut [Color], bound: Option<usize>) {
        let Some(bound) = bound else { return };
        let left = self.left;
        for ptr in (0..=bound.min(sorted.len().saturating_sub(2))).rev() {
            sorted[ptr] = map(sorted[ptr + 1], |i, c| c + left[i]);
        }
    }

    /// The dark shades: from `bound` up, each darker than the one before it.
    fn right_shades(&self, sorted: &mut [Color], bound: usize) {
        let right = self.right;
        for ptr in bound.max(1)..sorted.len() {
            sorted[ptr] = map(sorted[ptr - 1], |i, c| c - right[i]);
        }
    }

    /// Shades around the color the sibling shares with the main part.
    fn make_sibling_shades(&mut self, shared_color: Color, slots: &[i32], shared: usize) -> Vec<Color> {
        let Some(at) = slots.iter().position(|&s| as_index(s) == Some(shared)) else {
            return Vec::new();
        };
        let mut sorted = vec![Color::WHITE; slots.len()];
        let variation: [f64; 3] = [0, 1, 2].map(|_| {
            let v = f64::from(draws(self.rng, 3, VARIA) + VARIABASE) * 0.01;
            if below(self.rng, 2) == 1 { -v } else { v }
        });
        self.left = [0, 1, 2].map(|i| color_change(self.rng, at, shared_color.comp(i), true));
        let after = slots.len() - at - 1;
        self.right = [0, 1, 2].map(|i| color_change(self.rng, after, shared_color.comp(i), false));
        sorted[at] = shared_color;
        let (left, right) = (self.left, self.right);
        if at >= 1 {
            sorted[at - 1] = map(shared_color, |i, c| c + left[i] * (1.0 + variation[i]));
        }
        if at + 1 < sorted.len() {
            sorted[at + 1] = map(shared_color, |i, c| c - right[i] * (1.0 + variation[i]));
        }
        self.left_shades(&mut sorted, at.checked_sub(2));
        self.right_shades(&mut sorted, at + 2);
        sorted
    }
}

/// The color siblings start from: the shared slot's color, unless it is near black or white, then a dark or light
/// version of the part's base color.
fn sibling_color(color: Color, base: Color) -> Color {
    let all = |f: fn(u8) -> bool| (0..3).all(|i| f(color.comp(i)));
    if all(|c| c <= 3) {
        map(color, |i, _| f64::from(base.comp(i)) * 0.15)
    } else if all(|c| c >= 252) {
        map(color, |i, _| f64::from(base.comp(i)) * 0.85)
    } else {
        color
    }
}

/// How much lighter light shades get: brighter base colors get lighter shades still.
fn light_darken_coeff(base: Color) -> f64 {
    let mut comps = [base.r, base.g, base.b];
    comps.sort_unstable();
    let mut avg = f64::midpoint(f64::from(comps[1]), f64::from(comps[2]));
    if base.g >= 225 && base.r < 200 && base.b < 200 {
        avg += 75.0;
    }
    if base.g >= 225 {
        avg += 75.0;
    }
    let avg = avg.min(255.0);
    if avg < 192.0 {
        0.5
    } else {
        (0.0002 * (avg - 192.0).powf(3.5) + 50.0) * 0.01
    }
}

/// Each shade into its slot; slot -1 shades are dropped.
fn fill(palette: &mut [Color], shades: &[Color], slots: &[i32]) {
    for (&shade, &slot) in shades.iter().zip(slots) {
        if let Some(c) = as_index(slot).and_then(|s| palette.get_mut(s)) {
            *c = shade;
        }
    }
}

/// The first slot becomes the average of the others.
pub(super) fn populate_average(palette: &mut [Color], description: &PartDescription) {
    let (to, from) = description.average();
    let from: Vec<Color> = from
        .iter()
        .filter_map(|&s| as_index(s).and_then(|s| palette.get(s)).copied())
        .collect();
    let Some(target) = as_index(to).and_then(|t| palette.get_mut(t)) else {
        return;
    };
    if from.is_empty() {
        return;
    }
    let n = u32::try_from(from.len()).unwrap_or(u32::MAX);
    for i in 0..3 {
        let sum: u32 = from.iter().map(|c| u32::from(c.comp(i))).sum();
        target.set_comp(i, u8::try_from(sum / n).unwrap_or(u8::MAX));
    }
}

#[cfg(test)]
mod tests {
    use rand_core::SeedableRng;

    use super::*;

    #[test]
    fn shades_run_from_bright_to_dark() {
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(1);
        let mut palette = vec![Color::rgb(1, 2, 3); 16];
        let d = PartDescription::parse("3,4,5,6");
        Populator::populate(&mut rng, &mut palette, &d, Color::rgb(120, 60, 200), LightDark::Default);
        let lum = |c: Color| u32::from(c.r) + u32::from(c.g) + u32::from(c.b);
        assert!(palette[2..6].windows(2).all(|w| lum(w[0]) > lum(w[1])), "{palette:?}");
        assert_eq!(palette[0], Color::rgb(1, 2, 3), "other slots stay");
        // Siblings share their color with the main part.
        let d = PartDescription::parse("5,6,7;6,9,8-6");
        Populator::populate(&mut rng, &mut palette, &d, Color::rgb(200, 30, 30), LightDark::Light);
        assert_ne!(palette[8], Color::rgb(1, 2, 3));
        let avg = PartDescription::parse("A1,3,4");
        palette[2] = Color::rgb(10, 20, 30);
        palette[3] = Color::rgb(30, 40, 50);
        populate_average(&mut palette, &avg);
        assert_eq!(palette[0], Color::rgb(20, 30, 40));
    }

    #[test]
    fn color_changes_depend_on_brightness() {
        assert!((light_darken_coeff(Color::rgb(10, 10, 10)) - 0.5).abs() < f64::EPSILON);
        assert!(light_darken_coeff(Color::rgb(250, 250, 250)) > 0.5);
        assert_eq!(
            sibling_color(Color::rgb(0, 0, 0), Color::rgb(100, 200, 20)),
            Color::rgb(15, 30, 3)
        );
        assert_eq!(
            sibling_color(Color::WHITE, Color::rgb(100, 200, 20)),
            Color::rgb(85, 170, 17)
        );
    }
}
