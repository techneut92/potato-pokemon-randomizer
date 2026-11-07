//! Personal data: base stats, types, abilities, held items, TM and tutor compatibility of each species and
//! alternate forme (`PokemonStats`).
//!
//! The archive holds file 0, one 0x54-byte entry per species (1 to the species count), one per alternate forme with
//! stats of its own after them, and a last file with all those entries one after another (from offset 0x54), which
//! is kept equal to them. An alternate forme's entry number is its species number in the game data; it has its base
//! species' name. A species' entry names its first forme entry and its number of formes.
//!
//! Deliberate choices:
//! - A species with cosmetic formes but no entries for them (Vivillon's patterns) gets forme numbers 1 up to its
//!   number of formes less one; the original gave it one more, a forme the game doesn't have.
//! - A forme's suffix is its own (`-Midnight`), without its base species' (`-Midday`) before it.

use std::collections::BTreeMap;

use ppr_model::{BaseStats, DexNumber, ExpCurve, ItemId, Species, Type, WildHeldItems};

use super::constants::{self as c, Family};
use super::{put_u16, word};
use crate::RomError;
use crate::formes::{base_suffix, mega_suffix, suffix_in};

/// The Mega Evolutions by base species and forme number, with the Mega Stone each needs.
pub(super) type Megas = BTreeMap<(DexNumber, u8), Option<ItemId>>;

/// The game's type byte order: Normal, Fighting, … Dark, Fairy, the order of [`Type::ALL`].
pub(super) fn type_of(b: u8) -> Option<Type> {
    Type::ALL.get(usize::from(b)).copied()
}

pub(super) fn type_byte(t: Type) -> u8 {
    Type::ALL
        .iter()
        .position(|&x| x == t)
        .and_then(|i| u8::try_from(i).ok())
        .unwrap_or_default()
}

/// A species or forme number of the game: alternate formes get the numbers after the last species, in the order
/// of their entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Forme {
    pub(super) species: DexNumber,
    pub(super) forme: u8,
}

/// The species number of forme `forme` of `base`: the alternate forme's entry for a forme with an entry of its
/// own, else `base`.
pub(super) fn species_of(formes: &BTreeMap<usize, Forme>, base: DexNumber, forme: u8) -> DexNumber {
    if forme == 0 {
        return base;
    }
    formes
        .iter()
        .find(|&(_, f)| f.species == base && f.forme == forme)
        .and_then(|(&i, _)| DexNumber::try_from(i).ok())
        .unwrap_or(base)
}

/// The base species of species number `n`.
pub(super) fn base_of(formes: &BTreeMap<usize, Forme>, n: DexNumber) -> DexNumber {
    formes.get(&usize::from(n)).map_or(n, |f| f.species)
}

/// Where the alternate formes are, for mapping stored species and formes to game-data species: what wild
/// encounters keep of the personal entries.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct FormeIndex {
    formes: BTreeMap<usize, Forme>,
    cosmetic_counts: BTreeMap<DexNumber, u8>,
}

impl FormeIndex {
    /// An index of the alternate formes `formes` (species number, base species, forme number) and the cosmetic
    /// forme counts `cosmetic` (base species, formes in all), for tests.
    #[cfg(test)]
    pub(super) fn of(formes: &[(usize, DexNumber, u8)], cosmetic: &[(DexNumber, u8)]) -> Self {
        Self {
            formes: formes
                .iter()
                .map(|&(n, species, forme)| (n, Forme { species, forme }))
                .collect(),
            cosmetic_counts: cosmetic.iter().copied().collect(),
        }
    }

    /// The species number of forme `forme` of `base` ([`species_of`]).
    pub(super) fn species_of(&self, base: DexNumber, forme: u8) -> DexNumber {
        species_of(&self.formes, base, forme)
    }

    /// The base species of species number `n`.
    pub(super) fn base(&self, n: DexNumber) -> DexNumber {
        base_of(&self.formes, n)
    }

    /// The forme number of species number `n`: an alternate forme's, else 0.
    pub(super) fn forme_number(&self, n: DexNumber) -> u8 {
        self.formes.get(&usize::from(n)).map_or(0, |f| f.forme)
    }

    /// The forme a Pokémon stored as `base` with forme `stored` is in the game data: the stored one when the
    /// species has it (an entry of its own or a cosmetic forme), else 0.
    pub(super) fn holder_forme(&self, base: DexNumber, stored: u16) -> u8 {
        u8::try_from(stored)
            .ok()
            .filter(|&f| {
                f == 0
                    || self.formes.values().any(|e| e.species == base && e.forme == f)
                    || self.cosmetic_counts.get(&base).is_some_and(|&count| f < count)
            })
            .unwrap_or(0)
    }
}

/// The personal entries and where the alternate formes are.
#[derive(Debug, Clone)]
pub(super) struct Personal {
    /// Entries by index: 0 is unused, then species, then formes.
    pub(super) entries: Vec<Vec<u8>>,
    /// The species and forme of each alternate forme entry, by entry index.
    pub(super) formes: BTreeMap<usize, Forme>,
    /// The number of formes of each species whose formes have no entries of their own, counting the base forme.
    pub(super) cosmetic_counts: BTreeMap<DexNumber, u8>,
    pub(super) family: Family,
}

impl Personal {
    pub(super) fn read(garc: &ppr_formats::ctr::Garc, family: Family) -> Result<Self, RomError> {
        let total = family.pokemon_count() + family.forme_count();
        let mut entries = Vec::with_capacity(total + 1);
        for i in 0..=total {
            let e = garc.file(i)?;
            if i > 0 && e.len() < c::BS_SIZE {
                return Err(RomError::Malformed(format!("personal entry {i} is too short")));
            }
            entries.push(e);
        }
        let mut formes = BTreeMap::new();
        let mut cosmetic_counts = BTreeMap::new();
        for (n, e) in entries.iter().enumerate().take(family.pokemon_count() + 1).skip(1) {
            let count = e[c::BS_FORME_COUNT];
            let first = usize::from(word(e, c::BS_FORME)?);
            let species = DexNumber::try_from(n).unwrap_or_default();
            if count > 1 && first != 0 {
                for f in 1..count {
                    // The formes follow each other in the order of their numbers.
                    formes
                        .entry(first + usize::from(f) - 1)
                        .or_insert(Forme { species, forme: f });
                }
            } else if count > 1 {
                cosmetic_counts.insert(species, count);
            }
        }
        Ok(Self {
            entries,
            formes,
            cosmetic_counts,
            family,
        })
    }

    /// Where the alternate formes are.
    pub(super) fn forme_index(&self) -> FormeIndex {
        FormeIndex {
            formes: self.formes.clone(),
            cosmetic_counts: self.cosmetic_counts.clone(),
        }
    }

    /// The species number of forme `forme` of `base` ([`species_of`]).
    pub(super) fn species_of(&self, base: DexNumber, forme: u8) -> DexNumber {
        species_of(&self.formes, base, forme)
    }

    /// The base species of species number `n`.
    pub(super) fn base(&self, n: DexNumber) -> DexNumber {
        base_of(&self.formes, n)
    }

    /// The forme number of species number `n`: an alternate forme's, else 0.
    pub(super) fn forme_number(&self, n: DexNumber) -> u8 {
        self.formes.get(&usize::from(n)).map_or(0, |f| f.forme)
    }

    /// The forme a Pokémon stored as `base` with forme `stored` is in the game data ([`FormeIndex::holder_forme`]).
    pub(super) fn holder_forme(&self, base: DexNumber, stored: u16) -> u8 {
        u8::try_from(stored)
            .ok()
            .filter(|&f| {
                f == 0 || self.has_forme(base, f) || self.cosmetic_counts.get(&base).is_some_and(|&count| f < count)
            })
            .unwrap_or(0)
    }

    /// The number of entries: the species and the formes.
    pub(super) fn total(&self) -> usize {
        self.family.pokemon_count() + self.family.forme_count()
    }

    /// Whether the species has an alternate forme entry with that number.
    pub(super) fn has_forme(&self, species: DexNumber, forme: u8) -> bool {
        forme == 0 || self.formes.values().any(|f| f.species == species && f.forme == forme)
    }

    /// The base species of a game number (an alternate forme above the species count).
    #[cfg(test)]
    pub(super) fn base_of_number(&self, number: usize) -> Option<DexNumber> {
        let count = self.family.pokemon_count();
        if number <= count {
            return DexNumber::try_from(number).ok();
        }
        self.formes.values().nth(number - count - 1).map(|f| f.species)
    }

    /// The archive with the entries written back, the last file included.
    pub(super) fn write(&self, garc: &mut ppr_formats::ctr::Garc) -> Result<(), RomError> {
        let total = self.family.pokemon_count() + self.family.forme_count();
        let mut all = garc.file(total + 1)?;
        for i in 1..=total {
            let at = i * c::BS_SIZE;
            let e = &self.entries[i];
            if all.len() >= at + e.len() {
                all[at..at + e.len()].copy_from_slice(e);
            }
            garc.set_file(i, e.clone())?;
        }
        garc.set_file(total + 1, all)?;
        Ok(())
    }
}

fn item(id: u16) -> Option<ItemId> {
    (id != 0).then_some(id)
}

/// What makes species number `n` a forme, or the base species it is.
fn forme_of(personal: &Personal, n: DexNumber, megas: &Megas) -> ppr_model::Forme {
    let family = personal.family;
    match personal.formes.get(&usize::from(n)) {
        Some(f) => {
            let mega = megas.get(&(f.species, f.forme)).copied();
            ppr_model::Forme {
                base: Some(f.species),
                number: f.forme,
                suffix: if mega.is_some() {
                    mega_suffix(f.species, f.forme)
                } else {
                    suffix_in(&c::FORME_SUFFIXES, f.species, f.forme)
                }
                .to_owned(),
                cosmetic: c::is_cosmetic_forme(n, family),
                irregular: c::is_irregular_forme(n, family),
                mega: mega.is_some(),
                mega_item: mega.flatten(),
                conceptual_base: c::conceptual_base(n, family),
                ..ppr_model::Forme::default()
            }
        }
        None => ppr_model::Forme {
            suffix: base_suffix(n).to_owned(),
            cosmetic_formes: match personal.cosmetic_counts.get(&n) {
                Some(&count) if !c::IGNORED_COSMETIC_FORMES.contains(&n) => (1..count).collect(),
                _ => Vec::new(),
            },
            ..ppr_model::Forme::default()
        },
    }
}

/// The species by national number, then the alternate formes with entries of their own.
pub(super) fn species(personal: &Personal, names: &[String], megas: &Megas) -> Result<Vec<Species>, RomError> {
    (1..=personal.total())
        .map(|n| {
            let e = &personal.entries[n];
            let number = DexNumber::try_from(n).unwrap_or_default();
            let base = usize::from(personal.base(number));
            let primary = type_of(e[c::BS_PRIMARY_TYPE])
                .ok_or_else(|| RomError::Malformed(format!("species {n} has an unknown type")))?;
            let secondary = type_of(e[c::BS_SECONDARY_TYPE]).filter(|&t| t != primary);
            let (a1, a2, a3) = (e[c::BS_ABILITY_1], e[c::BS_ABILITY_2], e[c::BS_ABILITY_3]);
            Ok(Species {
                forme: forme_of(personal, number, megas),
                number,
                name: names.get(base).cloned().unwrap_or_default(),
                primary_type: primary,
                secondary_type: secondary,
                stats: BaseStats {
                    hp: e[c::BS_HP],
                    attack: e[c::BS_ATTACK],
                    defense: e[c::BS_DEFENSE],
                    speed: e[c::BS_SPEED],
                    sp_attack: e[c::BS_SP_ATTACK],
                    sp_defense: e[c::BS_SP_DEFENSE],
                },
                catch_rate: e[c::BS_CATCH_RATE],
                abilities: vec![u16::from(a1), if a2 == a1 { 0 } else { u16::from(a2) }, u16::from(a3)],
                growth_curve: ExpCurve::from_byte(e[c::BS_GROWTH_CURVE]).unwrap_or(ExpCurve::MediumFast),
            })
        })
        .collect()
}

/// What wild Pokémon of each species hold; the same item in both slots is always held.
pub(super) fn wild_held_items(personal: &Personal) -> Result<BTreeMap<DexNumber, WildHeldItems>, RomError> {
    let mut out = BTreeMap::new();
    for n in 1..=personal.total() {
        let e = &personal.entries[n];
        let (common, rare) = (word(e, c::BS_COMMON_HELD_ITEM)?, word(e, c::BS_RARE_HELD_ITEM)?);
        let held = if common == rare {
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
        };
        if !held.is_empty() {
            out.insert(DexNumber::try_from(n).unwrap_or_default(), held);
        }
    }
    Ok(out)
}

fn flags(e: &[u8], at: usize, bytes: usize, count: usize) -> Vec<bool> {
    (0..count)
        .map(|i| i / 8 < bytes && e[at + i / 8] >> (i % 8) & 1 == 1)
        .collect()
}

fn put_flags(e: &mut [u8], at: usize, bytes: usize, f: &[bool]) {
    for b in 0..bytes {
        let mut v = 0u8;
        for bit in 0..8 {
            if f.get(b * 8 + bit).copied().unwrap_or(false) {
                v |= 1 << bit;
            }
        }
        e[at + b] = v;
    }
}

/// TM compatibility of each species.
pub(super) fn tm_compat(personal: &Personal) -> BTreeMap<DexNumber, Vec<bool>> {
    (1..=personal.total())
        .map(|n| {
            let e = &personal.entries[n];
            (
                DexNumber::try_from(n).unwrap_or_default(),
                flags(e, c::BS_TM_COMPAT, c::TM_COMPAT_BYTES, c::TM_COUNT),
            )
        })
        .collect()
}

/// Move tutor compatibility of each species.
pub(super) fn tutor_compat(personal: &Personal) -> BTreeMap<DexNumber, Vec<bool>> {
    (1..=personal.total())
        .map(|n| {
            let e = &personal.entries[n];
            (
                DexNumber::try_from(n).unwrap_or_default(),
                flags(e, c::BS_TUTOR_COMPAT, c::TUTOR_COMPAT_BYTES, c::TUTOR_MOVE_COUNT),
            )
        })
        .collect()
}

/// Writes a species' compatibility flags.
pub(super) fn write_tm_compat(personal: &mut Personal, n: DexNumber, f: &[bool]) {
    if let Some(e) = personal.entries.get_mut(usize::from(n)) {
        put_flags(e, c::BS_TM_COMPAT, c::TM_COMPAT_BYTES, f);
    }
}

pub(super) fn write_tutor_compat(personal: &mut Personal, n: DexNumber, f: &[bool]) {
    if let Some(e) = personal.entries.get_mut(usize::from(n)) {
        put_flags(e, c::BS_TUTOR_COMPAT, c::TUTOR_COMPAT_BYTES, f);
    }
}

/// Writes a species' stats, types, catch rate, growth curve and abilities. A second ability of 0 is stored as the
/// first, as the game does for species with one.
pub(super) fn write_species(personal: &mut Personal, sp: &Species) {
    let Some(e) = personal.entries.get_mut(usize::from(sp.number)) else {
        return;
    };
    let b = sp.stats;
    e[c::BS_HP] = b.hp;
    e[c::BS_ATTACK] = b.attack;
    e[c::BS_DEFENSE] = b.defense;
    e[c::BS_SPEED] = b.speed;
    e[c::BS_SP_ATTACK] = b.sp_attack;
    e[c::BS_SP_DEFENSE] = b.sp_defense;
    e[c::BS_PRIMARY_TYPE] = type_byte(sp.primary_type);
    e[c::BS_SECONDARY_TYPE] = type_byte(sp.secondary_type.unwrap_or(sp.primary_type));
    e[c::BS_CATCH_RATE] = sp.catch_rate;
    e[c::BS_GROWTH_CURVE] = sp.growth_curve.to_byte();
    let ability = |i: usize| sp.abilities.get(i).and_then(|&a| u8::try_from(a).ok()).unwrap_or(0);
    let (a1, a2, a3) = (ability(0), ability(1), ability(2));
    e[c::BS_ABILITY_1] = a1;
    e[c::BS_ABILITY_2] = if a2 == 0 { a1 } else { a2 };
    e[c::BS_ABILITY_3] = a3;
}

/// Writes what wild Pokémon of a species hold.
pub(super) fn write_wild_held_items(personal: &mut Personal, n: DexNumber, held: WildHeldItems) {
    let Some(e) = personal.entries.get_mut(usize::from(n)) else {
        return;
    };
    let (common, rare) = match held.guaranteed {
        Some(g) => (g, g),
        None => (held.common.unwrap_or(0), held.rare.unwrap_or(0)),
    };
    put_u16(e, c::BS_COMMON_HELD_ITEM, common);
    put_u16(e, c::BS_RARE_HELD_ITEM, rare);
}

/// The misc tweaks that change every entry, alternate formes included.
pub(super) fn sos_battles_for_all(personal: &mut Personal) {
    for e in personal.entries.iter_mut().skip(1) {
        if e[c::BS_CALL_RATE] == 0 {
            e[c::BS_CALL_RATE] = 5;
        }
    }
}

pub(super) fn fast_egg_hatching(personal: &mut Personal) {
    for e in personal.entries.iter_mut().skip(1) {
        e[c::BS_EGG_CYCLES] = 1;
    }
}

pub(super) fn no_ev_yields(personal: &mut Personal) {
    for e in personal.entries.iter_mut().skip(1) {
        put_u16(e, c::BS_EV_YIELD, 0);
    }
}

/// The highest of Attack, Defense, Special Attack, Special Defense and Speed as an aura boost (1 to 5); ties go to
/// the stat at the species number modulo the number of tied stats.
pub(super) fn aura_for_highest_stat(sp: &Species) -> u8 {
    let b = sp.stats;
    let stats = [b.attack, b.defense, b.sp_attack, b.sp_defense, b.speed];
    let best = stats.iter().copied().max().unwrap_or_default();
    let tied: Vec<u8> = (1u8..=5).filter(|&a| stats[usize::from(a - 1)] == best).collect();
    tied[usize::from(sp.number) % tied.len()]
}
