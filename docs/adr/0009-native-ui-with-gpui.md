# 0009. A native GPUI front end next to the web UI, behind the same service layer

- Status: Proposed

## Context

The desktop app is a Svelte UI in a Tauri WebView (ADR 0004). On Linux the WebView is WebKitGTK, and that is where
most platform trouble comes from: NVIDIA's driver under Wayland needs a workaround at start (0.1.1), rendering
depends on the WebKit build of the distribution, and the bundles carry or require the whole engine.

GPUI, the UI framework of the Zed editor, draws natively on the GPU (Vulkan, Metal, DirectX) without a WebView. A
rebuild of the start screen in GPUI looks the same as the web UI and needs no workarounds. Rewriting the whole UI
at once is too large a step, and the two cannot share one window: a GPUI window cannot host the WebView, and the
WebView cannot host GPUI.

We want the native UI to grow screen by screen, without forking the logic, so that switching later is a matter of
finishing screens, not of untangling code.

## Decision

### A second front end, in its own crate

`src-gpui/` is the crate `ppr-gpui` (binary `potato-gpui`). It is a workspace member but not a default member, like
`src-tauri`, because it needs system libraries (on Linux: xkbcommon, Wayland, fontconfig, Vulkan headers). It is not
in the release bundles until it can do everything the web UI does.

### Only `ppr-service` is shared

`ppr-gpui` depends on `ppr-service` and nothing below it, exactly like `src-tauri` and `ppr-cli`. Anything both
desktop front ends need that is not drawing goes into `ppr-service` first, and both call it there. The first such
move is opening the Ko-fi page (`ppr_service::open_support_page`), which used to live in the Tauri commands.

Lower crates (`ppr-model`, `ppr-romio`, …) may appear in `ppr-gpui` only as dev-dependencies, for tests.

### Parity is tested, not remembered

Where the GPUI screen repeats something the web UI defines, a unit test in `ppr-gpui` reads the web source and
fails when the two drift apart:

- the colour tokens of `src/app.css` (dark and light) against `theme::Palette`, and the accent colour;
- the supported-games cards of `src/lib/data/roms.ts` and the file extensions of `Welcome.svelte`;
- every listed game against `ppr_service::supported_games()`, and every console name against the model.

A change to the web UI that breaks parity then fails CI until the GPUI side follows, or the test is changed on
purpose.

### What stays web-only for now

Everything after the start screen. The GPUI app identifies a ROM through the service layer and then says that
randomizing is in the web app for now; the same goes for premade seeds, the names editor and themes. Recent ROMs are
kept in memory only; the web UI keeps them in WebView storage, which the native app cannot read.

### CI

The main CI job excludes `ppr-gpui`; a separate job runs clippy and the tests for it on Linux, macOS and Windows.
`tools/ci.sh gpui` runs the same checks locally. `cargo deny` checks its dependencies like any other crate; the
unmaintained crates that only gpui brings in are ignored by ID, each with a reason.

## Consequences

- Each screen exists twice until the switch. The parity tests keep the shared facts in step; layout and behaviour
  still need a look by hand.
- New logic must be written in `ppr-service` to be usable by both, which is where it belongs anyway (ADR 0001).
- gpui 0.2 is young and its API changes between releases. It pins some dependencies low (libc 0.2.189, for
  `xattr`); that pin applies to the whole workspace lockfile.
- A switch needs, at least: the remaining screens, recent ROMs and settings in a file instead of WebView storage,
  packaging (the bundles are made by Tauri today), and a decision on the web UI (keep it for a web build or retire
  it). That is a later ADR.
