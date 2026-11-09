//! Stroke icons on a 24×24 grid, the paths of the web UI's icons, served to `svg()` as generated SVG files.

#[derive(Clone, Copy)]
pub(crate) enum Icon {
    Upload,
    Moon,
    Heart,
    Sliders,
    Check,
    Minimize,
    Maximize,
    Restore,
    Close,
}

impl Icon {
    const ALL: [Self; 9] = [
        Self::Upload,
        Self::Moon,
        Self::Heart,
        Self::Sliders,
        Self::Check,
        Self::Minimize,
        Self::Maximize,
        Self::Restore,
        Self::Close,
    ];

    /// The asset path that `svg().path(..)` loads.
    pub(crate) fn asset(self) -> &'static str {
        match self {
            Self::Upload => "icons/upload.svg",
            Self::Moon => "icons/moon.svg",
            Self::Heart => "icons/heart.svg",
            Self::Sliders => "icons/sliders.svg",
            Self::Check => "icons/check.svg",
            Self::Minimize => "icons/minimize.svg",
            Self::Maximize => "icons/maximize.svg",
            Self::Restore => "icons/restore.svg",
            Self::Close => "icons/close.svg",
        }
    }

    fn path_data(self) -> &'static str {
        match self {
            Self::Upload => "M12 15V3M7 8l5-5 5 5M4 15v4a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2v-4",
            Self::Moon => "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z",
            Self::Heart => "M12 20s-7-4.4-9.2-8.6A5 5 0 0 1 12 6.2a5 5 0 0 1 9.2 5.2C19 15.6 12 20 12 20z",
            Self::Sliders => "M4 7h9m4 0h3M4 17h3m4 0h9M15 5v4M9 15v4",
            Self::Check => "M5 12l5 5L20 7",
            Self::Minimize => "M5 12h14",
            Self::Maximize => "M5 5h14v14H5z",
            Self::Restore => "M8 8V5h11v11h-3M5 8h11v11H5z",
            Self::Close => "M6 6l12 12M18 6L6 18",
        }
    }

    pub(crate) fn from_asset(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|icon| icon.asset() == path)
    }

    pub(crate) fn svg(self) -> String {
        format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="{}"/></svg>"#,
            self.path_data()
        )
    }
}
