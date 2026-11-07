//! Abilities: their names, the highest index, the abilities that are variations of each other and the useless ones.

use std::collections::BTreeMap;

use super::entry_offset;
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;
use crate::text::TextTable;

/// The highest ability index: Air Lock.
pub(super) const HIGHEST: u16 = 77;

/// Cacophony, which no species has; Gen 3 only, so later games number Air Lock 76.
const CACOPHONY: u16 = 76;
const AIR_LOCK: u16 = 77;
const FORECAST: u16 = 59;

/// Insomnia, Clear Body, Huge Power, Battle Armor and Cloud Nine, each with the
/// ability that does the same.
pub(super) fn variations() -> BTreeMap<u16, Vec<u16>> {
    [(15, 72), (29, 73), (37, 74), (4, 75), (13, AIR_LOCK)]
        .into_iter()
        .map(|(base, twin)| (base, vec![base, twin]))
        .collect()
}

/// Abilities that do nothing in battle: Forecast and Cacophony.
pub(super) fn useless() -> Vec<u16> {
    vec![FORECAST, CACOPHONY]
}

/// The ability names, 0 to [`HIGHEST`].
pub(super) fn names(rom: &GbaRom, entry: &RomEntry, text: &TextTable) -> Result<Vec<String>, RomError> {
    let at = entry_offset(rom, entry, "AbilityNames")?;
    let len = entry
        .int("AbilityNameLength")
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| RomError::Malformed("no AbilityNameLength".into()))?;
    (0..=usize::from(HIGHEST))
        .map(|i| Ok(text.decode(rom.slice(at + i * len, len)?, len)))
        .collect()
}
