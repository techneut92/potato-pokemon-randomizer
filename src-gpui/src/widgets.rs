//! Small building blocks of the screen: buttons, chips and icons, styled like the web UI's `.btn` and `.chip`.

use gpui::{
    Div, ElementId, Hsla, InteractiveElement, IntoElement, MouseButton, ParentElement, SharedString, Stateful, Styled,
    Svg, div, px, svg, transparent_black,
};

use crate::icons::Icon;
use crate::theme::{Face, FaceExt, Theme};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Variant {
    /// `.btn`
    Default,
    /// `.btn.primary`
    Primary,
}

/// Button heights: regular, large (`.btn.lg`) and compact (short windows).
#[derive(Clone, Copy)]
pub(crate) enum Height {
    Regular,
    Large,
    Compact,
}

impl Height {
    fn px(self) -> f32 {
        match self {
            Self::Regular => 40.,
            Self::Large => 44.,
            Self::Compact => 36.,
        }
    }
}

pub(crate) fn icon(icon: Icon, size: f32, color: Hsla) -> Svg {
    svg().path(icon.asset()).size(px(size)).flex_none().text_color(color)
}

/// Stops a mouse-down here, so pressing a control in the title bar doesn't drag the window.
pub(crate) fn no_drag<E: InteractiveElement>(element: E) -> E {
    element.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// A text button.
pub(crate) fn button(id: impl Into<ElementId>, theme: Theme, variant: Variant, height: Height) -> Stateful<Div> {
    let base = no_drag(
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .h(px(height.px()))
            .px(px(if matches!(height, Height::Large) { 20. } else { 14. }))
            .rounded(px(10.))
            .border_1()
            .text_size(px(14.))
            .whitespace_nowrap()
            .cursor_pointer(),
    );
    match variant {
        Variant::Default => base
            .face(Face::BodyMedium)
            .border_color(theme.line2)
            .bg(theme.panel2)
            .text_color(theme.text)
            .hover(move |style| style.border_color(theme.faint)),
        Variant::Primary => base
            .face(Face::BodySemibold)
            .border_color(theme.accent)
            .bg(theme.accent)
            .text_color(theme.on_accent)
            .hover(move |style| style.bg(theme.accent_hover()).border_color(theme.accent_hover())),
    }
}

/// An icon-only ghost button (`.btn.ghost`). A muted icon brightens on hover; a coloured one keeps its colour and
/// gets a border in that colour instead.
pub(crate) fn icon_button(id: &'static str, theme: Theme, glyph: Icon, color: Hsla, height: Height) -> Stateful<Div> {
    let muted = color == theme.muted;
    let hover_color = if muted { theme.text } else { color };
    let hover_border = if muted {
        transparent_black()
    } else {
        color.opacity(0.45)
    };
    no_drag(
        div()
            .id(id)
            .group(id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(height.px()))
            .px(px(14.))
            .rounded(px(10.))
            .border_1()
            .border_color(transparent_black())
            .cursor_pointer()
            .hover(move |style| style.bg(theme.panel2).border_color(hover_border))
            .child(icon(glyph, 18., color).group_hover(id, move |style| style.text_color(hover_color))),
    )
}

/// A chip (`.chip`), in the monospace face for file types.
pub(crate) fn chip(text: impl Into<SharedString>, theme: Theme, mono: bool) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .h(px(28.))
        .px(px(10.))
        .rounded_full()
        .bg(theme.panel2)
        .text_color(theme.muted)
        .text_size(px(13.))
        .face(if mono { Face::Mono } else { Face::Body })
        .child(text.into())
}
