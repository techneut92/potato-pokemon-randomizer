//! Window chrome the app draws itself: window controls, edge resizing, the page's scrollbar and messages.

use gpui::{
    App, Bounds, BoxShadow, Context, CursorStyle, HitboxBehavior, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels,
    Point, ResizeEdge, SharedString, Size, canvas, div, hsla, point, prelude::*, px, white,
};

use super::StartScreen;
use crate::icons::Icon;
use crate::theme::{CLOSE_RED, Theme, color};
use crate::widgets::{icon, no_drag};

/// Width of the grab area along the window edges.
const RESIZE_EDGE: Pixels = px(6.);
/// Gap between the scrollbar thumb's track and the top and bottom of the page.
const SCROLLBAR_INSET: f32 = 4.;
/// The shortest the thumb gets, so it stays easy to grab.
const MIN_THUMB: f32 = 32.;

pub(super) fn shadow() -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: hsla(0., 0., 0., 0.35),
        offset: point(px(0.), px(18.)),
        blur_radius: px(50.),
        spread_radius: px(0.),
    }]
}

/// Minimize, maximize or restore, and close (`WindowControls.svelte`).
pub(super) fn window_controls(t: Theme, maximized: bool) -> impl IntoElement {
    let control = |id: &'static str, label: Icon, size: f32, close: bool| {
        no_drag(
            div()
                .id(id)
                .group(id)
                .flex()
                .items_center()
                .justify_center()
                .w(px(48.))
                .h_full()
                .cursor_pointer()
                .hover(move |s| s.bg(if close { color(CLOSE_RED) } else { t.panel2 }))
                .child(
                    icon(label, size, t.muted)
                        .group_hover(id, move |s| s.text_color(if close { white() } else { t.text })),
                ),
        )
    };
    let (zoom_icon, zoom_size) = if maximized {
        (Icon::Restore, 15.)
    } else {
        (Icon::Maximize, 15.)
    };
    div()
        .flex()
        .flex_none()
        .h_full()
        .child(control("minimize", Icon::Minimize, 16., false).on_click(|_, window, _| window.minimize_window()))
        .child(control("maximize", zoom_icon, zoom_size, false).on_click(|_, window, _| window.zoom_window()))
        .child(control("close", Icon::Close, 16., true).on_click(|_, window, _| window.remove_window()))
}

/// A message in the bottom right corner (`.toast`).
pub(super) fn toast(message: SharedString, t: Theme) -> impl IntoElement {
    div()
        .absolute()
        .right(px(24.))
        .bottom(px(24.))
        .max_w(px(420.))
        .px(px(16.))
        .py(px(14.))
        .rounded(px(14.))
        .border_1()
        .border_color(t.line2)
        .bg(t.panel)
        .text_size(px(14.))
        .shadow(shadow())
        .child(message)
}

/// The resize cursors along the window edges.
pub(super) fn resize_cursors() -> impl IntoElement {
    canvas(
        |_bounds, window, _cx| {
            window.insert_hitbox(
                Bounds::new(point(px(0.), px(0.)), window.viewport_size()),
                HitboxBehavior::Normal,
            )
        },
        |_bounds, hitbox, window, _cx: &mut App| {
            let Some(edge) = resize_edge(window.mouse_position(), window.viewport_size()) else {
                return;
            };
            let cursor = match edge {
                ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                ResizeEdge::TopLeft | ResizeEdge::BottomRight => CursorStyle::ResizeUpLeftDownRight,
                ResizeEdge::TopRight | ResizeEdge::BottomLeft => CursorStyle::ResizeUpRightDownLeft,
            };
            window.set_cursor_style(cursor, &hitbox);
        },
    )
    .absolute()
    .size_full()
}

/// The window edge or corner at `pos`, if it is within the grab area.
pub(super) fn resize_edge(pos: Point<Pixels>, size: Size<Pixels>) -> Option<ResizeEdge> {
    let top = pos.y < RESIZE_EDGE;
    let bottom = pos.y > size.height - RESIZE_EDGE;
    let left = pos.x < RESIZE_EDGE;
    let right = pos.x > size.width - RESIZE_EDGE;
    Some(match (top, bottom, left, right) {
        (true, _, true, _) => ResizeEdge::TopLeft,
        (true, _, _, true) => ResizeEdge::TopRight,
        (_, true, true, _) => ResizeEdge::BottomLeft,
        (_, true, _, true) => ResizeEdge::BottomRight,
        (true, ..) => ResizeEdge::Top,
        (_, true, ..) => ResizeEdge::Bottom,
        (_, _, true, _) => ResizeEdge::Left,
        (_, _, _, true) => ResizeEdge::Right,
        _ => return None,
    })
}

/// Thumb height and how far it can travel, for a page `view` px high that scrolls `max` px.
fn thumb_geometry(view: f32, max: f32) -> (f32, f32) {
    let track = view - 2. * SCROLLBAR_INSET;
    let thumb = (track * view / (view + max)).clamp(MIN_THUMB.min(track), track);
    (thumb, track - thumb)
}

impl StartScreen {
    /// The page's visible height, how far it can scroll and how far it has, in px.
    fn scroll_metrics(&self) -> (f32, f32, f32) {
        let view = self.scroll.bounds().size.height / px(1.);
        let max = self.scroll.max_offset().height / px(1.);
        let offset = -(self.scroll.offset().y / px(1.));
        (view, max, offset.clamp(0., max.max(0.)))
    }

    pub(super) fn drag_thumb(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some((start_y, start_offset)) = self.thumb_drag else {
            return;
        };
        if event.pressed_button != Some(MouseButton::Left) {
            self.thumb_drag = None;
            cx.notify();
            return;
        }
        let (view, max, _) = self.scroll_metrics();
        let (_, travel) = thumb_geometry(view, max);
        if travel <= 0. {
            return;
        }
        let moved = event.position.y / px(1.) - start_y;
        let offset = (start_offset + moved * max / travel).clamp(0., max);
        self.scroll.set_offset(point(px(0.), px(-offset)));
        cx.notify();
    }

    /// An overlay scrollbar for the page, as the web view draws one; none when the page fits.
    pub(super) fn scrollbar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let (view, max, offset) = self.scroll_metrics();
        if max <= 0.5 || view <= 0. {
            return None;
        }
        let t = self.theme();
        let (thumb, travel) = thumb_geometry(view, max);
        let dragging = self.thumb_drag.is_some();
        Some(
            div()
                .id("scrollbar-thumb")
                .absolute()
                .top(px(SCROLLBAR_INSET + travel * offset / max))
                .right(px(3.))
                .w(px(6.))
                .h(px(thumb))
                .rounded_full()
                .bg(if dragging { t.muted } else { t.faint.opacity(0.6) })
                .hover(move |s| s.bg(t.muted))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _, cx| {
                        let (_, _, offset) = this.scroll_metrics();
                        this.thumb_drag = Some((event.position.y / px(1.), offset));
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use gpui::size;

    use super::*;

    #[test]
    fn edges_and_corners() {
        let window = size(px(800.), px(600.));
        assert_eq!(resize_edge(point(px(2.), px(2.)), window), Some(ResizeEdge::TopLeft));
        assert_eq!(
            resize_edge(point(px(798.), px(598.)), window),
            Some(ResizeEdge::BottomRight)
        );
        assert_eq!(resize_edge(point(px(400.), px(2.)), window), Some(ResizeEdge::Top));
        assert_eq!(resize_edge(point(px(797.), px(300.)), window), Some(ResizeEdge::Right));
        assert_eq!(resize_edge(point(px(400.), px(300.)), window), None);
    }

    #[test]
    fn thumb_is_as_long_as_the_visible_share() {
        // Half the page visible: the thumb fills half the track.
        let (thumb, travel) = thumb_geometry(408., 408.);
        assert!((thumb - 200.).abs() < 0.01);
        assert!((travel - 200.).abs() < 0.01);
        // A very long page: the thumb keeps its minimum length.
        assert!((thumb_geometry(408., 100_000.).0 - MIN_THUMB).abs() < 0.01);
    }
}
