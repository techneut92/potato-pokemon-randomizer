//! The species' normal and shiny palettes, in the Pokémon graphics archive (`PokemonGraphics`): six files per
//! species, the palettes fifth and sixth. The species with graphical alternate formes (Unown, Castform, Deoxys,
//! Burmy, Wormadam, Cherrim, Shellos, Gastrodon and Arceus, from Platinum on Rotom, Giratina and Shaymin, in
//! `HeartGold` and `SoulSilver` Pichu) have theirs in the other Pokémon graphics archive
//! (`OtherPokemonGraphics`) instead, the base forme's there too.
//!
//! The palette descriptions are those of Diamond and Pearl in all of Gen 4.
//!
//! Deliberate choices:
//! - Only the palettes that changed are written; the alternate formes' palettes are left as they are.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, SpeciesPalettes};

use super::Rom;
use super::constants::{self as c, Family};
use crate::{RomError, ds_palette};

/// Each species has six graphics files; the normal and shiny palettes are the last two.
const GRAPHICS_PER_SPECIES: usize = 6;
const NORMAL_PALETTE: usize = 4;
const SHINY_PALETTE: usize = 5;

/// The base forme's normal and shiny palette files in the other Pokémon graphics archive of Diamond and Pearl.
const DP_FORME_PALETTES: [(DexNumber, usize, usize); 9] = [
    (201, 136, 137), // Unown
    (351, 138, 142), // Castform
    (386, 134, 135), // Deoxys
    (412, 146, 147), // Burmy
    (413, 152, 153), // Wormadam
    (421, 166, 168), // Cherrim
    (422, 158, 159), // Shellos
    (423, 162, 163), // Gastrodon
    (493, 170, 171), // Arceus
];
/// Platinum adds the images of Rotom's, Giratina's and Shaymin's formes before the palettes, which moves those of
/// Diamond and Pearl up by this many files.
const PT_SHIFT: usize = 20;
const PT_FORME_PALETTES: [(DexNumber, usize, usize); 3] = [
    (479, 232, 233), // Rotom
    (487, 244, 245), // Giratina
    (492, 228, 229), // Shaymin
];
/// `HeartGold` and `SoulSilver` add the images of Pichu's formes, which moves the palettes of Platinum up by this
/// many files.
const HGSS_SHIFT: usize = 4;
const HGSS_FORME_PALETTES: [(DexNumber, usize, usize); 1] = [
    (172, 252, 253), // Pichu
];

/// Where a species' palettes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// The normal and shiny palette files in the Pokémon graphics archive.
    Graphics(usize, usize),
    /// The normal and shiny palette files in the other Pokémon graphics archive.
    Other(usize, usize),
}

/// The base formes with palettes in the other Pokémon graphics archive, and their normal and shiny files there.
fn forme_palettes(family: Family) -> Vec<(DexNumber, usize, usize)> {
    let shift = |table: &[(DexNumber, usize, usize)], by: usize| {
        table
            .iter()
            .map(|&(n, normal, shiny)| (n, normal + by, shiny + by))
            .collect::<Vec<_>>()
    };
    match family {
        Family::DiamondPearl => DP_FORME_PALETTES.to_vec(),
        Family::Platinum => [shift(&DP_FORME_PALETTES, PT_SHIFT), PT_FORME_PALETTES.to_vec()].concat(),
        Family::HeartGoldSoulSilver => [
            shift(&forme_palettes(Family::Platinum), HGSS_SHIFT),
            HGSS_FORME_PALETTES.to_vec(),
        ]
        .concat(),
    }
}

fn place(formes: &[(DexNumber, usize, usize)], n: DexNumber) -> Place {
    formes.iter().find(|f| f.0 == n).map_or_else(
        || {
            let base = usize::from(n) * GRAPHICS_PER_SPECIES;
            Place::Graphics(base + NORMAL_PALETTE, base + SHINY_PALETTE)
        },
        |&(_, normal, shiny)| Place::Other(normal, shiny),
    )
}

/// The archive and the normal and shiny files of a place.
fn files(rom: &Rom, place: Place) -> Result<(String, usize, usize), RomError> {
    Ok(match place {
        Place::Graphics(normal, shiny) => (rom.entry.file("PokemonGraphics")?, normal, shiny),
        Place::Other(normal, shiny) => (rom.entry.file("OtherPokemonGraphics")?, normal, shiny),
    })
}

/// Every species' normal and shiny palette.
pub(super) fn read(rom: &mut Rom) -> Result<BTreeMap<DexNumber, SpeciesPalettes>, RomError> {
    let formes = forme_palettes(rom.family());
    let mut palettes = BTreeMap::new();
    for i in 1..=c::SPECIES_COUNT {
        let n = DexNumber::try_from(i).map_err(|_| RomError::Malformed("too many species".into()))?;
        let (path, normal, shiny) = files(rom, place(&formes, n))?;
        palettes.insert(
            n,
            SpeciesPalettes {
                normal: ds_palette::read(rom.files.narc_file(&path, normal)?),
                shiny: ds_palette::read(rom.files.narc_file(&path, shiny)?),
                id: None,
            },
        );
    }
    Ok(palettes)
}

/// Writes the palettes that differ from `old`.
pub(super) fn write(
    rom: &mut Rom,
    old: &BTreeMap<DexNumber, SpeciesPalettes>,
    new: &BTreeMap<DexNumber, SpeciesPalettes>,
) -> Result<(), RomError> {
    let formes = forme_palettes(rom.family());
    for (&n, p) in new {
        let Some(before) = old.get(&n) else { continue };
        if p == before {
            continue;
        }
        let (path, normal, shiny) = files(rom, place(&formes, n))?;
        if p.normal != before.normal {
            *rom.files.narc_file(&path, normal)? = ds_palette::to_bytes(&p.normal);
        }
        if p.shiny != before.shiny {
            *rom.files.narc_file(&path, shiny)? = ds_palette::to_bytes(&p.shiny);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formes_have_their_palettes_in_the_other_archive() {
        let dp = forme_palettes(Family::DiamondPearl);
        assert_eq!(place(&dp, 1), Place::Graphics(10, 11));
        assert_eq!(place(&dp, 351), Place::Other(138, 142));
        assert_eq!(place(&dp, 479), Place::Graphics(479 * 6 + 4, 479 * 6 + 5));
        let pt = forme_palettes(Family::Platinum);
        assert_eq!(place(&pt, 493), Place::Other(190, 191));
        assert_eq!(place(&pt, 492), Place::Other(228, 229));
        assert_eq!(place(&pt, 172), Place::Graphics(172 * 6 + 4, 172 * 6 + 5));
        let hgss = forme_palettes(Family::HeartGoldSoulSilver);
        assert_eq!(place(&hgss, 201), Place::Other(160, 161));
        assert_eq!(place(&hgss, 487), Place::Other(248, 249));
        assert_eq!(place(&hgss, 172), Place::Other(252, 253));
        assert_eq!(hgss.len(), 13);
    }
}
