//! Species data: base stats, types, abilities and wild held items (`PokemonStats`), names, palettes, TM, HM and
//! move tutor compatibility, level-up moves (`PokemonMovesets`), egg moves and evolutions.
//!
//! The alternate formes with data of their own follow the species from 650, in the order of their files; a species'
//! stats name the file of its first alternate forme and its number of formes. Their stats, held items, level-up
//! moves and compatibility are read and written like a species'; names, palettes, egg moves and evolutions are
//! their base species'.
//!
//! Deliberate choice: a species with cosmetic formes but no files for them (Unown's letters) gets forme numbers 1 up
//! to its number of formes less one; the original gave it one more, a forme the game doesn't have.

use std::collections::BTreeMap;

use ppr_model::{
    BaseStats, DexNumber, Evolution, EvolutionType, ExpCurve, Forme, ItemId, LevelUpMove, MoveId, Species,
    SpeciesPalettes, Type, WildHeldItems,
};

use super::constants as c;
use super::entry::Family;
use super::store::{Store, byte, put, put_byte, put_word, word};
use crate::{RomError, ds_palette};

/// The species of Gen 5.
pub(super) const SPECIES_COUNT: u16 = 649;

/// The types by their number in the game's data.
pub(super) const TYPES: [Type; 17] = [
    Type::Normal,
    Type::Fighting,
    Type::Flying,
    Type::Poison,
    Type::Ground,
    Type::Rock,
    Type::Bug,
    Type::Ghost,
    Type::Steel,
    Type::Fire,
    Type::Water,
    Type::Grass,
    Type::Electric,
    Type::Psychic,
    Type::Ice,
    Type::Dragon,
    Type::Dark,
];

/// The type with this number in the game's data.
pub(super) fn type_of(b: u8) -> Option<Type> {
    TYPES.get(usize::from(b)).copied()
}

/// The number the game's data gives this type; Normal for a type it doesn't have.
pub(super) fn type_byte(t: Type) -> u8 {
    TYPES
        .iter()
        .position(|&x| x == t)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or_default()
}

// Offsets in a species' `PokemonStats` file.
const HP: usize = 0;
const ATTACK: usize = 1;
const DEFENSE: usize = 2;
const SPEED: usize = 3;
const SP_ATTACK: usize = 4;
const SP_DEFENSE: usize = 5;
const PRIMARY_TYPE: usize = 6;
const SECONDARY_TYPE: usize = 7;
const CATCH_RATE: usize = 8;
pub(super) const EV_YIELD: usize = 10;
const COMMON_ITEM: usize = 12;
const RARE_ITEM: usize = 14;
const DARK_GRASS_ITEM: usize = 16;
pub(super) const GENDER_RATIO: usize = 18;
pub(super) const EGG_CYCLES: usize = 19;
const GROWTH_CURVE: usize = 21;
const ABILITIES: usize = 24;
const TM_COMPAT: usize = 40;
const TUTOR_COMPAT: usize = 60;
const FORME_FILE: usize = 28;
const FORME_COUNT: usize = 32;

/// TM and HM flags: 95 TMs, then 6 HMs, in 13 bytes.
pub(super) const TM_COUNT: usize = 95;
pub(super) const HM_COUNT: usize = 6;
const TM_COMPAT_BYTES: usize = 13;
/// Move tutors of Black 2 and White 2.
pub(super) const TUTOR_COUNT: usize = 60;
/// The tutors' flags are kept in four words of 32 flags, in another order than their moves: how many flags each word
/// holds, and which group of moves it is.
const TUTOR_COUNTS_BY_WORD: [usize; 4] = [15, 17, 13, 15];
const TUTOR_COUNTS_BY_MOVES: [usize; 4] = [13, 15, 15, 17];
const TUTOR_WORD_TO_MOVES: [usize; 4] = [1, 3, 0, 2];

fn malformed(what: impl Into<String>) -> RomError {
    RomError::Malformed(what.into())
}

/// The item number, `None` for 0.
fn item(id: u16) -> Option<ItemId> {
    (id != 0).then_some(id)
}

/// Every species' data, by national number, then the alternate formes with data of their own.
pub(super) fn read(store: &mut Store, path: &str, names: &[String], family: Family) -> Result<Vec<Species>, RomError> {
    let mut species = Vec::new();
    // The alternate formes by file: base species and forme number.
    let mut forme_files: BTreeMap<usize, (DexNumber, u8)> = BTreeMap::new();
    for n in 1..=SPECIES_COUNT {
        let data = store.narc_file(path, usize::from(n))?;
        let count = byte(data, FORME_COUNT)?;
        let first = usize::from(word(data, FORME_FILE)?);
        let mut forme = Forme {
            suffix: c::BASE_FORME_SUFFIXES
                .iter()
                .find(|&&(b, _)| b == n)
                .map(|&(_, suffix)| suffix.to_owned())
                .unwrap_or_default(),
            ..Forme::default()
        };
        if count > 1 {
            if first != 0 {
                for f in 1..count {
                    forme_files.insert(first + usize::from(f) - 1, (n, f));
                }
            } else if n <= c::LAST_COSMETIC_FORME_SPECIES && !c::IGNORED_COSMETIC_FORMES.contains(&n) {
                forme.cosmetic_formes = (1..count).collect();
            }
        }
        species.push(species_from(data, n, name_of(names, n), forme)?);
    }
    if forme_files.len() != usize::from(family.forme_count()) {
        return Err(malformed(format!(
            "{} alternate formes, expected {}",
            forme_files.len(),
            family.forme_count()
        )));
    }
    let irregular = family.pick(&c::BW1_IRREGULAR_FORMES[..], &c::BW2_IRREGULAR_FORMES[..]);
    for (n, (file, (base, number))) in (SPECIES_COUNT + 1..).zip(forme_files) {
        if file != family.species_file(n) {
            return Err(malformed(format!("alternate forme {n} in file {file}")));
        }
        let data = store.narc_file(path, file)?;
        let forme = Forme {
            base: Some(base),
            number,
            suffix: c::FORME_SUFFIXES
                .iter()
                .find(|&&(b, _)| b == base)
                .and_then(|(_, suffixes)| suffixes.get(usize::from(number) - 1))
                .map(|&suffix| suffix.to_owned())
                .unwrap_or_default(),
            cosmetic: n == c::KELDEO_RESOLUTE,
            irregular: irregular.contains(&n),
            ..Forme::default()
        };
        species.push(species_from(data, n, name_of(names, base), forme)?);
    }
    Ok(species)
}

fn name_of(names: &[String], n: DexNumber) -> String {
    names.get(usize::from(n)).cloned().unwrap_or_default()
}

/// The species data of stats file `data`.
fn species_from(data: &[u8], n: DexNumber, name: String, forme: Forme) -> Result<Species, RomError> {
    let primary = type_of(byte(data, PRIMARY_TYPE)?).ok_or_else(|| malformed(format!("species {n}: type")))?;
    let secondary = type_of(byte(data, SECONDARY_TYPE)?).filter(|&t| t != primary);
    Ok(Species {
        forme,
        number: n,
        name,
        primary_type: primary,
        secondary_type: secondary,
        stats: BaseStats {
            hp: byte(data, HP)?,
            attack: byte(data, ATTACK)?,
            defense: byte(data, DEFENSE)?,
            speed: byte(data, SPEED)?,
            sp_attack: byte(data, SP_ATTACK)?,
            sp_defense: byte(data, SP_DEFENSE)?,
        },
        catch_rate: byte(data, CATCH_RATE)?,
        abilities: (0..3)
            .map(|i| byte(data, ABILITIES + i).map(u16::from))
            .collect::<Result<_, _>>()?,
        growth_curve: ExpCurve::from_byte(byte(data, GROWTH_CURVE)?).unwrap_or(ExpCurve::MediumFast),
    })
}

/// What wild Pokémon hold: one item when common and rare are the same, else common, rare and in dark grass.
pub(super) fn read_held_items(
    store: &mut Store,
    path: &str,
    family: Family,
) -> Result<BTreeMap<DexNumber, WildHeldItems>, RomError> {
    let mut out = BTreeMap::new();
    for n in family.species_numbers() {
        let data = store.narc_file(path, family.species_file(n))?;
        let common = word(data, COMMON_ITEM)?;
        let rare = word(data, RARE_ITEM)?;
        let held = if common == rare {
            WildHeldItems {
                guaranteed: item(common),
                ..WildHeldItems::default()
            }
        } else {
            WildHeldItems {
                common: item(common),
                rare: item(rare),
                dark_grass: item(word(data, DARK_GRASS_ITEM)?),
                ..WildHeldItems::default()
            }
        };
        if !held.is_empty() {
            out.insert(n, held);
        }
    }
    Ok(out)
}

/// Writes the species that changed, with their wild held items.
pub(super) fn write(
    store: &mut Store,
    path: &str,
    family: Family,
    old: &[Species],
    new: &[Species],
    old_held: &BTreeMap<DexNumber, WildHeldItems>,
    new_held: &BTreeMap<DexNumber, WildHeldItems>,
) -> Result<(), RomError> {
    for (sp, before) in new.iter().zip(old) {
        let held = new_held.get(&sp.number).copied().unwrap_or_default();
        let held_before = old_held.get(&sp.number).copied().unwrap_or_default();
        let same = sp.primary_type == before.primary_type
            && sp.secondary_type == before.secondary_type
            && sp.stats == before.stats
            && sp.catch_rate == before.catch_rate
            && sp.abilities == before.abilities
            && sp.growth_curve == before.growth_curve;
        if same && held == held_before {
            continue;
        }
        let data = store.narc_file_mut(path, family.species_file(sp.number))?;
        if !same {
            let b = sp.stats;
            put(
                data,
                HP,
                &[b.hp, b.attack, b.defense, b.speed, b.sp_attack, b.sp_defense],
            )?;
            let primary = type_byte(sp.primary_type);
            put_byte(data, PRIMARY_TYPE, primary)?;
            put_byte(data, SECONDARY_TYPE, sp.secondary_type.map_or(primary, type_byte))?;
            put_byte(data, CATCH_RATE, sp.catch_rate)?;
            put_byte(data, GROWTH_CURVE, sp.growth_curve.to_byte())?;
            for i in 0..3 {
                let ability = sp.abilities.get(i).copied().unwrap_or_default();
                put_byte(data, ABILITIES + i, u8::try_from(ability).unwrap_or_default())?;
            }
        }
        if held != held_before {
            let (common, rare, dark) = match held.guaranteed {
                Some(g) => (g, g, 0),
                None => (
                    held.common.unwrap_or_default(),
                    held.rare.unwrap_or_default(),
                    held.dark_grass.unwrap_or_default(),
                ),
            };
            put_word(data, COMMON_ITEM, common)?;
            put_word(data, RARE_ITEM, rare)?;
            put_word(data, DARK_GRASS_ITEM, dark)?;
        }
    }
    Ok(())
}

/// The flags of `count` bits starting at byte `at`, low bit first.
fn read_flags(data: &[u8], at: usize, count: usize) -> Result<Vec<bool>, RomError> {
    (0..count)
        .map(|i| Ok(byte(data, at + i / 8)? >> (i % 8) & 1 == 1))
        .collect()
}

fn flags_to_bytes(flags: &[bool], bytes: usize) -> Vec<u8> {
    (0..bytes)
        .map(|b| {
            (0..8)
                .filter(|&i| flags.get(b * 8 + i).copied().unwrap_or_default())
                .fold(0u8, |acc, i| acc | 1 << i)
        })
        .collect()
}

/// Which TMs (then HMs) each species learns.
pub(super) fn read_tm_compat(
    store: &mut Store,
    path: &str,
    family: Family,
) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    family
        .species_numbers()
        .map(|n| {
            let data = store.narc_file(path, family.species_file(n))?;
            Ok((n, read_flags(data, TM_COMPAT, TM_COUNT + HM_COUNT)?))
        })
        .collect()
}

pub(super) fn write_tm_compat(
    store: &mut Store,
    path: &str,
    family: Family,
    old: &BTreeMap<DexNumber, Vec<bool>>,
    new: &BTreeMap<DexNumber, Vec<bool>>,
) -> Result<(), RomError> {
    for (&n, flags) in new {
        if old.get(&n) == Some(flags) || !family.species_numbers().contains(&n) {
            continue;
        }
        let bytes = flags_to_bytes(flags, TM_COMPAT_BYTES);
        put(store.narc_file_mut(path, family.species_file(n))?, TM_COMPAT, &bytes)?;
    }
    Ok(())
}

/// Where the flags of tutor word `w` start among the tutor moves.
fn tutor_start(w: usize) -> usize {
    TUTOR_COUNTS_BY_MOVES[..TUTOR_WORD_TO_MOVES[w]].iter().sum()
}

/// Which tutor moves each species learns, in the order of the tutor moves.
pub(super) fn read_tutor_compat(
    store: &mut Store,
    path: &str,
    family: Family,
) -> Result<BTreeMap<DexNumber, Vec<bool>>, RomError> {
    family
        .species_numbers()
        .map(|n| {
            let data = store.narc_file(path, family.species_file(n))?;
            let mut flags = vec![false; TUTOR_COUNT];
            for (w, &count) in TUTOR_COUNTS_BY_WORD.iter().enumerate() {
                let word_flags = read_flags(data, TUTOR_COMPAT + w * 4, count)?;
                let start = tutor_start(w);
                flags[start..start + count].copy_from_slice(&word_flags);
            }
            Ok((n, flags))
        })
        .collect()
}

pub(super) fn write_tutor_compat(
    store: &mut Store,
    path: &str,
    family: Family,
    old: &BTreeMap<DexNumber, Vec<bool>>,
    new: &BTreeMap<DexNumber, Vec<bool>>,
) -> Result<(), RomError> {
    for (&n, flags) in new {
        if old.get(&n) == Some(flags) || !family.species_numbers().contains(&n) {
            continue;
        }
        let data = store.narc_file_mut(path, family.species_file(n))?;
        for (w, &count) in TUTOR_COUNTS_BY_WORD.iter().enumerate() {
            let start = tutor_start(w);
            let part: Vec<bool> = (start..start + count)
                .map(|i| flags.get(i).copied().unwrap_or_default())
                .collect();
            put(data, TUTOR_COMPAT + w * 4, &flags_to_bytes(&part, 4))?;
        }
    }
    Ok(())
}

/// Every species' level-up moves.
pub(super) fn read_learnsets(
    store: &mut Store,
    path: &str,
    family: Family,
) -> Result<BTreeMap<DexNumber, Vec<LevelUpMove>>, RomError> {
    let mut out = BTreeMap::new();
    for n in family.species_numbers() {
        let data = store.narc_file(path, family.species_file(n))?;
        let mut moves = Vec::new();
        let mut at = 0;
        loop {
            let (mv, level) = (word(data, at)?, word(data, at + 2)?);
            if mv == 0xFFFF && level == 0xFFFF {
                break;
            }
            moves.push(LevelUpMove::new(u8::try_from(level).unwrap_or(u8::MAX), mv));
            at += 4;
        }
        out.insert(n, moves);
    }
    Ok(out)
}

pub(super) fn write_learnsets(
    store: &mut Store,
    path: &str,
    family: Family,
    old: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
    new: &BTreeMap<DexNumber, Vec<LevelUpMove>>,
) -> Result<(), RomError> {
    for (&n, moves) in new {
        if old.get(&n) == Some(moves) || !family.species_numbers().contains(&n) {
            continue;
        }
        let mut data = Vec::with_capacity(moves.len() * 4 + 4);
        for m in moves {
            data.extend_from_slice(&m.move_id.to_le_bytes());
            data.extend_from_slice(&u16::from(m.level).to_le_bytes());
        }
        data.extend_from_slice(&[0xFF; 4]);
        *store.narc_file_mut(path, family.species_file(n))? = data;
    }
    Ok(())
}

/// The egg moves of the species that have them.
pub(super) fn read_egg_moves(store: &mut Store, path: &str) -> Result<BTreeMap<DexNumber, Vec<MoveId>>, RomError> {
    let mut out = BTreeMap::new();
    for n in 1..=SPECIES_COUNT {
        let data = store.narc_file(path, usize::from(n))?;
        let count = usize::from(word(data, 0)?);
        let moves = (0..count)
            .map(|j| word(data, 2 + j * 2))
            .collect::<Result<Vec<_>, _>>()?;
        if !moves.is_empty() {
            out.insert(n, moves);
        }
    }
    Ok(out)
}

/// Writes the egg move lists that changed: in place when they keep their length, as a new file otherwise.
pub(super) fn write_egg_moves(
    store: &mut Store,
    path: &str,
    old: &BTreeMap<DexNumber, Vec<MoveId>>,
    new: &BTreeMap<DexNumber, Vec<MoveId>>,
) -> Result<(), RomError> {
    for n in 1..=SPECIES_COUNT {
        let empty = Vec::new();
        let moves = new.get(&n).unwrap_or(&empty);
        if old.get(&n).unwrap_or(&empty) == moves {
            continue;
        }
        let count = u16::try_from(moves.len()).map_err(|_| malformed("too many egg moves"))?;
        let data = store.narc_file_mut(path, usize::from(n))?;
        let needed = 2 + moves.len() * 2;
        if data.len() < needed || usize::from(word(data, 0)?) != moves.len() {
            data.clear();
            data.resize(needed.next_multiple_of(4), 0);
        }
        put_word(data, 0, count)?;
        for (j, &m) in moves.iter().enumerate() {
            put_word(data, 2 + j * 2, m)?;
        }
    }
    Ok(())
}

/// The evolution methods by their number in the game's data, from 1.
const EVOLUTION_TYPES: [EvolutionType; 27] = [
    EvolutionType::Happiness,
    EvolutionType::HappinessDay,
    EvolutionType::HappinessNight,
    EvolutionType::Level,
    EvolutionType::Trade,
    EvolutionType::TradeItem,
    EvolutionType::TradeSpecial,
    EvolutionType::Stone,
    EvolutionType::LevelAttackHigher,
    EvolutionType::LevelAtkDefSame,
    EvolutionType::LevelDefenseHigher,
    EvolutionType::LevelLowPv,
    EvolutionType::LevelHighPv,
    EvolutionType::LevelCreateExtra,
    EvolutionType::LevelIsExtra,
    EvolutionType::HighBeauty,
    EvolutionType::StoneMaleOnly,
    EvolutionType::StoneFemaleOnly,
    EvolutionType::ItemDay,
    EvolutionType::ItemNight,
    EvolutionType::WithMove,
    EvolutionType::WithOther,
    EvolutionType::LevelMaleOnly,
    EvolutionType::LevelFemaleOnly,
    EvolutionType::MagneticField,
    EvolutionType::MossRock,
    EvolutionType::IceRock,
];
const EVOLUTIONS_PER_SPECIES: usize = 7;
const EVOLUTION_SIZE: usize = 6;

fn evolution_method(kind: EvolutionType) -> Option<u16> {
    EVOLUTION_TYPES
        .iter()
        .position(|&k| k == kind)
        .and_then(|i| u16::try_from(i + 1).ok())
}

/// Every evolution; Feebas's beauty evolution is left out, as it splits from its other one.
pub(super) fn read_evolutions(store: &mut Store, path: &str) -> Result<Vec<Evolution>, RomError> {
    let mut out: Vec<Evolution> = Vec::new();
    for n in 1..=SPECIES_COUNT {
        let data = store.narc_file(path, usize::from(n))?;
        let mut own: Vec<Evolution> = Vec::new();
        for e in 0..EVOLUTIONS_PER_SPECIES {
            let method = word(data, e * EVOLUTION_SIZE)?;
            let to = word(data, e * EVOLUTION_SIZE + 4)?;
            let Some(&kind) = usize::from(method).checked_sub(1).and_then(|i| EVOLUTION_TYPES.get(i)) else {
                continue;
            };
            if to < 1 || kind == EvolutionType::HighBeauty {
                continue;
            }
            let evolution = Evolution::new(n, to, kind, word(data, e * EVOLUTION_SIZE + 2)?);
            if !own.contains(&evolution) {
                own.push(evolution);
            }
        }
        out.extend(own);
        // Burmy may become any of Wormadam's formes, as links the game doesn't store.
        if n == c::BURMY {
            out.extend(c::WORMADAM_FORMES.map(|to| Evolution::new(n, to, EvolutionType::None, 0)));
        }
    }
    Ok(out)
}

/// The evolutions of `from` as the game stores them: an evolution by item at any time becomes one by day and one by
/// night.
fn stored_evolutions(evolutions: &[Evolution], from: DexNumber) -> Vec<(u16, u16, u16)> {
    let mut out = Vec::new();
    for e in evolutions.iter().filter(|e| e.from == from) {
        let kinds: &[EvolutionType] = if e.kind == EvolutionType::Item {
            &[EvolutionType::ItemDay, EvolutionType::ItemNight]
        } else {
            std::slice::from_ref(&e.kind)
        };
        for &kind in kinds {
            if let Some(method) = evolution_method(kind) {
                out.push((method, e.parameter, e.to));
            }
        }
    }
    out
}

/// Writes the evolutions of the species whose evolutions changed.
pub(super) fn write_evolutions(
    store: &mut Store,
    path: &str,
    old: &[Evolution],
    new: &[Evolution],
) -> Result<(), RomError> {
    for n in 1..=SPECIES_COUNT {
        let stored = stored_evolutions(new, n);
        if stored == stored_evolutions(old, n) {
            continue;
        }
        let data = store.narc_file_mut(path, usize::from(n))?;
        for e in 0..EVOLUTIONS_PER_SPECIES {
            let (method, parameter, to) = stored.get(e).copied().unwrap_or_default();
            put_word(data, e * EVOLUTION_SIZE, method)?;
            put_word(data, e * EVOLUTION_SIZE + 2, parameter)?;
            put_word(data, e * EVOLUTION_SIZE + 4, to)?;
        }
    }
    Ok(())
}

/// Each species has 20 graphics files; the normal and shiny palettes are the last two.
const GRAPHICS_PER_SPECIES: usize = 20;
const NORMAL_PALETTE: usize = 18;
const SHINY_PALETTE: usize = 19;

/// Every species' normal and shiny palette.
pub(super) fn read_palettes(store: &mut Store, path: &str) -> Result<BTreeMap<DexNumber, SpeciesPalettes>, RomError> {
    (1..=SPECIES_COUNT)
        .map(|n| {
            let base = usize::from(n) * GRAPHICS_PER_SPECIES;
            Ok((
                n,
                SpeciesPalettes {
                    normal: ds_palette::read(store.narc_file(path, base + NORMAL_PALETTE)?),
                    shiny: ds_palette::read(store.narc_file(path, base + SHINY_PALETTE)?),
                    id: None,
                },
            ))
        })
        .collect()
}

pub(super) fn write_palettes(
    store: &mut Store,
    path: &str,
    old: &BTreeMap<DexNumber, SpeciesPalettes>,
    new: &BTreeMap<DexNumber, SpeciesPalettes>,
) -> Result<(), RomError> {
    for (&n, p) in new {
        let Some(before) = old.get(&n) else { continue };
        let base = usize::from(n) * GRAPHICS_PER_SPECIES;
        if p.normal != before.normal {
            *store.narc_file_mut(path, base + NORMAL_PALETTE)? = ds_palette::to_bytes(&p.normal);
        }
        if p.shiny != before.shiny {
            *store.narc_file_mut(path, base + SHINY_PALETTE)? = ds_palette::to_bytes(&p.shiny);
        }
    }
    Ok(())
}

/// Sets bytes of these species and alternate forme files, as the egg cycles and EV yields tweaks do.
pub(super) fn set_in_all(
    store: &mut Store,
    path: &str,
    files: &[usize],
    at: usize,
    bytes: &[u8],
) -> Result<(), RomError> {
    for &file in files {
        put(store.narc_file_mut(path, file)?, at, bytes)?;
    }
    Ok(())
}
