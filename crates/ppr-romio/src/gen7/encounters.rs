//! Wild encounters (`WildPokemon`) and field items, with area names from the zone data (`ZoneData`, `WorldData`).
//!
//! The wild Pokémon archive has 11 files per area. File 9 of an area is an `EA` pack of encounter tables; each
//! table (after 4 bytes) has a day half and a night half of 0x164 bytes: the minimum and maximum level, 10 slots
//! (species in the low 11 bits, forme in the high 5) each followed by its 7 SOS slots, and 6 weather SOS slots.
//! File 0 of an area is an `ED` pack whose files 10 and 11 hold the field items and berry piles.
//!
//! Without time of day, the sets are the day halves, one per table, and writing puts a set's Pokémon into both
//! halves of its table. With time of day, each table is two sets in a row, its day half then its night half,
//! named with ` (Day)` and ` (Night)`, and each half is written from its own set. The location tags, kinds and
//! post-game marks of the two halves are those of their table without time of day.
//!
//! Deliberate choices:
//! - Only the tables whose sets changed are written, in either view, so an unchanged table keeps its night half
//!   even without time of day.
//! - The tags by time of day are not a list of their own: each table's tags are used for both of its halves,
//!   which gives the same tags as the original's list.
//! - A slot's species is its alternate forme's own entry for a forme with an entry; a forme the species doesn't
//!   have is read as forme 0. A slot keeps its stored forme while its species and forme stay.

use ppr_formats::ctr::{Decompression, Garc, mini};
use ppr_model::{DexNumber, EncounterKind, EncounterSet, ItemId, WildEncounter};

use super::constants::{self as c, Family};
use super::files::Files;
use super::personal::{FormeIndex, Personal};
use super::{put_u16, put_u32, word};
use crate::RomError;

const HALF: usize = 0x164;
const TABLE: usize = 0x2C8;
const SLOTS: usize = 10;
const SOS_PER_SLOT: usize = 7;
const WEATHER_SLOTS: usize = 6;
const WEATHER_AT: usize = 0x14C;

/// The files of the wild Pokémon archive that are decompressed on reading: the area data and the encounters.
pub(super) fn decompression(family: Family) -> Decompression {
    Decompression::Only(
        (0..family.encounter_file_count())
            .map(|i| i % 11 == 0 || (i >= 9 && (i - 9) % 11 == 0))
            .collect(),
    )
}

/// One slot as stored: species and forme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    species: DexNumber,
    forme: u16,
}

fn slot(d: &[u8], at: usize) -> Result<Slot, RomError> {
    let v = word(d, at)?;
    Ok(Slot {
        species: v & 0x7FF,
        forme: v >> 11,
    })
}

/// The slots of a table half that are read, in order: each used slot with its SOS slots, then the used weather
/// slots.
fn slots(half: &[u8]) -> Result<Vec<Slot>, RomError> {
    let mut out = Vec::new();
    for i in 0..SLOTS {
        let at = 0xC + i * 4;
        let s = slot(half, at)?;
        if s.species != 0 {
            out.push(s);
            for j in 1..=SOS_PER_SLOT {
                out.push(slot(half, at + 40 * j)?);
            }
        }
    }
    for i in 0..WEATHER_SLOTS {
        let s = slot(half, WEATHER_AT + i * 4)?;
        if s.species != 0 {
            out.push(s);
        }
    }
    Ok(out)
}

/// One area's tables, as loaded.
#[derive(Debug, Clone)]
struct Area {
    file: usize,
    /// The unique location names of the area's zones, joined.
    name: String,
    tables: Vec<Vec<u8>>,
}

/// The encounter tables as loaded, in the order of the sets.
#[derive(Debug, Clone)]
pub(super) struct Encounters {
    family: Family,
    areas: Vec<Area>,
    /// The sets are by time of day: a day and a night set per table.
    timed: bool,
    /// Where the alternate formes are.
    formes: FormeIndex,
}

/// The 31-polynomial hash of a string's UTF-16 units, as a hash set orders its strings.
fn string_hash(s: &str) -> i32 {
    s.encode_utf16()
        .fold(0i32, |h, u| h.wrapping_mul(31).wrapping_add(i32::from(u)))
}

/// Unique names, in the order a hash set of 16 buckets lists them: by the bucket of their spread hash, ties in
/// the order they were added. The original names areas this way, so the names match.
fn hash_set_order(names: Vec<String>) -> Vec<String> {
    let mut unique: Vec<String> = Vec::new();
    for n in names {
        if !unique.contains(&n) {
            unique.push(n);
        }
    }
    // The table grows by doubling past three quarters full; buckets keep their order when it does.
    let mut buckets = 16usize;
    while unique.len() * 4 > buckets * 3 {
        buckets *= 2;
    }
    let bucket = |s: &str| {
        let h = string_hash(s);
        let spread = h.cast_unsigned() ^ (h.cast_unsigned() >> 16);
        (spread as usize) & (buckets - 1)
    };
    let mut indexed: Vec<(usize, usize, String)> = unique
        .into_iter()
        .enumerate()
        .map(|(i, s)| (bucket(&s), i, s))
        .collect();
    indexed.sort_by_key(|&(b, i, _)| (b, i));
    indexed.into_iter().map(|(_, _, s)| s).collect()
}

/// The location names by map name index: an area's subarea is added in brackets, and a name used before gets
/// the number of its use.
fn location_names(lines: &[String]) -> Vec<String> {
    let mut good = lines.to_vec();
    let mut i = 0;
    while i + 1 < lines.len() {
        let sub = &lines[i + 1];
        if !sub.is_empty() && !sub.starts_with('[') {
            good[i] = format!("{} ({sub})", good[i]);
        }
        if i > 0 && !good[i].is_empty() {
            let before = &good[..i - 1];
            let uses = before.iter().filter(|n| **n == good[i]).count();
            if uses > 0 {
                good[i] = format!("{} ({})", good[i], uses + 1);
            }
        }
        i += 2;
    }
    good
}

/// The name of each area: the unique location names of its zones.
fn area_names(
    files: &mut Files,
    zone_path: &str,
    world_path: &str,
    lines: &[String],
) -> Result<Vec<(usize, String)>, RomError> {
    let world_garc = files.garc(world_path, &Decompression::All)?;
    let mut worlds = Vec::new();
    for i in 0..world_garc.len() {
        let pack = world_garc.file(i)?;
        let first = mini::unpack(&pack, *b"WD")?
            .and_then(|f| f.into_iter().next())
            .unwrap_or_default();
        worlds.push(first);
    }
    let zone_garc = files.garc(zone_path, &Decompression::All)?;
    let zones = zone_garc.file(0)?;
    let world_of = zone_garc.file(1)?;
    let names = location_names(lines);
    let mut out = Vec::new();
    for z in 0..zones.len() / 0x54 {
        let parent = u32::from_le_bytes(
            zones
                .get(z * 0x54 + 0x1C..z * 0x54 + 0x20)
                .and_then(|b| b.try_into().ok())
                .unwrap_or_default(),
        ) as usize;
        let world = usize::from(word(&world_of, z * 2)?);
        let w = worlds.get(world).map_or(&[][..], Vec::as_slice);
        let mapping = w
            .get(8..12)
            .map_or(usize::MAX, |b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize);
        let mut area = 0;
        let mut at = mapping;
        while at + 4 <= w.len() {
            if usize::from(word(w, at)?) == z {
                area = usize::from(word(w, at + 2)?);
                break;
            }
            at += 4;
        }
        out.push((area, names.get(parent).cloned().unwrap_or_default()));
    }
    Ok(out)
}

fn kind_name(k: EncounterKind) -> &'static str {
    match k {
        EncounterKind::Walking => "walking",
        EncounterKind::Surfing => "surfing",
        EncounterKind::Interact => "interact",
        EncounterKind::Fishing => "fishing",
        EncounterKind::Ambush => "ambush",
        EncounterKind::Special => "special",
        EncounterKind::Unused => "unused",
    }
}

/// A family's location tags, encounter kinds and post-game sets, by set.
type SetTables = (
    &'static [(&'static str, usize)],
    &'static [(EncounterKind, usize)],
    &'static [usize],
);

impl Encounters {
    /// The encounter tables, and their sets (by time of day when `timed`), tagged with their locations, kinds and
    /// whether they are post-game.
    pub(super) fn read(
        files: &mut Files,
        paths: [&str; 3],
        family: Family,
        map_names: &[String],
        timed: bool,
        personal: &Personal,
    ) -> Result<(Self, Vec<EncounterSet>), RomError> {
        let [wild_path, zone_path, world_path] = paths;
        let zone_areas = area_names(files, zone_path, world_path, map_names)?;
        let garc = files.garc(wild_path, &decompression(family))?;
        let mut areas = Vec::new();
        for a in 0..garc.len() / 11 {
            let file = 9 + 11 * a;
            let data = garc.file(file)?;
            if data.is_empty() {
                continue;
            }
            let Some(packed) = mini::unpack(&data, *b"EA")? else {
                continue;
            };
            if packed.iter().all(Vec::is_empty) {
                continue;
            }
            let zone_names: Vec<String> = zone_areas
                .iter()
                .filter(|(area, _)| *area == a)
                .map(|(_, n)| n.clone())
                .collect();
            let name = hash_set_order(zone_names).join(" / ");
            let tables = packed
                .iter()
                .enumerate()
                .map(|(t, table)| {
                    table
                        .get(4..4 + TABLE)
                        .map(<[u8]>::to_vec)
                        .ok_or_else(|| RomError::Malformed(format!("encounter table {t} of area {a} is too short")))
                })
                .collect::<Result<_, _>>()?;
            areas.push(Area { file, name, tables });
        }
        let encounters = Self {
            family,
            areas,
            timed,
            formes: personal.forme_index(),
        };
        let sets = encounters.sets()?;
        Ok((encounters, sets))
    }

    /// Whether the sets are by time of day.
    pub(super) const fn timed(&self) -> bool {
        self.timed
    }

    /// Switches the view by time of day on or off, and gives the sets of the new view.
    pub(super) fn set_timed(&mut self, timed: bool) -> Result<Vec<EncounterSet>, RomError> {
        self.timed = timed;
        self.sets()
    }

    /// For each set, its table: the set without time of day it comes from.
    pub(super) fn origins(&self) -> Vec<usize> {
        let tables = self.areas.iter().map(|a| a.tables.len()).sum::<usize>();
        let per_table = if self.timed { 2 } else { 1 };
        (0..tables * per_table).map(|i| i / per_table).collect()
    }

    /// The sets of the tables as loaded: per table its day half, and its night half after it when by time of day.
    fn sets(&self) -> Result<Vec<EncounterSet>, RomError> {
        let (tags, kinds, post_game): SetTables = match self.family {
            Family::SunMoon => (&c::LOCATION_TAGS_SM, &c::ENCOUNTER_KINDS_SM, &c::POST_GAME_SETS_SM),
            Family::UltraSunMoon => (
                &c::LOCATION_TAGS_USUM,
                &c::ENCOUNTER_KINDS_USUM,
                &c::POST_GAME_SETS_USUM,
            ),
        };
        let tag_of = runs(tags);
        let kind_of = runs(kinds);
        let mut sets = Vec::new();
        let mut i = 0;
        for area in &self.areas {
            for (t, table) in area.tables.iter().enumerate() {
                let kind = kind_of.get(i).copied().unwrap_or(EncounterKind::Special);
                let name = format!("{} {}, Table {}", area.name, kind_name(kind), t + 1);
                let halves: &[(usize, &str)] = if self.timed {
                    &[(0, " (Day)"), (HALF, " (Night)")]
                } else {
                    &[(0, "")]
                };
                for &(at, suffix) in halves {
                    sets.push(EncounterSet {
                        kind,
                        map: None,
                        name: format!("{name}{suffix}"),
                        location: tag_of.get(i).map(|&s| s.to_owned()),
                        post_game: post_game.contains(&i),
                        post_game_from: None,
                        rate: 1,
                        banned: Vec::new(),
                        encounters: half_encounters(&table[at..at + HALF], &self.formes)?,
                    });
                }
                i += 1;
            }
        }
        Ok(sets)
    }

    /// Writes the tables whose sets changed. Without time of day a set goes into both halves of its table; by
    /// time of day the day and night sets go into their own halves. A slot keeps its stored forme while its species
    /// and forme stay the same. Writing a set needs as many encounters as were read: 8 per used slot and the weather
    /// slots.
    pub(super) fn write(
        &self,
        files: &mut Files,
        path: &str,
        loaded: &[EncounterSet],
        new: &[EncounterSet],
    ) -> Result<(), RomError> {
        let garc = files.garc_mut(path, &decompression(self.family))?;
        let per_table = if self.timed { 2 } else { 1 };
        let mut i = 0;
        for area in &self.areas {
            let mut tables = area.tables.clone();
            let mut changed = false;
            for table in &mut tables {
                let (Some(old), Some(sets)) = (loaded.get(i..i + per_table), new.get(i..i + per_table)) else {
                    return Err(RomError::Malformed("fewer encounter sets than tables".into()));
                };
                i += per_table;
                if old == sets {
                    continue;
                }
                changed = true;
                write_table(table, (&sets[0], sets.get(1)), (&old[0], old.get(1)), &self.formes)?;
            }
            if changed {
                let data = garc.file(area.file)?;
                let mut packed = mini::unpack(&data, *b"EA")?
                    .ok_or_else(|| RomError::Malformed("encounter tables are no EA pack".into()))?;
                for (p, t) in packed.iter_mut().zip(&tables) {
                    p[4..4 + TABLE].copy_from_slice(t);
                }
                garc.set_file(area.file, mini::pack(&packed, *b"EA")?)?;
            }
        }
        Ok(())
    }
}

/// The encounters of a table half, at its levels.
fn half_encounters(half: &[u8], formes: &FormeIndex) -> Result<Vec<WildEncounter>, RomError> {
    let (min, max) = (half[0], half[1]);
    Ok(slots(half)?
        .into_iter()
        .map(|s| {
            let forme = formes.holder_forme(s.species, s.forme);
            WildEncounter {
                species: formes.species_of(s.species, forme),
                forme,
                min_level: min,
                max_level: max,
            }
        })
        .collect())
}

/// A day set and, by time of day, a night set.
type Sets<'a> = (&'a EncounterSet, Option<&'a EncounterSet>);

/// Writes a table: the day set into the day half, and the night set into the night half, or the day set again
/// without one. Each half keeps the stored formes of its own slots while their species and formes stay as they
/// were loaded (`old`).
fn write_table(table: &mut [u8], new: Sets<'_>, old: Sets<'_>, formes: &FormeIndex) -> Result<(), RomError> {
    let (day, night) = new;
    let (old_day, old_night) = old;
    let day_formes = slots(&table[..HALF])?;
    // Without time of day, the night half takes the day set with the formes it was read with.
    let night_formes = match night {
        Some(_) => slots(&table[HALF..TABLE])?,
        None => day_formes.clone(),
    };
    write_half(
        &mut table[..HALF],
        &day.encounters,
        &old_day.encounters,
        &day_formes,
        formes,
    )?;
    write_half(
        &mut table[HALF..TABLE],
        &night.unwrap_or(day).encounters,
        &old_night.unwrap_or(old_day).encounters,
        &night_formes,
        formes,
    )?;
    Ok(())
}

/// The value at each set index, from runs of (value, count).
fn runs<T: Copy>(runs: &[(T, usize)]) -> Vec<T> {
    runs.iter().flat_map(|&(v, n)| std::iter::repeat_n(v, n)).collect()
}

fn write_half(
    half: &mut [u8],
    encounters: &[WildEncounter],
    loaded: &[WildEncounter],
    stored: &[Slot],
    formes: &FormeIndex,
) -> Result<(), RomError> {
    let Some(first) = encounters.first() else {
        return Ok(());
    };
    if let Some(e) = encounters
        .iter()
        .find(|e| (e.min_level, e.max_level) != (first.min_level, first.max_level))
    {
        return Err(RomError::Malformed(format!(
            "levels {}-{} and {}-{} in one encounter table, which stores one range",
            first.min_level, first.max_level, e.min_level, e.max_level
        )));
    }
    half[0] = first.min_level;
    half[1] = first.max_level;
    let value = |k: usize, e: &WildEncounter| {
        let unchanged = loaded
            .get(k)
            .is_some_and(|l| l.species == e.species && l.forme == e.forme);
        let forme = match stored.get(k) {
            Some(s) if unchanged => s.forme,
            _ => u16::from(match formes.forme_number(e.species) {
                0 => e.forme,
                f => f,
            }),
        };
        (forme << 11) + formes.base(e.species)
    };
    let main = encounters.len() / 8;
    let mut k = 0;
    for i in 0..main.min(SLOTS) {
        let at = 0xC + i * 4;
        for j in 0..=SOS_PER_SLOT {
            put_u16(half, at + 40 * j, value(k, &encounters[k]));
            k += 1;
        }
    }
    if encounters.len() != main * 8 {
        for i in 0..WEATHER_SLOTS {
            let e = encounters
                .get(k)
                .ok_or_else(|| RomError::Malformed("too few weather encounters".into()))?;
            put_u16(half, WEATHER_AT + i * 4, value(k, e));
            k += 1;
        }
    }
    Ok(())
}

/// Lets the Minior met in the wild keep the forme of its encounter instead of a random Meteor forme.
pub(super) fn patch_minior(code: &mut [u8]) -> Result<(), RomError> {
    if let Some(at) = super::find(code, c::MINIOR_WILD_ENCOUNTER_PREFIX)? {
        put_u32(code, at, 0);
    }
    Ok(())
}

/// Where each field item is: area data file, then the field item file and offset, or the berry pile file and
/// offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Spot {
    Item { area: usize, file: usize, at: usize },
    Berry { area: usize, file: usize, at: usize },
}

/// An area's item, item spot and berry spot packs.
type AreaPacks = (Vec<Vec<u8>>, Vec<Vec<u8>>, Vec<Vec<u8>>);

fn area_packs(garc: &Garc, area: usize) -> Result<Option<AreaPacks>, RomError> {
    let data = garc.file(area * 11)?;
    let Some(env) = mini::unpack(&data, *b"ED")? else {
        return Ok(None);
    };
    let (Some(items), Some(berries)) = (env.get(10), env.get(11)) else {
        return Ok(None);
    };
    let items = mini::unpack(items, *b"EI")?.unwrap_or_default();
    let berries = mini::unpack(berries, *b"EB")?.unwrap_or_default();
    Ok(Some((env, items, berries)))
}

/// Every field item and berry with where it is, in the game's order.
fn field_spots(garc: &Garc) -> Result<Vec<(Spot, ItemId)>, RomError> {
    let mut out = Vec::new();
    for area in 0..garc.len() / 11 {
        let Some((_, items, berries)) = area_packs(garc, area)? else {
            continue;
        };
        for (file, d) in items.iter().enumerate() {
            if let Some(&count) = d.first() {
                for j in 0..usize::from(count) {
                    let at = j * 64 + 52;
                    out.push((Spot::Item { area, file, at }, word(d, at)?));
                }
            }
        }
        for (file, d) in berries.iter().enumerate() {
            if let Some(&count) = d.first() {
                for j in 0..usize::from(count) {
                    for k in 0..7 {
                        let at = 4 + j * 68 + 54 + k * 2;
                        out.push((Spot::Berry { area, file, at }, word(d, at)?));
                    }
                }
            }
        }
    }
    Ok(out)
}

/// The field items the randomizers may change: those that are allowed items.
pub(super) fn read_field_items(
    files: &mut Files,
    path: &str,
    family: Family,
    allowed: impl Fn(ItemId) -> bool,
) -> Result<Vec<ItemId>, RomError> {
    let garc = files.garc(path, &decompression(family))?;
    Ok(field_spots(garc)?
        .into_iter()
        .map(|(_, id)| id)
        .filter(|&id| allowed(id))
        .collect())
}

/// Writes the field items: each allowed one takes the next of `new`.
pub(super) fn write_field_items(
    files: &mut Files,
    path: &str,
    family: Family,
    allowed: impl Fn(ItemId) -> bool,
    new: &[ItemId],
) -> Result<(), RomError> {
    let garc = files.garc_mut(path, &decompression(family))?;
    let spots = field_spots(garc)?;
    let mut next = new.iter();
    let mut changes: Vec<(Spot, ItemId)> = Vec::new();
    for (spot, id) in spots {
        if allowed(id)
            && let Some(&n) = next.next()
            && n != id
        {
            changes.push((spot, n));
        }
    }
    let mut areas: Vec<usize> = changes
        .iter()
        .map(|(s, _)| match *s {
            Spot::Item { area, .. } | Spot::Berry { area, .. } => area,
        })
        .collect();
    areas.dedup();
    for area in areas {
        let Some((mut env, mut items, mut berries)) = area_packs(garc, area)? else {
            continue;
        };
        for &(spot, id) in &changes {
            match spot {
                Spot::Item { area: a, file, at } if a == area => put_u16(&mut items[file], at, id),
                Spot::Berry { area: a, file, at } if a == area => put_u16(&mut berries[file], at, id),
                _ => {}
            }
        }
        env[10] = mini::pack(&items, *b"EI")?;
        env[11] = mini::pack(&berries, *b"EB")?;
        garc.set_file(area * 11, mini::pack(&env, *b"ED")?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table with `day` in the first slot and its SOS slots of the day half (levels 3 to 5), and `night` in those
    /// of the night half (levels 10 to 12), all of forme `forme`.
    fn table(day: DexNumber, night: DexNumber, forme: u16) -> Vec<u8> {
        let mut t = vec![0; TABLE];
        for (at, species, levels) in [(0, day, [3, 5]), (HALF, night, [10, 12])] {
            t[at..at + 2].copy_from_slice(&levels);
            for j in 0..=SOS_PER_SLOT {
                put_u16(&mut t, at + 0xC + 40 * j, (forme << 11) | species);
            }
        }
        t
    }

    fn encounters(tables: Vec<Vec<u8>>, timed: bool) -> Encounters {
        Encounters {
            family: Family::UltraSunMoon,
            areas: vec![Area {
                file: 9,
                name: "Route 1".into(),
                tables,
            }],
            timed,
            formes: FormeIndex::default(),
        }
    }

    fn species(set: &EncounterSet) -> Vec<DexNumber> {
        set.encounters.iter().map(|e| e.species).collect()
    }

    fn half_species(half: &[u8]) -> Vec<u16> {
        (0..=SOS_PER_SLOT)
            .map(|j| word(half, 0xC + 40 * j).expect("in the half"))
            .collect()
    }

    #[test]
    fn without_time_of_day_a_table_is_its_day_half() {
        let sets = encounters(vec![table(734, 19, 1), table(10, 10, 0)], false)
            .sets()
            .expect("reads");
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Route 1 walking, Table 1", "Route 1 walking, Table 2"]);
        assert_eq!(species(&sets[0]), [734; 8]);
        assert_eq!(
            (sets[0].encounters[0].min_level, sets[0].encounters[0].max_level),
            (3, 5)
        );
        assert_eq!(sets[0].location.as_deref(), Some("ROUTE 1"));
    }

    #[test]
    fn by_time_of_day_a_table_is_a_day_and_a_night_set_tagged_alike() {
        let mut e = encounters(vec![table(734, 19, 1), table(10, 10, 0)], false);
        let untimed = e.sets().expect("reads");
        let sets = e.set_timed(true).expect("reads");
        assert!(e.timed());
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "Route 1 walking, Table 1 (Day)",
                "Route 1 walking, Table 1 (Night)",
                "Route 1 walking, Table 2 (Day)",
                "Route 1 walking, Table 2 (Night)",
            ]
        );
        assert_eq!(species(&sets[1]), [19; 8]);
        assert_eq!(
            (sets[1].encounters[0].min_level, sets[1].encounters[0].max_level),
            (10, 12)
        );
        for (i, set) in sets.iter().enumerate() {
            let table = &untimed[i / 2];
            assert_eq!(
                (set.kind, &set.location, set.post_game),
                (table.kind, &table.location, table.post_game)
            );
        }
        assert_eq!(sets[0].encounters, untimed[0].encounters);
        assert_eq!(e.origins(), [0, 0, 1, 1]);
        assert_eq!(e.set_timed(false).expect("reads"), untimed);
        assert_eq!(e.origins(), [0, 1]);
    }

    #[test]
    fn by_time_of_day_both_halves_of_a_post_game_table_are_post_game() {
        // Table 25 of Ultra Sun and Ultra Moon is post-game, table 24 is not.
        let tables = vec![table(10, 10, 0); 26];
        let untimed = encounters(tables.clone(), false).sets().expect("reads");
        let timed = encounters(tables, true).sets().expect("reads");
        assert_eq!((untimed[24].post_game, untimed[25].post_game), (false, true));
        let post: Vec<bool> = timed[48..52].iter().map(|s| s.post_game).collect();
        assert_eq!(post, [false, false, true, true]);
    }

    #[test]
    fn by_time_of_day_each_half_is_written_from_its_own_set() {
        let mut t = table(734, 19, 1);
        let sets = encounters(vec![t.clone()], true).sets().expect("reads");
        let mut day = sets[0].clone();
        for e in &mut day.encounters {
            e.species = 25;
            e.min_level = 7;
        }
        let old = (&sets[0], Some(&sets[1]));
        write_table(&mut t, (&day, Some(&sets[1])), old, &FormeIndex::default()).expect("writes");
        assert_eq!(half_species(&t[..HALF]), [25; 8], "a new species gets forme 0");
        assert_eq!(t[0], 7);
        assert_eq!(t[HALF..], table(734, 19, 1)[HALF..], "the night half stays as it was");

        let mut night = sets[1].clone();
        night.encounters[0].species = 26;
        write_table(&mut t, (&day, Some(&night)), old, &FormeIndex::default()).expect("writes");
        let mut expected = vec![(1 << 11) + 19; 8];
        expected[0] = 26;
        assert_eq!(half_species(&t[HALF..]), expected, "the night slots keep their forme");
        assert_eq!(t[HALF], 10);
    }

    #[test]
    fn levels_of_their_own_in_one_table_fail() {
        let mut t = table(734, 19, 1);
        let sets = encounters(vec![t.clone()], false).sets().expect("reads");
        let mut day = sets[0].clone();
        day.encounters[3].max_level = 9;
        let err = write_table(&mut t, (&day, None), (&sets[0], None), &FormeIndex::default())
            .expect_err("a table stores one level range");
        assert!(err.to_string().contains("levels 3-5 and 3-9"), "{err}");
        assert_eq!(t, table(734, 19, 1), "nothing is written");
    }

    #[test]
    fn without_time_of_day_the_day_set_goes_into_both_halves() {
        let mut t = table(734, 19, 1);
        let sets = encounters(vec![t.clone()], false).sets().expect("reads");
        write_table(&mut t, (&sets[0], None), (&sets[0], None), &FormeIndex::default()).expect("writes");
        assert_eq!(
            half_species(&t[HALF..]),
            [(1 << 11) + 734; 8],
            "with the day half's formes"
        );
        assert_eq!((t[HALF], t[HALF + 1]), (3, 5));
        assert_eq!(t[..HALF], table(734, 19, 1)[..HALF]);
    }
}
