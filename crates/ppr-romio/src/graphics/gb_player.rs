//! The images of a Gen 1 or Gen 2 player graphics pack, checked against the sizes the games use, and for Gen 2 the
//! palettes of the player's pictures and overworld sprites.

use ppr_model::Color;

use super::gbc::{DEFAULT_PALETTE, GbcImage};
use super::pack::{Entry, Reader, SheetImage, framed, single};
use super::png::Source;

/// The generation a pack is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GbGames {
    Gen1,
    Gen2,
}

impl GbGames {
    /// The `RomType` of the packs for these games.
    pub(crate) const fn rom_type(self) -> &'static str {
        match self {
            Self::Gen1 => "Gen1",
            Self::Gen2 => "Gen2",
        }
    }

    /// The back image's width and height in tiles.
    const fn back_size(self) -> usize {
        match self {
            Self::Gen1 => 4,
            Self::Gen2 => 6,
        }
    }
}

const FRONT_SIZE: usize = 7;
/// Tiles of a walk or bike sprite: 4 tiles a frame, 3 directions, 2 frames each.
const OVERWORLD_TILES: usize = 4 * 3 * 2;
/// Tiles of the fishing sprite: 2 for each direction.
const FISH_TILES: usize = OVERWORLD_TILES / 4;
const TRAINER_CARD_WIDTH: usize = 5;
const TRAINER_CARD_HEIGHT: usize = 7;

const SIX_FRAMES: &[(usize, usize)] = &[(0, 0), (1, 0), (2, 0), (0, 1), (1, 1), (2, 1)];

const GEN1_SHEET: &[SheetImage] = &[
    single("FrontImage", 9, 3, 56, 56),
    single("BackImage", 68, 27, 32, 32),
    framed("WalkSprite", 4, 62, 16, 16, SIX_FRAMES),
    framed("BikeSprite", 55, 62, 16, 16, SIX_FRAMES),
    single("FishFrontSprite", 4, 104, 16, 8),
    single("FishBackSprite", 21, 112, 16, 8),
    single("FishSideSprite", 46, 112, 16, 8),
];

const GEN2_SHEET: &[SheetImage] = &[
    single("FrontImage", 9, 3, 56, 56),
    single("BackImage", 68, 11, 48, 48),
    framed("WalkSprite", 12, 62, 16, 16, SIX_FRAMES),
    framed("BikeSprite", 63, 62, 16, 16, SIX_FRAMES),
    single("FishFrontSprite", 12, 104, 16, 8),
    single("FishBackSprite", 29, 112, 16, 8),
    single("FishSideSprite", 54, 112, 16, 8),
];

/// The palettes of Gen 2's overworld sprites, by number: the colors after the transparent one.
const SPRITE_PALETTES: [(&str, [u32; 3]); 8] = [
    ("RED", [0xF8_98_50, 0xF8_38_08, 0x00_00_00]),
    ("BLUE", [0xF8_98_50, 0x50_48_F8, 0x00_00_00]),
    ("GREEN", [0xF8_98_50, 0x35_B7_21, 0x00_00_00]),
    ("BROWN", [0xF8_98_50, 0x78_50_18, 0x00_00_00]),
    ("PINK", [0xF8_98_50, 0xF0_50_30, 0x00_00_00]),
    ("EMOTE", [0xF8_F8_F8, 0x68_68_68, 0x00_00_00]),
    ("TREE", [0x60_C8_08, 0x28_70_00, 0x38_38_38]),
    ("ROCK", [0xC0_90_38, 0xA8_78_30, 0x38_38_38]),
];

/// The 4 colors of sprite palette `id`, the first white.
pub(crate) fn sprite_palette(id: u8) -> [Color; 4] {
    let colors = SPRITE_PALETTES[usize::from(id) % SPRITE_PALETTES.len()].1;
    [
        Color::WHITE,
        Color::hex(colors[0]),
        Color::hex(colors[1]),
        Color::hex(colors[2]),
    ]
}

/// The first sprite palette whose colors no color of `palette` exceeds by 8 or more in any component.
fn matching_sprite_palette(palette: [Color; 4]) -> Option<u8> {
    SPRITE_PALETTES
        .iter()
        .position(|(_, colors)| {
            (1..4).all(|col| {
                let ours = palette[col];
                let theirs = Color::hex(colors[col - 1]);
                (0..3).all(|c| i32::from(ours.comp(c)) - i32::from(theirs.comp(c)) < 8)
            })
        })
        .and_then(|i| u8::try_from(i).ok())
}

/// The images of a pack; those it doesn't have, or that have the wrong size, are `None`.
#[derive(Debug, Clone)]
pub(crate) struct GbPlayerGraphics {
    pub(crate) front: Option<GbcImage>,
    pub(crate) back: Option<GbcImage>,
    pub(crate) walk: Option<GbcImage>,
    pub(crate) bike: Option<GbcImage>,
    pub(crate) fish: Option<GbcImage>,
    /// Gen 2: the trainer card picture, by default part of the front image.
    pub(crate) trainer_card: Option<GbcImage>,
    /// Gen 2: the two colors of the front and back image between white and black.
    pub(crate) image_palette: Option<[Color; 2]>,
    /// Gen 2: the overworld sprite palette, red when the pack names none that exists and no palette matches.
    pub(crate) sprite_palette: u8,
}

fn image(reader: &Reader, key: &str, column_mode: bool) -> Option<GbcImage> {
    GbcImage::build(&reader.image(key)?, column_mode)
}

fn sized(image: Option<GbcImage>, width: usize, height: usize) -> Option<GbcImage> {
    image.filter(|i| i.width_in_tiles() == width && i.height_in_tiles() == height)
}

fn with_tiles(image: Option<GbcImage>, tiles: usize) -> Option<GbcImage> {
    image.filter(|i| i.tiles() == tiles)
}

impl GbPlayerGraphics {
    pub(crate) fn new(entry: Entry, games: GbGames) -> Self {
        let sheet = match games {
            GbGames::Gen1 => GEN1_SHEET,
            GbGames::Gen2 => GEN2_SHEET,
        };
        let mut reader = Reader::new(entry, &[sheet], &[]);
        if reader.uses_sheet() {
            reader.entry.put("FishSpriteMode", "separate");
        }
        let front = sized(image(&reader, "FrontImage", true), FRONT_SIZE, FRONT_SIZE);
        let back = sized(image(&reader, "BackImage", true), games.back_size(), games.back_size());
        let walk = with_tiles(image(&reader, "WalkSprite", false), OVERWORLD_TILES);
        let bike = with_tiles(image(&reader, "BikeSprite", false), OVERWORLD_TILES);
        let fish = Self::fish(&reader, walk.as_ref());
        let mut graphics = Self {
            front,
            back,
            walk,
            bike,
            fish,
            trainer_card: None,
            image_palette: None,
            sprite_palette: 0,
        };
        if games == GbGames::Gen2 {
            graphics.trainer_card = sized(
                image(&reader, "TrainerCardImage", true),
                TRAINER_CARD_WIDTH,
                TRAINER_CARD_HEIGHT,
            )
            .or_else(|| {
                graphics
                    .front
                    .as_ref()?
                    .tile_rect(1, 0, TRAINER_CARD_WIDTH, TRAINER_CARD_HEIGHT)
            });
            graphics.image_palette = match reader.palette("ImagePalette") {
                Some(p) => <[Color; 2]>::try_from(p).ok(),
                None => graphics
                    .front
                    .as_ref()
                    .or(graphics.back.as_ref())
                    .map(|i| [i.palette()[1], i.palette()[2]]),
            };
            let name = reader.entry.value("SpritePalette");
            graphics.sprite_palette = if name.is_empty() {
                graphics
                    .walk
                    .as_ref()
                    .and_then(|w| matching_sprite_palette(w.palette()))
            } else {
                SPRITE_PALETTES
                    .iter()
                    .position(|(n, _)| n.eq_ignore_ascii_case(name))
                    .and_then(|i| u8::try_from(i).ok())
            }
            .unwrap_or(0);
        }
        graphics
    }

    /// The fishing sprite: one image (`combined`) or three (`separate`), one below the other; otherwise, or when
    /// it has the wrong size, the walk sprite's still frames.
    fn fish(reader: &Reader, walk: Option<&GbcImage>) -> Option<GbcImage> {
        let mode = reader.entry.value("FishSpriteMode");
        let fish = if mode.eq_ignore_ascii_case("combined") {
            image(reader, "FishSprite", false)
        } else if mode.eq_ignore_ascii_case("separate") {
            let parts: Vec<Source> = ["FishFrontSprite", "FishBackSprite", "FishSideSprite"]
                .iter()
                .filter_map(|key| reader.image(key))
                .collect();
            Source::stack(&parts).and_then(|s| GbcImage::build(&s, false))
        } else {
            None
        };
        with_tiles(fish, FISH_TILES).or_else(|| {
            let walk = walk?;
            let parts = [walk.tile_range(2, 4), walk.tile_range(6, 8), walk.tile_range(10, 12)];
            GbcImage::build(&Source::stack(&parts)?, false)
        })
    }

    /// The overworld sprites' palette in samples.
    pub(crate) fn overworld_palette(&self, games: GbGames) -> [Color; 4] {
        match games {
            GbGames::Gen1 => DEFAULT_PALETTE,
            GbGames::Gen2 => sprite_palette(self.sprite_palette),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_palettes_match_one_way() {
        let red = sprite_palette(0);
        assert_eq!(matching_sprite_palette(red), Some(0));
        assert_eq!(matching_sprite_palette(sprite_palette(6)), Some(6));
        // Darker colors still match: only being 8 or more above a color fails.
        let darker = [
            Color::WHITE,
            Color::rgb(0xF0, 0x90, 0x40),
            Color::rgb(0xF0, 0x30, 0x00),
            red[3],
        ];
        assert_eq!(matching_sprite_palette(darker), Some(0));
        assert_eq!(matching_sprite_palette([Color::WHITE; 4]), None);
    }
}
