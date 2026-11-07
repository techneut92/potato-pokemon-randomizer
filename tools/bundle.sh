#!/usr/bin/env bash
# Builds the release bundles of the version in package.json on a Linux machine, for tools/release.sh to publish.
#
#   tools/bundle.sh            everything below
#   tools/bundle.sh linux      AppImage, .deb and .rpm               -> target/release/bundle/{appimage,deb,rpm}
#   tools/bundle.sh flatpak    single-file Flatpak bundle            -> target/release/bundle/flatpak
#                              (uses the binary of the linux step; builds it first when it is missing)
#   tools/bundle.sh windows    NSIS installer, cross-compiled        -> target/x86_64-pc-windows-msvc/release/bundle/nsis
#
# Needs: pnpm, Rust, the WebKitGTK 4.1 development packages; for flatpak: flatpak with org.gnome.Sdk//51 and
# flatpak-builder or org.flatpak.Builder from Flathub; for windows: rustup target x86_64-pc-windows-msvc, cargo-xwin,
# makensis (Fedora: mingw32-nsis), clang, lld and llvm. docs/release.md has the setup.
#
# PPR_RUN is put in front of the commands that compile Rust, for a toolchain in a container, e.g.
#   PPR_RUN="toolbox run -c ppr-build" tools/bundle.sh
# The flatpak step always runs on this machine, since flatpak-builder doesn't run inside a toolbox.
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(node -p 'require("./package.json").version')"
app_id="io.github.techneut92.potato-pokemon-randomizer"
manifest="packaging/flatpak/$app_id.yml"
read -ra run <<<"${PPR_RUN:-}"
step() { printf '\n== %s\n' "$*"; }

grep -q "^version = \"$version\"" Cargo.toml || {
  echo "Cargo.toml [workspace.package] has no version = \"$version\" (package.json); bump both" >&2
  exit 1
}

linux() {
  step "Linux bundles $version"
  "${run[@]}" pnpm tauri build
}

flatpak_bundle() {
  step "Flatpak $version"
  grep -q "<release version=\"$version\"" "packaging/flatpak/$app_id.metainfo.xml" || {
    echo "packaging/flatpak/$app_id.metainfo.xml has no <release version=\"$version\">; add it" >&2
    exit 1
  }
  [[ -x target/release/potato-pokemon-randomizer ]] || "${run[@]}" pnpm tauri build --no-bundle
  local builder=(flatpak-builder)
  command -v flatpak-builder >/dev/null || builder=(flatpak run org.flatpak.Builder)
  local work=target/flatpak out=target/release/bundle/flatpak
  mkdir -p "$work" "$out"
  "${builder[@]}" --force-clean --user --install-deps-from=flathub --disable-rofiles-fuse \
    --state-dir="$work/state" --repo="$work/repo" "$work/build" "$manifest"
  flatpak build-bundle --runtime-repo=https://flathub.org/repo/flathub.flatpakrepo \
    "$work/repo" "$out/potato-pokemon-randomizer_${version}_x86_64.flatpak" "$app_id"
  ls -l "$out/potato-pokemon-randomizer_${version}_x86_64.flatpak"
}

windows() {
  step "Windows installer $version"
  # WiX (.msi) only runs on Windows, so a Linux build makes the NSIS installer only.
  "${run[@]}" pnpm tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc --bundles nsis
}

case "${1:-all}" in
  linux) linux ;;
  flatpak) flatpak_bundle ;;
  windows) windows ;;
  all) linux; flatpak_bundle; windows ;;
  *) echo "usage: tools/bundle.sh [all|linux|flatpak|windows]" >&2; exit 2 ;;
esac
step "bundles of $version"
find target/release/bundle target/x86_64-pc-windows-msvc/release/bundle -maxdepth 2 -type f \
  \( -name "*${version}*.deb" -o -name "*${version}*.rpm" -o -name "*${version}*.AppImage" \
     -o -name "*${version}*.flatpak" -o -name "*${version}*-setup.exe" -o -name "*${version}*.msi" \) \
  -printf '%10s  %p\n' 2>/dev/null || true
