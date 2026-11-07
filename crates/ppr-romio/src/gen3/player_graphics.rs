//! Custom player graphics: a graphics pack's images and palettes written over one player character's.
//!
//! Deliberate choices:
//! - A missing entry key is an error rather than a write to index or offset 0.
//! - The graphics are written after everything else, Pokémon palettes included, so they take free space last.

use ppr_formats::lz10;
use ppr_model::{Color, PlayerCharacter};

use super::space::Space;
use super::{Family, need};
use crate::RomError;
use crate::entries::RomEntry;
use crate::graphics::{GbaImage, Gen3PlayerGraphics, palette_bytes};

/// Size of one entry in the trainer image and overworld tables: the pointer and its size or tag.
const TABLE_ENTRY: usize = 8;
/// Emerald's and `FireRed`'s trainer front image index of the first player character (Brendan and Red).
const EM_PLAYER_FRONT_IMAGE: usize = 71;
const FRLG_PLAYER_FRONT_IMAGE: usize = 135;

/// An offset the entry may give relative to another: the entry's own value, or the base key's plus `offset`.
fn relative(entry: &RomEntry, key: &str, base: &str, offset: isize) -> Result<usize, RomError> {
    match entry.int(key).filter(|&v| v != 0) {
        Some(_) => need(entry, key),
        None => need(entry, base)?
            .checked_add_signed(offset)
            .ok_or_else(|| RomError::Malformed(format!("{key} is out of range"))),
    }
}

struct Writer<'a, 'b> {
    space: &'a mut Space<'b>,
    entry: &'a RomEntry,
    family: Family,
    /// `Brendan` or `May`, `Red` or `Leaf`.
    name: &'static str,
}

impl Writer<'_, '_> {
    fn key(&self, suffix: &str) -> Result<usize, RomError> {
        need(self.entry, &format!("{}{suffix}", self.name))
    }

    fn lz_error(at: usize, e: &ppr_formats::FormatError) -> RomError {
        RomError::Malformed(format!("the compressed data at {at:#x}: {e}"))
    }

    /// Moves compressed data to free space, the old data's length being its decompressed bytes compressed again.
    fn rewrite_compressed(&mut self, pointer_offset: usize, uncompressed: &[u8]) -> Result<(), RomError> {
        let old = self.space.rom.pointer(pointer_offset)?;
        let old_bytes = lz10::decompress(self.space.rom.bytes(), old).map_err(|e| Self::lz_error(old, &e))?;
        let old_length = lz10::compress(&old_bytes).len();
        self.space
            .rewrite(pointer_offset, old_length, &lz10::compress(uncompressed), &[])
    }

    /// Ruby, Sapphire and Emerald share the player characters' front
    /// and back palettes, so the front ones get copies of their own.
    fn separate_front_and_back_palettes(&mut self, front_palettes: usize) -> Result<(), RomError> {
        for i in 0..=1 {
            let image = if self.family == Family::Emerald {
                i + EM_PLAYER_FRONT_IMAGE
            } else {
                i
            };
            let pointer_offset = front_palettes + image * TABLE_ENTRY;
            let at = self.space.rom.pointer(pointer_offset)?;
            let bytes = lz10::decompress(self.space.rom.bytes(), at).map_err(|e| Self::lz_error(at, &e))?;
            // The palette goes through colors and back.
            let colors: Vec<Color> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&w| Color::from_word(u16::from_le_bytes(w)))
                .collect();
            let compressed = lz10::compress(&palette_bytes(&colors));
            let new = self.space.find_and_unfree(compressed.len())?;
            self.space.rom.write(new, &compressed)?;
            self.space.rom.write_pointer(pointer_offset, new)?;
        }
        Ok(())
    }

    /// Writes the frame over an image of the same size.
    fn overworld_image(&mut self, table: usize, index: usize, image: &GbaImage) -> Result<(), RomError> {
        let pointer_offset = table + index * TABLE_ENTRY;
        let at = self.space.rom.pointer(pointer_offset)?;
        let size = self.space.rom.slice(pointer_offset + 4, 2)?;
        let length = usize::from(u16::from_le_bytes([size[0], size[1]]));
        let bytes = image.to_bytes();
        if bytes.len() != length {
            return Err(RomError::Malformed(format!(
                "wrong image size at {pointer_offset:#x}: expected {length} bytes, got {}",
                bytes.len()
            )));
        }
        self.space.rom.write(at, &bytes)
    }

    /// Writes every frame over the images from the entry's `<Name><key>` index on.
    fn sprite(&mut self, sprite: &GbaImage, key: &str) -> Result<(), RomError> {
        let table = need(self.entry, "OverworldSpriteImages")?;
        let first = self.key(key)?;
        for i in 0..sprite.frame_count() {
            self.overworld_image(table, first + i, &sprite.frame(i))?;
        }
        Ok(())
    }

    /// Writes an overworld palette over the one at `index`.
    fn overworld_palette(&mut self, index: usize, palette: &[Color]) -> Result<(), RomError> {
        let table = need(self.entry, "OverworldPalettes")?;
        let at = self.space.rom.pointer(table + index * TABLE_ENTRY)?;
        self.space.rom.write(at, &palette_bytes(palette))
    }

    /// Writes a trainer front image and its palette.
    fn front(&mut self, index: usize, image: &GbaImage, front_palettes: usize) -> Result<(), RomError> {
        let images = need(self.entry, "TrainerFrontImages")?;
        self.rewrite_compressed(images + index * TABLE_ENTRY, &image.to_bytes())?;
        self.rewrite_compressed(front_palettes + index * TABLE_ENTRY, &palette_bytes(image.palette()))
    }

    /// Writes a trainer back image and its palette: the image compressed in Ruby and Sapphire, raw elsewhere.
    fn back(&mut self, index: usize, image: &GbaImage) -> Result<(), RomError> {
        let images = need(self.entry, "TrainerBackImages")?;
        let palettes = relative(
            self.entry,
            "TrainerBackPalettes",
            "TrainerBackImages",
            match self.family {
                Family::RubySapphire => 0x18,
                Family::Emerald => 0x40,
                Family::FireRedLeafGreen => 0x30,
            },
        )?;
        let pointer_offset = images + index * TABLE_ENTRY;
        if self.family == Family::RubySapphire {
            self.rewrite_compressed(pointer_offset, &image.to_bytes())?;
        } else {
            let at = self.space.rom.pointer(pointer_offset)?;
            self.space.rom.write(at, &image.to_bytes())?;
        }
        self.rewrite_compressed(palettes + index * TABLE_ENTRY, &palette_bytes(image.palette()))
    }

    /// Writes the player's map icon and its palette.
    fn map_icon(&mut self, icon: &GbaImage) -> Result<(), RomError> {
        let palette = palette_bytes(icon.palette());
        if self.family == Family::FireRedLeafGreen {
            let red = "RedMapIconImagePointer";
            let (image_pointer, palette_pointer) = if self.name == "Red" {
                (
                    need(self.entry, red)?,
                    relative(self.entry, "RedMapIconPalettePointer", red, 0xC4)?,
                )
            } else {
                (
                    relative(self.entry, "LeafMapIconImagePointer", red, -0x30)?,
                    relative(self.entry, "LeafMapIconPalettePointer", red, 0xE0)?,
                )
            };
            self.rewrite_compressed(image_pointer, &icon.to_bytes())?;
            let at = self.space.rom.pointer(palette_pointer)?;
            self.space.rom.write(at, &palette)
        } else {
            let brendan = "BrendanMapIconImage";
            let (image, palette_at) = if self.name == "Brendan" {
                (
                    need(self.entry, brendan)?,
                    relative(self.entry, "BrendanMapIconPalette", brendan, -0x20)?,
                )
            } else {
                (
                    relative(self.entry, "MayMapIconImage", brendan, 0xA0)?,
                    relative(self.entry, "MayMapIconPalette", brendan, 0x80)?,
                )
            };
            self.space.rom.write(image, &icon.to_bytes())?;
            self.space.rom.write(palette_at, &palette)
        }
    }
}

/// Writes a graphics pack over a player character.
pub(super) fn write(
    space: &mut Space<'_>,
    entry: &RomEntry,
    family: Family,
    g: &Gen3PlayerGraphics,
    replace: PlayerCharacter,
) -> Result<(), RomError> {
    let frlg = family == Family::FireRedLeafGreen;
    let name = match (frlg, replace) {
        (false, PlayerCharacter::Pc1) => "Brendan",
        (false, PlayerCharacter::Pc2) => "May",
        (true, PlayerCharacter::Pc1) => "Red",
        (true, PlayerCharacter::Pc2) => "Leaf",
    };
    let ordinal = match replace {
        PlayerCharacter::Pc1 => 0,
        PlayerCharacter::Pc2 => 1,
    };
    let mut w = Writer {
        space,
        entry,
        family,
        name,
    };
    let front_palettes = relative(
        entry,
        "TrainerFrontPalettes",
        "TrainerFrontImages",
        match family {
            Family::RubySapphire => 0x298,
            Family::Emerald => 0x2E8,
            Family::FireRedLeafGreen => 0x4A0,
        },
    )?;
    if !frlg {
        w.separate_front_and_back_palettes(front_palettes)?;
    }
    if let Some(front) = &g.front {
        let index = ordinal
            + match family {
                Family::RubySapphire => 0,
                Family::Emerald => EM_PLAYER_FRONT_IMAGE,
                Family::FireRedLeafGreen => FRLG_PLAYER_FRONT_IMAGE,
            };
        w.front(index, front, front_palettes)?;
    }
    if let Some(back) = &g.back {
        w.back(ordinal, back)?;
    }
    if let (Some(normal), Some(reflection)) = (&g.normal_palette, &g.reflection_palette) {
        let normal_index = w.key("NormalPalette")?;
        w.overworld_palette(normal_index, normal)?;
        let reflection_index = w.key("ReflectionPalette")?;
        w.overworld_palette(reflection_index, reflection)?;
    }
    if let Some(walk) = &g.walk {
        w.sprite(walk, "WalkImage")?;
        if let Some(run) = &g.run {
            w.sprite(run, "RunImage")?;
        }
    }
    for (sprite, key) in [(&g.bike, "BikeImage"), (&g.fish, "FishImage"), (&g.sit, "SitImage")] {
        if let Some(sprite) = sprite {
            w.sprite(sprite, key)?;
        }
    }
    if let Some(blob) = &g.surf_blob {
        let table = need(entry, "OverworldSurfBlobImages")?;
        for i in 0..blob.frame_count() {
            w.overworld_image(table, i, &blob.frame(i))?;
        }
    }
    if let Some(bird) = &g.bird {
        let table = need(entry, "OverworldBirdImages")?;
        w.overworld_image(table, 0, &bird.frame(0))?;
        if frlg {
            let first = if replace == PlayerCharacter::Pc1 { 1 } else { 3 };
            w.overworld_image(table, first, &bird.frame(1))?;
            w.overworld_image(table, first + 1, &bird.frame(2))?;
        }
    }
    if let Some(icon) = &g.map_icon {
        w.map_icon(icon)?;
    }
    if frlg {
        frlg_extras(&mut w, g)
    } else {
        rse_extras(&mut w, g)
    }
}

/// The sprites only `FireRed` and `LeafGreen` have.
fn frlg_extras(w: &mut Writer<'_, '_>, g: &Gen3PlayerGraphics) -> Result<(), RomError> {
    for (sprite, key) in [(&g.item, "ItemImage"), (&g.item_bike, "ItemBikeImage")] {
        if let Some(sprite) = sprite {
            w.sprite(sprite, key)?;
        }
    }
    Ok(())
}

/// The sprites and palette only Ruby, Sapphire and Emerald have.
fn rse_extras(w: &mut Writer<'_, '_>, g: &Gen3PlayerGraphics) -> Result<(), RomError> {
    if let Some(sit_jump) = &g.sit_jump {
        w.sprite(sit_jump, "SitJumpImage")?;
    }
    if let Some(acro_bike) = &g.acro_bike {
        w.sprite(acro_bike, "AcroBikeImage")?;
    }
    if let Some(underwater) = &g.underwater {
        w.sprite(underwater, "UnderwaterImage")?;
        if let Some(palette) = &g.underwater_palette {
            let index = need(w.entry, "UnderwaterPalette")?;
            w.overworld_palette(index, palette)?;
        }
    }
    if let Some(can) = &g.watering_can {
        // The watering can's frames are not in order in the games.
        let table = need(w.entry, "OverworldSpriteImages")?;
        let first = w.key("WateringCanImage")?;
        for (frame, at) in [(0, 0), (2, 1), (4, 2), (1, 3), (3, 5), (5, 7)] {
            w.overworld_image(table, first + at, &can.frame(frame))?;
        }
    }
    if let Some(decorate) = &g.decorate {
        w.sprite(decorate, "DecorateImage")?;
    }
    if let Some(field_move) = &g.field_move {
        w.sprite(field_move, "FieldMoveImage")?;
    }
    Ok(())
}
