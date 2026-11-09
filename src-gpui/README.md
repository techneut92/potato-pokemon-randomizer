# ppr-gpui

An experimental native front end for Potato Pokémon Randomizer, built with [GPUI](https://www.gpui.rs/). For now it
has the start screen only: open or drop a ROM, recent ROMs and the supported games. Everything after that is in the
web UI (`src/`, run with `pnpm tauri dev`). [ADR 0009](../docs/adr/0009-native-ui-with-gpui.md) explains why it
exists and the rules it follows.

```sh
cargo run -p ppr-gpui
cargo test -p ppr-gpui     # includes the parity tests against the web UI's sources
tools/ci.sh gpui           # clippy and tests, as in CI
```

## Layout

```text
src/
  main.rs        opens the window
  assets.rs      fonts, logo and icons, embedded in the binary
  theme.rs       colours and type faces; tested against src/app.css
  games.rs       the supported-games cards; tested against roms.ts and ppr-service
  icons.rs       the web UI's stroke icons that the screen uses
  widgets.rs     buttons, icon buttons and chips
  start/         the start screen: state and actions (mod.rs), title bar, page, window chrome
```

All work on ROMs goes through `ppr-service`, like the Tauri commands and the CLI.

## System libraries

On Linux, GPUI needs the development files of xkbcommon (with X11), Wayland, fontconfig, FreeType, xcb and Vulkan.
On Debian and Ubuntu:

```sh
sudo apt-get install libxkbcommon-x11-dev libwayland-dev libfontconfig-dev libfreetype-dev libxcb1-dev \
  libx11-xcb-dev libvulkan-dev
```

On Fedora: `libxkbcommon-x11-devel wayland-devel fontconfig-devel freetype-devel libxcb-devel vulkan-loader-devel`.
macOS needs Xcode; Windows needs nothing extra.

The lockfile keeps `libc` at 0.2.189: gpui 0.2.2 depends on `xattr` 0.2, which does not build with later `libc`
releases. Don't run a plain `cargo update` without checking that.

## Fonts

`assets/fonts/` holds the web UI's fonts as TrueType: Inter, Space Grotesk and JetBrains Mono, each under the SIL
Open Font License 1.1 (`LICENSE-*.txt` next to them).
