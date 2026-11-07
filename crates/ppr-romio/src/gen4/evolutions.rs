//! Evolutions, read and written, with Nincada's extra evolution, the baby table and the patch that makes
//! happiness evolutions easier.
//!
//! An evolution by held item at any time is stored twice, by day and by night; the game data keeps the pair
//! as it is stored.
//!
//! Deliberate choices:
//! - The baby table (which species hatches from an egg of another) is updated whenever evolutions change, not
//!   only when species are left out, and only for the species whose first stage changed, so special cases like
//!   Manaphy's eggs hatching into Phione stay.

use ppr_model::{DexNumber, Evolution, EvolutionType};

use super::Rom;
use super::constants as c;
use super::files::{find, put, put_word, word};
use crate::RomError;

/// Gen 4's evolution methods, method `i` at `i - 1`.
const TYPES: [EvolutionType; 26] = [
    EvolutionType::Happiness,
    EvolutionType::HappinessDay,
    EvolutionType::HappinessNight,
    EvolutionType::Level,
    EvolutionType::Trade,
    EvolutionType::TradeItem,
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

/// Where the happiness of day and night evolutions is, from the locator.
const HAPPINESS_DAY_OFFSET: usize = 22;
const HAPPINESS_NIGHT_OFFSET: usize = 44;

fn method_of(kind: EvolutionType) -> Result<u16, RomError> {
    TYPES
        .iter()
        .position(|&t| t == kind)
        .and_then(|i| u16::try_from(i + 1).ok())
        .ok_or_else(|| RomError::Malformed(format!("Gen 4 has no evolutions of type {kind:?}")))
}

/// Reads the evolutions: up to seven per species of method, parameter and species. Burmy also gets links to
/// Wormadam's alternate formes ([`EvolutionType::None`]), as its evolution can take them.
pub(super) fn read(rom: &mut Rom) -> Result<Vec<Evolution>, RomError> {
    let path = rom.entry.file("PokemonEvolutions")?;
    let narc = rom.files.narc(&path)?;
    let mut evolutions: Vec<Evolution> = Vec::new();
    for i in 1..=c::SPECIES_COUNT {
        let from = DexNumber::try_from(i).map_err(|_| RomError::Malformed("too many species".into()))?;
        let data = narc
            .files
            .get(i)
            .ok_or_else(|| RomError::Malformed(format!("no evolutions for species {i}")))?;
        for j in 0..c::EVOLUTIONS_PER_SPECIES {
            let at = j * c::EVOLUTION_ENTRY_SIZE;
            let method = word(data, at)?;
            let to = word(data, at + 4)?;
            if !(1..=TYPES.len()).contains(&usize::from(method)) || to < 1 {
                continue;
            }
            let evo = Evolution::new(from, to, TYPES[usize::from(method - 1)], word(data, at + 2)?);
            if !evolutions.contains(&evo) {
                evolutions.push(evo);
            }
        }
        if from == c::BURMY {
            evolutions.extend(c::WORMADAM_FORMES.map(|to| Evolution::new(from, to, EvolutionType::None, 0)));
        }
    }
    Ok(evolutions)
}

/// Writes the evolutions of the species whose evolutions changed: the first seven, the rest of the slots cleared.
/// An evolution by held item at any time is written as one by day and one by night. Nincada's second evolution
/// is its extra one, whose species the game code holds; the baby table follows the new first stages.
pub(super) fn write(rom: &mut Rom, loaded: &[Evolution], evolutions: &[Evolution]) -> Result<(), RomError> {
    let path = rom.entry.file("PokemonEvolutions")?;
    let stored_only = |list: &[Evolution]| -> Vec<Evolution> {
        list.iter().filter(|e| e.kind != EvolutionType::None).copied().collect()
    };
    let (loaded, evolutions) = (&stored_only(loaded)[..], &stored_only(evolutions)[..]);
    let stored = split_item_evolutions(evolutions);
    let before = split_item_evolutions(loaded);
    for i in 1..=c::SPECIES_COUNT {
        let from = DexNumber::try_from(i).map_err(|_| RomError::Malformed("too many species".into()))?;
        let own: Vec<&Evolution> = stored.iter().filter(|e| e.from == from).collect();
        let old: Vec<&Evolution> = before.iter().filter(|e| e.from == from).collect();
        if own == old {
            continue;
        }
        let mut slots = [[0u16; 3]; c::EVOLUTIONS_PER_SPECIES];
        for (slot, e) in slots.iter_mut().zip(&own) {
            *slot = [method_of(e.kind)?, e.parameter, e.to];
        }
        let data = rom.files.narc_file(&path, i)?;
        for (j, slot) in slots.iter().enumerate() {
            for (k, &w) in slot.iter().enumerate() {
                put_word(data, j * c::EVOLUTION_ENTRY_SIZE + k * 2, w)?;
            }
        }
        if from == c::NINCADA {
            write_shedinja(rom, &own)?;
        }
    }
    write_babies(rom, loaded, evolutions)
}

/// The evolutions as stored: each evolution by held item at any time as one by day and one by night, the night
/// one after all the others of the species.
fn split_item_evolutions(evolutions: &[Evolution]) -> Vec<Evolution> {
    let mut out: Vec<Evolution> = Vec::with_capacity(evolutions.len());
    let mut species: Vec<DexNumber> = evolutions.iter().map(|e| e.from).collect();
    species.dedup();
    for from in species {
        let own = evolutions.iter().filter(|e| e.from == from);
        let mut nights = Vec::new();
        for e in own {
            if e.kind == EvolutionType::Item {
                let mut day = *e;
                day.kind = EvolutionType::ItemDay;
                let mut night = *e;
                night.kind = EvolutionType::ItemNight;
                out.push(day);
                nights.push(night);
            } else {
                out.push(*e);
            }
        }
        out.extend(nights);
    }
    out
}

/// After an evolution with the extra-species method, the game makes a Shedinja with `mov r0, #0x49; lsl r0, r0,
/// #2`; that becomes `mov r0, #low; add r0, #high` for Nincada's second evolution. Without one, nothing changes.
fn write_shedinja(rom: &mut Rom, nincada: &[&Evolution]) -> Result<(), RomError> {
    let Some(extra) = nincada.get(1).map(|e| e.to) else {
        return Ok(());
    };
    let arm9 = rom.files.arm9();
    let Some(at) = find(arm9, c::SHEDINJA_SPECIES_LOCATOR) else {
        return Ok(());
    };
    let (low, high) = match u8::try_from(extra) {
        Ok(low) => (low, 0),
        Err(_) => (u8::MAX, u8::try_from(extra - 255).unwrap_or(u8::MAX)),
    };
    put(arm9, at, &[low, 0x20, high, 0x30])
}

/// The species every species' line starts with: the first species it evolves from, again and again.
fn first_stages(evolutions: &[Evolution]) -> Vec<DexNumber> {
    (0..=c::SPECIES_COUNT)
        .map(|i| {
            let mut baby = DexNumber::try_from(i).unwrap_or_default();
            let mut seen = vec![baby];
            while let Some(e) = evolutions.iter().find(|e| e.to == baby) {
                if seen.contains(&e.from) {
                    break;
                }
                baby = e.from;
                seen.push(baby);
            }
            baby
        })
        .collect()
}

/// The baby table: a word per species, its first stage.
fn write_babies(rom: &mut Rom, loaded: &[Evolution], evolutions: &[Evolution]) -> Result<(), RomError> {
    if !rom.entry.has_file("BabyPokemon") {
        return Ok(());
    }
    let (old, new) = (first_stages(loaded), first_stages(evolutions));
    let path = rom.entry.file("BabyPokemon")?;
    let table = rom.files.file(&path)?;
    for (i, (&o, &n)) in old.iter().zip(&new).enumerate().skip(1) {
        if o != n {
            put_word(table, i * 2, n)?;
        }
    }
    Ok(())
}

/// Makes happiness evolutions (at any time, by day and by night) need 160 instead of 220.
pub(super) fn ease_happiness(rom: &mut Rom) {
    let arm9 = rom.files.arm9();
    let Some(at) = find(arm9, c::FRIENDSHIP_VALUE_FOR_EVO_LOCATOR) else {
        return;
    };
    for o in [at, at + HAPPINESS_DAY_OFFSET, at + HAPPINESS_NIGHT_OFFSET] {
        if arm9.get(o) == Some(&c::VANILLA_HAPPINESS_TO_EVOLVE) {
            arm9[o] = c::EASIER_HAPPINESS_TO_EVOLVE;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_round_trip() {
        for (i, &t) in TYPES.iter().enumerate() {
            assert_eq!(usize::from(method_of(t).expect("Gen 4")), i + 1);
        }
        assert!(method_of(EvolutionType::Item).is_err());
    }

    #[test]
    fn item_evolutions_are_stored_by_day_and_by_night() {
        let evolutions = [
            Evolution::new(440, 113, EvolutionType::Item, 110),
            Evolution::new(440, 999, EvolutionType::Level, 30),
        ];
        let stored = split_item_evolutions(&evolutions);
        let kinds: Vec<EvolutionType> = stored.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            [EvolutionType::ItemDay, EvolutionType::Level, EvolutionType::ItemNight]
        );
    }

    #[test]
    fn first_stages_follow_evolutions_back() {
        let evolutions = [
            Evolution::new(1, 2, EvolutionType::Level, 16),
            Evolution::new(2, 3, EvolutionType::Level, 32),
        ];
        let stages = first_stages(&evolutions);
        assert_eq!(stages[3], 1);
        assert_eq!(stages[4], 4);
    }
}
