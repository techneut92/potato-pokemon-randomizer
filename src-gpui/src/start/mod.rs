//! The start screen: title bar, drop zone, recent ROMs and supported games (`Welcome.svelte` and `Header.svelte`
//! in the web UI).

mod chrome;
mod page;
mod title_bar;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    Context, Decorations, FocusHandle, Image, KeyDownEvent, MouseButton, MouseMoveEvent, Pixels, ScrollHandle,
    SharedString, Size, Task, Window, div, prelude::*, px, relative,
};
use ppr_service::{ApiError, RomReport, SUPPORT_URL, ServiceError};

use crate::assets;
use crate::theme::{DARK, Face, FaceExt, LIGHT, Theme};

/// As many recent ROMs as the web UI keeps.
const MAX_RECENT: usize = 6;
/// How long a message stays up.
const TOAST_TIME: Duration = Duration::from_secs(4);

/// A ROM on the recent list, as the web UI stores it.
struct Recent {
    /// `G4`
    label: SharedString,
    /// `Pokémon Platinum (U)`
    name: SharedString,
    path: PathBuf,
}

/// Window-size dependent layout, with the web UI's breakpoints.
#[expect(
    clippy::struct_excessive_bools,
    reason = "independent breakpoints, as in the web UI's media queries"
)]
struct Layout {
    /// Height ≤ 720: lower title bar, smaller drop zone.
    short: bool,
    /// Width ≤ 900: no version line or Tools label.
    narrow: bool,
    /// Width ≤ 560: the logo stands for the name.
    tiny: bool,
    /// Drop zone and recent ROMs side by side.
    side_by_side: bool,
    /// Columns of the supported games grid.
    columns: u16,
}

impl Layout {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small, positive column count"
    )]
    fn new(viewport: Size<Pixels>) -> Self {
        let width = viewport.width / px(1.);
        let height = viewport.height / px(1.);
        // The page is at most 1160 px wide, with 24 px of padding on either side.
        let content = width.min(1160.) - 48.;
        Self {
            short: height <= 720.,
            narrow: width <= 900.,
            tiny: width <= 560.,
            // The drop zone's flex basis (520) + gap (24) + the recent list's (320).
            side_by_side: content >= 864.,
            // `repeat(auto-fill, minmax(250px, 1fr))` with a 12 px gap.
            columns: (((content + 12.) / 262.).floor() as u16).max(1),
        }
    }
}

pub(crate) struct StartScreen {
    focus: FocusHandle,
    logo: Arc<Image>,
    dark: bool,
    menu_open: bool,
    /// When the Tools menu last closed on an outside press.
    menu_closed_at: Option<Instant>,
    keep_loaded: bool,
    recent: Vec<Recent>,
    toast: Option<SharedString>,
    toast_timer: Option<Task<()>>,
    scroll: ScrollHandle,
    /// While the scrollbar thumb is dragged: the mouse y and scroll offset where the drag started.
    thumb_drag: Option<(f32, f32)>,
}

impl StartScreen {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        Self {
            focus,
            logo: assets::logo(),
            dark: true,
            menu_open: false,
            menu_closed_at: None,
            keep_loaded: false,
            recent: Vec::new(),
            toast: None,
            toast_timer: None,
            scroll: ScrollHandle::new(),
            thumb_drag: None,
        }
    }

    fn theme(&self) -> Theme {
        Theme::new(if self.dark { &DARK } else { &LIGHT })
    }

    /// Shows `message` for a few seconds.
    fn say(&mut self, message: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.toast = Some(message.into());
        self.toast_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_TIME).await;
            this.update(cx, |this, cx| {
                this.toast = None;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// For the parts of the app that are still only in the web UI.
    fn web_only(&mut self, what: &str, cx: &mut Context<Self>) {
        self.say(format!("{what} is in the web app for now."), cx);
    }

    #[expect(
        clippy::unused_self,
        reason = "a screen action like the others; the result comes back through `this`"
    )]
    fn open_rom(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open ROM".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = picked.await
                && let Some(path) = paths.into_iter().next()
            {
                this.update(cx, |this, cx| this.identify(path, cx)).ok();
            }
        })
        .detach();
    }

    /// Identifies the ROM at `path` off the UI thread, as the web UI does when a ROM is opened.
    #[expect(
        clippy::unused_self,
        reason = "a screen action like the others; the result comes back through `this`"
    )]
    fn identify(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let work = cx.background_executor().spawn({
            let path = path.clone();
            async move { ppr_service::identify_file(&path) }
        });
        cx.spawn(async move |this, cx| {
            let report = work.await;
            this.update(cx, |this, cx| this.identified(path, report, cx)).ok();
        })
        .detach();
    }

    fn identified(&mut self, path: PathBuf, report: Result<RomReport, ServiceError>, cx: &mut Context<Self>) {
        match report {
            Ok(RomReport::Supported { identity }) => {
                let name: SharedString = format!("Pokémon {}", identity.name).into();
                self.recent.retain(|r| r.path != path);
                self.recent.insert(
                    0,
                    Recent {
                        label: format!("G{}", identity.generation.number()).into(),
                        name: name.clone(),
                        path,
                    },
                );
                self.recent.truncate(MAX_RECENT);
                self.say(
                    format!(
                        "{name} · {}. Randomizing is in the web app for now.",
                        identity.summary()
                    ),
                    cx,
                );
            }
            Ok(RomReport::Unsupported { reason }) => self.say(reason, cx),
            Err(e) => self.say(ApiError::from(&e).message, cx),
        }
    }

    #[expect(
        clippy::unused_self,
        reason = "a screen action like the others; the result comes back through `this`"
    )]
    fn open_support(&mut self, cx: &mut Context<Self>) {
        let work = cx
            .background_executor()
            .spawn(async { ppr_service::open_support_page() });
        cx.spawn(async move |this, cx| {
            if work.await.is_err() {
                let page = SUPPORT_URL.trim_start_matches("https://");
                this.update(cx, |this, cx| {
                    this.say(format!("Couldn't open a browser. The Ko-fi page is {page}"), cx);
                })
                .ok();
            }
        })
        .detach();
    }

    fn toggle_menu(&mut self, cx: &mut Context<Self>) {
        // A press on the Tools button first closes an open menu as a press outside it; don't reopen it on the click
        // that follows.
        let just_closed = self
            .menu_closed_at
            .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
        self.menu_open = !self.menu_open && !just_closed;
        cx.notify();
    }

    fn close_menu(&mut self, cx: &mut Context<Self>) {
        if self.menu_open {
            self.menu_open = false;
            self.menu_closed_at = Some(Instant::now());
            cx.notify();
        }
    }
}

impl Render for StartScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme();
        let layout = Layout::new(window.viewport_size());
        let decorations = window.window_decorations();

        div()
            .id("root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" {
                    this.close_menu(cx);
                }
            }))
            // Presses along the edges resize the window: the app draws its own decorations.
            .capture_any_mouse_down(move |event, window, cx| {
                if event.button != MouseButton::Left {
                    return;
                }
                if let Decorations::Client { tiling } = decorations
                    && !tiling.is_tiled()
                    && let Some(edge) = chrome::resize_edge(event.position, window.viewport_size())
                {
                    window.start_window_resize(edge);
                    cx.stop_propagation();
                }
            })
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| this.drag_thumb(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    if this.thumb_drag.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(t.bg)
            .text_color(t.text)
            .face(Face::Body)
            .text_size(px(15.))
            .line_height(relative(1.21))
            .child(self.title_bar(window, &layout, cx))
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(
                        div()
                            .id("page")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .child(self.page(&layout, cx)),
                    )
                    .children(self.scrollbar(cx)),
            )
            .children(self.toast.clone().map(|message| chrome::toast(message, t)))
            .when(matches!(decorations, Decorations::Client { .. }), |root| {
                root.child(chrome::resize_cursors())
            })
    }
}
