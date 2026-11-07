//! The intro Pokémon: the Pokémon of the title intro (`FireRed` and `LeafGreen`), or the
//! one Birch or the professor shows (Ruby, Sapphire, Emerald).

use ppr_model::DexNumber;

use super::{Family, entry_offset, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;

/// Thumb `mov rX, #n`, `add rX, #n` and `nop`.
const MOV: u8 = 0x20;
const ADD: u8 = 0x30;
const NOP: u16 = 0x46C0;
/// The registers Ruby and Sapphire load the species into for the cry and the rest.
const CRY_REGISTER: u8 = 0;
const OTHER_REGISTER: u8 = 4;

/// The entry keys every game needs, and those Ruby and Sapphire or `FireRed` and `LeafGreen` need too.
fn keys(family: Family) -> &'static [&'static str] {
    match family {
        Family::Emerald => &["IntroImageOffset", "IntroCryOffset"],
        Family::FireRedLeafGreen => &[
            "IntroImageOffset",
            "IntroCryOffset",
            "IntroOtherOffset",
            "PokemonFrontImages",
            "PokemonNormalPalettes",
        ],
        Family::RubySapphire => &[
            "IntroImageOffset",
            "IntroCryOffset",
            "IntroOtherOffset",
            "IntroPaletteOffset",
            "PokemonFrontImages",
            "PokemonNormalPalettes",
        ],
    }
}

/// Whether the intro can show the species with this internal index: in `FireRed` and `LeafGreen` the first 255
/// only (one byte), in Ruby and Sapphire up to 510 but not Bulbasaur (two Thumb instructions), in Emerald any.
fn fits(family: Family, internal: u16) -> bool {
    match family {
        Family::Emerald => true,
        Family::FireRedLeafGreen => internal <= 255,
        Family::RubySapphire => internal != 1 && internal <= 510,
    }
}

/// The species the intro can show; none when the ROM entry lacks an offset.
pub(super) fn candidates(rom: &GbaRom, entry: &RomEntry, family: Family, dex_to_internal: &[u16]) -> Vec<DexNumber> {
    if keys(family).iter().any(|k| entry_offset(rom, entry, k).is_err()) {
        return Vec::new();
    }
    (1..dex_to_internal.len())
        .filter_map(|n| {
            let internal = dex_to_internal[n];
            (internal != 0 && fits(family, internal)).then(|| DexNumber::try_from(n).ok())?
        })
        .collect()
}

/// Ruby and Sapphire's two Thumb instructions that load `internal` into `register`: `mov` of up to 255, then
/// `add` of the rest or a `nop`.
fn load_species(internal: u16, register: u8) -> [u8; 4] {
    if let Ok(small) = u8::try_from(internal) {
        let [nop_lo, nop_hi] = NOP.to_le_bytes();
        [small, MOV | register, nop_lo, nop_hi]
    } else {
        let rest = u8::try_from(internal - 0xFF).unwrap_or(u8::MAX);
        [0xFF, MOV | register, rest, ADD | register]
    }
}

/// Makes the intro show the species with internal index `internal`, which [`fits`].
pub(super) fn write(rom: &mut GbaRom, entry: &RomEntry, family: Family, internal: u16) -> Result<(), RomError> {
    let image = need(entry, "IntroImageOffset")?;
    let cry = need(entry, "IntroCryOffset")?;
    let images = entry_offset(rom, entry, "PokemonFrontImages");
    let palettes = entry_offset(rom, entry, "PokemonNormalPalettes");
    match family {
        Family::Emerald => {
            rom.write_u16(image, internal)?;
            rom.write_u16(cry, internal)
        }
        Family::FireRedLeafGreen => {
            let [byte, ..] = internal.to_le_bytes();
            rom.write(cry, &[byte])?;
            rom.write(need(entry, "IntroOtherOffset")?, &[byte])?;
            let i = usize::from(internal) * 8;
            rom.write_pointer(image, images? + i)?;
            rom.write_pointer(image + 4, palettes? + i)
        }
        Family::RubySapphire => {
            rom.write(cry, &load_species(internal, CRY_REGISTER))?;
            rom.write(
                need(entry, "IntroOtherOffset")?,
                &load_species(internal, OTHER_REGISTER),
            )?;
            let i = usize::from(internal) * 8;
            rom.write_pointer(image, images? + i)?;
            rom.write_pointer(need(entry, "IntroPaletteOffset")?, palettes? + i)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ruby_loads_large_indices_in_two_steps() {
        assert_eq!(load_species(25, 0), [25, 0x20, 0xC0, 0x46]);
        assert_eq!(load_species(300, 4), [0xFF, 0x24, 45, 0x34]);
        assert!(!fits(Family::RubySapphire, 1));
        assert!(fits(Family::RubySapphire, 510) && !fits(Family::RubySapphire, 511));
        assert!(fits(Family::FireRedLeafGreen, 255) && !fits(Family::FireRedLeafGreen, 256));
        assert!(fits(Family::Emerald, 411));
    }
}
