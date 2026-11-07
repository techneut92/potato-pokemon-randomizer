//! Pokémon palettes, read and written: every species' normal and shiny
//! palette, LZ10-compressed behind the pointer tables `PokemonNormalPalettes` and `PokemonShinyPalettes` (8 bytes
//! per internal index).
//!
//! Deliberate choices:
//! - Only palettes that changed are written, so unchanged ones stay where they are.
//! - Unown's alternate formes follow its palette only where they shared it; one that does not is left alone.

use std::collections::BTreeMap;

use ppr_formats::lz10;
use ppr_model::{Color, DexNumber, SpeciesPalettes};

use super::space::Space;
use super::{c, entry_offset};
use crate::RomError;
use crate::entries::RomEntry;
use crate::gba::GbaRom;

/// Size of one pointer table entry: the pointer and the palette tag.
const TABLE_ENTRY: usize = 8;

/// Where the palette pointer tables are.
#[derive(Debug, Clone, Copy)]
pub(super) struct Tables {
    normal: usize,
    shiny: usize,
}

impl Tables {
    /// The tables, when the entry has both.
    pub(super) fn find(rom: &GbaRom, entry: &RomEntry) -> Option<Self> {
        Some(Self {
            normal: entry_offset(rom, entry, "PokemonNormalPalettes").ok()?,
            shiny: entry_offset(rom, entry, "PokemonShinyPalettes").ok()?,
        })
    }
}

fn lz_error(at: usize, e: &ppr_formats::FormatError) -> RomError {
    RomError::Malformed(format!("the palette at {at:#x}: {e}"))
}

/// The colors of the compressed palette the pointer at `pointer_offset` points to.
fn read_palette(rom: &GbaRom, pointer_offset: usize) -> Result<Vec<Color>, RomError> {
    let at = rom.pointer(pointer_offset)?;
    let bytes = lz10::decompress(rom.bytes(), at).map_err(|e| lz_error(at, &e))?;
    Ok(bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&w| Color::from_word(u16::from_le_bytes(w)))
        .collect())
}

/// The palettes of the species with these national numbers.
pub(super) fn read(
    rom: &GbaRom,
    tables: Tables,
    species: impl IntoIterator<Item = DexNumber>,
    dex_to_internal: &[u16],
) -> Result<BTreeMap<DexNumber, SpeciesPalettes>, RomError> {
    let mut out = BTreeMap::new();
    for n in species {
        let Some(&internal) = dex_to_internal.get(usize::from(n)) else {
            continue;
        };
        let i = usize::from(internal) * TABLE_ENTRY;
        out.insert(
            n,
            SpeciesPalettes {
                normal: read_palette(rom, tables.normal + i)?,
                shiny: read_palette(rom, tables.shiny + i)?,
                id: None,
            },
        );
    }
    Ok(out)
}

fn to_bytes(palette: &[Color]) -> Vec<u8> {
    palette.iter().flat_map(|c| c.to_word().to_le_bytes()).collect()
}

/// The palette compressed into free space, the old one freed, and the pointer (and the `copies` that shared it)
/// pointed there. The old data's length is that of the old palette
/// compressed again.
fn rewrite(space: &mut Space<'_>, pointer_offset: usize, palette: &[Color], copies: &[usize]) -> Result<(), RomError> {
    let old = space.rom.pointer(pointer_offset)?;
    let old_length = lz10::compress(&lz10::decompress(space.rom.bytes(), old).map_err(|e| lz_error(old, &e))?).len();
    let shared: Vec<usize> = copies
        .iter()
        .copied()
        .filter(|&copy| space.rom.pointer(copy).ok() == Some(old))
        .collect();
    space.rewrite(pointer_offset, old_length, &lz10::compress(&to_bytes(palette)), &shared)
}

/// Writes the palettes that differ from `old`.
pub(super) fn write(
    space: &mut Space<'_>,
    tables: Tables,
    palettes: &BTreeMap<DexNumber, SpeciesPalettes>,
    old: &BTreeMap<DexNumber, SpeciesPalettes>,
    dex_to_internal: &[u16],
) -> Result<(), RomError> {
    for (&n, p) in palettes {
        let Some(&internal) = dex_to_internal.get(usize::from(n)) else {
            continue;
        };
        let before = old.get(&n);
        let i = usize::from(internal) * TABLE_ENTRY;
        let unown_formes = |table: usize| -> Vec<usize> {
            if n == c::UNOWN {
                (0..c::UNOWN_FORME_COUNT - 1)
                    .map(|f| table + (c::UNOWN_B_INDEX + f) * TABLE_ENTRY)
                    .collect()
            } else {
                Vec::new()
            }
        };
        if before.is_none_or(|b| b.normal != p.normal) {
            rewrite(space, tables.normal + i, &p.normal, &unown_formes(tables.normal))?;
        }
        if before.is_none_or(|b| b.shiny != p.shiny) {
            rewrite(space, tables.shiny + i, &p.shiny, &unown_formes(tables.shiny))?;
        }
    }
    Ok(())
}
