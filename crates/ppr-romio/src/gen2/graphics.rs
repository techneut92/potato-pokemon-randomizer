//! Pokémon palettes and custom player graphics.
//!
//! A species has a normal and a shiny palette of two colors each (white and black are fixed), as 15-bit words. A
//! graphics pack replaces a player character's front, trainer card and back pictures, the pictures' palette, the
//! walking, cycling and fishing sprites and the sprites' palette: Chris's, or in Crystal Kris's.
//!
//! In Crystal the front pictures are stored as they are; in Gold and Silver Chris's is a compressed trainer
//! picture. Chris's back picture is compressed, in Gold and Silver followed by the catching tutorial dude's, which
//! moves along. Kris shares Falkner's trainer palette, so giving her one of her own adds a trainer class to the
//! palette table and points everything that used Falkner's for her to it.
//!
//! Deliberate choices:
//! - The graphics are written after everything else, so they take free space last.
//! - A missing entry key is an error rather than a write to offset 0.
//! - Gold and Silver have one player character; a choice of the second one gets the first's graphics.

use std::collections::BTreeMap;

use ppr_formats::lz3;
use ppr_model::{Color, DexNumber, PlayerCharacter, SpeciesPalettes};

use crate::RomError;
use crate::entries::RomEntry;
use crate::gb::{BANK, GbRom, need, offsets, optional};
use crate::graphics::{GbPlayerGraphics, GbcImage};

/// The bytes of a species' two palettes.
const SPECIES_PALETTES_SIZE: usize = 8;
/// The bytes of a two-color palette.
const PALETTE_SIZE: usize = 4;
/// Chris's trainer class in Gold and Silver, and the class whose palette he also uses.
const CHRIS_CLASS_GS: usize = 12;
const CHRIS_PALETTE_CLASS_GS: usize = 0;
/// Chris's trainer class in Crystal.
const CHRIS_CLASS_CRYSTAL: usize = 0;
const FALKNER_CLASS: u8 = 1;
/// In the routine that sets the trainer card's palettes: the trainer classes of palettes 1 (Falkner, to become
/// Kris) and 5 (Chuck, to become Falkner), and the palettes of the Falkner, Chuck and Clair badges.
const CARD_PALETTE_1: usize = 0xB;
const CARD_PALETTE_5: usize = 0x2B;
const CARD_FALKNER_ICON: usize = 0x7C;
const CARD_CHUCK_ICON: usize = 0xA8;
const CARD_CLAIR_ICON: usize = 0xD0;

/// Crystal's entry keys for Kris that lie at a known distance from Chris's.
const KRIS_RELATIVE: [(&str, &str, isize); 4] = [
    ("KrisFrontImage", "ChrisFrontImage", 16 * 7 * 7),
    ("KrisTrainerCardImage", "ChrisTrainerCardImage", 16 * 5 * 7),
    ("KrisPalettePointer", "ChrisPalettePointer", -4),
    ("KrisSpritePalette", "ChrisSpritePalette", 16),
];
/// Where the bank bytes of Chris's back picture are, after its two pointers, in Gold and Silver and in Crystal.
const CHRIS_BACK_BANKS_GS: [isize; 2] = [16, 6];
const CHRIS_BACK_BANKS_CRYSTAL: [isize; 2] = [-2, 3];
/// Where the pointer to the dude's back picture is in Gold and Silver, after the first to Chris's.
const DUDE_BACK_POINTER: usize = 10;

fn palette_offset(entry: &RomEntry, n: DexNumber) -> Result<usize, RomError> {
    let index = usize::from(n)
        .checked_sub(1)
        .ok_or_else(|| RomError::Malformed("species 0 has no palettes".into()))?;
    Ok(need(entry, "PokemonPalettes")? + SPECIES_PALETTES_SIZE + index * SPECIES_PALETTES_SIZE)
}

fn read_palette(rom: &GbRom, at: usize) -> Result<Vec<Color>, RomError> {
    Ok(vec![
        Color::from_word(rom.word(at)?),
        Color::from_word(rom.word(at + 2)?),
    ])
}

fn palette_bytes(colors: &[Color]) -> Vec<u8> {
    colors.iter().flat_map(|c| c.to_word().to_le_bytes()).collect()
}

/// Each species' normal and shiny palettes.
pub(super) fn read_palettes(
    rom: &GbRom,
    entry: &RomEntry,
    numbers: impl Iterator<Item = DexNumber>,
) -> Result<BTreeMap<DexNumber, SpeciesPalettes>, RomError> {
    if optional(entry, "PokemonPalettes") == 0 {
        return Ok(BTreeMap::new());
    }
    numbers
        .map(|n| {
            let at = palette_offset(entry, n)?;
            Ok((
                n,
                SpeciesPalettes {
                    normal: read_palette(rom, at)?,
                    shiny: read_palette(rom, at + PALETTE_SIZE)?,
                    id: None,
                },
            ))
        })
        .collect()
}

/// The palettes that changed.
pub(super) fn write_palettes(
    rom: &mut GbRom,
    entry: &RomEntry,
    new: &BTreeMap<DexNumber, SpeciesPalettes>,
    old: &BTreeMap<DexNumber, SpeciesPalettes>,
) -> Result<(), RomError> {
    for (&n, p) in new {
        if old.get(&n) == Some(p) {
            continue;
        }
        if p.normal.len() != 2 || p.shiny.len() != 2 {
            return Err(RomError::Malformed(format!("species {n} needs palettes of 2 colors")));
        }
        let at = palette_offset(entry, n)?;
        rom.write(at, &palette_bytes(&p.normal))?;
        rom.write(at + PALETTE_SIZE, &palette_bytes(&p.shiny))?;
    }
    Ok(())
}

fn add(base: usize, offset: isize, key: &str) -> Result<usize, RomError> {
    base.checked_add_signed(offset)
        .ok_or_else(|| RomError::Malformed(format!("{key} is out of range")))
}

/// The bank a trainer or Pokémon picture's bank byte stands for, and back.
fn picture_bank(byte: u8, crystal: bool) -> usize {
    let bank = usize::from(byte);
    if crystal {
        bank + 0x36
    } else {
        match bank {
            0x13 => 0x1F,
            0x14 => 0x20,
            0x1F => 0x2E,
            b => b,
        }
    }
}

fn picture_bank_byte(bank: usize, crystal: bool) -> Result<u8, RomError> {
    let byte = if crystal {
        bank.wrapping_sub(0x36)
    } else {
        match bank {
            0x1F => 0x13,
            0x20 => 0x14,
            0x2E => 0x1F,
            b => b,
        }
    };
    u8::try_from(byte).map_err(|_| RomError::Malformed(format!("bank {bank:#x} can't hold a picture")))
}

fn lz3_error(at: usize, e: &ppr_formats::FormatError) -> RomError {
    RomError::Malformed(format!("the compressed data at {at:#x}: {e}"))
}

fn compressed_length(rom: &GbRom, at: usize) -> Result<usize, RomError> {
    if at == 0 {
        return Err(RomError::Malformed("compressed data at offset 0".into()));
    }
    lz3::compressed_length(rom.bytes(), at).map_err(|e| lz3_error(at, &e))
}

fn compressed(image: &GbcImage) -> Result<Vec<u8>, RomError> {
    lz3::compress(&image.to_bytes()).map_err(|e| lz3_error(0, &e))
}

struct Writer<'a> {
    rom: &'a mut GbRom,
    entry: &'a RomEntry,
    crystal: bool,
    /// The character who gets the graphics: the second only in Crystal.
    kris: bool,
}

impl Writer<'_> {
    /// The entry's value for the player character's key, or in Crystal one at a known distance from Chris's.
    fn value(&self, suffix: &str) -> Result<usize, RomError> {
        let name = if self.kris { "Kris" } else { "Chris" };
        self.key(&format!("{name}{suffix}"))
    }

    fn key(&self, key: &str) -> Result<usize, RomError> {
        if optional(self.entry, key) == 0
            && self.crystal
            && let Some(&(_, base, offset)) = KRIS_RELATIVE.iter().find(|(k, ..)| *k == key)
            && optional(self.entry, base) != 0
        {
            return add(optional(self.entry, base), offset, key);
        }
        need(self.entry, key)
    }

    fn class_amount(&self) -> Result<usize, RomError> {
        need(self.entry, "TrainerClassAmount")
    }

    /// Replaces a trainer class's compressed picture; it may move to any unreserved bank.
    fn rewrite_trainer_image(&mut self, class: usize, image: &GbcImage) -> Result<(), RomError> {
        if class < 1 || class >= self.class_amount()? {
            return Err(RomError::Malformed(format!("there is no trainer class {class}")));
        }
        let pointer = need(self.entry, "TrainerImages")? + (class - 1) * 3;
        let data = compressed(image)?;
        let bank = picture_bank(self.rom.byte(pointer)?, self.crystal);
        let old = self.rom.pointer_in(pointer + 1, bank)?;
        let old_length = compressed_length(self.rom, old)?;
        self.rom.free_space(old, old_length)?;
        let at = self
            .rom
            .take_unreserved(data.len())
            .ok_or_else(|| RomError::Malformed(format!("ROM full; can't find {} free bytes", data.len())))?;
        self.rom
            .write_byte(pointer, picture_bank_byte(at / BANK, self.crystal)?)?;
        self.rom.write_pointer(pointer + 1, at)?;
        self.rom.write(at, &data)
    }

    fn write_front(&mut self, front: &GbcImage, trainer_card: Option<&GbcImage>) -> Result<(), RomError> {
        if self.crystal {
            let at = self.value("FrontImage")?;
            self.rom.write(at, &front.to_bytes())?;
        } else {
            self.rewrite_trainer_image(CHRIS_CLASS_GS, front)?;
        }
        if let Some(card) = trainer_card {
            // Gold and Silver store the trainer card picture row by row.
            let card = card
                .rebuilt(self.crystal)
                .ok_or_else(|| RomError::Malformed("the trainer card picture can't be rebuilt".into()))?;
            let at = self.value("TrainerCardImage")?;
            self.rom.write(at, &card.to_bytes())?;
        }
        Ok(())
    }

    fn chris_back_pointers(&self) -> Result<(Vec<usize>, Vec<usize>), RomError> {
        let pointers = offsets(self.entry, "ChrisBackImagePointers");
        let mut banks = offsets(self.entry, "ChrisBackImageBankOffsets");
        if pointers.len() == 2 && banks.is_empty() {
            let relative = if self.crystal {
                CHRIS_BACK_BANKS_CRYSTAL
            } else {
                CHRIS_BACK_BANKS_GS
            };
            banks = vec![
                add(pointers[0], relative[0], "ChrisBackImageBankOffsets")?,
                add(pointers[1], relative[1], "ChrisBackImageBankOffsets")?,
            ];
        }
        Ok((pointers, banks))
    }

    fn dude_back_pointer(&self, chris_back: &[usize]) -> Result<usize, RomError> {
        match (optional(self.entry, "DudeBackImagePointer"), chris_back) {
            (0, &[first, _]) => Ok(first + DUDE_BACK_POINTER),
            _ => need(self.entry, "DudeBackImagePointer"),
        }
    }

    /// Chris's compressed back picture; in Gold and Silver the dude's follows it.
    fn rewrite_chris_back(&mut self, picture: &GbcImage) -> Result<(), RomError> {
        let (pointers, banks) = self.chris_back_pointers()?;
        let (&primary, secondary) = pointers
            .split_first()
            .ok_or_else(|| RomError::Malformed("the ROM entry has no pointers to Chris's back picture".into()))?;
        let first_bank = *banks
            .first()
            .ok_or_else(|| RomError::Malformed("the ROM entry has no bank of Chris's back picture".into()))?;
        let bank = usize::from(self.rom.byte(first_bank)?);
        let mut data = compressed(picture)?;
        let dude_pointer = if self.crystal {
            None
        } else {
            let p = self.dude_back_pointer(&pointers)?;
            let at = self.rom.pointer_in(p, bank)?;
            data.extend_from_slice(self.rom.slice(at, compressed_length(self.rom, at)?)?);
            Some(p)
        };
        let old = self.rom.pointer_in(primary, bank)?;
        let mut old_length = compressed_length(self.rom, old)?;
        if dude_pointer.is_some() {
            old_length += compressed_length(self.rom, old + old_length)?;
        }
        self.rom
            .rewrite_indirect_bank_shared(primary, secondary, &banks, &data, old_length)?;
        if let Some(p) = dude_pointer {
            let bank = usize::from(self.rom.byte(first_bank)?);
            let new = self.rom.pointer_in(primary, bank)?;
            let dude = new + compressed_length(self.rom, new)?;
            self.rom.write_pointer(p, dude)?;
        }
        Ok(())
    }

    fn trainer_palettes_table(&self) -> Result<usize, RomError> {
        match offsets(self.entry, "TrainerPalettesPointers").first() {
            Some(&p) => self.rom.pointer(p),
            None => need(self.entry, "TrainerPalettes"),
        }
    }

    fn write_trainer_palette(&mut self, class: usize, palette: [Color; 2]) -> Result<(), RomError> {
        let at = self.trainer_palettes_table()? + class * PALETTE_SIZE;
        self.rom.write(at, &palette_bytes(&palette))
    }

    fn write_image_palette(&mut self, palette: [Color; 2]) -> Result<(), RomError> {
        if !self.kris {
            if self.crystal {
                return self.write_trainer_palette(CHRIS_CLASS_CRYSTAL, palette);
            }
            // Chris mostly uses the palette of class 0, but in some places his own.
            self.write_trainer_palette(CHRIS_CLASS_GS, palette)?;
            return self.write_trainer_palette(CHRIS_PALETTE_CLASS_GS, palette);
        }
        let classes = self.class_amount()?;
        self.resize_trainer_palettes(classes + 1)?;
        self.write_trainer_palette(classes, palette)?;
        let kris_class = u8::try_from(classes).map_err(|_| RomError::Malformed("too many trainer classes".into()))?;
        self.write_kris_trainer_class(kris_class)?;
        // A routine points to Chris's and Kris's palettes directly.
        let table = self.trainer_palettes_table()?;
        let chris = self.key("ChrisPalettePointer")?;
        self.rom.write_pointer(chris, table)?;
        let kris = self.key("KrisPalettePointer")?;
        self.rom.write_pointer(kris, table + PALETTE_SIZE * classes)?;
        self.fix_trainer_card_palettes(kris_class)
    }

    /// Moves the trainer palette table, in its bank, to make room for `classes` classes.
    fn resize_trainer_palettes(&mut self, classes: usize) -> Result<(), RomError> {
        let pointers = offsets(self.entry, "TrainerPalettesPointers");
        let (&primary, secondary) = pointers
            .split_first()
            .ok_or_else(|| RomError::Malformed("the ROM entry has no pointers to the trainer palettes".into()))?;
        let old_size = PALETTE_SIZE * self.class_amount()?;
        let old = self.rom.slice(self.rom.pointer(primary)?, old_size)?;
        let mut data = vec![0; PALETTE_SIZE * classes];
        let kept = old_size.min(data.len());
        data[..kept].copy_from_slice(&old[..kept]);
        let bank = need(self.entry, "TrainerPalettesBank")?;
        self.rom
            .rewrite_specific_bank(primary, secondary, bank, &data, old_size)?;
        Ok(())
    }

    /// Kris's pictures take the palette of `class`.
    fn write_kris_trainer_class(&mut self, class: u8) -> Result<(), RomError> {
        let at = offsets(self.entry, "KrisTrainerClassOffsets");
        let Some(&first) = at.first() else {
            return Err(RomError::Malformed(
                "the ROM entry has no KrisTrainerClassOffsets".into(),
            ));
        };
        let original = self.rom.byte(first)?;
        for &o in &at {
            if self.rom.byte(o)? != original {
                return Err(RomError::Malformed(format!(
                    "Kris's trainer class at {o:#x} differs from the one at {first:#x}"
                )));
            }
        }
        for o in at {
            self.rom.write_byte(o, class)?;
        }
        Ok(())
    }

    /// The trainer card shows Kris in her palette; Falkner's badge gets his palette, Chuck's Pryce's.
    fn fix_trainer_card_palettes(&mut self, kris_class: u8) -> Result<(), RomError> {
        let start = optional(self.entry, "TrainerCardPalettesRoutine");
        if start == 0 {
            return Ok(());
        }
        self.rom.write_byte(start + CARD_PALETTE_1, kris_class)?;
        self.rom.write_byte(start + CARD_PALETTE_5, FALKNER_CLASS)?;
        self.rom.write_byte(start + CARD_FALKNER_ICON, 5)?;
        self.rom.write_byte(start + CARD_CHUCK_ICON, 7)?;
        self.rom.write_byte(start + CARD_CLAIR_ICON, 5)
    }
}

/// Writes a pack's graphics over a player character's.
pub(super) fn write_player_graphics(
    rom: &mut GbRom,
    entry: &RomEntry,
    crystal: bool,
    graphics: &GbPlayerGraphics,
    replace: PlayerCharacter,
) -> Result<(), RomError> {
    let mut w = Writer {
        rom,
        entry,
        crystal,
        kris: crystal && replace == PlayerCharacter::Pc2,
    };
    if let Some(front) = &graphics.front {
        w.write_front(front, graphics.trainer_card.as_ref())?;
    }
    if let Some(back) = &graphics.back {
        if w.kris {
            let at = w.key("KrisBackImage")?;
            w.rom.write(at, &back.to_bytes())?;
        } else {
            w.rewrite_chris_back(back)?;
        }
    }
    if let Some(palette) = graphics.image_palette {
        w.write_image_palette(palette)?;
    }
    for (image, suffix) in [
        (&graphics.walk, "WalkSprite"),
        (&graphics.bike, "BikeSprite"),
        (&graphics.fish, "FishSprite"),
    ] {
        if let Some(image) = image {
            let at = w.value(suffix)?;
            w.rom.write(at, &image.to_bytes())?;
        }
    }
    let at = w.value("SpritePalette")?;
    w.rom.write_byte(at, (graphics.sprite_palette | 0b1000) << 4)
}
