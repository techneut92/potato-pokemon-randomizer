//! The main window. It is built here rather than in `tauri.conf.json`, so its first size can follow the monitor
//! and its frame can follow the platform: the app draws its own title bar on Linux and Windows, and keeps the
//! native traffic lights over that bar on macOS.

use tauri::{AppHandle, LogicalSize, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Label of the main window; the capabilities in `capabilities/default.json` refer to it.
pub(crate) const MAIN: &str = "main";

/// Size on a large enough screen, in logical pixels.
const PREFERRED: Size = Size {
    width: 1440.0,
    height: 900.0,
};
/// Smallest size the layout supports.
pub(crate) const MINIMUM: Size = Size {
    width: 480.0,
    height: 480.0,
};
/// Share of the work area the first window may take.
const SCREEN_SHARE: f64 = 0.9;

/// A size in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Size {
    pub(crate) width: f64,
    pub(crate) height: f64,
}

impl Size {
    /// The first window size on a work area of this size: the preferred size where it fits, otherwise 90% of the
    /// work area, but never below the minimum.
    pub(crate) fn initial(work_area: Self) -> Self {
        let fit = |preferred: f64, available: f64, minimum: f64| (available * SCREEN_SHARE).min(preferred).max(minimum);
        Self {
            width: fit(PREFERRED.width, work_area.width, MINIMUM.width),
            height: fit(PREFERRED.height, work_area.height, MINIMUM.height),
        }
    }
}

/// The work area of the primary monitor, if the platform reports one (Wayland may not).
fn work_area(app: &AppHandle) -> Option<Size> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let area = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
    Some(Size {
        width: area.width,
        height: area.height,
    })
}

/// Builds the main window. `tauri-plugin-window-state` then restores the user's own size and position.
pub(crate) fn create(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let size = work_area(app).map_or(PREFERRED, Size::initial);
    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::default())
        .title("Potato Pokémon Randomizer")
        .inner_size(size.width, size.height)
        .min_inner_size(MINIMUM.width, MINIMUM.height)
        .center();
    #[cfg(target_os = "macos")]
    let builder = builder
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true);
    #[cfg(not(target_os = "macos"))]
    let builder = builder.decorations(false);
    let window = builder.build()?;
    // Keep the minimum consistent if the restored state is smaller (e.g. from an older version).
    window.set_min_size(Some(LogicalSize::new(MINIMUM.width, MINIMUM.height)))?;
    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_screens_get_the_preferred_size() {
        assert_eq!(
            Size::initial(Size {
                width: 2560.0,
                height: 1400.0
            }),
            PREFERRED
        );
    }

    #[test]
    fn small_screens_get_most_of_the_work_area() {
        assert_eq!(
            Size::initial(Size {
                width: 1280.0,
                height: 760.0
            }),
            Size {
                width: 1152.0,
                height: 684.0
            }
        );
    }

    #[test]
    fn the_minimum_wins_on_tiny_screens() {
        assert_eq!(
            Size::initial(Size {
                width: 500.0,
                height: 400.0
            }),
            MINIMUM
        );
    }
}
