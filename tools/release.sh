#!/usr/bin/env bash
# Publishes a GitHub Release from bundles built on this machine, with the version's CHANGELOG entry as notes.
#
#   tools/bundle.sh             # on Linux: Linux bundles, the Flatpak and the Windows installer (or pnpm tauri build)
#   tools/release.sh 0.2.0      # tag v0.2.0 must exist, CHANGELOG.md must have "## [0.2.0] - YYYY-MM-DD"
#
# It takes the bundles of that version from target/release/bundle (this OS, including the Flatpak) and
# target/x86_64-pc-windows-msvc/release/bundle (the Windows cross-build). A second run, e.g. on macOS, uploads to
# the existing release. docs/release.md has the whole procedure.
set -euo pipefail
cd "$(dirname "$0")/.."

version="${1:?usage: tools/release.sh X.Y.Z}"
tag="v$version"
[[ $(node -p 'require("./package.json").version') == "$version" ]] || {
  echo "package.json has version $(node -p 'require("./package.json").version'), not $version" >&2
  exit 1
}
git rev-parse -q --verify "refs/tags/$tag" >/dev/null || { echo "no tag $tag" >&2; exit 1; }

notes="$(mktemp)"
trap 'rm -f "$notes"' EXIT
awk -v v="$version" '
  index($0, "## [" v "]") == 1 { on = 1; next }
  on && /^## \[/ { exit }
  on { print }
' CHANGELOG.md >"$notes"
[[ -s $notes ]] || { echo "CHANGELOG.md has no entry for [$version]; rename [Unreleased] first" >&2; exit 1; }

dirs=()
for dir in target/release/bundle target/*/release/bundle; do [[ -d $dir ]] && dirs+=("$dir"); done
mapfile -t bundles < <( ((${#dirs[@]})) && find "${dirs[@]}" -maxdepth 2 -type f \
  \( -name "*_${version}_*.deb" -o -name "*-${version}-*.rpm" -o -name "*_${version}_*.AppImage" \
     -o -name "*_${version}_*.dmg" -o -name "*_${version}_*.msi" -o -name "*_${version}_*-setup.exe" \
     -o -name "*_${version}_*.flatpak" \) | sort)
(( ${#bundles[@]} )) || { echo "no bundles for $version under target/; run tools/bundle.sh" >&2; exit 1; }
printf 'Bundles:\n'; printf '  %s\n' "${bundles[@]}"

if gh release view "$tag" >/dev/null 2>&1; then
  gh release upload "$tag" "${bundles[@]}" --clobber
else
  gh release create "$tag" "${bundles[@]}" --verify-tag --title "$version" --notes-file "$notes"
fi
