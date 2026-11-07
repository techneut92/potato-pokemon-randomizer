# Where the randomizer differs from the original

This randomizer ports an existing one and mostly behaves the same way. Where it deliberately does something else,
it is listed here, by area. Most differences make a run end where the original would crash, fail or loop, or keep
data intact that the original would change or damage.

## General

- Random choices come from a separate ChaCha8 stream per randomization step. The same seed and settings make the
  same game every time in this app, but not the game the original makes from them, premade seed files included.
- Only what changed is written: a game saved without changes is byte for byte the input, and unchanged data stays
  where it was.
- Support patches (roamer, music, fleeing, Unown, Shedinja and Distortion World fixes) are applied only when the
  data they serve changes, not on every save.
- Where the original redraws a value until it is allowed, a value is drawn once from the allowed ones. The
  distribution is the same, and the draw always ends.
- Species are walked in national order, so a result never depends on how a set was built.
- Where the original crashes or fails with an internal error, the run stops with a message before anything is
  written, or settles for a sensible result (listed below).

## Settings strings and settings files

- Options of features that are not ported are written with their defaults and ignored when read. Custom player
  graphics and custom names are not part of settings strings.
- A level modifier, update generation or percentage whose checkbox is off is not kept, and neither are custom
  starters outside Custom mode.
- Strings of older versions are refused; they need to be updated to version 428 first.
- The generations of Limit Pokémon apply only while it is ticked, not after it is unticked again.
- The Race Mode check value reads the wild encounters by time of day only when time of day is on. The app keeps it
  on whenever wild Pokémon are not randomized, so this only differs for settings strings or command lines that turn
  it off.

## Pokémon traits

- Base stats: copying stats up to Shedinja copies all but HP, and a random spread whose total is below the
  minimums gives the stats nothing beyond them, instead of failing.
- Base stats in Gen 1: an evolution with a lower total than its pre-evolution keeps each stat at its minimum, as in
  the other generations, instead of failing.
- Base stat totals: shuffled totals are the ones after "Update Base Stats to Generation", a buff or nerf of 0%
  changes nothing, and shuffling that follows evolutions keeps a species' own total when its line has no match.
- "Update Base Stats to Generation" finds the formes it updates (Mega Alakazam, Aegislash's Blade Forme) by their
  base species rather than by fixed species numbers.
- Following evolutions, an evolution copies from the first pre-evolution in the set, never from one outside it.
- Without evolution sanity, an alternate forme always copies from its base species; the original copies Wormadam's
  formes from Burmy even then.
- Species with cosmetic formes but no data for them (Unown, Burmy, Shellos, Gastrodon, Vivillon) get forme numbers
  up to their number of formes less one; the original gives them one more, a forme the game doesn't have.
- FireRed and LeafGreen are patched to evolve species outside the Kanto Pokédex whenever evolutions change, not
  only when impossible evolutions are changed.
- Change Impossible Evolutions in Gen 2 follows the Gen 3 rules (trade evolutions by level or stone). In Gen 6 it
  follows the Gen 5 rules, and Make Evolutions Easier also turns Inkay's upside-down evolution into one at its level.

## Starters

- With custom starters and the Unique rule, only the slots without a custom species are picked.
- With the type triangle rule, a triangle without valid starters is not drawn again, so the draw always ends.
- FireRed and LeafGreen starter texts keep names in one piece: "MR. MIME" keeps its space.
- Custom starters are always base species, since the settings name them by national number; only random starters can
  be alternate formes.

## Static Pokémon

- Emerald's roaming Latios and Latias become whatever replaced them on Southern Island.
- A Ghost Marowak replacement that can be female is made female again, so re-randomizing a game never leaves a
  female-only species male.
- The Gen 3 music fix never searches past the end of its table, which matters for a game randomized before.
- "Limit Main-Game Legendaries" falls back to the whole pool when no species is weak enough, instead of failing.
- A static encounter that keeps its species and forme keeps its moves; the original resets them whenever it
  randomizes.

## Moves and movesets

- A very inaccurate move's 10% chance of 50% more accuracy rounds down to a multiple of 5, so accuracies stay
  multiples of 5.
- Random move names are chosen after the other move data, so they fit a changed category. The fallback name fits
  the game's name length.
- Future Sight and Doom Desire (and Spit Up in Gen 3) don't count as flinching moves.
- "Evolution Moves for All Pokémon" only adds evolution moves in Gen 7, the games that have them.
- Egg move lists keep their length where the game needs it (Gen 3, 6 and 7); in Gen 4 a list that grows too long is
  refused.

## Trainers

- Random group types go to the groups in a random order, use only types with species, and "Force diverse types"
  repeats a type when none is left, instead of failing.
- "Random (even distribution, main-game)" is the same as "Random" in Gen 3, which has no main-game trainer list.
- With "Swap Mega Evolvables", a Pokémon whose replacement has no Mega Evolution (only possible with a type theme
  without species that can Mega Evolve) holds nothing, where the original fails the run.
- A trainer or class name is picked from the names that fit; when none fits, the old name stays, instead of
  retrying a fixed number of times or forever.
- A chosen battle style the game lacks (triple and rotation battles outside Gen 5 and 6) becomes double battles,
  and Gen 1 and 2 trainers keep their single battles.
- Better trainer movesets: a Pokémon with no moves to choose from keeps its moves, and a last pick with nothing
  left gives three moves, instead of failing.
- Better trainer movesets in Gen 3: Air Lock counts as Air Lock (not Tangled Feet), and Lock-On and Mind Reader only
  pair with moves of accuracy 1–50, not with moves that never miss.
- Emerald's Mossdeep Steven: Pokémon whose moves are reset get the moves of their new species.

## Wild Pokémon

- With Only Basic Pokémon and a type rule, the per-type pools hold only basic species.

## Totem Pokémon

- A totem or ally that keeps its species and forme keeps its moves, also when the randomizer happened to choose the
  same species; the original resets its moves.
- "Randomize Held Items" stops the run with a message when the game has no consumable held items, instead of
  crashing.
- An aura byte past 18, which no game has, reads as no aura instead of failing.

## Items, shops and prices

- A price that doesn't fit is rounded down to the game's step and capped at its highest price. The original keeps
  only the low bits or digits, so 70,000 would cost 4,464 in Gen 2.
- Gen 1 TM prices go up to 9,000, as the game reads their digit as thousands; the original writes any number below
  16 there.
- Gen 6 and 7 shops keep their size, so Add Cheap Rare Candies only makes them cheap there.
- When the shops can't take the items (more guaranteed items than slots, or nothing to choose from), the run stops
  with a message instead of an internal error.
- Random field items in Gen 6 and 7: when nothing but Mega Stones placed already is left, the run stops with an
  error, where the original crashes.
- The Luminous Moss counts among the items worth holding in Gen 6.

## Graphics and palettes

- Custom player graphics packs are built in, not read from a folder at run time.
- An image in a pack with the wrong size or too many colors leaves out only that image, not the whole pack.
- Games with one player character (Gen 1, Gold and Silver) give a choice of the second character the first's
  graphics.
- A Gen 2 pack without a walk sprite or sprite palette gets the red sprite palette.
- Random palette colors are drawn against their exact weight; when no color is found in the number of tries, the
  last one is used instead of failing. Species past the end of the palette description file keep their colors.
- In Gen 3, Unown's letters follow its palette only where they shared it.

## Misc tweaks

- "Run Without Running Shoes" patches only when the code it looks for is found exactly once; otherwise it writes
  nothing, instead of patching at a fallback offset.
- The randomized catching tutorial in FireRed and LeafGreen only uses species whose internal number fits its single
  byte, so Treecko to Torchic can't be written as other species.
- Each side of the catching tutorial is drawn once from the species it can take.
- The Gen 2 reusable TMs patch jumps to the right place (a relative jump, as the ROM entry's jump length means).
- Forgettable HMs in Gen 4 are only offered when the ROM entry has the code offsets the patch needs.
- Fast Egg Hatching and No EV Yields in Gen 5 and 6 change the alternate formes too.

## Log and batches

- The log compares the game before and after the run, so a section appears when its data changed, not because its
  randomizer ran. Evolution improvements are listed only when evolutions were not randomized as well.
- The log's head names this app, its version and its issue page, and gives the settings as JSON rather than a settings string. The statistics give the time instead of RNG call
  counts, trainers have no ROM offsets, and wild areas are in the game's order.
- The log names a loaded game update by its file name and the version its title screen shows, instead of
  checksums.
- A batch can be cancelled and keeps the ROMs it wrote. Each ROM keeps the input's extension, and a log replaces it
  with the log ending (`random3.log`).

## Game data, per generation

- Gen 1: TM prices are read and written in TM order, damaging moves' stat drops have a 33% chance, a fifth
  level-1 move is kept, starter and TM texts are only written when they fit (a starter text may drop its "So! "),
  and abbreviated class names are written at their own offsets.
- Gen 2: accuracies and percentages are written as the byte that reads back the same, texts only when they fit,
  and class names only when they take as many bytes as before.
- Gen 2: shared egg move lists stay in place and a changed one moves to free space, and texts the ROM entries store
  double-encoded are repaired before use.
- Gen 2 and 4: the Odd Egg and the Mystery Egg are eggs at level 1, like every egg.
- Gen 3: only changed trainers are written, so unchanged teams stay where they are; trade names are read only
  within their fields.
- Gen 4: texts are rewritten only for what changed, by the names as loaded, so other texts keep their line breaks.
- Gen 4: the baby table is updated only for species whose first stage changed, so Manaphy's eggs still hatch into
  Phione.
- Gen 4: partly used tutor compatibility bytes keep their unused bits.
- Gen 4: in HeartGold and SoulSilver, every Pokémon of a changed trainer gets the ability slot of the last, as the
  game makes them share it.
- Gen 5: a Black or White shop that grows moves into the extended ARM9, as the overlay has no room.
- Gen 5: the Driftveil tournament teams keep their sizes.
- Gen 6: names and trainer classes are written to every language's text when one changed.
- Gen 6: the title names the seed only when the game changed, and an update of another version than the ROM entry
  expects loads with a warning instead of being refused.
- Gen 7: trainer Pokémon added by the randomizer copy the nature, EVs, IVs and gender of the team's first Pokémon.
- Gen 7: in-game trades keep their full 32-bit original trainer ID.
- DS games: only replaced overlays are compressed again, and the banner keeps its full size, so the icon data of
  later banner versions survives.
