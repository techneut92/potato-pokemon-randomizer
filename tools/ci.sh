#!/usr/bin/env bash
# Runs every check of .github/workflows/ci.yml locally, plus cargo-deny and, with PPR_TEST_ROMS set,
# the tests on real ROMs. GitHub Actions only runs on demand, so this is the check before a merge.
#
#   tools/ci.sh          everything
#   tools/ci.sh web      the frontend checks (pnpm)
#   tools/ci.sh rust     the Rust checks (cargo, cargo-deny)
#   tools/ci.sh gpui     the GPUI front end (needs its system libraries, see src-gpui/README.md)
set -euo pipefail
cd "$(dirname "$0")/.."

part="${1:-all}"
step() { printf '\n== %s\n' "$*"; }

if [[ $part == all || $part == web ]]; then
  step "pnpm install"; pnpm install --frozen-lockfile --silent
  step "pnpm check"; pnpm -s check
  step "pnpm test"; pnpm -s test
  step "pnpm build"; pnpm -s build
fi

if [[ $part == all || $part == rust ]]; then
  export RUSTFLAGS="-D warnings" CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
  step "cargo fmt"; cargo fmt --all --check && cargo fmt --manifest-path fuzz/Cargo.toml --check
  step "cargo clippy"; cargo clippy -q --workspace --exclude ppr-gpui --all-targets --locked
  step "cargo test"; cargo test -q --workspace --exclude ppr-gpui --locked
  step "generated TypeScript bindings"
  if [[ -n $(git status --porcelain -- src/lib/generated) ]]; then
    git status --short -- src/lib/generated
    echo "Run cargo test and commit src/lib/generated" >&2
    exit 1
  fi
  step "no entropy source in ppr-random (ADR 0005)"
  if cargo tree -p ppr-random -e normal --prefix none | grep -E '^(rand|getrandom) '; then exit 1; fi
  if command -v cargo-deny >/dev/null; then
    step "cargo deny"; cargo deny check -s 2>&1 | tail -4
  else
    echo "cargo-deny is not installed (cargo install --locked cargo-deny); skipping" >&2
  fi
  if [[ -n ${PPR_TEST_ROMS:-} ]]; then
    step "tests on the ROMs in $PPR_TEST_ROMS"; cargo test -q --workspace --exclude ppr-gpui --locked -- --include-ignored
  fi
fi
if [[ $part == all || $part == gpui ]]; then
  export RUSTFLAGS="-D warnings ${PPR_GPUI_RUSTFLAGS:-}" CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}"
  step "cargo clippy (GPUI)"; cargo clippy -q -p ppr-gpui --all-targets --locked
  step "cargo test (GPUI)"; cargo test -q -p ppr-gpui --locked
fi
step "all checks passed"
