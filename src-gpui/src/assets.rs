//! Everything the app shows is compiled in: fonts, the logo and the icons.

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::Result;
use gpui::{AssetSource, Image, ImageFormat, SharedString};

use crate::icons::Icon;

/// The web UI's three font families, converted to TrueType. Their licences are beside them.
const FONTS: [&[u8]; 9] = [
    include_bytes!("../assets/fonts/Inter-400.ttf"),
    include_bytes!("../assets/fonts/Inter-500.ttf"),
    include_bytes!("../assets/fonts/Inter-600.ttf"),
    include_bytes!("../assets/fonts/Inter-700.ttf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-500.ttf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-600.ttf"),
    include_bytes!("../assets/fonts/SpaceGrotesk-700.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-400.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-500.ttf"),
];

/// The app icon of the Tauri app, so both show the same logo.
const LOGO: &[u8] = include_bytes!("../../src-tauri/icons/128x128@2x.png");

pub(crate) fn fonts() -> Vec<Cow<'static, [u8]>> {
    FONTS.iter().map(|font| Cow::Borrowed(*font)).collect()
}

/// The logo as an in-memory image. (`img()` would take a bare name like `icon.png` for a URL.)
pub(crate) fn logo() -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Png, LOGO.to_vec()))
}

/// Serves the icons to `svg()`.
pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Icon::from_asset(path).map(|icon| Cow::Owned(icon.svg().into_bytes())))
    }

    fn list(&self, _path: &str) -> Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
