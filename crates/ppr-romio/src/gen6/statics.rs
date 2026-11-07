//! Starters, gifts, static encounters and the trash cans of X and Y.
//!
//! Gifts and static encounters are tables in the field module; the starters are gifts the entry names, also shown
//! by the starter selection module. Encounters that must stay the same species are read as one encounter with
//! linked levels: those the entry links, and the trash cans of one place that hold the same species.
//!
//! Deliberate choices:
//! - An encounter's species is its alternate forme's own number (from 722) for a forme with data of its own. The
//!   forme markers 30 and 31, and formes the species doesn't have, are read as forme 0; a slot keeps its forme as
//!   stored while its species and forme stay. A starter's forme is that of its species.
//! - Gifts and static encounters hold their item (none for 0 and -1); an item is written only when it changed,
//!   none as -1.
//! - Trash can encounters linked by species are written back to the cans they were read from.
//! - The box legendary, roamer and Rayquaza fixes are applied when the static encounters change.

use ppr_model::{DexNumber, ItemId, StaticEncounter};

use super::amx::Amx;
use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom, find_after};
use super::species::FormeTable;
use crate::RomError;
use crate::entries::RomEntry;

/// Where one encounter is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    /// A static encounter in the field module: species, forme at +2, level at +3, held item at +4 (16 bits).
    Static(usize),
    /// A gift in the field module: species, forme at +4, level at +5, held item at +12 (32 bits).
    Gift(usize),
    /// A trash can encounter in the executable: species, forme and level as 32-bit words.
    Trash(usize),
}

/// One stored encounter: its species, forme and held item as read, and its forme as stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Slot {
    pub(crate) place: Place,
    pub(crate) species: DexNumber,
    pub(crate) forme: u8,
    pub(crate) stored: u32,
    pub(crate) item: Option<ItemId>,
}

/// A held item as stored: 0 and negative numbers are none.
fn held_item(raw: i32) -> Option<ItemId> {
    ItemId::try_from(raw).ok().filter(|&id| id != 0)
}

/// The slots of the static encounters in game-data order: each encounter's own slot, then its linked ones.
#[derive(Debug, Clone, Default)]
pub(crate) struct Statics {
    pub(crate) slots: Vec<Vec<Slot>>,
    /// For each encounter before linking (the order the entry's indices count in), the encounter it became part of.
    pub(crate) unlinked: Vec<usize>,
}

/// The field module offsets and sizes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Tables {
    pub(crate) statics: usize,
    pub(crate) gifts: usize,
    pub(crate) gift_size: usize,
}

impl Tables {
    pub(crate) fn new(entry: &RomEntry, family: Family) -> Result<Self, RomError> {
        let need = |key: &str| {
            entry
                .int(key)
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| RomError::Malformed(format!("ROM entry has no {key}")))
        };
        Ok(Self {
            statics: need("StaticPokemonOffset")?,
            gifts: need("GiftPokemonOffset")?,
            gift_size: family.gift_size(),
        })
    }

    fn at(&self, place: Place) -> usize {
        match place {
            Place::Static(i) => self.statics + i * 0xC,
            Place::Gift(i) => self.gifts + i * self.gift_size,
            Place::Trash(at) => at,
        }
    }
}

/// The entry's integer array, as indices.
pub(crate) fn indices(entry: &RomEntry, key: &str) -> Vec<usize> {
    entry
        .array(key)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|v| usize::try_from(v).ok())
        .collect()
}

/// `LinkedStaticEncounterOffsets=[1:3, 2:12]`: (base, linked) pairs.
pub(crate) fn linked_pairs(entry: &RomEntry) -> Vec<(usize, usize)> {
    let raw = entry.value("LinkedStaticEncounterOffsets").unwrap_or_default();
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .filter_map(|pair| {
            let (base, linked) = pair.split_once(':')?;
            Some((base.trim().parse().ok()?, linked.trim().parse().ok()?))
        })
        .collect()
}

fn read_slot(
    field: &[u8],
    code: &[u8],
    tables: &Tables,
    formes: &FormeTable,
    place: Place,
) -> Result<(Slot, u8), RomError> {
    let at = tables.at(place);
    let (base, stored, level) = match place {
        Place::Static(_) => (field.u16_at(at)?, u32::from(field.u8_at(at + 2)?), field.u8_at(at + 3)?),
        Place::Gift(_) => (field.u16_at(at)?, u32::from(field.u8_at(at + 4)?), field.u8_at(at + 5)?),
        Place::Trash(_) => (
            u16::try_from(code.u32_at(at)?).unwrap_or(0),
            code.u32_at(at + 4)?,
            u8::try_from(code.u32_at(at + 8)?).unwrap_or(u8::MAX),
        ),
    };
    let forme = formes.holder_forme(base, stored);
    let species = formes.species(base, forme);
    let item = match place {
        Place::Static(_) => held_item(i32::from(field.u16_at(at + 4)?.cast_signed())),
        Place::Gift(_) => held_item(field.u32_at(at + 12)?.cast_signed()),
        Place::Trash(_) => None,
    };
    Ok((
        Slot {
            place,
            species,
            forme,
            stored,
            item,
        },
        level,
    ))
}

/// The starters, their held items and where they are.
pub(crate) type Starters = (Vec<DexNumber>, Vec<Option<ItemId>>, Vec<Slot>);

/// Reads the starters (the gifts the entry names) and their held items.
pub(crate) fn read_starters(
    field: &[u8],
    tables: &Tables,
    formes: &FormeTable,
    entry: &RomEntry,
    family: Family,
) -> Result<Starters, RomError> {
    let starters = indices(entry, "StarterIndices");
    let mut species = Vec::new();
    let mut items = Vec::new();
    let mut slots = Vec::new();
    for i in (0..family.gift_count()).filter(|i| starters.contains(i)) {
        let (slot, _) = read_slot(field, &[], tables, formes, Place::Gift(i))?;
        species.push(slot.species);
        slots.push(slot);
        let item = field.u32_at(tables.at(Place::Gift(i)) + 12)?;
        items.push(u16::try_from(item).ok().filter(|&id| id != 0));
    }
    Ok((species, items, slots))
}

/// Reads the static encounters, gifts and trash cans, with linked encounters merged.
pub(crate) fn read(
    field: &[u8],
    code: &[u8],
    tables: &Tables,
    formes: &FormeTable,
    entry: &RomEntry,
    family: Family,
) -> Result<(Vec<StaticEncounter>, Statics), RomError> {
    let starters = indices(entry, "StarterIndices");
    // Each encounter: its slots (the first its own) and levels, and whether it is an egg.
    let mut list: Vec<(Vec<Slot>, Vec<u8>, bool)> = Vec::new();
    for i in 0..family.static_count() {
        let (slot, level) = read_slot(field, code, tables, formes, Place::Static(i))?;
        list.push((vec![slot], vec![level], false));
    }
    for i in (0..family.gift_count()).filter(|i| !starters.contains(i)) {
        let (slot, level) = read_slot(field, code, tables, formes, Place::Gift(i))?;
        let egg = family == Family::Oras && field.u16_at(tables.at(slot.place) + 18)? == 0xEA64;
        list.push((vec![slot], vec![level], egg));
    }
    if family == Family::Xy
        && let Some(table) = find_after(code, c::XY_TRASH_ENCOUNTERS_PREFIX)
    {
        for &(first, count, merge) in &c::XY_TRASH_GROUPS {
            let mut group: Vec<(Vec<Slot>, Vec<u8>, bool)> = Vec::new();
            for i in first..first + count {
                let place = Place::Trash(table + i * c::XY_TRASH_ENCOUNTER_LENGTH);
                let (slot, level) = read_slot(field, code, tables, formes, place)?;
                let same = group
                    .iter_mut()
                    .find(|(s, _, _)| merge && s[0].species == slot.species && s[0].forme == slot.forme);
                match same {
                    Some((slots, levels, _)) => {
                        slots.push(slot);
                        levels.push(level);
                    }
                    None => group.push((vec![slot], vec![level], false)),
                }
            }
            list.extend(group);
        }
    }
    let pairs = linked_pairs(entry);
    let mut owner: Vec<usize> = (0..list.len()).collect();
    for &(base, linked) in &pairs {
        if base >= list.len() || linked >= list.len() {
            return Err(RomError::Malformed(format!(
                "linked static encounter {base}:{linked} out of range"
            )));
        }
        let (slots, levels, _) = list[linked].clone();
        list[base].0.extend(slots);
        list[base].1.extend(levels);
        owner[linked] = base;
    }
    let removed: Vec<usize> = pairs.iter().map(|&(_, linked)| linked).collect();
    let kept: Vec<usize> = (0..list.len()).filter(|i| !removed.contains(i)).collect();
    let unlinked = owner
        .iter()
        .map(|o| kept.iter().position(|k| k == o).unwrap_or(0))
        .collect();
    let mut encounters = Vec::new();
    let mut slots = Vec::new();
    for &k in &kept {
        let (s, levels, egg) = &list[k];
        encounters.push(StaticEncounter {
            forme: s[0].forme,
            species: s[0].species,
            level: levels[0],
            egg: *egg,
            held_item: s[0].item,
            linked_levels: levels[1..].to_vec(),
            allowed_species: None,
        });
        slots.push(s.clone());
    }
    Ok((encounters, Statics { slots, unlinked }))
}

/// The forme to write for a slot: the stored one while the species and forme are the same.
fn forme_for(slot: &Slot, species: DexNumber, forme: u8) -> u32 {
    if (slot.species, slot.forme) == (species, forme) {
        slot.stored
    } else {
        u32::from(forme)
    }
}

/// Writes one slot of an encounter: `species` is the base species, `forme` its forme as stored.
fn write_slot(
    rom: &mut Rom,
    field_path: &str,
    tables: &Tables,
    slot: &Slot,
    species: DexNumber,
    forme: u32,
    level: u8,
) -> Result<(), RomError> {
    let at = tables.at(slot.place);
    match slot.place {
        Place::Static(_) => {
            let field = rom.file_mut(field_path)?;
            field.set_u16(at, species)?;
            field.set_u8(at + 2, u8::try_from(forme).unwrap_or(0))?;
            field.set_u8(at + 3, level)?;
        }
        Place::Gift(_) => {
            let field = rom.file_mut(field_path)?;
            field.set_u16(at, species)?;
            field.set_u8(at + 4, u8::try_from(forme).unwrap_or(0))?;
            field.set_u8(at + 5, level)?;
        }
        Place::Trash(_) => {
            let code = rom.code_mut();
            code.set_u32(at, u32::from(species))?;
            code.set_u32(at + 4, forme)?;
            code.set_u32(at + 8, u32::from(level))?;
        }
    }
    Ok(())
}

/// Writes the held item of a gift or static encounter, none as -1.
fn write_held_item(
    rom: &mut Rom,
    field_path: &str,
    tables: &Tables,
    place: Place,
    item: Option<ItemId>,
) -> Result<(), RomError> {
    let at = tables.at(place);
    match place {
        Place::Static(_) => rom.file_mut(field_path)?.set_u16(at + 4, item.unwrap_or(u16::MAX)),
        Place::Gift(_) => rom
            .file_mut(field_path)?
            .set_u32(at + 12, item.map_or(u32::MAX, u32::from)),
        Place::Trash(_) => Ok(()),
    }
}

/// The files and entry values the static encounter fixes need.
pub(crate) struct Files<'a> {
    pub(crate) field: &'a str,
    pub(crate) wild: &'a str,
    pub(crate) scripts: &'a str,
    pub(crate) entry: &'a RomEntry,
    pub(crate) family: Family,
    pub(crate) tables: Tables,
    pub(crate) formes: &'a FormeTable,
}

/// Writes the static encounters, then makes the scripts and code that name the box legendary, the roamers and
/// Rayquaza name the new species.
pub(crate) fn write(
    rom: &mut Rom,
    files: &Files<'_>,
    statics: &Statics,
    new: &[StaticEncounter],
) -> Result<(), RomError> {
    if new.len() != statics.slots.len() {
        return Err(RomError::Malformed(format!(
            "this game has {} static encounters, not {}",
            statics.slots.len(),
            new.len()
        )));
    }
    for (slots, st) in statics.slots.iter().zip(new) {
        for (i, slot) in slots.iter().enumerate() {
            let level = if i == 0 {
                st.level
            } else {
                st.linked_levels.get(i - 1).copied().unwrap_or(st.level)
            };
            let forme = forme_for(slot, st.species, st.forme);
            let base = files.formes.base(st.species);
            write_slot(rom, files.field, &files.tables, slot, base, forme, level)?;
            if st.held_item != slot.item {
                write_held_item(rom, files.field, &files.tables, slot.place, st.held_item)?;
            }
        }
    }
    let at = |index: usize| -> Result<&StaticEncounter, RomError> {
        statics
            .unlinked
            .get(index)
            .and_then(|&k| new.get(k))
            .ok_or_else(|| RomError::Malformed(format!("no static encounter {index}")))
    };
    match files.family {
        Family::Xy => {
            if let Some(&first) = indices(files.entry, "BoxLegendaryOffsets").first() {
                fix_box_legendary(rom, files, files.formes.base(at(first)?.species))?;
            }
            let roamers = indices(files.entry, "RoamingLegendaryOffsets")
                .into_iter()
                .map(|i| at(i).map(|s| (files.formes.base(s.species), s.level)))
                .collect::<Result<Vec<_>, _>>()?;
            set_roamers(rom, files, &roamers)?;
        }
        Family::Oras => {
            let index = files
                .entry
                .int("RayquazaEncounterNumber")
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| RomError::Malformed("no RayquazaEncounterNumber".into()))?;
            fix_rayquaza(rom, files, files.formes.base(at(index)?.species))?;
        }
    }
    Ok(())
}

/// Replaces the comparisons with a species (each followed by a nop) by `sub r0, r0, #high << 8` and
/// `subs r0, r0, #low`, which work for any species.
fn patch_species_checks(
    field: &mut [u8],
    function: usize,
    offsets: &[usize],
    species: DexNumber,
) -> Result<(), RomError> {
    let [low, high] = species.to_le_bytes();
    for &o in offsets {
        let at = function + o;
        let target = field
            .get_mut(at..at + 8)
            .ok_or_else(|| RomError::Malformed("species check past the end of the module".into()))?;
        target.copy_from_slice(&[high, 0x0C, 0x40, 0xE2, low, 0x00, 0x50, 0xE2]);
    }
    Ok(())
}

/// Writes `species` at `offsets` of script `n` of a file, and the packed script back at `script_at`.
fn patch_area_script(
    rom: &mut Rom,
    wild: &str,
    file: usize,
    script_at: usize,
    words: &[(usize, DexNumber)],
) -> Result<(), RomError> {
    let mut data = rom.garc_file(wild, true, file)?;
    let mut script = Amx::find(&data, 1)?;
    for &(o, species) in words {
        script.data.set_u16(o, species)?;
    }
    let packed = script.to_bytes()?;
    data.get_mut(script_at..script_at + packed.len())
        .ok_or_else(|| RomError::Malformed("an area script does not fit its file".into()))?
        .copy_from_slice(&packed);
    rom.set_garc_file(wild, true, file, data)
}

fn fix_box_legendary(rom: &mut Rom, files: &Files<'_>, species: DexNumber) -> Result<(), RomError> {
    // The room's script names the species.
    let words: Vec<(usize, DexNumber)> = indices(files.entry, "BoxLegendaryScriptOffsets")
        .into_iter()
        .map(|o| (o, species))
        .collect();
    patch_area_script(
        rom,
        files.wild,
        c::BOX_LEGENDARY_ENCOUNTER_FILE_XY,
        c::BOX_LEGENDARY_LOCAL_SCRIPT_OFFSET_XY,
        &words,
    )?;
    // The field module checks for the species.
    if let Some(function) = find_after(rom.file(files.field)?, c::BOX_LEGENDARY_FUNCTION_PREFIX_XY) {
        patch_species_checks(
            rom.file_mut(files.field)?,
            function,
            &c::BOX_LEGENDARY_CODE_OFFSETS_XY,
            species,
        )?;
    }
    Ok(())
}

fn set_roamers(rom: &mut Rom, files: &Files<'_>, roamers: &[(DexNumber, u8)]) -> Result<(), RomError> {
    let species_at = super::rom::find(rom.code(), c::XY_ROAMER_SPECIES_LOCATOR);
    let free = super::rom::find(rom.code(), c::XY_ROAMER_FREE_SPACE_POSTFIX);
    if let (Some(species_at), Some(free)) = (species_at, free) {
        // The free space is the five instructions before the pattern: nop, bl, nop, nop, b. Both branches move to
        // the top (their offsets grow by the words they moved) to leave three words for the species.
        let free = free
            .checked_sub(20)
            .ok_or_else(|| RomError::Malformed("roamer code at the start".into()))?;
        let code = rom.code_mut();
        if code.len() < free + 20 || code.len() < species_at + 12 * roamers.len() {
            return Err(RomError::Malformed("the roamer code is cut off".into()));
        }
        code[free] = code[free + 4].wrapping_add(1);
        code.copy_within(free + 5..free + 8, free + 1);
        code[free + 4] = code[free + 16].wrapping_add(3);
        code.copy_within(free + 17..free + 20, free + 5);
        for (i, &(species, _)) in roamers.iter().enumerate() {
            code.set_u32(free + 8 + i * 4, u32::from(species))?;
        }
        // The species was moved into r4 per starter choice; load it from the free space instead.
        for i in 0..roamers.len() {
            let at = species_at + i * 12;
            let offset = 0xACu8.wrapping_sub(u8::try_from(8 * i).unwrap_or(0));
            code[at..at + 4].copy_from_slice(&[offset, 0x41, 0x9F, 0xE5]);
        }
    }
    if let Some(&(_, level)) = roamers.first()
        && let Some(at) = find_after(rom.file(files.field)?, c::XY_ROAMER_LEVEL_PREFIX)
    {
        rom.file_mut(files.field)?.set_u8(at, level)?;
    }
    // The Sea Spirit's Den spawns the roamer as a static encounter.
    let words: Vec<(usize, DexNumber)> = c::SEA_SPIRITS_DEN_SCRIPT_OFFSETS_XY
        .iter()
        .zip(roamers)
        .map(|(&o, &(species, _))| (o, species))
        .collect();
    patch_area_script(
        rom,
        files.wild,
        c::SEA_SPIRITS_DEN_ENCOUNTER_FILE_XY,
        c::SEA_SPIRITS_DEN_LOCAL_SCRIPT_OFFSET_XY,
        &words,
    )
}

fn fix_rayquaza(rom: &mut Rom, files: &Files<'_>, species: DexNumber) -> Result<(), RomError> {
    let file = files
        .entry
        .int("RayquazaEncounterScriptNumber")
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed("no RayquazaEncounterScriptNumber".into()))?;
    let mut script = Amx::parse(&rom.garc_file(files.scripts, false, file)?)?;
    for &o in &c::RAYQUAZA_SCRIPT_OFFSETS_ORAS {
        script.data.set_u16(o, species)?;
    }
    rom.set_garc_file(files.scripts, false, file, script.to_bytes()?)?;
    if let Some(function) = find_after(rom.file(files.field)?, c::RAYQUAZA_FUNCTION_PREFIX_ORAS) {
        patch_species_checks(
            rom.file_mut(files.field)?,
            function,
            &c::RAYQUAZA_CODE_OFFSETS_ORAS,
            species,
        )?;
    }
    Ok(())
}

/// The starter selection module's table of the starters shown.
fn display_offset(display: &[u8], entry: &RomEntry) -> Result<usize, RomError> {
    let offset_at = entry
        .int("StarterOffsetOffset")
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed("no StarterOffsetOffset".into()))?;
    let extra = entry
        .int("StarterExtraOffset")
        .and_then(|v| usize::try_from(v).ok())
        .unwrap_or(0);
    Ok(usize::from(display.u16_at(offset_at)?) + extra)
}

/// Writes the starters as gifts and in the starter selection, and makes the texts that named them name the shown
/// Pokémon.
#[expect(clippy::too_many_arguments, reason = "the files the starters are in")]
pub(crate) fn write_starters(
    rom: &mut Rom,
    field: &str,
    display: &str,
    text: &str,
    entry: &RomEntry,
    tables: &Tables,
    formes: &FormeTable,
    slots: &[Slot],
    starters: &[DexNumber],
) -> Result<(), RomError> {
    let display_at = display_offset(rom.file(display)?, entry)?;
    let text_file = entry
        .int("StarterTextOffset")
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed("no StarterTextOffset".into()))?;
    let mut texts = rom.texts(text, text_file)?;
    let text_lines = indices(entry, "SpecificStarterTextOffsets");
    for (i, (slot, &species)) in slots.iter().zip(starters).enumerate() {
        let own = formes.formes.get(&usize::from(species)).map_or(0, |f| f.number);
        let forme = u8::try_from(forme_for(slot, species, own)).unwrap_or(0);
        let species = formes.base(species);
        let at = tables.at(slot.place);
        let f = rom.file_mut(field)?;
        f.set_u16(at, species)?;
        f.set_u8(at + 4, forme)?;
        let d = rom.file_mut(display)?;
        d.set_u16(display_at + i * 0x54, species)?;
        d.set_u8(display_at + i * 0x54 + 2, forme)?;
        if i < 3
            && let Some(line) = text_lines.get(i).and_then(|&l| texts.get_mut(l))
        {
            "[VAR PKNAME(0000)]".clone_into(line);
        }
    }
    rom.set_texts(text, text_file, &texts)
}

/// Writes the starters' held items; none is -1.
pub(crate) fn write_starter_items(
    rom: &mut Rom,
    field: &str,
    tables: &Tables,
    slots: &[Slot],
    items: &[Option<ItemId>],
) -> Result<(), RomError> {
    for (slot, item) in slots.iter().zip(items) {
        let value = item.map_or(u32::MAX, u32::from);
        rom.file_mut(field)?.set_u32(tables.at(slot.place) + 12, value)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn species_checks_take_any_species() {
        let mut field = vec![0u8; 16];
        patch_species_checks(&mut field, 0, &[4], 0x1AB).expect("fits");
        assert_eq!(&field[4..12], &[0x01, 0x0C, 0x40, 0xE2, 0xAB, 0x00, 0x50, 0xE2]);
        assert!(patch_species_checks(&mut field, 0, &[12], 1).is_err());
    }

    #[test]
    fn a_kept_species_keeps_its_forme() {
        let slot = Slot {
            place: Place::Static(0),
            species: 25,
            forme: 0,
            stored: 31,
            item: None,
        };
        assert_eq!(forme_for(&slot, 25, 0), 31);
        assert_eq!(forme_for(&slot, 25, 2), 2);
        assert_eq!(forme_for(&slot, 26, 0), 0);
    }

    #[test]
    fn no_item_is_zero_or_negative() {
        assert_eq!(held_item(0), None);
        assert_eq!(held_item(-1), None);
        assert_eq!(held_item(i32::from(0xFFFF_u16.cast_signed())), None);
        assert_eq!(held_item(673), Some(673));
    }
}
