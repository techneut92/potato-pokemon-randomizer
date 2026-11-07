# Releasing

A release is a `vX.Y.Z` tag with a GitHub Release that carries the bundles and the version's CHANGELOG entry as
notes. There are two ways to build the bundles:

- **Tag → CI** (once the repository is public): pushing the tag runs `.github/workflows/release.yml` on hosted
  runners and leaves a draft release with every bundle.
- **Local fallback** (now, and whenever CI is unavailable): `tools/bundle.sh` builds the Linux bundles, the Flatpak
  and the Windows installer on a Linux machine, and `tools/release.sh` publishes them.

| Bundle | CI | Local (`tools/bundle.sh`) |
| --- | --- | --- |
| AppImage, `.deb`, `.rpm` | ubuntu-22.04 | `linux` |
| Flatpak (`potato-pokemon-randomizer_X.Y.Z_x86_64.flatpak`) | flatpak-builder container | `flatpak` |
| Windows NSIS installer (`*_x64-setup.exe`) | windows-latest | `windows` (cross-compiled) |
| Windows `.msi` | windows-latest | no: WiX only runs on Windows |
| macOS universal `.dmg` | macos-latest | no |

Bundles are not signed yet: Windows SmartScreen and macOS Gatekeeper warn on first start.

## 1. Prepare the release commit

1. Bump the version in `package.json` (`"version"`) and in `Cargo.toml` (`[workspace.package] version`); the Tauri
   config takes the version from `package.json`. Run `cargo check` so `Cargo.lock` follows.
2. Check that `CHANGELOG.md` is complete: every change since the last tag (`git log vPREVIOUS..HEAD`) has an entry
   under `## [Unreleased]`. Then rename `## [Unreleased]` to `## [X.Y.Z] - YYYY-MM-DD` and start a new empty
   `## [Unreleased]` above it.
3. Add the release to `packaging/flatpak/io.github.techneut92.potato-pokemon-randomizer.metainfo.xml`, newest
   first: `<release version="X.Y.Z" date="YYYY-MM-DD">` with a short description. `tools/bundle.sh flatpak` refuses
   to build without it. Validate with `appstreamcli validate --no-net packaging/flatpak/*.metainfo.xml`.
4. Run `tools/ci.sh`, then commit (`chore: release X.Y.Z`) and merge to `main`.
5. Tag the merged commit and push the tag:

   ```sh
   git tag -a vX.Y.Z -m "X.Y.Z"
   git push origin vX.Y.Z
   ```

## 2a. Tag → CI release (public repository)

The pushed tag starts the `Release` workflow (or run it by hand: Actions → Release → Run workflow, with the tag). It
builds the bundles on Linux, Windows and macOS, the Flatpak from the Linux binary, and creates a **draft** release
for the tag with the CHANGELOG entry as notes. Download a few bundles, try them (see the checks below), then publish
the draft on GitHub.

`ci.yml` stays on demand; once the repository is public, add `push` and `pull_request` triggers there (see the
comment at its top).

## 2b. Local fallback: `tools/bundle.sh` + `tools/release.sh`

### One-time setup (Fedora)

Rust builds need the [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/). On Fedora Atomic, put them in
a toolbox (here `ppr-build`) and pass it as `PPR_RUN`; pnpm must be installed in the toolbox too (`dnf install pnpm`).

```sh
# Windows cross-build (in the toolbox)
sudo dnf install clang lld llvm mingw32-nsis
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin

# Flatpak (on the host; flatpak-builder doesn't run in a toolbox)
flatpak install --user flathub org.flatpak.Builder org.gnome.Platform//51 org.gnome.Sdk//51
```

cargo-xwin downloads the Windows SDK and CRT headers on first use (into `~/.cache/cargo-xwin`), which accepts
Microsoft's licence for them. Tauri downloads the NSIS plugins and the AppImage tools on first use.

### Build

```sh
PPR_RUN="toolbox run -c ppr-build" tools/bundle.sh          # everything; or: linux | flatpak | windows
```

| Output | Path |
| --- | --- |
| AppImage, `.deb`, `.rpm` | `target/release/bundle/{appimage,deb,rpm}/` |
| Flatpak | `target/release/bundle/flatpak/potato-pokemon-randomizer_X.Y.Z_x86_64.flatpak` |
| Windows installer | `target/x86_64-pc-windows-msvc/release/bundle/nsis/Potato Pokemon Randomizer_X.Y.Z_x64-setup.exe` |

The Flatpak packages the release binary from `target/release` (a `simple` module, `packaging/flatpak`); the
`linux` step builds that binary. Flatpak build state stays in `target/flatpak`.

### Publish

```sh
tools/release.sh X.Y.Z
```

It checks that `package.json` has the version, the tag exists and `CHANGELOG.md` has its entry, then creates the
GitHub Release with every bundle of that version under `target/release/bundle` and
`target/x86_64-pc-windows-msvc/release/bundle` (a second run uploads to the existing release).

## Checks before publishing

The final checks happen at release time, on the bundles that will be published:

- **Flatpak:** `flatpak install --user target/release/bundle/flatpak/potato-pokemon-randomizer_X.Y.Z_x86_64.flatpak`,
  start it from the app menu or with `flatpak run io.github.techneut92.potato-pokemon-randomizer`, open a ROM,
  randomize, save with a log; then `flatpak uninstall --user io.github.techneut92.potato-pokemon-randomizer`.
- **Windows:** run the setup exe on Windows (or under Wine for a rough check) and start the app; WebView2 is
  installed by the installer when missing.
- **AppImage / .deb / .rpm:** start the app once on a matching distribution.

## Flatpak notes

- App ID `io.github.techneut92.potato-pokemon-randomizer`, the Tauri `identifier`, so the app has one ID in the
  Tauri config, the desktop file and the metainfo.
- Runtime `org.gnome.Platform` 51, which has WebKitGTK 4.1. Raise `runtime-version` when GNOME releases move on.
- Permissions: Wayland with X11 fallback, IPC, DRI for WebKit's GPU rendering, no network. ROMs, the batch folder
  and settings files are chosen through the file chooser portal, but after a single run the log is written next to
  the randomized ROM without a dialog, and the recent list reopens ROMs by path. Neither works with only the files
  the portal grants, so the app also gets `--filesystem=home`. Without it a log next to the ROM would fail to save.
  If the log gets its own save dialog, `--filesystem=home` can go.
- Flathub would need a source build instead of a prebuilt binary: the `rust-stable` SDK extension, vendored cargo
  and pnpm dependencies (flatpak-builder-tools), and screenshots in the metainfo. AppStream advises against dashes in
  the app ID, which Flathub review may raise.
