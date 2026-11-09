//! The page under the title bar: drop zone, recent ROMs and supported games (`Welcome.svelte`).

use gpui::{AnyElement, Context, ExternalPaths, div, prelude::*, px, relative};

use super::{Layout, Recent, StartScreen};
use crate::games::{EXTENSIONS, SUPPORTED_GAMES};
use crate::icons::Icon;
use crate::theme::{Face, FaceExt};
use crate::widgets::{Height, Variant, button, chip, icon};

impl StartScreen {
    pub(super) fn page(&self, layout: &Layout, cx: &mut Context<Self>) -> impl IntoElement {
        let short = layout.short;
        let drop_zone = self.drop_zone(layout, cx);
        let recents = self.recents(cx);
        let top = if layout.side_by_side {
            div()
                .flex()
                .gap(px(24.))
                .child(drop_zone.flex_1().min_w(px(0.)))
                .child(recents.w(px(320.)).flex_none())
        } else {
            div().flex().flex_col().gap(px(24.)).child(drop_zone).child(recents)
        };

        div().w_full().flex().justify_center().child(
            div()
                .w_full()
                .max_w(px(1160.))
                .px(px(24.))
                .pt(px(if short { 24. } else { 56. }))
                .pb(px(if short { 40. } else { 64. }))
                .flex()
                .flex_col()
                .gap(px(if short { 28. } else { 40. }))
                .child(top)
                .child(self.supported_games(layout)),
        )
    }

    fn drop_zone(&self, layout: &Layout, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let t = self.theme();
        let short = layout.short;
        div()
            .id("drop-zone")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(if short { 14. } else { 18. }))
            .py(px(if short { 28. } else { 56. }))
            .px(px(if short { 24. } else { 32. }))
            .rounded(px(22.))
            .border_2()
            .border_dashed()
            .border_color(t.line2)
            .bg(t.panel)
            .drag_over::<ExternalPaths>(move |style, _, _, _| style.border_color(t.accent).bg(t.panel2))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                if let Some(path) = paths.paths().first() {
                    this.identify(path.clone(), cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(if short { 56. } else { 72. }))
                    .rounded(px(if short { 16. } else { 20. }))
                    .bg(t.panel2)
                    .child(icon(Icon::Upload, 34., t.accent)),
            )
            .child(
                div()
                    .face(Face::DisplayBold)
                    .text_size(px(if short { 28. } else { 34. }))
                    .line_height(relative(1.1))
                    .text_center()
                    .child("Drop a ROM here to start"),
            )
            .child(div().max_w(px(440.)).text_size(px(15.)).text_color(t.muted).text_center().child(
                "Or open one from disk. The randomizer reads the game, then shows only the options that game supports.",
            ))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(10.))
                    .child(
                        button("open-rom", t, Variant::Primary, Height::Large)
                            .child("Open ROM…")
                            .on_click(cx.listener(|this, _, _, cx| this.open_rom(cx))),
                    )
                    .child(
                        button("premade-seed", t, Variant::Default, Height::Large)
                            .child("Premade Seed")
                            .on_click(cx.listener(|this, _, _, cx| this.web_only("Premade seeds", cx))),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .justify_center()
                    .gap(px(6.))
                    .children(EXTENSIONS.map(|ext| chip(ext, t, true))),
            )
    }

    fn recents(&self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme();
        let list: Vec<AnyElement> = if self.recent.is_empty() {
            vec![
                div()
                    .text_size(px(14.))
                    .text_color(t.faint)
                    .child("ROMs you open appear here.")
                    .into_any_element(),
            ]
        } else {
            self.recent
                .iter()
                .enumerate()
                .map(|(ix, rom)| self.recent_row(ix, rom, cx).into_any_element())
                .collect()
        };

        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(20.))
            .rounded(px(16.))
            .border_1()
            .border_color(t.line)
            .bg(t.panel)
            .child(
                div()
                    .mb(px(8.))
                    .face(Face::DisplayBold)
                    .text_size(px(18.))
                    .child("Recent ROMs"),
            )
            .children(list)
            .child(div().flex_1())
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .pt(px(14.))
                    .border_t_1()
                    .border_color(t.line)
                    .child(
                        button("names", t, Variant::Default, Height::Regular)
                            .child("Custom Names Editor")
                            .on_click(cx.listener(|this, _, _, cx| this.web_only("The custom names editor", cx))),
                    )
                    .child(
                        button("ui-theme", t, Variant::Default, Height::Regular)
                            .child("Change UI Theme")
                            .on_click(cx.listener(|this, _, _, cx| this.web_only("Changing the UI theme", cx))),
                    ),
            )
    }

    fn recent_row(&self, ix: usize, rom: &Recent, cx: &mut Context<Self>) -> gpui::Stateful<gpui::Div> {
        let t = self.theme();
        let path = rom.path.clone();
        div()
            .id(("recent", ix))
            .flex()
            .items_center()
            .gap(px(12.))
            .w_full()
            .min_h(px(56.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(12.))
            .cursor_pointer()
            .hover(move |s| s.bg(t.panel2))
            .on_click(cx.listener(move |this, _, _, cx| this.identify(path.clone(), cx)))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded(px(9.))
                    .bg(t.panel2)
                    .face(Face::BodySemibold)
                    .text_size(px(12.))
                    .text_color(t.muted)
                    .child(rom.label.clone()),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .child(
                        div()
                            .face(Face::BodyMedium)
                            .text_size(px(14.))
                            .truncate()
                            .child(rom.name.clone()),
                    )
                    .child(
                        div()
                            .face(Face::Mono)
                            .text_size(px(12.))
                            .text_color(t.faint)
                            .truncate()
                            .child(rom.path.display().to_string()),
                    ),
            )
    }

    fn supported_games(&self, layout: &Layout) -> impl IntoElement {
        let t = self.theme();
        let cards = SUPPORTED_GAMES.iter().map(|card| {
            div()
                .flex()
                .flex_col()
                .gap(px(10.))
                .p(px(16.))
                .rounded(px(16.))
                .border_1()
                .border_color(t.line)
                .bg(t.panel)
                .child(
                    div()
                        .flex()
                        .items_baseline()
                        .justify_between()
                        .gap(px(8.))
                        .child(div().face(Face::DisplaySemibold).text_size(px(16.)).child(card.title()))
                        .child(div().text_size(px(12.)).text_color(t.faint).child(card.console)),
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(card.games.iter().map(|game| chip(*game, t, false))),
                )
        });
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .face(Face::DisplayBold)
                    .text_size(px(20.))
                    .child("Supported games"),
            )
            .child(div().grid().grid_cols(layout.columns).gap(px(12.)).children(cards))
    }
}
