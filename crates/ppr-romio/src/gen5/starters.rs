//! The starters (in scripts, with their sprites, cries and the texts that name them) and the Pokémon of the
//! introduction.

use ppr_model::{DexNumber, Species};

use super::entry::{Family, file_path, need, starter_offsets, story_text};
use super::store::{Store, Text, find, find_after, put_long, put_relative_pointer, put_word, word};
use crate::RomError;
use crate::entries::RomEntry;

const BW1_STARTER_SCRIPT: [u8; 16] = [
    0x24, 0x00, 0xA7, 0x02, 0xE7, 0x00, 0x00, 0x00, 0xDE, 0x00, 0x00, 0x00, 0xF8, 0x01, 0x05, 0x00,
];
const BW1_STARTER_SCRIPT_MAGIC: &str = "2400A702";
const BW2_STARTER_SCRIPT: [u8; 14] = [
    0x28, 0x00, 0xA1, 0x40, 0x04, 0x00, 0xDE, 0x00, 0x00, 0x00, 0xFD, 0x01, 0x05, 0x00,
];
const BW2_STARTER_SCRIPT_MAGIC: &str = "2800A1400400";

const BW1_STARTER_TEXT: usize = 18;
const BW1_CHEREN_TEXT_1: usize = 26;
const BW1_CHEREN_TEXT_2: usize = 53;
const BW2_STARTER_TEXT: usize = 37;
const BW2_RIVAL_TEXT: usize = 60;

const INTRO_GRAPHIC_PREFIX: &str = "5A0000010000001700000001000000";
const BW1_INTRO_CRY_PREFIX: &str = "0021009101910291";
const BW2_INTRO_CRY_LOCATOR: &str = "3D020000F8B51C1C";

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// The three starters.
pub(super) fn read(store: &mut Store, entry: &RomEntry, scripts: &str) -> Result<Vec<DexNumber>, RomError> {
    (0..3)
        .map(|i| {
            let at = *starter_offsets(entry, i)
                .first()
                .ok_or_else(|| malformed(format!("no offsets for starter {}", i + 1)))?;
            word(store.narc_file(scripts, at.file)?, at.offset)
        })
        .collect()
}

/// Writes the starters, gives back the Purrloin of the introduction, and updates their sprites, cries and texts.
pub(super) fn write(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    scripts: &str,
    graphics: &str,
    species: &[Species],
    starters: &[DexNumber],
) -> Result<(), RomError> {
    if starters.len() != 3 {
        return Err(malformed(format!("Gen 5 has 3 starters, got {}", starters.len())));
    }
    for (i, &starter) in starters.iter().enumerate() {
        for at in starter_offsets(entry, i) {
            put_word(store.narc_file_mut(scripts, at.file)?, at.offset, starter)?;
        }
    }
    give_back_purrloin(store, entry, family, scripts)?;

    let starter_graphics = file_path(entry, "StarterGraphics")?;
    for (i, &starter) in starters.iter().enumerate() {
        let n = usize::from(starter);
        let palette = store.narc_file(graphics, n * 20 + 18)?.to_vec();
        let packed = store.narc_file(graphics, n * 20)?.to_vec();
        let picture = ppr_formats::lz::decompress(&packed, 0)
            .map_err(|e| malformed(format!("the sprite of species {starter}: {e}")))?;
        *store.narc_file_mut(&starter_graphics, i * 2)? = palette;
        *store.narc_file_mut(&starter_graphics, 12 + i)? = picture;
    }

    let cries = need(entry, "StarterCryOvlNumber")?;
    let prefix = entry.value("StarterCryTablePrefix").unwrap_or_default();
    if let Some(at) = find_after(store.overlay(cries)?, prefix)? {
        let overlay = store.overlay_mut(cries)?;
        for (i, &starter) in starters.iter().enumerate() {
            put_word(overlay, at + i * 2, starter)?;
        }
    }

    let starter_text = story_text(entry, "Starter");
    let (first, extra): (usize, Vec<(usize, &str)>) = match family {
        Family::Bw1 => (
            BW1_STARTER_TEXT,
            vec![
                (BW1_CHEREN_TEXT_1, story_text(entry, "Cheren1")),
                (BW1_CHEREN_TEXT_2, story_text(entry, "Cheren2")),
            ],
        ),
        Family::Bw2 => (BW2_STARTER_TEXT, vec![(BW2_RIVAL_TEXT, story_text(entry, "Rival"))]),
    };
    let lines = store.strings_mut(Text::Story, need(entry, "StarterLocationTextOffset")?)?;
    if !starter_text.is_empty() {
        for (i, &starter) in starters.iter().enumerate() {
            let s = species.iter().find(|s| s.number == starter);
            let type_name = s.map(|s| s.primary_type.to_string()).unwrap_or_default();
            let name = s.map(|s| s.name.as_str()).unwrap_or_default();
            if let Some(line) = lines.get_mut(first - i) {
                *line = starter_text.replace("%1$s", &type_name).replace("%2$s", name);
            }
        }
    }
    for (at, text) in extra {
        if let Some(line) = lines.get_mut(at).filter(|_| !text.is_empty()) {
            text.clone_into(line);
        }
    }
    Ok(())
}

/// The script that gives the starters also gave the Purrloin the player sees; a jump to an appended script brings
/// it back.
fn give_back_purrloin(store: &mut Store, entry: &RomEntry, family: Family, scripts: &str) -> Result<(), RomError> {
    let file = need(entry, "PokedexGivenFileOffset")?;
    let japanese = entry.code.as_bytes().get(3) == Some(&b'J');
    let (script, magic, jump): (&[u8], &str, u8) = match family {
        Family::Bw1 => (&BW1_STARTER_SCRIPT, BW1_STARTER_SCRIPT_MAGIC, 0x04),
        Family::Bw2 => (&BW2_STARTER_SCRIPT, BW2_STARTER_SCRIPT_MAGIC, 0x1E),
    };
    let data = store.narc_file_mut(scripts, file)?;
    let Some(at) = find(data, magic)? else { return Ok(()) };
    let old_len = data.len();
    data.extend_from_slice(script);
    if japanese {
        let fixes: &[usize] = family.pick(&[4, 8], &[6]);
        for &f in fixes {
            data[old_len + f] = data[old_len + f].wrapping_sub(4);
        }
    }
    data[at] = jump;
    data[at + 1] = 0;
    put_relative_pointer(data, at + 2, old_len)
}

/// Shows and cries the Pokémon in the introduction.
pub(super) fn write_intro(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    species: DexNumber,
) -> Result<(), RomError> {
    let graphic = need(entry, "IntroGraphicOvlNumber")?;
    if let Some(at) = find_after(store.overlay(graphic)?, INTRO_GRAPHIC_PREFIX)? {
        put_word(store.overlay_mut(graphic)?, at, species)?;
    }
    let cry = need(entry, "IntroCryOvlNumber")?;
    match family {
        Family::Bw1 => {
            let Some(at) = find_after(store.overlay(cry)?, BW1_INTRO_CRY_PREFIX)? else {
                return Ok(());
            };
            // The cry's species is computed (`mov r0, #0x8f; lsl r0, r0, #0x2`). Without the shift, the rest of the
            // function moves up 2 bytes, leaving room for the species after it, loaded relative to the PC instead.
            let overlay = store.overlay_mut(cry)?;
            if overlay.len() < at + 42 {
                return Err(malformed("the introduction's cry"));
            }
            if overlay[at..at + 2] == [0x09, 0x48] {
                // Moved before.
                return put_long(overlay, at + 38, u32::from(species));
            }
            overlay.copy_within(at + 6..at + 40, at + 4);
            // The call moved with it.
            overlay[at + 10] = overlay[at + 10].wrapping_add(1);
            put_long(overlay, at + 38, u32::from(species))?;
            overlay[at] = 0x09;
            overlay[at + 1] = 0x48;
        }
        Family::Bw2 => {
            if let Some(at) = find(store.overlay(cry)?, BW2_INTRO_CRY_LOCATOR)? {
                put_word(store.overlay_mut(cry)?, at, species)?;
            }
        }
    }
    Ok(())
}
