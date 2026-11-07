//! What the 3DS handlers share about alternate formes.

use ppr_model::{DexNumber, ItemId};

/// The Mega Stones, Gengarite to Latiosite, Swampertite to Diancite and Cameruptite to Beedrillite: the same in all
/// 3DS games, even where a game lacks some of them.
pub(crate) fn mega_stones() -> Vec<ItemId> {
    (656..=685).chain(752..=764).chain(767..=770).collect()
}

/// Base species whose full name has a suffix too, as their formes are told apart from them: Wormadam's Plant
/// Cloak, the Red-Striped Basculin, the male Meowstic, the small Pumpkaboo and Gourgeist, Oricorio's Baile Style
/// and the Midday Lycanroc.
const BASE_FORME_SUFFIXES: [(DexNumber, &str); 7] = [
    (413, "-Plant"),
    (550, "-Red"),
    (678, "-M"),
    (710, "-S"),
    (711, "-S"),
    (741, "-Baile"),
    (745, "-Midday"),
];

/// The suffix of base species `n`'s full name; empty for most.
pub(crate) fn base_suffix(n: DexNumber) -> &'static str {
    BASE_FORME_SUFFIXES
        .iter()
        .find(|&&(b, _)| b == n)
        .map_or("", |&(_, s)| s)
}

/// Charizard and Mewtwo, which have two Mega Evolutions.
const TWO_MEGAS: [DexNumber; 2] = [6, 150];

/// The suffix of a Mega Evolution, by its base species and forme number: `-Mega`, or `-Mega-X` and `-Mega-Y`.
pub(crate) fn mega_suffix(base: DexNumber, forme: u8) -> &'static str {
    match (TWO_MEGAS.contains(&base), forme) {
        (false, _) => "-Mega",
        (true, 1) => "-Mega-X",
        (true, _) => "-Mega-Y",
    }
}

/// The suffix in a table of (base species, suffixes of formes 1 on), else empty.
pub(crate) fn suffix_in(table: &[(DexNumber, &[&'static str])], base: DexNumber, forme: u8) -> &'static str {
    table
        .iter()
        .find(|&&(b, _)| b == base)
        .and_then(|&(_, suffixes)| suffixes.get(usize::from(forme).checked_sub(1)?))
        .copied()
        .unwrap_or("")
}

/// A species' entry of the Mega Evolution table: `slots` entries of 8 bytes, each a forme number, a method (0 for
/// none, 1 with a Mega Stone, 2 without, as Mega Rayquaza) and the item. Gives each Mega Evolution's forme number and
/// Mega Stone.
pub(crate) fn parse_megas(data: &[u8], slots: usize) -> Vec<(u8, Option<ItemId>)> {
    let word = |at: usize| data.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]));
    (0..slots)
        .filter_map(|slot| {
            let at = slot * 8;
            let (forme, method, item) = (word(at)?, word(at + 2)?, word(at + 4)?);
            (method >= 1).then(|| (u8::try_from(forme).unwrap_or_default(), (method == 1).then_some(item)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mega_entries_name_their_forme_and_stone() {
        let mut data = vec![0u8; 24];
        data[..6].copy_from_slice(&[1, 0, 1, 0, 0x93, 0x02]);
        data[8..14].copy_from_slice(&[2, 0, 2, 0, 0, 0]);
        assert_eq!(parse_megas(&data, 3), [(1, Some(0x293)), (2, None)]);
        assert_eq!(parse_megas(&data[..8], 3), [(1, Some(0x293))]);
    }

    #[test]
    fn suffixes_by_species_and_forme() {
        assert_eq!((mega_suffix(6, 2), mega_suffix(3, 1)), ("-Mega-Y", "-Mega"));
        assert_eq!(suffix_in(&[(479, &["-Heat", "-Wash"])], 479, 2), "-Wash");
        assert_eq!(suffix_in(&[(479, &["-Heat"])], 479, 3), "");
        assert_eq!((base_suffix(413), base_suffix(1)), ("-Plant", ""));
    }
}
