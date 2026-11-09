//! The title bar: logo and name, the support, theme and Tools buttons, and the window controls (`Header.svelte`).

use gpui::{Context, MouseButton, Window, deferred, div, img, prelude::*, px, relative};

use super::{Layout, StartScreen, chrome};
use crate::icons::Icon;
use crate::theme::{Face, FaceExt, KOFI, color};
use crate::widgets::{Height, Variant, button, icon, icon_button, no_drag};

impl StartScreen {
    pub(super) fn title_bar(&self, window: &Window, layout: &Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let height = if layout.short { Height::Compact } else { Height::Regular };

        let names = div()
            .flex()
            .flex_col()
            .min_w(px(0.))
            .line_height(relative(1.15))
            .child(
                div()
                    .face(Face::DisplayBold)
                    .text_size(px(16.))
                    .truncate()
                    .child("Potato Pokémon Randomizer"),
            )
            .when(!layout.narrow && !layout.short, |names| {
                names.child(
                    div()
                        .text_size(px(12.))
                        .text_color(t.faint)
                        .truncate()
                        .child(concat!("Version ", env!("CARGO_PKG_VERSION"))),
                )
            });
        let brand = div()
            .flex()
            .flex_shrink()
            .min_w(px(0.))
            .items_center()
            .gap(px(12.))
            .child(img(self.logo.clone()).size(px(34.)).flex_none())
            .when(!layout.tiny, |brand| brand.child(names));

        let tools = div()
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            // On the smallest windows both are in the Tools menu only.
            .when(!layout.tiny, |tools| {
                tools
                    .child(
                        icon_button("support", t, Icon::Heart, color(KOFI), height)
                            .on_click(cx.listener(|this, _, _, cx| this.open_support(cx))),
                    )
                    .child(
                        icon_button("theme", t, Icon::Moon, t.muted, height).on_click(cx.listener(|this, _, _, cx| {
                            this.dark = !this.dark;
                            cx.notify();
                        })),
                    )
            })
            .child(
                button("tools", t, Variant::Default, height)
                    .child(icon(Icon::Sliders, 18., t.text))
                    .when(!layout.narrow, |b| b.child("Tools"))
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_menu(cx))),
            )
            .when(self.menu_open, |tools| tools.child(self.tools_menu(layout, cx)));

        div()
            .id("title-bar")
            .flex()
            .flex_none()
            .items_center()
            .gap(px(if layout.narrow { 10. } else { 16. }))
            .h(px(if layout.short { 48. } else { 58. }))
            .pl(px(if layout.narrow { 12. } else { 20. }))
            .border_b_1()
            .border_color(t.line)
            .bg(t.panel)
            // Empty space and the brand drag the window; a double-click maximizes it.
            .on_mouse_down(MouseButton::Left, |event, window, _| {
                if event.click_count == 2 {
                    window.zoom_window();
                } else {
                    window.start_window_move();
                }
            })
            .child(brand)
            .child(div().flex_1().min_w(px(8.)))
            .child(tools)
            .child(chrome::window_controls(t, window.is_maximized()))
    }

    fn tools_menu(&self, layout: &Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let row = |id: &'static str| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(10.))
                .w_full()
                .h(px(42.))
                .px(px(12.))
                .rounded(px(9.))
                .text_size(px(14.))
        };
        let item = |id: &'static str, label: &'static str| {
            row(id).cursor_pointer().hover(move |s| s.bg(t.panel2)).child(label)
        };
        let web_only = |what: &'static str| {
            cx.listener(move |this: &mut Self, _, _, cx| {
                this.close_menu(cx);
                this.web_only(what, cx);
            })
        };
        let separator = || div().h(px(1.)).mx(px(4.)).my(px(6.)).bg(t.line);
        let checked = self.keep_loaded;
        let checkbox = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(18.))
            .rounded(px(4.))
            .border_1()
            .map(|b| {
                if checked {
                    b.bg(t.accent)
                        .border_color(t.accent)
                        .child(icon(Icon::Check, 14., t.on_accent))
                } else {
                    b.border_color(t.faint)
                }
            });

        deferred(no_drag(
            div()
                .id("tools-menu")
                .absolute()
                .top(px(if layout.short { 44. } else { 48. }))
                .right(px(0.))
                .w(px(300.))
                .p(px(6.))
                .rounded(px(14.))
                .border_1()
                .border_color(t.line2)
                .bg(t.panel)
                .text_color(t.text)
                .shadow(chrome::shadow())
                .on_mouse_down_out(cx.listener(|this, _, _, cx| this.close_menu(cx)))
                // Needs a loaded game, as in the web UI.
                .child(row("mi-string").text_color(t.faint).child("Get / Load Settings String"))
                .child(item("mi-batch", "Batch Randomization Settings").on_click(web_only("Batch randomization")))
                .child(item("mi-names", "Custom Names Editor").on_click(web_only("The custom names editor")))
                .child(item("mi-theme", "Change UI Theme").on_click(web_only("Changing the UI theme")))
                .child(separator())
                .child(
                    row("mi-keep")
                        .cursor_pointer()
                        .hover(move |s| s.bg(t.panel2))
                        .child(checkbox)
                        .child("Keep game loaded after randomizing")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.keep_loaded = !this.keep_loaded;
                            cx.notify();
                        })),
                )
                .child(separator())
                .child(
                    item("mi-kofi", "Support on Ko-fi…").on_click(cx.listener(|this, _, _, cx| {
                        this.close_menu(cx);
                        this.open_support(cx);
                    })),
                ),
        ))
        .with_priority(1)
    }
}
