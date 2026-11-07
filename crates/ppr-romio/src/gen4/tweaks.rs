//! The misc tweaks Gen 4 supports, and the patches that apply them; with guaranteed catching.
//!
//! Lower case names, the catching tutorial, banning the Lucky Egg and the Rotom forme types change the game data;
//! the others are patched into the ROM here.
//!
//! Deliberate choices:
//! - Forgettable HMs are only offered when the ROM entry has the code offsets the patch needs.

use ppr_model::MiscTweak;

use super::constants::{self as c, Family};
use super::files::{find, find_after, put, put_word};
use super::{Rom, ips_patch};
use crate::RomError;

/// Where the running shoes check is, `0xE` bytes before the branch that needs the shoes.
const DPPT_RUNNING_SHOES_PREFIX: &str = "281C0C24";
const HGSS_RUNNING_SHOES_PREFIX: &str = "301C0C24";
const RUNNING_SHOES_BRANCH: usize = 0xE;
/// What comes before the HP bar's speed, the EXP bar's `add r0, r6, #0`, and the shift of both bars' speed.
const HP_BAR_SPEED_PREFIX: &str = "0CD106200090";
const EXP_BAR_SPEED_PREFIX: &str = "011C00D101212E6C";
const BOTH_BARS_SPEED_PREFIX: &str = "70BD90421DDA";
/// The Spear Pillar portal script, which the fast Distortion World patch grows by 12 bytes.
const SPEAR_PILLAR_PORTAL_SCRIPT: usize = 237;
const SPEAR_PILLAR_PORTAL_GROWTH: usize = 12;
/// The national Pokédex patch of `HeartGold` and `SoulSilver` adds an instruction of 4 bytes.
const HGSS_NATIONAL_DEX_GROWTH: usize = 4;
/// The branch that makes TMs used up, and what makes it never taken.
const TMS_REUSABLE_BEFORE: u8 = 0xD1;
const TMS_REUSABLE_AFTER: u8 = 0xE0;
/// `nop; movs r0, #0`, in place of the call that tells whether a move is an HM; the call is followed by `cmp r0,
/// #1`.
const R0_FALSE: [u8; 4] = [0xC0, 0x46, 0x00, 0x20];
const CMP_R0_1: [u8; 2] = [0x01, 0x28];
/// The overlay of the battle code that checks for HMs.
const DP_HM_OVERLAY: usize = 9;
const PT_HM_OVERLAY: usize = 13;
const HGSS_HM_OVERLAY: usize = 8;

/// The number of code offsets the forgettable HMs patch needs.
fn hm_offset_count(family: Family) -> usize {
    if family == Family::HeartGoldSoulSilver { 3 } else { 2 }
}

/// The tweaks the game supports.
pub(super) fn available(rom: &Rom) -> Vec<MiscTweak> {
    let e = &rom.entry;
    let family = rom.family();
    let mut tweaks = vec![MiscTweak::LowerCaseNames, MiscTweak::RandomizeCatchingTutorial];
    if e.has_tweak("FastestTextTweak") {
        tweaks.push(MiscTweak::FastestText);
    }
    tweaks.push(MiscTweak::BanLuckyEgg);
    if e.has_tweak("NationalDexAtStartTweak") {
        tweaks.push(MiscTweak::NationalDexAtStart);
    }
    tweaks.extend([MiscTweak::RunWithoutRunningShoes, MiscTweak::FasterHpAndExpBars]);
    if e.has_tweak("FastDistortionWorldTweak") {
        tweaks.push(MiscTweak::FastDistortionWorld);
    }
    if family != Family::DiamondPearl {
        tweaks.push(MiscTweak::UpdateRotomFormeTyping);
    }
    if e.int("TMMovesReusableFunctionOffset") != 0 {
        tweaks.push(MiscTweak::ReusableTms);
    }
    if e.array("HMMovesForgettableFunctionOffsets").len() == hm_offset_count(family) {
        tweaks.push(MiscTweak::ForgettableHms);
    }
    tweaks.extend([MiscTweak::FastEggHatching, MiscTweak::NoEvYields]);
    tweaks
}

/// Patches `tweak` into the ROM; tweaks that only change the game data do nothing here.
pub(super) fn apply(rom: &mut Rom, tweak: MiscTweak) -> Result<(), RomError> {
    match tweak {
        MiscTweak::FastestText => {
            let entry = rom.entry.clone();
            ips_patch(&entry, rom.files.arm9(), "FastestTextTweak")
        }
        MiscTweak::NationalDexAtStart => national_dex(rom),
        MiscTweak::RunWithoutRunningShoes => run_without_running_shoes(rom),
        MiscTweak::FasterHpAndExpBars => faster_bars(rom),
        MiscTweak::FastDistortionWorld => grow_and_patch_script(
            rom,
            SPEAR_PILLAR_PORTAL_SCRIPT,
            SPEAR_PILLAR_PORTAL_GROWTH,
            "FastDistortionWorldTweak",
        ),
        MiscTweak::ReusableTms => reusable_tms(rom),
        MiscTweak::ForgettableHms => forgettable_hms(rom),
        MiscTweak::FastEggHatching => every_stats_file(rom, |stats| put(stats, c::BS_EGG_CYCLES, &[0])),
        MiscTweak::NoEvYields => every_stats_file(rom, |stats| put_word(stats, c::BS_EV_YIELD, 0)),
        _ => Ok(()),
    }
}

/// Grows script file `file` by `growth` zero bytes (the patch was made for that), then applies the patch.
fn grow_and_patch_script(rom: &mut Rom, file: usize, growth: usize, key: &str) -> Result<(), RomError> {
    let entry = rom.entry.clone();
    let path = entry.file("Scripts")?;
    let script = rom.files.narc_file(&path, file)?;
    script.resize(script.len() + growth, 0);
    ips_patch(&entry, script, key)
}

/// The Pokédex script gives the national Pokédex from the start.
fn national_dex(rom: &mut Rom) -> Result<(), RomError> {
    let file = rom.entry.need("NationalDexScriptOffset")?;
    let growth = if rom.family() == Family::HeartGoldSoulSilver {
        HGSS_NATIONAL_DEX_GROWTH
    } else {
        0
    };
    grow_and_patch_script(rom, file, growth, "NationalDexAtStartTweak")
}

/// The branch taken without running shoes becomes a `nop`, so B runs from the start.
fn run_without_running_shoes(rom: &mut Rom) -> Result<(), RomError> {
    let prefix = if rom.family() == Family::HeartGoldSoulSilver {
        HGSS_RUNNING_SHOES_PREFIX
    } else {
        DPPT_RUNNING_SHOES_PREFIX
    };
    let arm9 = rom.files.arm9();
    if let Some(at) = find(arm9, prefix) {
        put_word(arm9, at + RUNNING_SHOES_BRANCH, 0)?;
    }
    Ok(())
}

/// The bars move twice as fast, as the game runs at 30 frames a second: the HP bar adds 2 instead of 1, the EXP
/// bar doubles the experience it divides (`add r0, r6, r6`), and bars shorter than their maximum shift by 9
/// instead of 8.
fn faster_bars(rom: &mut Rom) -> Result<(), RomError> {
    let overlay = rom.files.overlay(rom.entry.need("BattleOvlNumber")?)?;
    if let Some(at) = find_after(overlay, HP_BAR_SPEED_PREFIX) {
        put(overlay, at, &[0x02])?;
    }
    if let Some(at) = find_after(overlay, EXP_BAR_SPEED_PREFIX) {
        put(overlay, at, &[0xB0, 0x19])?;
    }
    if let Some(at) = find_after(overlay, BOTH_BARS_SPEED_PREFIX) {
        put(overlay, at, &[0x40])?;
    }
    Ok(())
}

/// TMs are no longer used up: the branch after using one is always taken.
fn reusable_tms(rom: &mut Rom) -> Result<(), RomError> {
    let at = rom.entry.int("TMMovesReusableFunctionOffset");
    if at == 0 {
        return Ok(());
    }
    let arm9 = rom.files.arm9();
    match arm9.get(at) {
        Some(&TMS_REUSABLE_BEFORE) => put(arm9, at, &[TMS_REUSABLE_AFTER]),
        other => Err(RomError::Malformed(format!(
            "expected {TMS_REUSABLE_BEFORE:#04x} at TMMovesReusableFunctionOffset, found {other:?}"
        ))),
    }
}

/// HMs can be forgotten: the checks in battle and in the field ask whether a move is an HM and get no.
fn forgettable_hms(rom: &mut Rom) -> Result<(), RomError> {
    let family = rom.family();
    let offsets = rom.entry.array("HMMovesForgettableFunctionOffsets");
    if offsets.len() != hm_offset_count(family) {
        return Err(RomError::Malformed(format!(
            "HMMovesForgettableFunctionOffsets has {} offsets, not {}",
            offsets.len(),
            hm_offset_count(family)
        )));
    }
    let overlay = match family {
        Family::DiamondPearl => DP_HM_OVERLAY,
        Family::Platinum => PT_HM_OVERLAY,
        Family::HeartGoldSoulSilver => HGSS_HM_OVERLAY,
    };
    patch_hm_check(rom.files.overlay(overlay)?, offsets[0])?;
    for &at in &offsets[1..] {
        patch_hm_check(rom.files.arm9(), at)?;
    }
    Ok(())
}

fn patch_hm_check(data: &mut [u8], at: usize) -> Result<(), RomError> {
    if data.get(at + 4..at + 6) != Some(&CMP_R0_1) {
        return Err(RomError::Malformed(format!(
            "no HM check at {at:#x}; HMMovesForgettableFunctionOffsets may be wrong"
        )));
    }
    put(data, at, &R0_FALSE)
}

/// Runs `change` on the stats file of every species and forme.
fn every_stats_file(rom: &mut Rom, change: impl Fn(&mut Vec<u8>) -> Result<(), RomError>) -> Result<(), RomError> {
    let path = rom.entry.file("PokemonStats")?;
    let formes = rom.family().forme_count();
    let files = (1..=c::SPECIES_COUNT).chain((1..=formes).map(|i| c::SPECIES_COUNT + c::FORME_OFFSET + i));
    for file in files {
        change(rom.files.narc_file(&path, file)?)?;
    }
    Ok(())
}

/// Every ball catches: the check whether the odds pass 254 is removed, so they always do.
pub(super) fn guaranteed_catching(rom: &mut Rom) -> Result<(), RomError> {
    let overlay = rom.files.overlay(rom.entry.need("BattleOvlNumber")?)?;
    if let Some(at) = find(overlay, c::PERFECT_ODDS_BRANCH_LOCATOR) {
        put(overlay, at, &[0, 0, 0, 0])?;
    }
    Ok(())
}
