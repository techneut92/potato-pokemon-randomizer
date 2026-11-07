//! Gen 3's misc tweaks: which ones a game has and their ROM side, the catching tutorial Pokémon and the potion in
//! the PC.
//!
//! Deliberate choices:
//! - "Run Without Running Shoes" patches only when the check it looks for is found exactly once; otherwise it
//!   writes nothing, rather than patching at a fallback offset inside the ROM header.
//! - In `FireRed` and `LeafGreen`, the catching tutorial takes the first 255 internal indices, the ones its single
//!   byte can hold. Checking the national number instead would let Treecko to Torchic (252–255) be written as other
//!   species.

use ppr_model::{DexNumber, ItemId, MiscTweak};

use super::space::Space;
use super::{Family, c, found_at, items};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::{GbaRom, hex};
use crate::ips;

/// Thumb `mov r1, #n`, `add r1, #n` and `nop`, as in `intro.rs`.
const MOV_R1: u8 = 0x21;
const ADD_R1: u8 = 0x31;
const NOP: [u8; 2] = [0xC0, 0x46];
/// Gender ratios above this mean no males; the catching tutorial takes only Pokémon that can be male.
const MOST_FEMALE: u8 = 0xFD;
/// The text speed values "Fastest Text" writes when there is no patch: slow is medium, medium is fast, fast is
/// instant.
const TEXT_SPEEDS: [u8; 3] = [4, 1, 0];
/// The base stats offsets of the EV yield (a 16-bit word) and egg cycles.
const BS_EV_YIELD: usize = 10;
const BS_EGG_CYCLES: usize = 17;

/// The running shoes check; the jump to patch is 0x12 bytes past it.
fn running_shoes_check(family: Family) -> &'static str {
    match family {
        Family::RubySapphire => "0440002C1DD08620",
        Family::FireRedLeafGreen => "02200540002D29D0",
        Family::Emerald => "0640002E1BD08C20",
    }
}

/// An offset the entry gives, if it is above 0.
fn positive(entry: &RomEntry, key: &str) -> Option<usize> {
    entry.int(key).and_then(|v| usize::try_from(v).ok()).filter(|&v| v > 0)
}

/// The offsets of an array the entry gives under `key`, or a single one under the key without its final `s`
/// (Ruby, Sapphire and Emerald give one).
fn offset_list(entry: &RomEntry, key: &str) -> Vec<usize> {
    let values = entry
        .array(key)
        .or_else(|| {
            key.strip_suffix('s')
                .and_then(|single| entry.int(single))
                .map(|v| vec![v])
        })
        .unwrap_or_default();
    values.into_iter().filter_map(|v| usize::try_from(v).ok()).collect()
}

/// The misc tweaks the game has, in a fixed order.
pub(super) fn available(entry: &RomEntry, family: Family) -> Vec<MiscTweak> {
    let mut out = vec![
        MiscTweak::LowerCaseNames,
        MiscTweak::BanLuckyEgg,
        MiscTweak::RunWithoutRunningShoes,
    ];
    out.extend([MiscTweak::FastEggHatching, MiscTweak::NoEvYields]);
    if entry.int("NationalDexTweakPossible").is_some_and(|v| v != 0) {
        out.push(MiscTweak::NationalDexAtStart);
    }
    if positive(entry, "RunIndoorsTweakOffset").is_some() {
        out.push(MiscTweak::RunningShoesIndoors);
    }
    if positive(entry, "TextSpeedValuesOffset").is_some() || entry.value("InstantTextTweak").is_some() {
        out.push(MiscTweak::FastestText);
    }
    if positive(entry, "CatchingTutorialOpponentMonOffset").is_some()
        || positive(entry, "CatchingTutorialPlayerMonOffset").is_some()
    {
        out.push(MiscTweak::RandomizeCatchingTutorial);
    }
    if entry.int("PCPotionOffset").is_some_and(|v| v != 0) {
        out.push(MiscTweak::RandomizePcPotion);
    }
    if family == Family::FireRedLeafGreen {
        out.push(MiscTweak::BalanceStaticLevels);
    }
    if !offset_list(entry, "TMMovesReusableFunctionOffsets").is_empty() {
        out.push(MiscTweak::ReusableTms);
    }
    if !offset_list(entry, "HMMovesForgettableFunctionOffsets").is_empty() {
        out.push(MiscTweak::ForgettableHms);
    }
    out.sort_unstable();
    out
}

/// The species [`write_catching_tutorial`] accepts as the one caught and as the catcher: with no offset for a side, any
/// species.
pub(super) fn catching_tutorial_candidates(
    rom: &GbaRom,
    entry: &RomEntry,
    family: Family,
    stats_offset: usize,
    dex_to_internal: &[u16],
) -> (Vec<DexNumber>, Vec<DexNumber>) {
    let species = || (1..dex_to_internal.len()).filter(|&n| dex_to_internal[n] != 0);
    let dex = |n: usize| DexNumber::try_from(n).ok();
    let opponents = species()
        .filter(|&n| {
            if positive(entry, "CatchingTutorialOpponentMonOffset").is_none() {
                return true;
            }
            let internal = dex_to_internal[n];
            let gender_ratio = rom
                .slice(
                    stats_offset + usize::from(internal) * c::BASE_STATS_ENTRY_SIZE + c::BS_GENDER_RATIO,
                    1,
                )
                .map_or(u8::MAX, |b| b[0]);
            let fits = match family {
                Family::FireRedLeafGreen => internal <= 0xFF,
                _ => internal <= 510,
            };
            gender_ratio <= MOST_FEMALE && fits
        })
        .filter_map(dex)
        .collect();
    let players = species()
        .filter(|&n| dex_to_internal[n] <= 510)
        .filter_map(dex)
        .collect();
    (opponents, players)
}

/// Thumb instructions that load `internal` into r1: `mov` of up to 255, then `add` of the rest or a `nop`.
fn load_r1(internal: u16) -> [u8; 4] {
    match u8::try_from(internal) {
        Ok(small) => [small, MOV_R1, NOP[0], NOP[1]],
        Err(_) => [0xFF, MOV_R1, u8::try_from(internal - 0xFF).unwrap_or(u8::MAX), ADD_R1],
    }
}

/// Sets the Pokémon of the catching tutorial, for species the candidates allow.
pub(super) fn write_catching_tutorial(
    rom: &mut GbaRom,
    entry: &RomEntry,
    family: Family,
    opponent: u16,
    player: u16,
) -> Result<(), RomError> {
    if let Some(at) = positive(entry, "CatchingTutorialOpponentMonOffset") {
        if family == Family::FireRedLeafGreen {
            rom.write(at, &[u8::try_from(opponent).unwrap_or(u8::MAX), MOV_R1])?;
        } else {
            rom.write(at, &load_r1(opponent))?;
        }
    }
    if let Some(at) = positive(entry, "CatchingTutorialPlayerMonOffset") {
        rom.write(at, &load_r1(player))?;
    }
    Ok(())
}

/// Sets the item that the player's PC starts with in place of the Potion.
pub(super) fn write_pc_potion(rom: &mut GbaRom, entry: &RomEntry, item: ItemId) -> Result<(), RomError> {
    let Some(at) = entry
        .int("PCPotionOffset")
        .and_then(|v| usize::try_from(v).ok())
        .filter(|&v| v != 0)
    else {
        return Ok(());
    };
    let internal = items::internal(item).ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {item}")))?;
    rom.write_u16(at, internal)
}

/// The ROM side of one misc tweak; tweaks that change game data are done by the randomizer.
pub(super) fn apply(
    space: &mut Space<'_>,
    entry: &RomEntry,
    family: Family,
    species_slots: &[usize],
    tweak: MiscTweak,
) -> Result<(), RomError> {
    let rom = &mut *space.rom;
    match tweak {
        MiscTweak::RunningShoesIndoors => {
            if let Some(at) = positive(entry, "RunIndoorsTweakOffset") {
                rom.write(at, &[0])?;
            }
        }
        MiscTweak::FastestText => {
            if let Some(name) = entry.value("InstantTextTweak") {
                ips::named(name)?.apply(rom.bytes_mut())?;
            } else if let Some(at) = positive(entry, "TextSpeedValuesOffset") {
                rom.write(at, &TEXT_SPEEDS)?;
            }
        }
        MiscTweak::RunWithoutRunningShoes => {
            if let Some(at) = found_at(rom.find(&hex(running_shoes_check(family))?)) {
                rom.write_u16(at + 0x12, 0)?;
            }
        }
        MiscTweak::ReusableTms => {
            // The game uses up a TM when its item number is below HM01's; compare with 0 instead.
            let hm01 = items::internal(items::HM01).ok_or_else(|| RomError::Malformed("no HM01".into()))?;
            let expected = u8::try_from(hm01 / 2).unwrap_or(u8::MAX);
            patch_bytes(rom, &offset_list(entry, "TMMovesReusableFunctionOffsets"), expected)?;
        }
        MiscTweak::ForgettableHms => {
            // The checks for moves that can't be forgotten answer "no" instead of "yes".
            patch_bytes(rom, &offset_list(entry, "HMMovesForgettableFunctionOffsets"), 1)?;
        }
        MiscTweak::FastEggHatching => {
            for &slot in species_slots {
                rom.write(slot + BS_EGG_CYCLES, &[0])?;
            }
        }
        MiscTweak::NoEvYields => {
            for &slot in species_slots {
                rom.write_u16(slot + BS_EV_YIELD, 0)?;
            }
        }
        MiscTweak::NationalDexAtStart => national_dex(space, family)?,
        _ => {}
    }
    Ok(())
}

/// Sets the byte at each offset to 0, after checking it holds `expected` (else the offsets are wrong).
fn patch_bytes(rom: &mut GbaRom, offsets: &[usize], expected: u8) -> Result<(), RomError> {
    for &at in offsets {
        let found = rom.slice(at, 1)?[0];
        if found != expected {
            return Err(RomError::Malformed(format!(
                "expected {expected:#04x} at {at:#x}, found {found:#04x}: the ROM entry's tweak offsets are wrong"
            )));
        }
    }
    for &at in offsets {
        rom.write(at, &[0])?;
    }
    Ok(())
}

/// The bytes of a pointer to `offset`.
fn pointer_bytes(offset: usize) -> Vec<u8> {
    u32::try_from(offset + 0x0800_0000).unwrap_or(0).to_le_bytes().to_vec()
}

fn nat_dex_failed(why: &str) -> RomError {
    RomError::Malformed(format!("Patch for National Dex at Start of Game unsuccessful: {why}"))
}

/// Makes the National Dex available from the start.
fn national_dex(space: &mut Space<'_>, family: Family) -> Result<(), RomError> {
    match family {
        Family::RubySapphire | Family::Emerald => {
            let (identifier, length, part1, part2) = if family == Family::Emerald {
                (
                    "3229610825F00129E40816CD40010003",
                    27,
                    "31720167",
                    "3229610825F00129E40825F30116CD40010003",
                )
            } else {
                (
                    "326629010803",
                    44,
                    "31720167",
                    "32662901082B00801102006B02021103016B020211DABE4E020211675A6A02022A008003",
                )
            };
            let rom = &*space.rom;
            let script = found_at(rom.find(&hex(identifier)?)).ok_or_else(|| nat_dex_failed("no script"))?;
            let text = rom.pointer(script - 4)?;
            let pointer = found_at(rom.find(&pointer_bytes(script - 8)))
                .ok_or_else(|| nat_dex_failed("no pointer to the script"))?;
            let at = space.find_and_unfree(length)?;
            let rom = &mut *space.rom;
            rom.write_pointer(pointer, at)?;
            rom.write(at, &hex(part1)?)?;
            rom.write_pointer(at + 4, text)?;
            rom.write(at + 8, &hex(part2)?)
        }
        Family::FireRedLeafGreen => {
            let script = found_at(space.rom.find(&hex("292908258101")?)).ok_or_else(|| nat_dex_failed("no script"))?;
            let at = space.find_and_unfree(10)?;
            let rom = &mut *space.rom;
            // Call the new routine in place of the old one.
            rom.write(script, &[4])?;
            rom.write_pointer(script + 1, at)?;
            rom.write(script + 5, &[0])?;
            rom.write(at, &hex("292908258101256F0103")?)?;
            // People who checked for the National Dex check for beating the Elite Four once.
            let checker = hex("260D809301210D800100")?;
            let checks = rom.search(0, rom.bytes().len(), &checker);
            let e4 = hex("2B2C0800000000000000")?;
            for check in checks {
                rom.write(check, &e4)?;
            }
            // Oak in his lab and outside the player's house count the National Dex.
            if let Some(oak) = found_at(rom.find(&hex("257D011604800000260D80D400")?)) {
                rom.write(oak, &hex("257D011604800100")?)?;
            }
            if let Some(oak) = found_at(rom.find(&hex(
                "1604800000260D80D4001908800580190980068083000880830109802109803C",
            )?)) {
                rom.write(oak, &hex("1604800100")?)?;
            }
            // Oak's aides always look at the National Dex: their `bne` becomes a plain branch.
            let prefix = hex("00B5064800880028")?;
            if let Some(aide) = found_at(rom.find(&prefix)) {
                rom.write(aide + prefix.len() + 1, &[0xE0])?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_load_in_one_or_two_instructions() {
        assert_eq!(load_r1(25), [25, 0x21, 0xC0, 0x46]);
        assert_eq!(load_r1(300), [0xFF, 0x21, 45, 0x31]);
        assert_eq!(pointer_bytes(0x12_3456), [0x56, 0x34, 0x12, 0x08]);
    }
}
