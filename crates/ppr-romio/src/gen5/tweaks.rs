//! The misc. tweaks of Gen 5, guaranteed catching, easier happiness evolutions and the type chart.
//!
//! Deliberate choice: Fast Egg Hatching and No EV Yields change the species files, alternate formes included, when
//! they are applied, as they are not part of the species data.

use ppr_model::{Effectiveness, MiscTweak, Type, TypeChart};

use super::entry::{Family, need};
use super::species::{EGG_CYCLES, EV_YIELD, set_in_all, type_byte};
use super::store::{Store, byte, find, hex, put, put_byte, put_word, relative_pointer, word};
use crate::entries::RomEntry;
use crate::{RomError, ips};

const SCRIPT_LIST_END: u16 = 0xFD13;
const SET_VAR_GIFT: u8 = 0x2A;
const GIFT_VAR: u16 = 0x8000;
const LUCKY_EGG: u16 = 231;
const GOOEY_MULCH: u16 = 98;
/// The byte that starts a new block of a script file.
const BLOCK_START: u8 = 0x2E;

const FOSSIL_SCRIPT_FILE: usize = 877;
const FOSSIL_LEVEL_AT: usize = 0x3F7;
const FOSSIL_LEVEL: u16 = 20;

const RUNNING_SHOES_PREFIX: &str = "01D0012008BD002008BD63";
const FORCE_CHALLENGE_MODE_LOCATOR: &str = "816A406B0B1C07490022434090000858834201D1";
const LOW_HEALTH_MUSIC_LOCATOR: &str = "00D10127";
const PERFECT_ODDS_BRANCH_LOCATOR: &str = "08DB002801D0012000E0";
const HAPPINESS_LOCATOR: &str = "DC282FD3";
const VANILLA_HAPPINESS: u8 = 220;
const EASIER_HAPPINESS: u8 = 160;

/// The function that tells whether a move is an HM, and what it becomes: always "no". The last 4 bytes differ
/// between games, so they are not checked.
const HMS_FORGETTABLE_BEFORE: &str = "084A002359005118B8310988884201D101207047591C09060B0E062BF2D300207047C046";
const HMS_FORGETTABLE_AFTER: &str = "00000000000000000000000000000000000000000000000000000000000000207047000000000000";

/// The tweaks the game offers, in order.
pub(super) fn available(entry: &RomEntry, family: Family) -> Vec<MiscTweak> {
    let mut out = Vec::new();
    if entry.value("FastestTextTweak").is_some() {
        out.push(MiscTweak::FastestText);
    }
    out.extend([
        MiscTweak::BanLuckyEgg,
        MiscTweak::NoFreeLuckyEgg,
        MiscTweak::BanBigManiacItems,
    ]);
    if family == Family::Bw1 {
        out.push(MiscTweak::BalanceStaticLevels);
    }
    if entry.value("NationalDexAtStartTweak").is_some() {
        out.push(MiscTweak::NationalDexAtStart);
    }
    out.push(MiscTweak::RunWithoutRunningShoes);
    if family == Family::Bw2 {
        out.push(MiscTweak::ForceChallengeMode);
    }
    out.push(MiscTweak::DisableLowHpMusic);
    if entry.int("HMMovesForgettableFunctionOffset").unwrap_or(0) != 0 {
        out.push(MiscTweak::ForgettableHms);
    }
    out.extend([MiscTweak::FastEggHatching, MiscTweak::NoEvYields]);
    out
}

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// Applies a tweak to the game. `species_files` are the species and alternate forme files of `stats`.
pub(super) fn apply(
    store: &mut Store,
    entry: &RomEntry,
    scripts: &str,
    stats: &str,
    species_files: &[usize],
    tweak: MiscTweak,
) -> Result<(), RomError> {
    match tweak {
        MiscTweak::FastestText => {
            if let Some(patch) = entry.value("FastestTextTweak") {
                ips::named(patch)?.apply(store.arm9_mut()?)?;
            }
        }
        MiscTweak::NoFreeLuckyEgg => remove_free_lucky_egg(store, entry, scripts)?,
        MiscTweak::BalanceStaticLevels => {
            put_word(
                store.narc_file_mut(scripts, FOSSIL_SCRIPT_FILE)?,
                FOSSIL_LEVEL_AT,
                FOSSIL_LEVEL,
            )?;
        }
        MiscTweak::NationalDexAtStart => {
            if let Some(patch) = entry.value("NationalDexAtStartTweak") {
                // The patch adds an instruction to the script, in four bytes added for it.
                let script = store.narc_file_mut(scripts, need(entry, "NationalDexScriptOffset")?)?;
                script.extend_from_slice(&[0; 4]);
                ips::named(patch)?.apply(script)?;
            }
        }
        MiscTweak::RunWithoutRunningShoes => {
            // The field overlay checks for the Running Shoes' flag and returns 0 without; the branch goes.
            let field = need(entry, "FieldOvlNumber")?;
            if let Some(at) = find(store.overlay(field)?, RUNNING_SHOES_PREFIX)? {
                put_word(store.overlay_mut(field)?, at, 0)?;
            }
        }
        MiscTweak::ForceChallengeMode => {
            // The function that gives the difficulty returns 2 (`mov r0, #0x2; bx lr`).
            if let Some(at) = find(store.arm9()?, FORCE_CHALLENGE_MODE_LOCATOR)? {
                put(store.arm9_mut()?, at, &[0x02, 0x20, 0x70, 0x47])?;
            }
        }
        MiscTweak::DisableLowHpMusic => {
            // The branch past the low HP music becomes unconditional.
            let number = need(entry, "LowHealthMusicOvlNumber")?;
            if let Some(at) = find(store.overlay(number)?, LOW_HEALTH_MUSIC_LOCATOR)? {
                put_byte(store.overlay_mut(number)?, at + 1, 0xE0)?;
            }
        }
        MiscTweak::ForgettableHms => {
            let at = need(entry, "HMMovesForgettableFunctionOffset")?;
            let before = hex(HMS_FORGETTABLE_BEFORE)?;
            let arm9 = store.arm9_mut()?;
            if arm9.get(at..at + before.len()) != Some(&before[..]) {
                return Err(malformed("the HM check is not where the ROM entry says"));
            }
            put(arm9, at, &hex(HMS_FORGETTABLE_AFTER)?)?;
        }
        MiscTweak::FastEggHatching => set_in_all(store, stats, species_files, EGG_CYCLES, &[1])?,
        MiscTweak::NoEvYields => set_in_all(store, stats, species_files, EV_YIELD, &[0, 0])?,
        // Banning items happens in the game data.
        _ => {}
    }
    Ok(())
}

/// The professor's Lucky Egg (two in Black 2 and White 2) becomes Gooey Mulch.
fn remove_free_lucky_egg(store: &mut Store, entry: &RomEntry, scripts: &str) -> Result<(), RomError> {
    let file = need(entry, "LuckyEggScriptOffset")?;
    let family = Family::parse(entry)?;
    let mut looking_for = family.pick(1, 2);
    let data = store.narc_file_mut(scripts, file)?;
    let mut offset = 0;
    while looking_for > 0 {
        if word(data, offset)? == SCRIPT_LIST_END {
            break;
        }
        let mut at = relative_pointer(data, offset)?;
        offset += 4;
        if at > data.len() {
            break;
        }
        // Gift items are not aligned, so the block is read byte by byte.
        loop {
            at += 1;
            let b = byte(data, at)?;
            if b == SET_VAR_GIFT
                && word(data, at)? == u16::from(SET_VAR_GIFT)
                && word(data, at + 2)? == GIFT_VAR
                && word(data, at + 4)? == LUCKY_EGG
            {
                put_word(data, at + 4, GOOEY_MULCH)?;
                looking_for -= 1;
            }
            if b == BLOCK_START {
                break;
            }
        }
    }
    Ok(())
}

/// Every ball catches.
pub(super) fn enable_guaranteed_catching(store: &mut Store, entry: &RomEntry) -> Result<(), RomError> {
    // The branch taken for odds below 255 goes.
    let battle = need(entry, "BattleOvlNumber")?;
    if let Some(at) = find(store.overlay(battle)?, PERFECT_ODDS_BRANCH_LOCATOR)? {
        put(store.overlay_mut(battle)?, at, &[0, 0])?;
    }
    Ok(())
}

/// Happiness evolutions (any time, day and night) need 160 instead of 220.
pub(super) fn ease_happiness(store: &mut Store) -> Result<(), RomError> {
    let Some(at) = find(store.arm9()?, HAPPINESS_LOCATOR)? else {
        return Ok(());
    };
    let arm9 = store.arm9_mut()?;
    for f in [0, 20, 38] {
        if byte(arm9, at + f)? == VANILLA_HAPPINESS {
            arm9[at + f] = EASIER_HAPPINESS;
        }
    }
    Ok(())
}

fn effectiveness_byte(e: Effectiveness) -> u8 {
    match e {
        Effectiveness::Double => 8,
        Effectiveness::Neutral => 4,
        Effectiveness::Half => 2,
        Effectiveness::Zero => 0,
    }
}

/// The type chart: a byte for each attacking and defending type, in the game's type order.
pub(super) fn read_type_chart(store: &mut Store, entry: &RomEntry, types: &[Type]) -> Result<TypeChart, RomError> {
    let at = need(entry, "TypeEffectivenessOffset")?;
    let overlay = store.overlay(need(entry, "BattleOvlNumber")?)?;
    let width = types.len();
    let mut chart = TypeChart::neutral(types);
    for &attacker in types {
        for &defender in types {
            let b = byte(
                overlay,
                at + usize::from(type_byte(attacker)) * width + usize::from(type_byte(defender)),
            )?;
            let e = match b {
                8 => Effectiveness::Double,
                4 => Effectiveness::Neutral,
                2 => Effectiveness::Half,
                0 => Effectiveness::Zero,
                _ => return Err(malformed(format!("type effectiveness {b}"))),
            };
            chart.set(attacker, defender, e);
        }
    }
    Ok(chart)
}

/// Writes the type chart.
pub(super) fn write_type_chart(
    store: &mut Store,
    entry: &RomEntry,
    chart: &TypeChart,
    types: &[Type],
) -> Result<(), RomError> {
    let at = need(entry, "TypeEffectivenessOffset")?;
    let overlay = store.overlay_mut(need(entry, "BattleOvlNumber")?)?;
    let width = types.len();
    for &attacker in types {
        for &defender in types {
            let Some(e) = chart.get(attacker, defender) else {
                continue;
            };
            put_byte(
                overlay,
                at + usize::from(type_byte(attacker)) * width + usize::from(type_byte(defender)),
                effectiveness_byte(e),
            )?;
        }
    }
    Ok(())
}

/// The files of the species and their alternate formes.
pub(super) fn species_files(family: Family) -> Vec<usize> {
    family.species_numbers().map(|n| family.species_file(n)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_files_take_in_the_formes() {
        let bw1 = species_files(Family::Bw1);
        assert_eq!(
            (bw1.len(), bw1[648], bw1[649], bw1.last().copied()),
            (667, 649, 650, Some(667))
        );
        let bw2 = species_files(Family::Bw2);
        assert_eq!((bw2.len(), bw2[649], bw2.last().copied()), (673, 685, Some(708)));
        assert_eq!(hex(HMS_FORGETTABLE_BEFORE).map(|b| b.len()).ok(), Some(36));
        assert_eq!(hex(HMS_FORGETTABLE_AFTER).map(|b| b.len()).ok(), Some(40));
    }
}
