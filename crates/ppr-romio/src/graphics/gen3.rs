//! The images and palettes of a Gen 3 player graphics pack, checked against the sizes the games use.

use ppr_model::Color;

use super::image::{GbaImage, PALETTE_SIZE};
use super::pack::{Entry, Reader, SheetImage, SheetPalette, framed, single};
use super::png::Source;

const FRONT: usize = 8;
const MAP_ICON: usize = 2;
const MEDIUM: (usize, usize) = (2, 4);
const BIG: (usize, usize) = (4, 4);
const HUGE: (usize, usize) = (8, 8);

/// Frames of the walk, run and bike sheets, in the order the games use.
const WALKING: &[(usize, usize)] = &[(0, 0), (0, 1), (0, 2), (1, 0), (2, 0), (1, 1), (2, 1), (1, 2), (2, 2)];
const THREE_DOWN: &[(usize, usize)] = &[(0, 0), (0, 1), (0, 2)];

const SHEET_IMAGES: &[SheetImage] = &[
    single("FrontImage", 11, 53, 64, 64),
    framed("BackImage", 90, 53, 64, 64, &[(0, 0), (1, 0), (2, 0), (3, 0)]),
    single("MapIcon", 11, 15, 16, 16),
    framed("WalkSprite", 11, 139, 16, 32, WALKING),
    framed("RunSprite", 66, 139, 16, 32, WALKING),
    framed("BikeSprite", 121, 139, 32, 32, WALKING),
    framed(
        "FishSprite",
        224,
        139,
        32,
        32,
        &[
            (0, 2),
            (1, 2),
            (2, 2),
            (3, 2),
            (0, 1),
            (1, 1),
            (2, 1),
            (3, 1),
            (0, 0),
            (1, 0),
            (2, 0),
            (3, 0),
        ],
    ),
];

const SHEET_PALETTES: &[SheetPalette] = &[
    SheetPalette {
        key: "FrontImagePalette",
        x: 43,
        y: 49,
    },
    SheetPalette {
        key: "MapIconPalette",
        x: 11,
        y: 11,
    },
    SheetPalette {
        key: "SpriteNormalPalette",
        x: 323,
        y: 135,
    },
    SheetPalette {
        key: "SpriteReflectionPalette",
        x: 323,
        y: 130,
    },
];

const RSE_SHEET_IMAGES: &[SheetImage] = &[
    framed("BackImage", 90, 53, 64, 64, &[(1, 0), (2, 0), (3, 0), (0, 0)]),
    framed("SitSprite", 81, 360, 32, 32, THREE_DOWN),
    framed("SurfBlobSprite", 155, 360, 32, 32, THREE_DOWN),
    single("BirdSprite", 192, 426, 32, 32),
    framed("SitJumpSprite", 118, 360, 32, 32, THREE_DOWN),
    framed(
        "AcroBikeSprite",
        11,
        250,
        32,
        32,
        &[
            (0, 0),
            (0, 1),
            (0, 2),
            (1, 0),
            (2, 0),
            (1, 1),
            (2, 1),
            (1, 2),
            (2, 2),
            (3, 0),
            (4, 0),
            (5, 0),
            (6, 0),
            (3, 1),
            (4, 1),
            (5, 1),
            (6, 1),
            (3, 2),
            (4, 2),
            (5, 2),
            (6, 2),
            (7, 0),
            (8, 0),
            (7, 1),
            (8, 1),
            (7, 2),
            (8, 2),
        ],
    ),
    framed("UnderwaterSprite", 239, 404, 32, 32, THREE_DOWN),
    framed(
        "WateringCanSprite",
        11,
        360,
        32,
        32,
        &[(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)],
    ),
    single("DecorateSprite", 180, 470, 16, 32),
    framed(
        "FieldMoveSprite",
        11,
        470,
        32,
        32,
        &[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)],
    ),
];

const RSE_SHEET_PALETTES: &[SheetPalette] = &[
    SheetPalette {
        key: "BackImagePalette",
        x: 317,
        y: 49,
    },
    SheetPalette {
        key: "SpriteUnderwaterPalette",
        x: 239,
        y: 385,
    },
];

const FRLG_SHEET_IMAGES: &[SheetImage] = &[
    framed("BackImage", 90, 53, 64, 64, &[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0)]),
    framed("SitSprite", 11, 327, 16, 32, THREE_DOWN),
    framed(
        "SurfBlobSprite",
        32,
        327,
        32,
        32,
        &[(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)],
    ),
    framed("BirdSprite", 102, 327, 64, 64, &[(0, 0), (1, 0), (2, 0)]),
    framed(
        "ItemSprite",
        11,
        250,
        16,
        32,
        &[(0, 0), (1, 0), (2, 1), (3, 1), (4, 1), (2, 0), (3, 0), (4, 0), (5, 0)],
    ),
    framed(
        "ItemBikeSprite",
        117,
        250,
        32,
        32,
        &[(0, 0), (1, 0), (2, 0), (3, 0), (4, 0), (5, 0)],
    ),
];

/// Which Gen 3 games a pack is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Gen3Games {
    /// Ruby, Sapphire and Emerald.
    Rse,
    /// `FireRed` and `LeafGreen`.
    Frlg,
}

impl Gen3Games {
    /// The pack entries' `RomType`.
    pub(crate) fn rom_type(self) -> &'static str {
        match self {
            Self::Rse => "RSE",
            Self::Frlg => "FRLG",
        }
    }
}

/// The images and palettes of a Gen 3 pack; missing or wrongly sized ones are `None`.
#[derive(Debug, Clone, Default)]
pub(crate) struct Gen3PlayerGraphics {
    pub(crate) front: Option<GbaImage>,
    pub(crate) back: Option<GbaImage>,
    pub(crate) walk: Option<GbaImage>,
    /// The run sprite, frames in the order of the pack's games; the walk sprite when the pack has none.
    pub(crate) run: Option<GbaImage>,
    pub(crate) bike: Option<GbaImage>,
    pub(crate) fish: Option<GbaImage>,
    pub(crate) sit: Option<GbaImage>,
    pub(crate) surf_blob: Option<GbaImage>,
    pub(crate) bird: Option<GbaImage>,
    pub(crate) map_icon: Option<GbaImage>,
    pub(crate) normal_palette: Option<Vec<Color>>,
    pub(crate) reflection_palette: Option<Vec<Color>>,
    // Ruby, Sapphire and Emerald only.
    pub(crate) sit_jump: Option<GbaImage>,
    pub(crate) acro_bike: Option<GbaImage>,
    pub(crate) underwater: Option<GbaImage>,
    pub(crate) underwater_palette: Option<Vec<Color>>,
    pub(crate) watering_can: Option<GbaImage>,
    pub(crate) decorate: Option<GbaImage>,
    pub(crate) field_move: Option<GbaImage>,
    // FireRed and LeafGreen only.
    pub(crate) item: Option<GbaImage>,
    pub(crate) item_bike: Option<GbaImage>,
}

struct Gen3Reader {
    reader: Reader,
    games: Gen3Games,
}

impl Gen3Reader {
    /// A palette of the pack, if it has exactly 16 colors.
    fn palette(&self, key: &str) -> Option<Vec<Color>> {
        self.reader.palette(key).filter(|p| p.len() == PALETTE_SIZE)
    }

    fn build(source: &Source, palette: Option<&[Color]>) -> Option<GbaImage> {
        GbaImage::build(source, palette)
    }

    /// The image `key` with its own palette, if it is `width` × `height` tiles.
    fn image(&self, key: &str, width: usize, height: usize) -> Option<GbaImage> {
        let source = self.reader.image(key)?;
        let image = Self::build(&source, self.palette(&format!("{key}Palette")).as_deref())?;
        (image.width_in_tiles() == width && image.height_in_tiles() == height).then_some(image)
    }

    /// The sprite `key` split into `frames` frames of `w` × `h` tiles, with the normal sprite palette or another.
    fn sprite_with(&self, key: &str, palette_key: &str, frames: usize, (w, h): (usize, usize)) -> Option<GbaImage> {
        let source = self.reader.image(key)?;
        let image = Self::build(&source, self.palette(palette_key).as_deref())?;
        if image.width_in_tiles() * image.height_in_tiles() != frames * w * h {
            return None;
        }
        // A sprite whose tiles do not split into frames is left out.
        image.with_frames(w, h)
    }

    fn sprite(&self, key: &str, frames: usize, size: (usize, usize)) -> Option<GbaImage> {
        self.sprite_with(key, "SpriteNormalPalette", frames, size)
    }

    /// The run sprite, its frames reordered when the pack's run
    /// sprite is laid out for the other games.
    fn run(&self, walk: Option<&GbaImage>) -> Option<GbaImage> {
        let mut mode = self.reader.entry.value("RunSpriteMode").to_owned();
        let run = if let Some(run) = self.sprite("RunSprite", 9, MEDIUM) {
            run
        } else {
            mode = "RSE".into();
            walk?.clone()
        };
        let order: &[usize] = match (self.games, mode.to_ascii_lowercase().as_str()) {
            (Gen3Games::Rse, "frlg") => &[0, 3, 6, 1, 2, 4, 5, 7, 8],
            (Gen3Games::Frlg, "rse") => &[0, 3, 4, 1, 5, 6, 2, 7, 8],
            _ => return Some(run),
        };
        let frames: Vec<Source> = order.iter().map(|&i| run.frame(i).source()).collect();
        GbaImage::build(&Source::stack(&frames)?, None)?.with_frames(MEDIUM.0, MEDIUM.1)
    }
}

impl Gen3PlayerGraphics {
    /// Reads the pack `entry` for Ruby/Sapphire/Emerald or FireRed/LeafGreen.
    pub(crate) fn new(entry: Entry, games: Gen3Games) -> Self {
        let (images, palettes): (&[SheetImage], &[SheetPalette]) = match games {
            Gen3Games::Rse => (RSE_SHEET_IMAGES, RSE_SHEET_PALETTES),
            Gen3Games::Frlg => (FRLG_SHEET_IMAGES, &[]),
        };
        let mut reader = Reader::new(entry, &[SHEET_IMAGES, images], &[SHEET_PALETTES, palettes]);
        if reader.uses_sheet() {
            reader.entry.put("RunSpriteMode", "rse");
        }
        let r = Gen3Reader { reader, games };
        let (back_height, sit, surf_blob_frames, bird) = match games {
            Gen3Games::Rse => (32, BIG, 3, (1, BIG)),
            Gen3Games::Frlg => (40, MEDIUM, 6, (3, HUGE)),
        };
        let walk = r.sprite("WalkSprite", 9, MEDIUM);
        let normal_palette = r
            .palette("SpriteNormalPalette")
            .or_else(|| walk.as_ref().map(|w| w.palette().to_vec()));
        // A sheet's first two colors being the same means it has no reflection palette.
        let reflection_palette = r
            .palette("SpriteReflectionPalette")
            .filter(|p| p[0] != p[1])
            .or_else(|| normal_palette.clone());
        let mut g = Self {
            front: r.image("FrontImage", FRONT, FRONT),
            back: r.image("BackImage", 8, back_height),
            run: r.run(walk.as_ref()),
            walk,
            bike: r.sprite("BikeSprite", 9, BIG),
            fish: r.sprite("FishSprite", 12, BIG),
            sit: r.sprite("SitSprite", 3, sit),
            surf_blob: r.sprite("SurfBlobSprite", surf_blob_frames, BIG),
            bird: r.sprite("BirdSprite", bird.0, bird.1),
            map_icon: r.image("MapIcon", MAP_ICON, MAP_ICON),
            normal_palette,
            reflection_palette,
            ..Self::default()
        };
        match games {
            Gen3Games::Rse => {
                g.sit_jump = r.sprite("SitJumpSprite", 3, BIG);
                g.acro_bike = r.sprite("AcroBikeSprite", 27, BIG);
                g.underwater = r.sprite_with("UnderwaterSprite", "SpriteUnderwaterPalette", 3, BIG);
                g.watering_can = r.sprite("WateringCanSprite", 6, BIG);
                g.decorate = r.sprite("DecorateSprite", 1, MEDIUM);
                g.field_move = r.sprite("FieldMoveSprite", 5, BIG);
                g.underwater_palette = r
                    .palette("SpriteUnderwaterPalette")
                    .or_else(|| g.underwater.as_ref().map(|u| u.palette().to_vec()));
            }
            Gen3Games::Frlg => {
                g.item = r.sprite("ItemSprite", 9, MEDIUM);
                g.item_bike = r.sprite("ItemBikeSprite", 6, BIG);
            }
        }
        g
    }
}
