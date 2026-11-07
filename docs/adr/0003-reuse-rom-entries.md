# 0003. Reuse existing ROM entry files as embedded data

- Status: Accepted

## Context

An existing set of GPL-3.0 data files describes every supported ROM (about 180 entries over seven generations) in
`genN_offsets.ini`: the game code, version and type, offsets, files, static encounters and tweak patches. The files share one syntax:
`[Entry]` sections, `Key=Value`, `//` comments, `[a, b]` integer arrays, repeated `Key{}` list entries,
parameterized `Key<x>` keys and `CopyFrom=` inheritance.

## Decision

- Keep the files unchanged under `crates/ppr-romio/data/romentries/` and embed them with `include_str!`, so
  updates are a plain diff.
- `ppr-romio` has one generic reader (`IniDocument`) that keeps every key in order with its raw value, and typed
  accessors for ints (`0x` and `&h` hex), arrays and strings. Each generation's handler builds its typed
  `RomEntry` from that, including the special keys it needs.
- `CopyFrom` copies every generic value of the named entry, overwriting what is there, but never the identity
  fields `Game`, `Version` and `Type`. Generations 1–4 match on entry name, generation 5 on game code.
- Detection checks the headers the entries describe: the GB title at 0x134 and code at 0x13F, the GBA code at
  0xAC, the NDS code at 0x0C and the 3DS product code and title ID in the NCCH header. Only headers are read, so a
  multi-GB 3DS image is never loaded just to identify it.

## Consequences

- Fixes and new ROM revisions carry over with little effort.
- The data files are GPL-3.0 (see ADR 0007).
- Malformed values do not abort parsing: they read as 0 and log a `tracing` warning. A test parses every embedded file and fails on any warning, so typos in the data are caught in CI.
