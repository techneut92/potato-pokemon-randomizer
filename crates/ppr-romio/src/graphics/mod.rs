//! Custom player graphics: the graphics packs, embedded, and the GBA and Game Boy images built from them.
//!
//! Deliberate choices:
//! - The packs are built in (`data/player_graphics`, by `build.rs`), not read from a folder at run time.
//! - An image whose size is not whole tiles, or a sheet too small for its descriptions, leaves only that image
//!   out, not the whole pack; so does a Game Boy image with more than 4 colors.
//! - Only indexed, grayscale and 8-bit RGB and RGBA PNGs are read, the formats the packs use. Game Boy images treat
//!   a pixel as either transparent (alpha 0) or opaque; the packs have nothing in between.
//! - A folder with two entries for the same games gives the second the id `<folder>/2`.
//! - A Gen 2 pack without a walk sprite or sprite palette gets the red sprite palette.

mod gb_player;
mod gbc;
mod gen3;
mod image;
mod pack;
mod png;

pub(crate) use gb_player::{GbGames, GbPlayerGraphics};
pub(crate) use gbc::GbcImage;
pub(crate) use gen3::{Gen3Games, Gen3PlayerGraphics};
pub(crate) use image::{GbaImage, palette_bytes};

use ppr_model::PlayerGraphicsPack;

/// The packs for these Gen 3 games, by category, then source, then name.
pub(crate) fn gen3_packs(games: Gen3Games) -> Vec<PlayerGraphicsPack> {
    pack::entries_for(games.rom_type())
        .iter()
        .map(pack::Entry::info)
        .collect()
}

/// The packs for these Gen 1 or Gen 2 games, by category, then source, then name.
pub(crate) fn gb_packs(games: GbGames) -> Vec<PlayerGraphicsPack> {
    pack::entries_for(games.rom_type())
        .iter()
        .map(pack::Entry::info)
        .collect()
}

/// The graphics of the pack with this id, for these Gen 1 or Gen 2 games.
pub(crate) fn gb_graphics(id: &str, games: GbGames) -> Option<GbPlayerGraphics> {
    let entry = pack::entries_for(games.rom_type()).into_iter().find(|e| e.id == id)?;
    Some(GbPlayerGraphics::new(entry, games))
}

/// Sample images of the pack with this id and `RomType`: its front image and the first frames of its
/// back image and walk sprite, as PNGs with the first color transparent. Empty for an unknown pack.
pub fn player_graphics_samples(rom_type: &str, id: &str) -> Vec<Vec<u8>> {
    let games = match rom_type.to_ascii_uppercase().as_str() {
        "RSE" => Gen3Games::Rse,
        "FRLG" => Gen3Games::Frlg,
        "GEN1" => return gb_samples(id, GbGames::Gen1),
        "GEN2" => return gb_samples(id, GbGames::Gen2),
        _ => return Vec::new(),
    };
    let Some(g) = gen3_graphics(id, games) else {
        return Vec::new();
    };
    [
        g.front,
        g.back.map(|b| b.frame_sized(0, 8, 8)),
        g.walk.map(|w| w.frame_sized(0, 2, 4)),
    ]
    .into_iter()
    .flatten()
    .map(|image| image.to_png())
    .collect()
}

/// The samples of a Gen 1 or 2 pack: the front and back image, and the first frame of the walk sprite in the
/// overworld palette.
fn gb_samples(id: &str, games: GbGames) -> Vec<Vec<u8>> {
    let Some(g) = gb_graphics(id, games) else {
        return Vec::new();
    };
    let walk = g.walk.as_ref().and_then(|w| {
        let frame = w.source().crop(0, 0, 16, 16)?;
        Some(GbcImage::build(&frame, false)?.to_png(Some(g.overworld_palette(games))))
    });
    [
        g.front.as_ref().map(|i| i.to_png(None)),
        g.back.as_ref().map(|i| i.to_png(None)),
        walk,
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The graphics of the pack with this id, for these Gen 3 games.
pub(crate) fn gen3_graphics(id: &str, games: Gen3Games) -> Option<Gen3PlayerGraphics> {
    let entry = pack::entries_for(games.rom_type()).into_iter().find(|e| e.id == id)?;
    Some(Gen3PlayerGraphics::new(entry, games))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gen3_pack_has_its_graphics() {
        for games in [Gen3Games::Rse, Gen3Games::Frlg] {
            for pack in gen3_packs(games) {
                let g = gen3_graphics(&pack.id, games).expect("listed packs load");
                let id = &pack.id;
                assert!(g.front.is_some() && g.back.is_some() && g.map_icon.is_some(), "{id}");
                assert!(
                    g.walk.is_some() && g.run.is_some() && g.bike.is_some() && g.sit.is_some(),
                    "{id}"
                );
                assert!(g.normal_palette.is_some() && g.reflection_palette.is_some(), "{id}");
                // Joseph's pack names a fishing sprite it does not have.
                assert_eq!(g.fish.is_some(), id != "joseph", "{id}");
                assert_eq!(g.run.as_ref().map(GbaImage::frame_count), Some(9), "{id}");
                match games {
                    Gen3Games::Rse => assert!(g.acro_bike.is_some() && g.underwater_palette.is_some(), "{id}"),
                    Gen3Games::Frlg => assert!(g.item.is_some() && g.item_bike.is_some(), "{id}"),
                }
            }
        }
        assert!(gen3_graphics("leaf", Gen3Games::Rse).is_none());
    }

    #[test]
    fn every_gb_pack_has_its_graphics() {
        for games in [GbGames::Gen1, GbGames::Gen2] {
            let packs = gb_packs(games);
            assert!(packs.len() > 20, "{games:?}");
            for pack in packs {
                let g = gb_graphics(&pack.id, games).expect("listed packs load");
                let id = &pack.id;
                assert!(
                    g.front.is_some() && g.walk.is_some() && g.bike.is_some() && g.fish.is_some(),
                    "{id}"
                );
                if games == GbGames::Gen2 {
                    assert!(g.trainer_card.is_some() && g.image_palette.is_some(), "{id}");
                }
                assert_eq!(
                    player_graphics_samples(games.rom_type(), id).len(),
                    2 + usize::from(g.back.is_some())
                );
            }
        }
        let ids: Vec<String> = gb_packs(GbGames::Gen2).into_iter().map(|p| p.id).collect();
        assert!(ids.contains(&"chris".to_owned()) && ids.contains(&"chris/2".to_owned()));
    }
}
