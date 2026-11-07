//! Evolutions, read and written, and the patch that makes happiness evolutions easier.

use ppr_model::{DexNumber, Evolution, EvolutionType, Species};

use super::constants as c;
use super::{PokedexOrder, found_at, items, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::{GbaRom, hex};

/// Gen 3's evolution methods, method `i` at `i - 1`.
const TYPES: [EvolutionType; 15] = [
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
];

/// Locates the happiness check of evolutions: the happiness a happiness evolution needs, minus one, starts it.
const FRIENDSHIP_VALUE_LOCATOR: &str = "DB2900D8";
/// Where the happiness of day and night evolutions is, from the locator; `FireRed` and `LeafGreen` have no clock.
const FRIENDSHIP_DAY_OFFSET: usize = 38;
const FRIENDSHIP_NIGHT_OFFSET: usize = 66;
/// The happiness a happiness evolution needs in the games, and with the easier evolutions patch.
const VANILLA_HAPPINESS: u8 = 220;
const EASIER_HAPPINESS: u8 = 160;

fn method_of(kind: EvolutionType) -> Result<u16, RomError> {
    TYPES
        .iter()
        .position(|&t| t == kind)
        .and_then(|i| u16::try_from(i + 1).ok())
        .ok_or_else(|| RomError::Malformed(format!("Gen 3 has no evolutions of type {kind:?}")))
}

/// Reads the evolutions: up to five per species; item parameters become standard item numbers.
pub(super) fn read(
    rom: &GbaRom,
    entry: &RomEntry,
    species: &[Species],
    order: &PokedexOrder,
    internal_count: usize,
) -> Result<Vec<Evolution>, RomError> {
    let base = need(entry, "PokemonEvolutions")?;
    let mut evolutions: Vec<Evolution> = Vec::new();
    for sp in species {
        let index = usize::from(order.dex_to_internal[usize::from(sp.number)]);
        let at = base + index * c::EVOLUTION_ENTRY_SIZE;
        for j in 0..c::EVOLUTIONS_PER_SPECIES {
            let method = rom.u16(at + j * 8)?;
            let target = usize::from(rom.u16(at + j * 8 + 4)?);
            if !(1..=c::EVOLUTION_METHOD_COUNT).contains(&method) || !(1..=internal_count).contains(&target) {
                continue;
            }
            let to = order.internal_to_dex[target];
            if to == 0 {
                continue;
            }
            let kind = TYPES[usize::from(method - 1)];
            let mut parameter = rom.u16(at + j * 8 + 2)?;
            if kind.uses_item() {
                parameter = items::standard(parameter);
            }
            let evo = Evolution::new(sp.number, to, kind, parameter);
            if !evolutions.contains(&evo) {
                evolutions.push(evo);
            }
        }
    }
    Ok(evolutions)
}

/// Writes each species' first five evolutions, the rest of its slots cleared.
pub(super) fn write(
    rom: &mut GbaRom,
    entry: &RomEntry,
    species: &[Species],
    evolutions: &[Evolution],
    internal_of: impl Fn(DexNumber) -> Option<u16>,
) -> Result<(), RomError> {
    let base = need(entry, "PokemonEvolutions")?;
    for sp in species {
        let index = internal_of(sp.number).ok_or_else(|| RomError::Malformed(format!("no species {}", sp.number)))?;
        let at = base + usize::from(index) * c::EVOLUTION_ENTRY_SIZE;
        let own = evolutions.iter().filter(|e| e.from == sp.number);
        let mut slots = [[0u16; 4]; c::EVOLUTIONS_PER_SPECIES];
        for (slot, e) in slots.iter_mut().zip(own) {
            let parameter = if e.kind.uses_item() {
                items::internal(e.parameter)
                    .ok_or_else(|| RomError::Malformed(format!("Gen 3 has no item {}", e.parameter)))?
            } else {
                e.parameter
            };
            let to = internal_of(e.to).ok_or_else(|| RomError::Malformed(format!("no species {}", e.to)))?;
            *slot = [method_of(e.kind)?, parameter, to, 0];
        }
        for (j, slot) in slots.iter().enumerate() {
            for (k, &word) in slot.iter().enumerate() {
                rom.write_u16(at + j * 8 + k * 2, word)?;
            }
        }
    }
    Ok(())
}

/// Makes happiness evolutions need 160 instead of 220.
pub(super) fn ease_happiness(rom: &mut GbaRom, has_clock: bool) -> Result<(), RomError> {
    let Some(at) = found_at(rom.find(&hex(FRIENDSHIP_VALUE_LOCATOR)?)) else {
        return Ok(());
    };
    let mut offsets = vec![at];
    if has_clock {
        offsets.extend([at + FRIENDSHIP_DAY_OFFSET, at + FRIENDSHIP_NIGHT_OFFSET]);
    }
    for o in offsets {
        if rom.slice(o, 1)?[0] == VANILLA_HAPPINESS - 1 {
            rom.write(o, &[EASIER_HAPPINESS - 1])?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn methods_round_trip() {
        for (i, &t) in TYPES.iter().enumerate() {
            assert_eq!(usize::from(method_of(t).expect("Gen 3")), i + 1);
        }
        assert!(method_of(EvolutionType::LevelDay).is_err());
    }
}
