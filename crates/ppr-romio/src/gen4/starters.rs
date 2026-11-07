//! The starters and their held item, with everything that names or depends on them: the rival's battles, the
//! starter selection texts and graphics, and (in `HeartGold` and `SoulSilver`) the starters' cries.
//!
//! Diamond, Pearl and Platinum keep the starters in an overlay; the overlay's graphics routine adds to a pointer
//! to find each starter's picture, and is patched to add any species. `HeartGold` and `SoulSilver` keep them in
//! the ARM9 binary.
//!
//! Deliberate choices:
//! - The starter texts are written only when the starters changed.

use ppr_model::{DexNumber, GameData, ItemId, Species};

use super::Rom;
use super::constants::{self as c, Family};
use super::files::{find_after, long, put, put_long, put_word, search, word};
use crate::RomError;

/// What follows the starters in the `HeartGold` and `SoulSilver` ARM9, and how far before it they are.
const HGSS_STARTER_CODE_SUFFIX: [u8; 8] = [0x03, 0x03, 0x1A, 0x12, 0x01, 0x23, 0x00, 0x00];
const HGSS_STARTER_BEFORE_SUFFIX: usize = 13;
/// What comes before the starter cries in the `HeartGold` and `SoulSilver` starter overlay.
const STARTER_CRIES_PREFIX: &str = "0004000C10BD0000000000000000000000E000000000000000E0000000000200";

/// The script files with the rival's battles, which pick the rival's team by the player's starter.
const HGSS_FILES_WITH_RIVAL_SCRIPT: [usize; 7] = [7, 23, 96, 110, 819, 850, 866];
const PT_FILES_WITH_RIVAL_SCRIPT: [usize; 8] = [31, 36, 112, 123, 186, 427, 429, 1096];
const DP_FILES_WITH_RIVAL_SCRIPT: [usize; 6] = [34, 90, 118, 180, 195, 394];
/// "Store the starter in 0x800C; if 0x800C is Chikorita, jump if not equal".
const HGSS_RIVAL_SCRIPT_MAGIC: [u8; 13] = [
    0xCE, 0x00, 0x0C, 0x80, 0x11, 0x00, 0x0C, 0x80, 152, 0x00, 0x1C, 0x00, 0x05,
];
/// The same with Turtwig, in Diamond, Pearl and Platinum.
const DPPT_RIVAL_SCRIPT_MAGIC: [u8; 13] = [
    0xDE, 0x00, 0x0C, 0x80, 0x11, 0x00, 0x0C, 0x80, 0x83, 0x01, 0x1C, 0x00, 0x01,
];
/// The two parts of a tag battle with the rival or a friend, which check the player's starter.
const DPPT_TAG_BATTLE_SCRIPT_MAGIC_1: [u8; 8] = [0xDE, 0x00, 0x0C, 0x80, 0x28, 0x00, 0x04, 0x80];
const DPPT_TAG_BATTLE_SCRIPT_MAGIC_2: [u8; 9] = [0x11, 0x00, 0x0C, 0x80, 0x86, 0x01, 0x1C, 0x00, 0x01];
const PT_FILES_WITH_TAG_SCRIPT: [usize; 4] = [2, 136, 201, 236];
const DP_FILES_WITH_TAG_SCRIPT: [usize; 3] = [2, 131, 230];
/// Script commands a rival script jumps to: a trainer battle, the Hall of Fame check, and Platinum's starter
/// battle; a tag battle script jumps to a return.
const SCRIPT_TRAINER_BATTLE: u16 = 0xE5;
const SCRIPT_HALL_OF_FAME_CHECK: u16 = 0x28F;
const SCRIPT_STARTER_BATTLE: u16 = 0x125;
const SCRIPT_RETURN: u16 = 0x1B;
/// The "compare variable" command HGSS's second rival check starts with.
const SCRIPT_COMPARE: u8 = 0x11;
/// The rival's line after the first battle, by the lake in Diamond and Pearl and on Route 201 in Platinum.
const DP_STARTER_STRING_INDEX: usize = 19;
const PT_STARTER_STRING_INDEX: usize = 36;

/// Where the `HeartGold` and `SoulSilver` starters are in the ARM9.
fn hgss_offset(arm9: &[u8]) -> Option<usize> {
    match search(arm9, &HGSS_STARTER_CODE_SUFFIX).as_slice() {
        [at] => at.checked_sub(HGSS_STARTER_BEFORE_SUFFIX),
        _ => None,
    }
}

/// The three starters.
pub(super) fn read(rom: &mut Rom) -> Result<Vec<DexNumber>, RomError> {
    if rom.family() == Family::HeartGoldSoulSilver {
        let arm9 = rom.files.arm9_ref();
        return match hgss_offset(arm9) {
            Some(at) => (0..3).map(|i| word(arm9, at + i * 4)).collect(),
            None => Ok(vec![c::CHIKORITA, c::CYNDAQUIL, c::TOTODILE]),
        };
    }
    let at = rom.entry.need("StarterPokemonOffset")?;
    let overlay = rom.files.overlay(rom.entry.need("StarterPokemonOvlNumber")?)?;
    (0..3).map(|i| word(overlay, at + i * 4)).collect()
}

/// The item the starter holds, in Diamond, Pearl and Platinum.
pub(super) fn read_held_items(rom: &mut Rom) -> Result<Vec<Option<ItemId>>, RomError> {
    if !rom.family().is_dppt() {
        return Ok(Vec::new());
    }
    let (file, at) = held_item_place(rom)?;
    let path = rom.entry.file("Scripts")?;
    let item = word(rom.files.narc_file(&path, file)?, at)?;
    Ok(vec![(item != 0).then_some(item)])
}

fn held_item_place(rom: &Rom) -> Result<(usize, usize), RomError> {
    Ok((
        rom.entry.need("StarterPokemonScriptOffset")?,
        rom.entry.need("StarterPokemonHeldItemOffset")?,
    ))
}

/// Writes the starters and their held item, where they changed.
pub(super) fn write(rom: &mut Rom, loaded: &GameData, data: &GameData) -> Result<(), RomError> {
    if data.starters != loaded.starters {
        let starters: Vec<&Species> = data
            .starters
            .iter()
            .map(|&n| {
                data.species(n)
                    .ok_or_else(|| RomError::Malformed(format!("no species {n}")))
            })
            .collect::<Result<_, _>>()?;
        let [a, b, s3] = <[&Species; 3]>::try_from(starters)
            .map_err(|_| RomError::Malformed(format!("Gen 4 has 3 starters, got {}", data.starters.len())))?;
        let starters = [a, b, s3];
        if rom.family() == Family::HeartGoldSoulSilver {
            write_hgss(rom, starters)?;
        } else {
            write_dppt(rom, starters)?;
        }
    }
    if data.starter_held_items != loaded.starter_held_items && rom.family().is_dppt() {
        let [item] = data.starter_held_items.as_slice() else {
            return Err(RomError::Malformed(format!(
                "Gen 4 has 1 starter held item, got {}",
                data.starter_held_items.len()
            )));
        };
        let (file, at) = held_item_place(rom)?;
        let path = rom.entry.file("Scripts")?;
        put_word(rom.files.narc_file(&path, file)?, at, item.unwrap_or(0))?;
    }
    Ok(())
}

/// `HeartGold` and `SoulSilver`: the starters in the ARM9, the rival's team choice, the starter texts and cries.
fn write_hgss(rom: &mut Rom, starters: [&Species; 3]) -> Result<(), RomError> {
    let Some(at) = hgss_offset(rom.files.arm9_ref()) else {
        return Err(RomError::Malformed("no starters in the ARM9".into()));
    };
    for (i, s) in starters.iter().enumerate() {
        put_word(rom.files.arm9(), at + i * 4, s.number)?;
    }

    // The rival picks the starter that beats the player's: "if the player has Chikorita, Cyndaquil's team; if
    // Cyndaquil, Totodile's; else Chikorita's". The species checked become the first and second starters.
    let path = rom.entry.file("Scripts")?;
    for file_number in HGSS_FILES_WITH_RIVAL_SCRIPT {
        let file = rom.files.narc_file(&path, file_number)?;
        let [base] = search(file, &HGSS_RIVAL_SCRIPT_MAGIC)[..] else {
            continue;
        };
        put_word(file, base + 8, starters[0].number)?;
        let second = jump_target(file, base + 13)?;
        if file.get(second) == Some(&SCRIPT_COMPARE) && file.get(second + 4) == Some(&c::CYNDAQUIL.to_le_bytes()[0]) {
            put_word(file, second + 4, starters[1].number)?;
        }
    }

    let index = rom.entry.need("StarterScreenTextOffset")?;
    let mut strings = rom.files.strings(index)?;
    let intros = ["So, you like", "You'll take", "Do you want"];
    for (i, s) in starters.iter().enumerate() {
        let color = if i == 0 { 3 } else { i };
        let t = s.primary_type;
        set(
            &mut strings,
            i + 1,
            format!(
                "Professor Elm: {} \\vFF00\\z000{color}{}\\vFF00\\z0000,\\nthe {t}-type Pokémon?",
                intros[i], s.name
            ),
        );
        set(
            &mut strings,
            i + 4,
            format!(
                "\\vFF00\\z000{color}{}\\vFF00\\z0000, the {t}-type Pokémon, is\\nin this Poké Ball!",
                s.name
            ),
        );
    }
    rom.files.set_strings(index, strings)?;

    let overlay = rom.files.overlay(rom.entry.need("StarterPokemonOvlNumber")?)?;
    if let Some(cries) = find_after(overlay, STARTER_CRIES_PREFIX) {
        for (i, s) in starters.iter().enumerate() {
            put_long(overlay, cries + i * 4, u32::from(s.number))?;
        }
    }
    Ok(())
}

fn set(strings: &mut [String], i: usize, s: String) {
    if let Some(slot) = strings.get_mut(i) {
        *slot = s;
    }
}

/// Diamond, Pearl and Platinum: the starters in their overlay and its graphics routine, the rival's battles and
/// tag battles, and the starter texts.
fn write_dppt(rom: &mut Rom, starters: [&Species; 3]) -> Result<(), RomError> {
    let at = rom.entry.need("StarterPokemonOffset")?;
    let outer = rom.entry.string("StarterPokemonGraphicsPrefix").to_owned();
    let inner = rom.entry.string("StarterPokemonGraphicsPrefixInner").to_owned();
    let overlay = rom.files.overlay(rom.entry.need("StarterPokemonOvlNumber")?)?;
    for (i, s) in starters.iter().enumerate() {
        put_word(overlay, at + i * 4, s.number)?;
    }
    if let Some(offset) = find_after(overlay, &outer) {
        patch_graphics(overlay, offset, starters)?;
        if let Some(offset) = find_after(overlay, &inner) {
            put(overlay, offset + 1, &[0x68])?;
        }
    }
    patch_rival_scripts(rom, starters)?;
    patch_tag_battle_scripts(rom, starters)?;

    let index = rom.entry.need("StarterScreenTextOffset")?;
    let mut strings = rom.files.strings(index)?;
    let categories = rom.strings_of("PokedexSpeciesTextOffset")?;
    for (i, s) in starters.iter().enumerate() {
        let color = if i == 0 { 3 } else { i };
        let category = categories.get(usize::from(s.number)).cloned().unwrap_or_default();
        set(
            &mut strings,
            i + 1,
            format!(
                "\\vFF00\\z000{color}{category} {}\\vFF00\\z0000!\\nWill you take this Pokémon?",
                s.name
            ),
        );
    }
    rom.files.set_strings(index, strings)?;

    // The rival's line after the first battle names the starters' kinds; it becomes a general one.
    let index = rom.entry.need("StarterLocationTextOffset")?;
    let mut strings = rom.files.strings(index)?;
    if rom.family() == Family::DiamondPearl {
        set(
            &mut strings,
            DP_STARTER_STRING_INDEX,
            "\\v0103\\z0000: Fwaaah!\\nYour Pokémon totally rocked!\\pBut mine was way tougher\\nthan yours!\\p...They \
             were other people's\\nPokémon, though...\\pBut we had to use them...\\nThey won't mind, will they?\\p"
                .to_owned(),
        );
    } else {
        set(
            &mut strings,
            PT_STARTER_STRING_INDEX,
            "\\v0103\\z0000\\z0000: Then, I choose you!\\nI'm picking this one!\\p".to_owned(),
        );
    }
    rom.files.set_strings(index, strings)
}

/// Thumb `add r0, #imm` (`sub` with bit 11).
const THUMB_ADD_R0: u16 = 0x3200;
const THUMB_SUB_BIT: u16 = 0x800;

/// The graphics routine uses a value as an offset from a pointer and adds to it to get each starter's species.
/// The pointer's base moves up so that the offset is 4, 8 and 12, and two `add`/`sub` instructions per starter
/// make the species from there; some instructions move down to make room, and a PC-relative load and a branch
/// that move are corrected.
fn patch_graphics(overlay: &mut [u8], start: usize, starters: [&Species; 3]) -> Result<(), RomError> {
    let mut offset = start;
    let copy = |o: &mut [u8], to: usize, from: usize| -> Result<(), RomError> {
        let w = word(o, from)?;
        put_word(o, to, w)
    };
    copy(overlay, offset + 0xC, offset + 0xA)?;
    if offset.is_multiple_of(4) {
        overlay[offset + 0xC] = overlay[offset + 0xC].wrapping_sub(1);
    }
    copy(overlay, offset + 0xA, offset + 0x8)?;
    overlay[offset + 0xA] = overlay[offset + 0xA].wrapping_sub(1);
    copy(overlay, offset + 0x8, offset + 0x6)?;
    copy(overlay, offset + 0x6, offset + 0x4)?;
    copy(overlay, offset + 0x4, offset + 0x2)?;
    // `add r5, r5, r0` (the 0x200 offset goes into the base), then `ldr r0, [r5]` without an offset.
    put_word(overlay, offset + 0x2, 0x6828)?;
    put_word(overlay, offset, 0x182D)?;
    offset += 0x16;
    put_word(overlay, offset, 0x6828)?;
    offset += 0xA;
    for (i, s) in starters.iter().enumerate() {
        let base = 4 * (i + 1);
        let diff = i32::from(s.number) - i32::try_from(base).unwrap_or_default();
        let (mut first, mut second) = (THUMB_ADD_R0, THUMB_ADD_R0);
        let mut amount = diff;
        if diff < 0 {
            first |= THUMB_SUB_BIT;
            amount = -diff;
        } else if diff > 255 {
            second |= 0xFF;
            amount -= 255;
        }
        first |= u16::try_from(amount & 0xFF).unwrap_or_default();
        overlay[offset] = u8::try_from(base).unwrap_or_default();
        copy(overlay, offset + 2, offset + 4)?;
        put_word(overlay, offset + 4, first)?;
        put_word(overlay, offset + 8, second)?;
        offset += 0xE;
    }
    // A value loaded as 0x81 becomes 1, as the pointer moved.
    *overlay
        .get_mut(offset)
        .ok_or_else(|| RomError::Malformed("starter graphics routine cut short".into()))? = 1;
    Ok(())
}

/// The rival scripts check for Turtwig and Chimchar to pick the rival's team; they get the first and second
/// starters. Only checks that jump to a battle (or the Hall of Fame check, or Platinum's starter battle) count.
fn patch_rival_scripts(rom: &mut Rom, starters: [&Species; 3]) -> Result<(), RomError> {
    let files: &[usize] = if rom.family() == Family::Platinum {
        &PT_FILES_WITH_RIVAL_SCRIPT
    } else {
        &DP_FILES_WITH_RIVAL_SCRIPT
    };
    let platinum = rom.family() == Family::Platinum;
    let path = rom.entry.file("Scripts")?;
    for &number in files {
        let file = rom.files.narc_file(&path, number)?;
        for base in search(file, &DPPT_RIVAL_SCRIPT_MAGIC) {
            let jump_at = base + DPPT_RIVAL_SCRIPT_MAGIC.len();
            let to = jump_target(file, jump_at)?;
            let command = word(file, to)?;
            if command != SCRIPT_TRAINER_BATTLE
                && command != SCRIPT_HALL_OF_FAME_CHECK
                && (command != SCRIPT_STARTER_BATTLE || !platinum)
            {
                continue;
            }
            put_word(file, base + 0x8, starters[0].number)?;
            put_word(file, base + 0x15, starters[1].number)?;
        }
    }
    Ok(())
}

/// Where a script's relative jump at `at` goes.
fn jump_target(file: &[u8], at: usize) -> Result<usize, RomError> {
    let relative = i64::from(long(file, at)?.cast_signed());
    usize::try_from(i64::try_from(at + 4).unwrap_or_default() + relative)
        .map_err(|_| RomError::Malformed("script jump before the file".into()))
}

/// The tag battles with the rival or a friend check the player's starter: Turtwig (or else the third starter) and
/// Chimchar.
fn patch_tag_battle_scripts(rom: &mut Rom, starters: [&Species; 3]) -> Result<(), RomError> {
    let files: &[usize] = if rom.family() == Family::Platinum {
        &PT_FILES_WITH_TAG_SCRIPT
    } else {
        &DP_FILES_WITH_TAG_SCRIPT
    };
    let path = rom.entry.file("Scripts")?;
    for &number in files {
        let file = rom.files.narc_file(&path, number)?;
        for base in search(file, &DPPT_TAG_BATTLE_SCRIPT_MAGIC_1) {
            let second = base + DPPT_TAG_BATTLE_SCRIPT_MAGIC_1.len() + 2;
            if file.get(second..second + DPPT_TAG_BATTLE_SCRIPT_MAGIC_2.len()) != Some(&DPPT_TAG_BATTLE_SCRIPT_MAGIC_2)
            {
                continue;
            }
            let Ok(to) = jump_target(file, second + DPPT_TAG_BATTLE_SCRIPT_MAGIC_2.len()) else {
                continue;
            };
            if word(file, to).ok() != Some(SCRIPT_RETURN) {
                continue;
            }
            let first = if word(file, base + 0x21)? == c::TURTWIG {
                starters[0]
            } else {
                starters[2]
            };
            put_word(file, base + 0x21, first.number)?;
            put_word(file, base + 0xE, starters[1].number)?;
        }
    }
    Ok(())
}
