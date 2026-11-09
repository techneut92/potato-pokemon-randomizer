//! Design tokens. They mirror the web UI's CSS variables in `src/app.css`; a test keeps the two equal.

use gpui::{FontWeight, Hsla, Styled, rgb};

/// The colours of one theme, as `0xRRGGBB`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Palette {
    pub(crate) bg: u32,
    pub(crate) panel: u32,
    pub(crate) panel2: u32,
    pub(crate) line: u32,
    pub(crate) line2: u32,
    pub(crate) text: u32,
    pub(crate) muted: u32,
    pub(crate) faint: u32,
    /// Text on the accent (`--on`).
    pub(crate) on: u32,
}

/// `.app` in `src/app.css`.
pub(crate) const DARK: Palette = Palette {
    bg: 0x11_1216,
    panel: 0x19_1B21,
    panel2: 0x20_232B,
    line: 0x2C_2F39,
    line2: 0x3A_3E4A,
    text: 0xEC_EDF2,
    muted: 0xA9_ACBA,
    faint: 0x8A_8EA0,
    on: 0x1B_0A0C,
};

/// `.app[data-theme=light]` in `src/app.css`.
pub(crate) const LIGHT: Palette = Palette {
    bg: 0xF3_F3F6,
    panel: 0xFF_FFFF,
    panel2: 0xF0_F0F4,
    line: 0xE0_E1E8,
    line2: 0xC9_CBD5,
    text: 0x16_171C,
    muted: 0x4F_5361,
    faint: 0x64_6877,
    on: 0xFF_FFFF,
};

/// The default accent (`accent` in `src/lib/app.svelte.ts`).
pub(crate) const ACCENT: u32 = 0xFF_5A5F;
/// Ko-fi's own colour, so the support button reads as the support link.
pub(crate) const KOFI: u32 = 0xFF_5E5B;
/// Background of the close button on hover.
pub(crate) const CLOSE_RED: u32 = 0xE5_484D;

/// A palette resolved to GPUI colours.
#[derive(Clone, Copy)]
pub(crate) struct Theme {
    pub(crate) bg: Hsla,
    pub(crate) panel: Hsla,
    pub(crate) panel2: Hsla,
    pub(crate) line: Hsla,
    pub(crate) line2: Hsla,
    pub(crate) text: Hsla,
    pub(crate) muted: Hsla,
    pub(crate) faint: Hsla,
    pub(crate) accent: Hsla,
    pub(crate) on_accent: Hsla,
}

pub(crate) fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

impl Theme {
    pub(crate) fn new(palette: &Palette) -> Self {
        Self {
            bg: color(palette.bg),
            panel: color(palette.panel),
            panel2: color(palette.panel2),
            line: color(palette.line),
            line2: color(palette.line2),
            text: color(palette.text),
            muted: color(palette.muted),
            faint: color(palette.faint),
            accent: color(ACCENT),
            on_accent: color(palette.on),
        }
    }

    /// The accent a little brighter, for the primary button's hover (`filter: brightness(1.08)`).
    pub(crate) fn accent_hover(&self) -> Hsla {
        let mut c = self.accent;
        c.l = (c.l * 1.08).min(1.0);
        c
    }
}

/// A typeface and weight. The bundled fonts are web subsets without a typographic family name, so each weight
/// registers as a family of its own.
#[derive(Clone, Copy)]
pub(crate) enum Face {
    Body,
    BodyMedium,
    BodySemibold,
    DisplaySemibold,
    DisplayBold,
    Mono,
}

impl Face {
    fn family(self) -> &'static str {
        match self {
            Self::Body => "Inter",
            Self::BodyMedium => "Inter Medium",
            Self::BodySemibold => "Inter SemiBold",
            Self::DisplaySemibold => "Space Grotesk SemiBold",
            Self::DisplayBold => "Space Grotesk",
            Self::Mono => "JetBrains Mono",
        }
    }

    fn weight(self) -> FontWeight {
        match self {
            Self::Body | Self::Mono => FontWeight::NORMAL,
            Self::BodyMedium => FontWeight::MEDIUM,
            Self::BodySemibold | Self::DisplaySemibold => FontWeight::SEMIBOLD,
            Self::DisplayBold => FontWeight::BOLD,
        }
    }
}

/// Sets a [`Face`] on any styled element.
pub(crate) trait FaceExt: Styled + Sized {
    fn face(self, face: Face) -> Self {
        self.font_family(face.family()).font_weight(face.weight())
    }
}

impl<T: Styled> FaceExt for T {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    const APP_CSS: &str = include_str!("../../src/app.css");
    const APP_STATE: &str = include_str!("../../src/lib/app.svelte.ts");

    /// The `--name:#RRGGBB` declarations of the first CSS rule whose selector is `selector`.
    fn css_colors(selector: &str) -> HashMap<String, u32> {
        let start = APP_CSS
            .find(&format!("\n{selector}{{"))
            .unwrap_or_else(|| panic!("{selector} is in app.css"))
            + selector.len()
            + 2;
        let body = &APP_CSS[start..start + APP_CSS[start..].find('}').expect("the rule ends")];
        body.split(';')
            .filter_map(|decl| {
                let (name, value) = decl.split_once(':')?;
                let hex = value.trim().strip_prefix('#')?;
                Some((
                    name.trim().trim_start_matches("--").to_owned(),
                    u32::from_str_radix(hex, 16).ok()?,
                ))
            })
            .collect()
    }

    fn assert_matches(palette: &Palette, selector: &str) {
        let css = css_colors(selector);
        let pairs = [
            ("bg", palette.bg),
            ("panel", palette.panel),
            ("panel2", palette.panel2),
            ("line", palette.line),
            ("line2", palette.line2),
            ("text", palette.text),
            ("muted", palette.muted),
            ("faint", palette.faint),
            ("on", palette.on),
        ];
        for (name, ours) in pairs {
            assert_eq!(
                css.get(name),
                Some(&ours),
                "--{name} of {selector} differs from app.css"
            );
        }
    }

    #[test]
    fn palettes_match_the_web_ui() {
        assert_matches(&DARK, ".app");
        assert_matches(&LIGHT, ".app[data-theme=light]");
    }

    #[test]
    fn accent_matches_the_web_ui() {
        assert!(
            APP_STATE.contains(&format!("accent = $state('#{ACCENT:06X}')")),
            "the default accent in app.svelte.ts differs"
        );
    }
}
