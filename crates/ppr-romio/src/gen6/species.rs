//! Species: the personal data (stats, types, abilities, held items, TM and tutor compatibility) of every species
//! and alternate forme, the Mega Evolutions and the names.
//!
//! Alternate formes with data of their own follow the 721 species in the personal data GARC; a species' first
//! forme file and its number of formes are in its own data. The file number is the forme's species number in the
//! game data, as in the game's other tables. A forme has its base species' name. The Mega Evolution table names
//! each species' Mega Evolutions by forme number, with the Mega Stone they need. The last file of the personal
//! data holds a copy of every species' and forme's data after a header as long as one entry; it is kept in step.
//!
//! Deliberate choice: a species with cosmetic formes but no files for them (Vivillon's patterns) gets forme numbers
//! 1 up to its number of formes less one; the original gave it one more, a forme the game doesn't have.

use std::collections::BTreeMap;

use ppr_model::{BaseStats, DexNumber, ExpCurve, ItemId, Species, WildHeldItems};

use super::constants::{self as c, Family};
use super::rom::{Bytes, BytesMut, Rom};
use crate::RomError;
use crate::formes::{base_suffix, mega_suffix, parse_megas, suffix_in};

/// An alternate forme with data of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Forme {
    pub(crate) base: DexNumber,
    pub(crate) number: u8,
}

/// The alternate formes by file number (722 on), which is also how the game numbers them in its other tables.
pub(crate) type Formes = BTreeMap<usize, Forme>;

/// Where each species' formes are.
#[derive(Debug, Clone, Default)]
pub(crate) struct FormeTable {
    /// The alternate formes with data of their own, by file number.
    pub(crate) formes: Formes,
    /// The number of formes of each species whose formes have no data of their own, counting the base forme.
    pub(crate) cosmetic_counts: BTreeMap<DexNumber, u8>,
}

impl FormeTable {
    /// The species number of forme `forme` of `base`: the alternate forme's file number for a forme with data of
    /// its own, else `base`.
    pub(crate) fn species(&self, base: DexNumber, forme: u8) -> DexNumber {
        if forme == 0 {
            return base;
        }
        self.formes
            .iter()
            .find(|&(_, f)| f.base == base && f.number == forme)
            .and_then(|(&file, _)| DexNumber::try_from(file).ok())
            .unwrap_or(base)
    }

    /// The base species of species number `n`.
    pub(crate) fn base(&self, n: DexNumber) -> DexNumber {
        self.formes.get(&usize::from(n)).map_or(n, |f| f.base)
    }

    /// Whether `base` has forme `forme`: one with data of its own or a cosmetic one.
    pub(crate) fn is_valid(&self, base: DexNumber, forme: u8) -> bool {
        forme == 0
            || self.formes.values().any(|f| f.base == base && f.number == forme)
            || self.cosmetic_counts.get(&base).is_some_and(|&count| forme < count)
    }

    /// The forme a Pokémon stored as `base` with forme `stored` is in the game data: the stored one when the
    /// species has it, else 0.
    pub(crate) fn holder_forme(&self, base: DexNumber, stored: u32) -> u8 {
        u8::try_from(stored)
            .ok()
            .filter(|&f| self.is_valid(base, f))
            .unwrap_or(0)
    }
}

/// Reads which files are which formes, from the species' first forme file and forme count.
pub(crate) fn read_formes(rom: &mut Rom, path: &str) -> Result<FormeTable, RomError> {
    let mut table = FormeTable::default();
    for n in 1..=c::SPECIES_COUNT {
        let data = rom.garc_file(path, false, n)?;
        let count = data.u8_at(c::BS_FORME_COUNT)?;
        let first = usize::from(data.u16_at(c::BS_FORME_OFFSET)?);
        let base = DexNumber::try_from(n).unwrap_or_default();
        if count > 1 && first != 0 {
            for f in 1..count {
                table
                    .formes
                    .entry(first + usize::from(f) - 1)
                    .or_insert(Forme { base, number: f });
            }
        } else if count > 1 {
            table.cosmetic_counts.insert(base, count);
        }
    }
    Ok(table)
}

/// The Mega Evolutions by base species and forme number, with the Mega Stone each needs (`None` for Mega
/// Rayquaza, which needs none).
pub(crate) fn read_megas(rom: &mut Rom, path: &str) -> Result<BTreeMap<(DexNumber, u8), Option<ItemId>>, RomError> {
    let mut megas = BTreeMap::new();
    for n in 1..=c::SPECIES_COUNT {
        let base = DexNumber::try_from(n).unwrap_or_default();
        for (forme, item) in parse_megas(&rom.garc_file(path, false, n)?, 3) {
            megas.insert((base, forme), item);
        }
    }
    Ok(megas)
}

/// What makes species number `n` a forme, or the base species it is.
pub(crate) fn forme_of(
    n: DexNumber,
    table: &FormeTable,
    family: Family,
    megas: &BTreeMap<(DexNumber, u8), Option<ItemId>>,
) -> ppr_model::Forme {
    match table.formes.get(&usize::from(n)) {
        Some(f) => {
            let mega = megas.get(&(f.base, f.number)).copied();
            ppr_model::Forme {
                base: Some(f.base),
                number: f.number,
                suffix: if mega.is_some() {
                    mega_suffix(f.base, f.number)
                } else {
                    suffix_in(&c::FORME_SUFFIXES, f.base, f.number)
                }
                .to_owned(),
                cosmetic: c::is_cosmetic_forme(n),
                irregular: c::is_irregular_forme(n, family),
                mega: mega.is_some(),
                mega_item: mega.flatten(),
                ..ppr_model::Forme::default()
            }
        }
        None => ppr_model::Forme {
            suffix: base_suffix(n).to_owned(),
            cosmetic_formes: match table.cosmetic_counts.get(&n) {
                Some(&count) if !c::IGNORED_COSMETIC_FORMES.contains(&n) => (1..count).collect(),
                _ => Vec::new(),
            },
            ..ppr_model::Forme::default()
        },
    }
}

/// The number of files with personal data: the species and the formes, from 1.
pub(crate) fn entry_count(family: Family) -> usize {
    c::SPECIES_COUNT + family.forme_count()
}

/// The species' names, 0 to 721.
pub(crate) fn names(texts: &[String]) -> Result<Vec<String>, RomError> {
    if texts.len() <= c::SPECIES_COUNT {
        return Err(RomError::Malformed("too few species names".into()));
    }
    Ok(texts[..=c::SPECIES_COUNT].to_vec())
}

fn item(id: u16) -> Option<ItemId> {
    (id != 0).then_some(id)
}

/// One species from its personal data.
pub(crate) fn read_species(number: DexNumber, name: &str, data: &[u8]) -> Result<Species, RomError> {
    let stats = BaseStats {
        // Shedinja always has 1 HP.
        hp: if number == c::SHEDINJA {
            1
        } else {
            data.u8_at(c::BS_HP)?
        },
        attack: data.u8_at(c::BS_ATTACK)?,
        defense: data.u8_at(c::BS_DEFENSE)?,
        speed: data.u8_at(c::BS_SPEED)?,
        sp_attack: data.u8_at(c::BS_SP_ATK)?,
        sp_defense: data.u8_at(c::BS_SP_DEF)?,
    };
    let primary = c::type_of(data.u8_at(c::BS_TYPE1)?)
        .ok_or_else(|| RomError::Malformed(format!("species {number} has an unknown type")))?;
    let secondary = c::type_of(data.u8_at(c::BS_TYPE2)?).filter(|&t| t != primary);
    let a1 = u16::from(data.u8_at(c::BS_ABILITY1)?);
    let mut a2 = u16::from(data.u8_at(c::BS_ABILITY2)?);
    if a2 == a1 {
        a2 = 0;
    }
    let a3 = u16::from(data.u8_at(c::BS_ABILITY3)?);
    let growth = data.u8_at(c::BS_GROWTH)?;
    Ok(Species {
        forme: ppr_model::Forme::default(),
        number,
        name: name.to_owned(),
        primary_type: primary,
        secondary_type: secondary,
        stats,
        catch_rate: data.u8_at(c::BS_CATCH_RATE)?,
        abilities: vec![a1, a2, a3],
        growth_curve: ExpCurve::from_byte(growth)
            .ok_or_else(|| RomError::Malformed(format!("species {number} has growth curve {growth}")))?,
    })
}

/// A species' wild held items: one item in both slots is guaranteed.
pub(crate) fn read_held_items(data: &[u8]) -> Result<WildHeldItems, RomError> {
    let common = data.u16_at(c::BS_COMMON_ITEM)?;
    let rare = data.u16_at(c::BS_RARE_ITEM)?;
    Ok(if common == rare {
        WildHeldItems {
            guaranteed: item(common),
            ..WildHeldItems::default()
        }
    } else {
        WildHeldItems {
            common: item(common),
            rare: item(rare),
            ..WildHeldItems::default()
        }
    })
}

/// Writes a species' stats, types, catch rate, growth curve and abilities into its personal data. A second ability
/// of 0 means the first; one type means it twice.
pub(crate) fn write_species(sp: &Species, data: &mut [u8]) -> Result<(), RomError> {
    let s = sp.stats;
    data.set_u8(c::BS_HP, s.hp)?;
    data.set_u8(c::BS_ATTACK, s.attack)?;
    data.set_u8(c::BS_DEFENSE, s.defense)?;
    data.set_u8(c::BS_SPEED, s.speed)?;
    data.set_u8(c::BS_SP_ATK, s.sp_attack)?;
    data.set_u8(c::BS_SP_DEF, s.sp_defense)?;
    let primary = c::type_byte(sp.primary_type);
    data.set_u8(c::BS_TYPE1, primary)?;
    data.set_u8(c::BS_TYPE2, sp.secondary_type.map_or(primary, c::type_byte))?;
    data.set_u8(c::BS_CATCH_RATE, sp.catch_rate)?;
    data.set_u8(c::BS_GROWTH, sp.growth_curve.to_byte())?;
    let ability = |i: usize| u8::try_from(sp.abilities.get(i).copied().unwrap_or(0)).unwrap_or(0);
    let a1 = ability(0);
    data.set_u8(c::BS_ABILITY1, a1)?;
    data.set_u8(c::BS_ABILITY2, if ability(1) == 0 { a1 } else { ability(1) })?;
    data.set_u8(c::BS_ABILITY3, ability(2))?;
    // The female Meowstic is a forme of its own; Espurr's evolution method picks it, so each forme has one
    // gender.
    if sp.number == c::MEOWSTIC {
        data.set_u8(c::BS_GENDER, 0)?;
    }
    Ok(())
}

/// Writes a species' wild held items.
pub(crate) fn write_held_items(held: &WildHeldItems, data: &mut [u8]) -> Result<(), RomError> {
    let (common, rare) = match held.guaranteed {
        Some(g) => (g, g),
        None => (held.common.unwrap_or(0), held.rare.unwrap_or(0)),
    };
    data.set_u16(c::BS_COMMON_ITEM, common)?;
    data.set_u16(c::BS_RARE_ITEM, rare)
}

/// Copies every entry's data into the aggregate file at the end of the personal data GARC.
pub(crate) fn update_aggregate(rom: &mut Rom, path: &str, family: Family) -> Result<(), RomError> {
    let size = family.personal_size();
    let count = entry_count(family);
    let mut aggregate = rom.garc_file(path, false, count + 1)?;
    for i in 1..=count {
        let data = rom.garc_file(path, false, i)?;
        let at = size * i;
        let target = aggregate
            .get_mut(at..at + data.len())
            .ok_or_else(|| RomError::Malformed("the aggregate personal data is too short".into()))?;
        target.copy_from_slice(&data);
    }
    rom.set_garc_file(path, false, count + 1, aggregate)
}

/// The female Meowstic forme's gender byte: always female.
pub(crate) const MEOWSTIC_F_GENDER: u8 = 0xFE;

#[cfg(test)]
mod tests {
    use ppr_model::Type;

    use super::*;

    fn personal() -> Vec<u8> {
        let mut d = vec![0u8; 0x40];
        d[..6].copy_from_slice(&[45, 49, 49, 45, 65, 65]);
        d[c::BS_TYPE1] = 11;
        d[c::BS_TYPE2] = 3;
        d[c::BS_CATCH_RATE] = 45;
        d[c::BS_GROWTH] = 3;
        d[c::BS_ABILITY1] = 65;
        d[c::BS_ABILITY2] = 65;
        d[c::BS_ABILITY3] = 34;
        d[c::BS_COMMON_ITEM] = 7;
        d[c::BS_RARE_ITEM] = 7;
        d
    }

    #[test]
    fn species_read_and_write_back() {
        let data = personal();
        let sp = read_species(1, "Bulbasaur", &data).expect("reads");
        assert_eq!((sp.primary_type, sp.secondary_type), (Type::Grass, Some(Type::Poison)));
        assert_eq!(sp.abilities, [65, 0, 34]);
        assert_eq!(sp.stats.sp_defense, 65);
        let held = read_held_items(&data).expect("reads");
        assert_eq!(held.guaranteed, Some(7));
        let mut out = vec![0u8; 0x40];
        write_species(&sp, &mut out).expect("writes");
        write_held_items(&held, &mut out).expect("writes");
        assert_eq!(out, data);
    }

    #[test]
    fn one_type_is_written_twice() {
        let mut data = personal();
        data[c::BS_TYPE2] = 11;
        let sp = read_species(1, "", &data).expect("reads");
        assert_eq!(sp.secondary_type, None);
        let mut out = data.clone();
        write_species(&sp, &mut out).expect("writes");
        assert_eq!(out[c::BS_TYPE2], 11);
    }
}
