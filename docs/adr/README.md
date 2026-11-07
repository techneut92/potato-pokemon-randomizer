# Architecture decision records

Each file records one decision: the context, what we chose, and what it costs. Records are numbered and never
rewritten; a later record supersedes an earlier one instead.

| # | Decision | Status |
| --- | --- | --- |
| [0001](0001-cargo-workspace-and-crates.md) | Cargo workspace with layered crates | Accepted |
| [0002](0002-game-data-model-as-the-seam.md) | The game data model is the seam between ROM I/O and randomization | Accepted |
| [0003](0003-reuse-rom-entries.md) | Reuse existing ROM entry files as embedded data | Accepted |
| [0004](0004-frontend-contract.md) | Frontend contract: Tauri commands, events and generated types | Accepted |
| [0005](0005-rng-and-seeds.md) | Deterministic RNG with one stream per randomization step | Accepted |
| [0006](0006-errors-progress-cancellation.md) | Errors, logging, progress and cancellation | Accepted |
| [0007](0007-licensing.md) | GPL-3.0-or-later, and no ROMs in the repository | Proposed |
| [0008](0008-manual-edits.md) | Manual edits: a versioned edit list applied after the randomizers | Accepted |
