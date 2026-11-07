//! Static encounters: those in scripts (`StaticPokemon{}`), the Foongus and Amoonguss posing as items (species in
//! scripts, levels in map files), the hidden hollows of Black 2 and White 2 (`HiddenHollows`), and the roamers of
//! Black and White (roamer overlay and scripts). Also the fix for the box legendary of Black and White and the music
//! fix for special statics.
//!
//! Hidden hollow encounters may only become species from before Unova or the Unova species the hidden hollows hold
//! in the original games ([`HOLLOW_UNOVA_SPECIES`]), and no alternate formes.
//!
//! Deliberate choices:
//! - A static's species is its alternate forme's own number (from 650) for a forme with data of its own. Linked
//!   hidden hollow groups keep the forme they were read with while the encounter keeps its species and forme.
//! - Hidden hollow Pokémon of the same species in one rarity are one encounter, the others linked to the first, and
//!   each is written to the group it was read from. Their gender byte changes only with their species, and their
//!   maximum level keeps its ratio to the level.
//! - The roamer patch of Black and White (the game computes Thundurus from Tornadus) is applied when statics are
//!   written, not on loading; the roamers are read as if it were applied.

use ppr_model::{DexNumber, StaticEncounter};

use super::entry::{Family, Roamer, StaticSpec, file_path, indices, int_or_zero, need};
use super::species::GENDER_RATIO;
use super::store::{Store, byte, find_after, put_byte, put_long, put_word, word};
use crate::entries::RomEntry;
use crate::{RomError, ips};

const THUNDURUS: u32 = 642;
/// Hidden hollows: bytes per version and per rarity, the groups of a rarity, and where the fields start.
const HOLLOW_VERSION: usize = 78;
const HOLLOW_RARITY: usize = 26;
const HOLLOW_RARITIES: usize = 3;
const HOLLOW_GROUPS: usize = 4;
const HOLLOW_MAX_LEVEL: usize = 8;
const HOLLOW_LEVEL: usize = 12;
const HOLLOW_GENDER: usize = 16;
const HOLLOW_FORME: usize = 20;

/// The last species from before Unova, all of which hidden hollows may hold.
const LAST_BEFORE_UNOVA: DexNumber = 493;
/// The Unova species hidden hollows may hold.
const HOLLOW_UNOVA_SPECIES: [DexNumber; 53] = [
    505, 507, 510, 511, 513, 515, 519, 523, 525, 527, 529, 531, 533, 535, 538, 539, 542, 545, 546, 548, 550, 553, 556,
    558, 559, 561, 564, 569, 572, 575, 578, 580, 583, 587, 588, 594, 596, 601, 605, 607, 610, 613, 616, 618, 619, 621,
    622, 624, 626, 628, 630, 631, 632,
];

/// The species hidden hollow encounters may become.
fn hollow_species() -> Vec<DexNumber> {
    (1..=LAST_BEFORE_UNOVA).chain(HOLLOW_UNOVA_SPECIES).collect()
}

const BLACK_BOX_LEGENDARY_PREFIX_1: &str = "79F6BAEF07B0F0BDC046";
const BLACK_BOX_LEGENDARY_PREFIX_2: &str = "DEDB0020C04302B0F8BDC046";
const WHITE_BOX_LEGENDARY_PREFIX_1: &str = "00F0FEF8002070BD";
const WHITE_BOX_LEGENDARY_PREFIX_2: &str = "64F62EF970BD0000";

/// One hidden hollow encounter: the groups of one rarity that hold its species, the first being its own.
#[derive(Debug, Clone)]
struct Hollow {
    file: usize,
    base: usize,
    groups: Vec<usize>,
    levels: Vec<u8>,
    max_levels: Vec<u8>,
    formes: Vec<u8>,
}

/// Where the statics are kept, in the order of the game data.
#[derive(Debug, Clone, Default)]
pub(super) struct Statics {
    regular: Vec<StaticSpec>,
    fake_ball: Vec<StaticSpec>,
    hollows: Vec<Hollow>,
    roamers: Vec<Roamer>,
}

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// The roamer overlay with the roamer patch applied when it is needed.
fn patch_roamers(entry: &RomEntry, overlay: &mut [u8], roamers: &[Roamer]) -> Result<(), RomError> {
    let Some(&first) = roamers.first().and_then(|r| r.species.first()) else {
        return Ok(());
    };
    // Unpatched, the first species offset holds an instruction (`mov r0, #0x2`), far above any species.
    // As the original: above the species and Black and White's 18 alternate formes, counting from 0.
    if usize::from(word(overlay, first)?) <= usize::from(super::species::SPECIES_COUNT) + 18 + 1 {
        return Ok(());
    }
    // The function gives 0 for Thundurus, 1 for Tornadus and 2 for others, which never happens: it returns right away
    // for others, and loads Thundurus's number from where the 2 was instead of adding 1 to Tornadus's.
    let at = need(entry, "GetRoamerFlagOffsetStartOffset")?;
    put_byte(overlay, at + 10, 0)?;
    put_long(overlay, at + 20, THUNDURUS)?;
    put_byte(overlay, at + 6, 0x03)?;
    put_byte(overlay, at + 7, 0x49)
}

/// The hidden hollow encounters of Black 2 and White 2, those of a species in one rarity linked together.
fn read_hollows(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
) -> Result<(Vec<StaticEncounter>, Vec<Hollow>), RomError> {
    let path = file_path(entry, "HiddenHollows")?;
    let version = int_or_zero(entry, "HiddenHollowIndex");
    let files = store.narc(&path)?.files.clone();
    let (mut out, mut hollows): (Vec<StaticEncounter>, Vec<Hollow>) = (Vec::new(), Vec::new());
    let allowed = hollow_species();
    for (file, data) in files.iter().enumerate() {
        for rarity in 0..HOLLOW_RARITIES {
            let base = version * HOLLOW_VERSION + rarity * HOLLOW_RARITY;
            let mut in_rarity: Vec<(DexNumber, usize)> = Vec::new();
            for group in 0..HOLLOW_GROUPS {
                let species = word(data, base + group * 2)?;
                let level = byte(data, base + HOLLOW_LEVEL + group)?;
                let max_level = byte(data, base + HOLLOW_MAX_LEVEL + group)?;
                let forme = byte(data, base + HOLLOW_FORME + group)?;
                if let Some(&(_, h)) = in_rarity.iter().find(|(s, _)| *s == species) {
                    let hollow = &mut hollows[h];
                    hollow.groups.push(group);
                    hollow.levels.push(level);
                    hollow.max_levels.push(max_level);
                    hollow.formes.push(forme);
                    out[h].linked_levels.push(level);
                    continue;
                }
                in_rarity.push((species, hollows.len()));
                hollows.push(Hollow {
                    file,
                    base,
                    groups: vec![group],
                    levels: vec![level],
                    max_levels: vec![max_level],
                    formes: vec![forme],
                });
                out.push(StaticEncounter {
                    forme,
                    allowed_species: Some(allowed.clone()),
                    ..StaticEncounter::new(super::forme_species(family, species, forme), level)
                });
            }
        }
    }
    Ok((out, hollows))
}

/// Every static encounter.
pub(super) fn read(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    scripts: &str,
) -> Result<(Vec<StaticEncounter>, Statics), RomError> {
    let mut out = Vec::new();
    let mut statics = Statics::default();
    if entry.int("StaticPokemonSupport") != Some(1) {
        return Ok((out, statics));
    }
    let eggs = indices(entry, "StaticEggPokemonOffsets");
    statics.regular = entry
        .list("StaticPokemon{}")
        .iter()
        .map(|s| StaticSpec::parse(s))
        .collect();
    statics.fake_ball = entry
        .list("StaticPokemonFakeBall{}")
        .iter()
        .map(|s| StaticSpec::parse(s))
        .collect();
    for (i, spec) in statics.regular.iter().enumerate() {
        let first = spec
            .species
            .first()
            .ok_or_else(|| malformed("a static without species"))?;
        let species = word(store.narc_file(scripts, first.file)?, first.offset)?;
        let forme = match spec.formes.first() {
            Some(f) => byte(store.narc_file(scripts, f.file)?, f.offset)?,
            None => 0,
        };
        let mut levels = Vec::new();
        for l in &spec.levels {
            levels.push(byte(store.narc_file(scripts, l.file)?, l.offset)?);
        }
        out.push(StaticEncounter {
            egg: eggs.contains(&i),
            linked_levels: levels.iter().skip(1).copied().collect(),
            forme,
            ..StaticEncounter::new(
                super::forme_species(family, species, forme),
                levels.first().copied().unwrap_or(1),
            )
        });
    }
    if !statics.fake_ball.is_empty() {
        let maps = file_path(entry, "MapFiles")?;
        for spec in &statics.fake_ball {
            let first = spec
                .species
                .first()
                .ok_or_else(|| malformed("a static without species"))?;
            let species = word(store.narc_file(scripts, first.file)?, first.offset)?;
            let mut levels = Vec::new();
            for l in &spec.levels {
                levels.push(byte(store.narc_file(&maps, l.file)?, l.offset)?);
            }
            out.push(StaticEncounter {
                linked_levels: levels.iter().skip(1).copied().collect(),
                ..StaticEncounter::new(species, levels.first().copied().unwrap_or(1))
            });
        }
    }
    if family == Family::Bw2 {
        let (encounters, hollows) = read_hollows(store, entry, family)?;
        out.extend(encounters);
        statics.hollows = hollows;
    }
    statics.roamers = entry
        .list("RoamingPokemon{}")
        .iter()
        .map(|s| Roamer::parse(s))
        .collect();
    if !statics.roamers.is_empty() {
        let mut overlay = store.overlay(need(entry, "RoamerOvlNumber")?)?.to_vec();
        patch_roamers(entry, &mut overlay, &statics.roamers)?;
        for r in &statics.roamers {
            let at = *r.species.first().ok_or_else(|| malformed("a roamer without species"))?;
            let level = match r.levels.first() {
                Some(&l) => byte(&overlay, l)?,
                None => 1,
            };
            out.push(StaticEncounter::new(word(&overlay, at)?, level));
        }
    }
    Ok((out, statics))
}

/// The gender byte of a hidden hollow Pokémon of a species with this gender ratio: always female, never female, or
/// 30% female.
fn hollow_gender(ratio: u8) -> u8 {
    match ratio {
        0xFE => 100,
        0x00 | 0xFF => 0,
        _ => 30,
    }
}

/// The maximum level for a new level, keeping its ratio to the old level.
fn scaled_max(max: u8, old_level: u8, level: u8) -> u8 {
    if old_level == level || old_level == 0 {
        return max;
    }
    let scaled = (u32::from(max) * u32::from(level) + u32::from(old_level) / 2) / u32::from(old_level);
    u8::try_from(scaled.clamp(u32::from(level), 100)).unwrap_or(100)
}

/// Writes the statics that changed, and the box legendary fix of Black and White when its static changed.
#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the parts of the game the statics are spread over"
)]
pub(super) fn write(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    scripts: &str,
    stats: &str,
    statics: &Statics,
    old: &[StaticEncounter],
    new: &[StaticEncounter],
) -> Result<(), RomError> {
    if new.len() != old.len() {
        return Err(malformed("the statics must keep their number"));
    }
    let mut i = 0;
    for spec in &statics.regular {
        let (s, before) = (&new[i], &old[i]);
        i += 1;
        if s == before {
            continue;
        }
        for at in &spec.species {
            put_word(
                store.narc_file_mut(scripts, at.file)?,
                at.offset,
                super::base_species(s.species),
            )?;
        }
        for at in &spec.formes {
            put_byte(store.narc_file_mut(scripts, at.file)?, at.offset, s.forme)?;
        }
        write_levels(store, scripts, &spec.levels, s)?;
    }
    if !statics.fake_ball.is_empty() {
        let maps = file_path(entry, "MapFiles")?;
        for spec in &statics.fake_ball {
            let (s, before) = (&new[i], &old[i]);
            i += 1;
            if s == before {
                continue;
            }
            for at in &spec.species {
                put_word(
                    store.narc_file_mut(scripts, at.file)?,
                    at.offset,
                    super::base_species(s.species),
                )?;
            }
            write_levels(store, &maps, &spec.levels, s)?;
        }
    }
    if !statics.hollows.is_empty() {
        let path = file_path(entry, "HiddenHollows")?;
        for h in &statics.hollows {
            let (s, before) = (&new[i], &old[i]);
            i += 1;
            if s == before {
                continue;
            }
            let new_species = s.species != before.species;
            let new_forme = new_species || s.forme != before.forme;
            let base = super::base_species(s.species);
            let gender = hollow_gender(byte(store.narc_file(stats, usize::from(base))?, GENDER_RATIO)?);
            let data = store.narc_file_mut(&path, h.file)?;
            for (j, &group) in h.groups.iter().enumerate() {
                let level = if j == 0 {
                    s.level
                } else {
                    s.linked_levels.get(j - 1).copied().unwrap_or(h.levels[j])
                };
                put_word(data, h.base + group * 2, base)?;
                if new_species {
                    put_byte(data, h.base + HOLLOW_GENDER + group, gender)?;
                }
                put_byte(
                    data,
                    h.base + HOLLOW_FORME + group,
                    if new_forme { s.forme } else { h.formes[j] },
                )?;
                put_byte(data, h.base + HOLLOW_LEVEL + group, level)?;
                put_byte(
                    data,
                    h.base + HOLLOW_MAX_LEVEL + group,
                    scaled_max(h.max_levels[j], h.levels[j], level),
                )?;
            }
        }
    }
    if !statics.roamers.is_empty() && new[i..] != old[i..] {
        let number = need(entry, "RoamerOvlNumber")?;
        let overlay = store.overlay_mut(number)?;
        patch_roamers(entry, overlay, &statics.roamers)?;
        for (r, s) in statics.roamers.iter().zip(&new[i..]) {
            let overlay = store.overlay_mut(number)?;
            for &at in &r.species {
                put_word(overlay, at, super::base_species(s.species))?;
            }
            for &at in &r.levels {
                put_byte(overlay, at, s.level)?;
            }
            for at in &r.scripts {
                put_word(
                    store.narc_file_mut(scripts, at.file)?,
                    at.offset,
                    super::base_species(s.species),
                )?;
            }
        }
    }
    if family == Family::Bw1
        && let Some(box_index) = entry.int("BoxLegendaryOffset").and_then(|v| usize::try_from(v).ok())
        && let (Some(s), Some(before)) = (new.get(box_index), old.get(box_index))
        && s.species != before.species
    {
        fix_box_legendary(store, entry, super::base_species(s.species))?;
    }
    Ok(())
}

fn write_levels(
    store: &mut Store,
    path: &str,
    levels: &[super::entry::InFile],
    s: &StaticEncounter,
) -> Result<(), RomError> {
    for (j, at) in levels.iter().enumerate() {
        let level = if j == 0 {
            Some(s.level)
        } else {
            s.linked_levels.get(j - 1).copied()
        };
        if let Some(level) = level {
            put_byte(store.narc_file_mut(path, at.file)?, at.offset, level)?;
        }
    }
    Ok(())
}

/// Black and White check for Reshiram or Zekrom by number to move it from the box into the party; the checks get
/// the species of its static instead.
fn fix_box_legendary(store: &mut Store, entry: &RomEntry, species: DexNumber) -> Result<(), RomError> {
    let overlay = store.overlay_mut(need(entry, "FieldOvlNumber")?)?;
    let value = u32::from(species);
    if entry.int("IsBlack") == Some(1) {
        // Reshiram's number is loaded from two constants.
        for prefix in [BLACK_BOX_LEGENDARY_PREFIX_1, BLACK_BOX_LEGENDARY_PREFIX_2] {
            if let Some(at) = find_after(overlay, prefix)? {
                put_long(overlay, at, value)?;
            }
        }
        return Ok(());
    }
    // Zekrom's number is computed (161 shifted left by 2); load it from a constant instead.
    if let Some(at) = find_after(overlay, WHITE_BOX_LEGENDARY_PREFIX_1)? {
        // The debug string pointer load goes; its constant holds the species.
        overlay[at + 66] = 0;
        overlay[at + 67] = 0;
        put_long(overlay, at + 320, value)?;
        overlay[at + 18] = 0;
        overlay[at + 19] = 0;
        overlay[at + 26] = 0x49;
        overlay[at + 27] = 0x49;
    }
    if let Some(at) = find_after(overlay, WHITE_BOX_LEGENDARY_PREFIX_2)? {
        // `mov r4, #0` before `add r6, r0, #0`, so the flags stay as the call left them; its constant holds the
        // species.
        overlay[at + 502] = 0x00;
        overlay[at + 503] = 0x24;
        overlay[at + 504] = 0x06;
        overlay[at + 505] = 0x1C;
        put_long(overlay, at + 556, value)?;
        overlay[at + 78] = 0x77;
        overlay[at + 79] = 0x49;
        overlay[at + 80] = 0x00;
        overlay[at + 81] = 0x00;
    }
    Ok(())
}

/// Gives the music of replaced special statics to their replacements: a table in the extended ARM9 (patched in, with
/// the field overlay) maps species to music. `changes` maps old numbers to new ones.
pub(super) fn correct_music(
    store: &mut Store,
    entry: &RomEntry,
    family: Family,
    changes: &[(u16, u16)],
    special: &[u16],
) -> Result<(), RomError> {
    let (Some(arm9_patch), Some(overlay_patch)) = (
        entry.value("NewIndexToMusicTweak"),
        entry.value("NewIndexToMusicOvlTweak"),
    ) else {
        return Ok(());
    };
    let field = need(entry, "FieldOvlNumber")?;
    ips::named(overlay_patch)?.apply(store.overlay_mut(field)?)?;
    let arm9 = store.arm9_mut()?;
    ips::named(arm9_patch)?.apply(arm9)?;
    let prefix = entry.value("NewIndexToMusicPrefix").unwrap_or_default();
    let pool = find_after(arm9, prefix)?.ok_or_else(|| malformed("the music table is not in the ARM9"))?;
    let mut replaced: Vec<usize> = Vec::new();
    let mut last = None;
    for &(old, new) in changes {
        let mut at = pool;
        while word(arm9, at)? != old || replaced.contains(&at) {
            at += 4;
        }
        // Kyurem's formes need a check of the forme removed.
        if family == Family::Bw2 && old > super::species::SPECIES_COUNT {
            put_word(arm9, at - 0xFE, 0)?;
            put_word(arm9, at - 0xFC, 0)?;
            put_word(arm9, at - 0xFA, 0)?;
            put_word(arm9, at - 0xF8, 0x4290)?;
        }
        put_word(arm9, at, new)?;
        replaced.push(at);
        last = last.max(Some(at));
    }
    // The other entries up to the last replaced one belong to statics that are gone, unless they are still there.
    if let Some(last) = last {
        for at in (pool..=last).step_by(4) {
            if !replaced.contains(&at) && !special.contains(&word(arm9, at)?) {
                put_word(arm9, at, 0)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hollow_levels_and_genders_follow_the_species() {
        assert_eq!(scaled_max(25, 20, 20), 25, "unchanged");
        assert_eq!(scaled_max(25, 20, 40), 50);
        assert_eq!(scaled_max(90, 60, 80), 100, "at most 100");
        assert_eq!(scaled_max(20, 20, 10), 10);
        assert_eq!(
            (
                hollow_gender(0xFE),
                hollow_gender(0),
                hollow_gender(0xFF),
                hollow_gender(127)
            ),
            (100, 0, 0, 30)
        );
    }

    #[test]
    fn hollows_allow_older_species_and_some_from_unova() {
        let allowed = hollow_species();
        assert_eq!(allowed.len(), 493 + 53);
        assert!(allowed.is_sorted());
        assert!(allowed.contains(&1) && allowed.contains(&493) && allowed.contains(&505) && allowed.contains(&632));
        assert!(!allowed.contains(&494) && !allowed.contains(&506) && !allowed.contains(&633));
        assert!(
            !allowed.contains(&649) && !allowed.contains(&650),
            "no legendaries of Unova, no alternate formes"
        );
    }
}
