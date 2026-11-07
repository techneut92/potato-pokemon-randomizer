# 0002. The game data model is the seam between ROM I/O and randomization

- Status: Accepted

## Context

If randomizers called getters and setters on a ROM handler directly (get the species, set the starters, …), every
randomizer would be coupled to a ROM handler, its tests would need a fake handler, and the order of reads and writes
would be spread over many modules.

## Decision

A run has three explicit stages:

1. **Load:** `GameHandler::load(&mut RomSource) -> GameData`. It parses everything the randomizers need into
   `ppr-model` types.
2. **Randomize:** each randomizer is a function over `&mut GameData` with `&Settings` and an RNG. Randomizers
   implement one trait (Strategy pattern) and the pipeline runs them in a fixed order (Pipeline pattern):

   ```rust
   pub trait Randomizer {
       fn id(&self) -> StepId;
       fn applies(&self, settings: &Settings, caps: &Capabilities) -> bool;
       fn run(&self, ctx: &mut RunContext<'_>) -> Result<(), RandomizeError>;
   }
   ```

3. **Write:** `GameHandler::write(&GameData, &mut RomSink)` writes the changes back. Tweaks that patch code rather
   than data (instant text, running shoes, …) are applied by the handler as `MiscTweak` values listed in
   `GameData`.

`GameData` is plain, owned, `serde`-serializable data. `Capabilities` (what this ROM supports: mega evolutions,
time-based encounters, …) is part of it and is also what the UI uses to show or hide options.

## Consequences

- Randomizers are unit-tested on an in-memory `GameData` fixture, with no ROM and no handler.
- `GameData` can be dumped to JSON. That is the basis for snapshot tests and for differential tests against a
  reference implementation.
- Some randomizers need ROM-specific details (free space, script offsets). These become fields or capability flags
  on the model, or handler-side post-processing. We decide that per randomizer when implementing it.
- Memory: a 3DS game's data is a few MB once parsed, so keeping it all in memory is fine.
