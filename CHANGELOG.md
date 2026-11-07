# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-08

First release.

### Added

- Games and formats:
  - Gen 1 to 7: Red, Blue, Green and Yellow; Gold, Silver and Crystal; Ruby, Sapphire, Emerald, FireRed and
    LeafGreen; Diamond, Pearl, Platinum, HeartGold and SoulSilver; Black, White, Black 2 and White 2; X, Y,
    Omega Ruby and Alpha Sapphire; Sun, Moon, Ultra Sun and Ultra Moon, with their translations.
  - Game Boy, Game Boy Color, Game Boy Advance and Nintendo DS ROM files, and FireRed 1.0 ROM hacks.
  - Decrypted 3DS games (CCI, CXI or CIA), saved as a CXI or as a LayeredFS folder.
  - Decrypted game updates (`.cia`) for Gen 6 and 7, laid over the game and remembered per game.
  - Only what changed is written, so a game saved without changes is byte for byte the input.
- Randomizers:
  - Pokémon traits: base stats, base stat totals, types, abilities, EXP curves, evolutions and evolution
    improvements, including alternate formes and Mega Evolutions where the game has them.
  - Starters: custom or random, with type rules (including type triangles) and held items; starter texts follow.
  - Static Pokémon (legendary for legendary and Ultra Beast for Ultra Beast, random or similar strength, with roamers
    and level modifier) and in-game trades (Pokémon, nicknames, original trainers, IVs and held items).
  - Moves: power, accuracy, PP, types, categories, names and updates to a later generation; movesets, egg moves
    and Metronome Only Mode.
  - TMs, HMs and move tutors: their moves and each species' compatibility.
  - Trainers: Pokémon, type themes, levels, evolutions, held items, better movesets, battle styles, names and class
    names, Mega Evolutions and shiny Pokémon (Gen 7).
  - Wild Pokémon: by slot, set, location or the whole game, with themes, similar strength, Catch 'Em All,
    time-based encounters where the game has them, held items, levels and minimum catch rates.
  - Totem Pokémon in Gen 7, with their allies, auras, held items and levels.
  - Items: field items, shops and prices, Pickup items.
  - Type chart: random, balanced, keeping type identities, inverse, or updated to Gen 6.
  - Graphics: Pokémon palettes (Gen 1 to 5), custom player graphics from built-in packs (Gen 1 to 3) and the
    intro Pokémon.
  - Misc tweaks, offered where the game supports them, such as fastest text, running shoes indoors and reusable TMs.
  - Limit Pokémon to chosen generations, custom names for trainers and nicknames, and Race Mode with a check value.
- Settings: settings strings, `.rnqs` settings files and `.rndp` premade seed files of UPR FVX 1.6.2 load as they
  are. The app saves them too. A seed reproduces the same game in this app.
- Batch randomization: one ROM many times, each with its own seed, with optional logs; it can be cancelled and keeps
  the ROMs it wrote.
- Manual editor: change starters, Pokémon, trainers, wild Pokémon, static encounters, trades, shops, item prices and
  field items by hand, applied after the randomizers. Edits are checked against what the game can store, and are
  exported and imported as `.ppredits.json` files.
- Log: everything that changed, section by section, saved next to the randomized game (not in Race Mode).
- Command-line tool `ppr`: `identify`, `games`, `randomize`, `batch`, `player-graphics` and `edit-view`, with the
  same options, settings strings, premade seeds, game updates and edit files as the app, and JSON output.
- Desktop app:
  - Open a ROM with the file dialog, by dropping it on the window or from the recent list; options in eleven sections
    with search, help on hover and only the options the game supports.
  - Live progress with cancelling, and a result dialog with the seed and the settings string.
  - Dark and light themes with accent colors, and a custom title bar.
  - A support button in the header and the Tools menu that opens the Ko-fi page.
  - Works at any window size down to 480×480, with a drawer for the categories on narrow windows.
- Packaging: AppImage, `.deb`, `.rpm` and a Flatpak for Linux, an installer and an `.msi` for Windows, and a
  universal `.dmg` for macOS. The bundles are not signed yet.

Where this randomizer deliberately behaves differently from the original is listed in
[docs/differences.md](https://github.com/techneut92/potato-pokemon-randomizer/blob/v0.1.0/docs/differences.md).
