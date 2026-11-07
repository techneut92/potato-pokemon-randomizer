# 0001. Cargo workspace with layered crates

- Status: Accepted

## Context

The randomizer is written in Rust behind a Svelte UI, and it must build on Linux, macOS and Windows. Its work
splits into binary formats (compressors, containers), game data with per-generation ROM handlers, and the
randomizers with their settings, CLI and GUI. The core rule is that randomizers depend on game data and never on
file formats.

We want crates that:

- compile and test independently, with no GUI dependency;
- keep binary-format parsing (untrusted input, fuzzable) away from game logic;
- serve a desktop app and a headless CLI from the same code.

## Decision

One Cargo workspace. Dependencies only point downwards:

```text
            ppr-desktop (src-tauri)     ppr-cli
                          \             /
                           ppr-service            application facade: open, randomize, batch
                          /     |      \
               ppr-random  ppr-settings  ppr-romio   randomizers | settings + codec | ROM handlers
                     \          |        /     \
                       ppr-model            ppr-formats   game data model | containers, compression
```

| Crate | Responsibility |
| --- | --- |
| `ppr-formats` | Binary containers and codecs with no Pokémon knowledge: GB/GBA/NDS/3DS headers, NitroFS, NARC, NCCH/RomFS/GARC, LZ77 variants, text tables |
| `ppr-model` | Plain data: species, moves, types, items, trainers, encounters, evolutions, ROM identity and capabilities |
| `ppr-romio` | `GameHandler` trait, ROM detection, one handler per generation behind cargo features, ROM entry data |
| `ppr-settings` | `Settings`, settings-string codec, `.rnqs` presets, settings updater |
| `ppr-random` | RNG, one randomizer per domain, the run pipeline, the randomization log |
| `ppr-service` | Use cases used by every front end: open ROM, randomize with progress and cancellation, batch |
| `ppr-cli` | Headless binary, used by tests and power users |
| `ppr-desktop` | Tauri 2 app: thin command layer over `ppr-service` |

Rules:

- `ppr-random` does not depend on `ppr-romio` or `ppr-formats`. It only sees `ppr-model` (see ADR 0002).
- Only `ppr-desktop` depends on Tauri. Only binaries depend on `anyhow` or `tracing-subscriber`.
- Versions, edition, lints and metadata live once in the root `Cargo.toml` (`[workspace.package]`,
  `[workspace.dependencies]`, `[workspace.lints]`).
- The Tauri crate lives in `src-tauri/`, the Tauri convention. It is a workspace member but not a default member,
  so `cargo test` on a machine without the WebView system libraries still covers everything else.

## Consequences

- More crates means more `Cargo.toml` files, but builds are incremental per crate and the boundaries are enforced
  by the compiler instead of by convention.
- Handlers per generation are cargo features of `ppr-romio`, so a build or test run can be limited to one
  generation while that generation is being implemented.
