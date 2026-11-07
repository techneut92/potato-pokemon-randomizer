//! The intro Pokémon and the catching tutorial.
//!
//! In Diamond, Pearl and Platinum the intro Pokémon is in the intro overlay, after two code patterns. In
//! `HeartGold` and `SoulSilver` it is Lyra's or Ethan's Marill instead: its overworld sprite in the field overlay,
//! its cry in scripts, its name in texts, and (with the new catching tutorial routine) the catcher of the catching
//! tutorial.
//!
//! The catching tutorial's wild Pokémon is a word in the ARM9 in Diamond, Pearl and Platinum, whose catcher can't
//! change; with the new routine of `HeartGold` and `SoulSilver` the wild Pokémon follows the catcher, which is the
//! intro Pokémon; without it both are bytes, so both are at most 255, and the catcher's level becomes 10.
//!
//! Deliberate choices:
//! - Every species is a catching tutorial catcher candidate where the catcher can't change, so the tutorial's
//!   wild Pokémon can still be randomized; the catcher is then ignored.

use std::collections::BTreeMap;

use ppr_model::{CatchingTutorial, DexNumber, GameData};

use super::Rom;
use super::constants::{self as c, Family};
use super::files::{byte, find_after, put, put_word};
use crate::RomError;

/// The code patterns the intro Pokémon follows in Diamond, Pearl and Platinum.
const DPPT_INTRO_PREFIXES: [&str; 2] = ["381CF8BDC046", "08B0F8BD"];
/// What comes before Lyra's and Ethan's Marill's overworld sprite in the field overlay.
const MARILL_SPRITE_PREFIX: &str = "274E0604C301274E0704E101274E0804";
/// The first overworld sprite, Bulbasaur's; species after it have one sprite each, except these.
const BULBASAUR_SPRITE: u16 = 297;
const SPRITE_COUNTS: [(DexNumber, u16); 22] = [
    (3, 2),
    (25, 2),
    (154, 2),
    (172, 2),
    (201, 28),
    (202, 2),
    (208, 2),
    (214, 2),
    (386, 4),
    (412, 3),
    (413, 3),
    (415, 2),
    (422, 2),
    (423, 2),
    (443, 2),
    (444, 2),
    (445, 2),
    (449, 2),
    (450, 2),
    (479, 6),
    (487, 2),
    (493, 18),
];
/// Species whose overworld sprites are big.
const BIG_OVERWORLD: [DexNumber; 12] = [208, 249, 250, 321, 382, 383, 384, 483, 484, 486, 487, 493];
/// The overworld sprite sizes: 0x208 | (20 << 10) for big ones, 0x227 | (19 << 10) for the others.
const BIG_SPRITE: u16 = 0x5208;
const NORMAL_SPRITE: u16 = 0x4E27;
const FEMALE_ONLY: u8 = 0xFE;
/// The catcher's level in the catching tutorial without the new routine.
const CATCHER_LEVEL: u8 = 10;

fn overworld_sprite(species: DexNumber) -> u16 {
    (1..species).fold(BULBASAUR_SPRITE, |sprite, n| {
        sprite
            + SPRITE_COUNTS
                .iter()
                .find(|(s, _)| *s == n)
                .map_or(1, |&(_, count)| count)
    })
}

/// The species the intro can show: in Diamond, Pearl and Platinum not the female-only ones, which the intro's
/// sprite loading can't show.
pub(super) fn candidates(rom: &mut Rom) -> Result<Vec<DexNumber>, RomError> {
    let mut out = Vec::new();
    for n in 1..=u16::try_from(c::SPECIES_COUNT).unwrap_or_default() {
        if rom.family().is_dppt() && byte(rom.stats(n)?, c::BS_GENDER_RATIO)? == FEMALE_ONLY {
            continue;
        }
        out.push(n);
    }
    Ok(out)
}

fn new_routine(rom: &Rom) -> bool {
    rom.entry.has_tweak("NewCatchingTutorialSubroutineTweak")
}

/// Where the new catching tutorial routine loads the catcher from, the wild Pokémon following.
fn tutorial_table(rom: &mut Rom) -> Option<usize> {
    let prefix = rom.entry.string("CatchingTutorialMonTablePrefix").to_owned();
    find_after(rom.files.arm9_ref(), &prefix)
}

/// Writes the intro Pokémon, if it is one of the candidates.
pub(super) fn write(rom: &mut Rom, data: &GameData) -> Result<(), RomError> {
    let Some(n) = data.intro_pokemon.filter(|n| data.intro_candidates.contains(n)) else {
        return Ok(());
    };
    if rom.family().is_dppt() {
        let overlay = rom.files.overlay(rom.entry.need("IntroOvlNumber")?)?;
        for prefix in DPPT_INTRO_PREFIXES {
            if let Some(at) = find_after(overlay, prefix) {
                put_word(overlay, at, n)?;
            }
        }
        return Ok(());
    }
    let overlay = rom.files.overlay(rom.entry.need("FieldOvlNumber")?)?;
    if let Some(at) = find_after(overlay, MARILL_SPRITE_PREFIX) {
        put_word(overlay, at, overworld_sprite(n))?;
        let size = if BIG_OVERWORLD.contains(&n) {
            BIG_SPRITE
        } else {
            NORMAL_SPRITE
        };
        put_word(overlay, at + 2, size)?;
    }
    let scripts = rom.entry.file("Scripts")?;
    for at in rom.entry.marill_cry_scripts.clone() {
        put_word(rom.files.narc_file(&scripts, at.file)?, at.offset, n)?;
    }
    let old = data
        .species(c::MARILL)
        .map(|s| s.name.to_uppercase())
        .unwrap_or_default();
    let new = data.species(n).map(|s| s.name.clone()).unwrap_or_default();
    let replacements = BTreeMap::from([(old, new)]);
    let line = rom.chars_per_line();
    for text in rom.entry.array("MarillTextFiles") {
        rom.files.replace_names(text, &replacements, line)?;
    }
    if new_routine(rom)
        && let Some(at) = tutorial_table(rom)
    {
        put_word(rom.files.arm9(), at, n)?;
    }
    Ok(())
}

/// The species the catching tutorial accepts as the wild one and as the catcher; none without the offsets it
/// needs.
pub(super) fn tutorial_candidates(rom: &Rom) -> (Vec<DexNumber>, Vec<DexNumber>) {
    let all: Vec<DexNumber> = (1..=u16::try_from(c::SPECIES_COUNT).unwrap_or_default()).collect();
    if new_routine(rom) {
        return (all.clone(), all);
    }
    if rom.entry.int("CatchingTutorialOpponentMonOffset") == 0 {
        return (Vec::new(), Vec::new());
    }
    if rom.family() == Family::HeartGoldSoulSilver {
        if rom.entry.int("CatchingTutorialPlayerMonOffset") == 0 {
            return (Vec::new(), Vec::new());
        }
        let bytes: Vec<DexNumber> = all.into_iter().filter(|&n| n <= 0xFF).collect();
        return (bytes.clone(), bytes);
    }
    (all.clone(), all)
}

/// Writes the catching tutorial's Pokémon.
pub(super) fn write_tutorial(rom: &mut Rom, tutorial: CatchingTutorial) -> Result<(), RomError> {
    let opponent_at = rom.entry.int("CatchingTutorialOpponentMonOffset");
    if new_routine(rom) {
        if let Some(at) = tutorial_table(rom) {
            put_word(rom.files.arm9(), at + 4, tutorial.opponent)?;
        }
    } else if rom.family() == Family::HeartGoldSoulSilver {
        let player_at = rom.entry.need("CatchingTutorialPlayerMonOffset")?;
        let level_at = rom.entry.need("CatchingTutorialPlayerLevelOffset")?;
        let (Ok(opponent), Ok(player)) = (u8::try_from(tutorial.opponent), u8::try_from(tutorial.player)) else {
            return Err(RomError::Malformed(
                "this catching tutorial takes species up to 255".into(),
            ));
        };
        let arm9 = rom.files.arm9();
        put(arm9, opponent_at, &[opponent])?;
        put(arm9, player_at, &[player])?;
        put(arm9, level_at, &[CATCHER_LEVEL])?;
    } else {
        super::files::put_long(rom.files.arm9(), opponent_at, u32::from(tutorial.opponent))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameHandler;
    use crate::gen4::tests::{load, real_rom, save};

    /// With `PPR_TEST_ROMS`: Platinum's intro Pokémon and catching tutorial are written where the game reads them.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_platinum_intro_and_catching_tutorial() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes);
        let mut data = h.game_data().clone();
        assert!(!data.intro_candidates.contains(&29), "Nidoran♀ is female only");
        assert!(data.intro_candidates.contains(&25));
        assert_eq!(data.catching_tutorial_opponents.len(), 493);
        data.intro_pokemon = Some(25);
        data.catching_tutorial = Some(CatchingTutorial {
            opponent: 300,
            player: 1,
        });
        h.apply(&data).expect("applies");
        let mut again = load(save(&h));
        let rom = &mut again.rom;
        let at = rom.entry.need("CatchingTutorialOpponentMonOffset").expect("offset");
        assert_eq!(super::super::files::long(rom.files.arm9_ref(), at).ok(), Some(300));
        let overlay = rom
            .files
            .overlay(rom.entry.need("IntroOvlNumber").expect("overlay"))
            .expect("overlay");
        for prefix in DPPT_INTRO_PREFIXES {
            let at = find_after(overlay, prefix).expect("prefix");
            assert_eq!(super::super::files::word(overlay, at).ok(), Some(25));
        }
    }

    #[test]
    fn overworld_sprites_count_the_species_with_more() {
        assert_eq!(overworld_sprite(1), 297);
        assert_eq!(overworld_sprite(4), 297 + 2 + 2);
        assert_eq!(overworld_sprite(183), 297 + 182 + 1 + 1 + 1 + 1);
    }
}
