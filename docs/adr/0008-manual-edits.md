# 0008. Manual edits: a versioned edit list applied after the randomizers

- Status: Accepted

## Context

Next to the randomizer options, the app gets a "Manual edits" tab. In it the user changes objects of the loaded
game by hand: the starters; a species' types, base stats and abilities; a trainer's team with levels, held items
and moves; the species and levels of a wild set; static encounters; in-game trades (species, nickname, original
trainer, IVs, held item); what shops sell; item prices; and field items. The editor marks edited objects, can
revert one or all of them, lists every change, and blocks saving while an edit breaks a rule of the game. Edits
can be saved to a file and loaded again.

This needs a representation of "what the user changed" that the frontend can hold and show, Rust can check and
apply, and a file can carry. It must survive a reload of the same ROM, and must not be applied to another game.

## Decision

### An edit list is data

Manual edits are an `EditList` in `ppr-model` (`edits` module): plain, owned, `serde` data, like `GameData`
(ADR 0002).

```rust
pub struct EditList {
    pub version: u32,     // EDIT_LIST_VERSION
    pub game: EditGame,   // which game it is for
    pub edits: Vec<Edit>, // at most one per object
}
```

`Edit` is an enum with one variant per object kind: `Starter`, `StarterHeldItem`, `Species`, `Trainer`,
`WildSet`, `Static`, `Trade`, `Shop`, `ItemPrice` and `FieldItem`. In JSON it is tagged by `kind`
(`{ "kind": "trainer", "index": 7, "pokemon": [...] }`).

### Keyed by stable identity, not by display position

Each edit names its object by an identity the game itself gives, never by where the UI lists it. `Edit::target()`
returns that identity as an `ObjectRef`, which is also how errors and (later) validation messages point at the
object.

| Object | Identity | Stability |
| --- | --- | --- |
| Starter, starter held item | place in the game's order | fixed by the game |
| Species | national number | fixed |
| Trainer | the game's trainer number (`Trainer::index`) | fixed by the game |
| Item price | standard item number | fixed: standard numbers, the same in every generation |
| Wild set | place in the handler's list of sets (`GameData::encounters`) | fixed for a handler; see below |
| Static encounter, shop | place in the handler's fixed list | fixed for a handler |
| Trade, field item | place in the game's order | fixed by the game |

Wild sets and field items have no number of their own in every generation. Their place in the handler's list is
the least fragile identity there is: a handler lists them in ROM order, deduplicated the same way on every load,
so the same ROM gives the same places. It only moves when a handler changes how it enumerates them. That is a
handler change we would make knowingly, and it must raise `EDIT_LIST_VERSION` or migrate the indices. Display names
(`EncounterSet::name`) are not used as identity, as several sets can share one and they are only text.

### Per-object edits store new values, not operations

An edit holds the new value of every editable field of its object, not a sequence of operations or only the
fields that changed. A trainer edit holds the whole team; a wild set edit holds every slot.

- **Changes list:** one entry per edit, its "before" being the loaded game's value of the same object.
- **Revert:** remove the edit (`EditList::revert`). There is no inverse operation to compute.
- **Editing again:** replace the edit of the same object (`EditList::set`), so a list never holds two edits of
  one object. `apply` rejects a list that does.
- **Import and export:** the list is self-contained and order-independent between objects.

The edit types are a format of their own (`TrainerEdit`, `StatsEdit`, `WildSlotEdit`, …), with camelCase fields
like the settings, rather than the model types they change. Internal model fields (`TrainerPokemon::added`,
`EncounterSet::rate`) stay out of saved files, and a change to the model doesn't change the file format. Fields an
edit doesn't cover, such as a trainer Pokémon's IVs, are kept from the object as it is when the edit is applied.

### The format is versioned and bound to one game

`EditList::version` is the format version, `EDIT_LIST_VERSION` (1). A list in another version is rejected with
`EditError::UnsupportedVersion`. A future version can add a migration step instead.

`EditList::game` is an `EditGame { name, code, version }` taken from `RomIdentity`. All three must match the loaded
game, or `EditList::check` returns `EditError::WrongGame`, with both games' names for the message. The ROM
entry name is unique among the supported games (ADR 0003), while the header code and version byte alone are not for
every platform: Game Boy games are told apart by checksum as well. So a list for Emerald is rejected by Ruby, and a
list for FireRed 1.0 by FireRed 1.1, whose data can sit elsewhere.

### Edits files

The frontend saves and loads an edit list as an edits file: the `EditList` as pretty-printed UTF-8 JSON, in
the camelCase of the edit types, with a trailing newline. The extension is `.ppredits.json`: the `.json` ending lets
any editor or JSON viewer open it and lets file dialogs filter on it, and `ppredits` tells it apart from other JSON.
Saving adds the extension when the name doesn't end in `.json`. Loading accepts any JSON file name.

```json
{
  "version": 1,
  "game": { "name": "Emerald (U)", "code": "BPEE", "version": 0 },
  "edits": [{ "kind": "starter", "slot": 1, "species": 258 }]
}
```

Loading (`ppr_service::load_edits_file`) checks the game the editor shows (`EditList::check_game`, the same check
as `check`) before the edits replace the current ones, so nothing changes when a file is refused. It reads the
`version` field on its own first: a file from a newer build is reported as newer (`edits_too_new`) even when its
edits don't parse here. Then `edits_unsupported_version` for an older format, `edits_wrong_game` with a message
naming both games, and `edits_invalid` for anything that isn't an edit list. A list that loads is validated like
any other edit list, so edits a person changed by hand that break a rule show as issues and block saving.

### Applied after the randomizers

The run is load, randomize, **apply edits**, write (extending ADR 0002's stages). `EditList::apply(&mut GameData)`
runs after the last randomizer, so an edit always wins over a random result. With every randomizer option off, the
written ROM differs from the original only by the edits. The pipeline step and its "Applying N manual edits" progress
belong to the service.

Applying is all or nothing. `apply` works on a copy, collects every error of every edit, and only replaces the
game's data when there is none. A run never writes half the user's edits.

### What `apply` checks, and what validation checks

`apply` returns `Vec<EditError>` for edits that cannot fit any game of this kind:

- an unknown object: a trainer number, set index or item price the game doesn't have;
- unknown values: a species, item, move, ability or type the game doesn't have;
- a field the game doesn't have: abilities before Gen 3, held items in Gen 1, a Gen 1 trade's original trainer and
  IVs, an egg's level;
- a list of another length than the game fixes: a wild set's slots, a species' ability slots, a trade's IVs;
- ranges that hold whatever the handler: levels 1–100, teams of 1–6, base stats 1–255, DVs 0–15 in Gen 2 and IVs
  0–31 later, the lowest level not above the highest, at least one item in a shop, no empty nickname.

Limits that differ by handler are rules the handler reports, and validation checks an edit list against
them.

### Rules the handler reports, and validation

The rules are an `EditRules` value in `ppr-model`, not more flags on `Capabilities`: capabilities stay one flag per
feature, while rules carry numbers and lists. Each handler computes them from the game as loaded
(`GameHandler::edit_rules`), starting from `EditRules::from_game`, which reads what the game data already says
(which trainer kinds can grow, hold items or have moves of their own; trade text lengths; the TMs only found in the
field) and setting what only the handler knows:

| | Shops | Wild levels | Trainers |
| --- | --- | --- | --- |
| Gen 1 | grow into free space in their bank, at most 255 items; only shrink in Japanese Yellow | one per slot | regular trainers can't grow; no held items or own moves; Yellow's rival starter link |
| Gen 2 | as Gen 1; only shrink in the Japanese games | one per slot, a range in the Bug Catching Contest | regular trainers can't grow or get own moves; Crystal's can't get held items |
| Gen 3 | grow into free space | a range per slot | Emerald's Mossdeep Steven: exactly 3, no held items |
| Gen 4 | share the room they had plus the ARM9 extension (200 bytes): 4 bytes an item in the progressive shop, 2 an item and 2 for the end in the others | grass one per slot; slots that only store a species, and honey trees, fixed; water, headbutt and the contest a range | any |
| Gen 5 | Black and White: changed shops share the 600-byte ARM9 extension, 2 bytes an item; Black 2 and White 2: at most 255 | a range per slot | the Driftveil tournament keeps its team sizes and levels |
| Gen 6 | fixed sizes | a range; X and Y's falling and rustling spots one per slot | any |
| Gen 7 | fixed sizes | one range per table | any |

Every trainer can have at most 6 Pokémon, 3 if it always battles together with another, and at least as many as its
battle style needs. Prices follow the generation's format.

`EditList::validate(&GameData, &EditRules)` runs the checks of `apply` and the rules on the loaded game and returns
every `EditIssue { target, field, severity, message }`. A list in another format or for another game is an
`EditError` instead, as nothing in it is checked. The message names the object as the editor does ("Shop 3
(Cherrygrove City): 12 items, but this game's shops hold at most 10"); `target` is its `ObjectRef`, `None` only for
a limit all shops share. `EditIssue::from_error` turns an `apply` error into the same type, so the frontend has one
list. An `Error` blocks saving; a `Warning` doesn't:

- Errors: a rule broken, as above; a trainer slot that follows a starter set to another species; a banned wild
  species; a trade nickname or original trainer name longer than the game holds; a trade that asked for a species
  set to ask for none.
- Warnings: a field item the game needs (a TM only found there) replaced; a team made bigger than 3 for a trainer
  that may battle together with another (a team the game itself has bigger is no warning).

Some choices:

- Trainer limits are listed per trainer, so the frontend needs no rule to tell bosses from regular trainers. A team
  that already has held items or moves keeps them where its kind couldn't get them.
- Where the original only lets its randomizers grow some trainer teams for want of room, manual edits follow it,
  even though a single grown team may fit.
- Byte budgets are approximate: they count bytes, while the handler places data first-fit. A write that still doesn't
  fit fails with an error, never silently.
- Gen 4 honey trees share one level range across all trees, so their levels stay as they are.
- A trainer's place for the rival's starter (`force_starter_position`) is reported for information only: edits are
  applied after the randomizers, so nothing moves the starter afterwards.

### How it reaches the frontend

The edit types, `EditGame` and `ObjectRef` derive `ts_rs::TS` and are exported to `src/lib/generated/` like the
other boundary types (ADR 0004): `Edit` becomes a union tagged by `kind`. The frontend holds the edit list of the
loaded ROM, sends it to Rust to validate and to run with, and saves and loads it as JSON. `EditRules`,
`EditIssue` and `EditSeverity` are exported too, and the service's editable view carries the rules. `EditError`
stays a Rust error (ADR 0006); for the frontend it becomes an `EditIssue`.

## Consequences

- The frontend never needs to know how a field is stored in the ROM; it edits values and Rust decides.
- Manual edits are unit-tested like randomizers, on an in-memory `GameData`, with no ROM.
- Applying a list costs one clone of `GameData`, a few MB at most.
- An edit list for one ROM revision doesn't load on another revision of the same game, even where their data
  is the same. That is the safe default; relaxing it per family would need a reason.
- Reordering wild sets, statics, shops or field items in a handler breaks saved edit lists for that game, so it
  needs a format version bump or a migration.
- Objects added later (moves, TMs, evolutions) are new `Edit` variants; adding a variant doesn't break existing
  files, removing or renaming one does.
