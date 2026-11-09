//! Native desktop front end of the Potato Pokémon Randomizer, drawn with GPUI (ADR 0009).
//!
//! Only the start screen exists so far; the Tauri app in `src-tauri` is the one that ships. Like it, this app reaches
//! the randomizer only through `ppr-service`.

mod assets;
mod games;
mod icons;
mod start;
mod theme;
mod widgets;

use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBounds, WindowDecorations, WindowOptions, prelude::*, px, size,
};

/// Smallest window, as for the Tauri app (`src-tauri/src/window.rs`).
const MIN_WINDOW: f32 = 480.;

fn main() {
    Application::new().with_assets(assets::Assets).run(|cx: &mut App| {
        cx.text_system()
            .add_fonts(assets::fonts())
            .expect("the bundled fonts load");

        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Potato Pokémon Randomizer".into()),
                    ..Default::default()
                }),
                // The app draws its own title bar, as the Tauri app does.
                window_decorations: Some(WindowDecorations::Client),
                window_min_size: Some(size(px(MIN_WINDOW), px(MIN_WINDOW))),
                app_id: Some("potato-pokemon-randomizer-gpui".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| start::StartScreen::new(window, cx)),
        )
        .expect("the window opens");

        cx.on_window_closed(|cx| cx.quit()).detach();
        cx.activate(true);
    });
}
