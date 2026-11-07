use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use ppr_model::{
    Capabilities, DexNumber, EditRules, GameData, Generation, MiscTweak, Platform, PlayerGraphicsPack, RomIdentity,
};
use ppr_romio::{Detection, RomError, Unsupported, detect};
use serde::Serialize;
use ts_rs::TS;

use crate::ServiceError;

/// What a file turned out to be.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "status", rename_all = "camelCase")]
#[ts(export)]
pub enum RomReport {
    /// A supported game.
    Supported {
        /// Which game.
        identity: RomIdentity,
    },
    /// Not a supported game.
    Unsupported {
        /// Why, for the user.
        reason: String,
    },
}

/// Identifies the ROM at `path`. Only its header is read.
pub fn identify_file(path: &Path) -> Result<RomReport, ServiceError> {
    let file = File::open(path).map_err(|source| ServiceError::Open {
        path: path.to_owned(),
        source,
    })?;
    Ok(match detect(&mut BufReader::new(file))? {
        Detection::Supported(identity) => RomReport::Supported { identity },
        Detection::Unsupported(Unsupported::UnknownFile) => RomReport::Unsupported {
            reason: "This is not a Game Boy, Game Boy Advance, Nintendo DS or decrypted Nintendo 3DS ROM.".into(),
        },
        Detection::Unsupported(Unsupported::UnknownGame { platform, code }) => RomReport::Unsupported {
            reason: format!(
                "This {} ROM ({code}) is not a supported game or revision.",
                platform.display_name()
            ),
        },
    })
}

/// A ROM opened in the app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
#[expect(clippy::struct_excessive_bools, reason = "one flag per capability the UI asks about")]
pub struct LoadedRom {
    /// Which game.
    pub identity: RomIdentity,
    /// File name without the directory, for display.
    pub file_name: String,
    /// Whether randomizing this generation is ported yet. When `false`, the fields below are empty.
    pub randomizable: bool,
    /// National numbers of the starters, in slot order.
    pub starters: Vec<DexNumber>,
    /// Number of species in the game, without alternate formes.
    pub species_count: u16,
    /// Whether the game names legendaries the story makes the player face, for "Limit Main-Game Legendaries".
    pub main_game_legendaries: bool,
    /// Whether the special music of static legendaries can follow their replacements ("Fix Music").
    pub static_music_fix: bool,
    /// The misc tweaks the game supports.
    pub tweaks: Vec<MiscTweak>,
    /// Whether the game's Pokémon palettes can be randomized.
    pub palettes: bool,
    /// The custom player graphics packs the game can use, in a fixed order.
    pub player_graphics_packs: Vec<PlayerGraphicsOption>,
    /// Whether the player can be either of two characters, so custom player graphics can replace the second.
    pub multiple_player_characters: bool,
    /// What the game supports; all off when its generation isn't ported.
    pub capabilities: Capabilities,
    /// With a game update laid over the game: what its title screen shows, e.g. `Ver. 1.5`.
    pub update_version: Option<String>,
}

/// A custom player graphics pack with its sample images.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct PlayerGraphicsOption {
    /// The pack.
    pub pack: PlayerGraphicsPack,
    /// PNG data URLs of the front image and the first frames of the back image and walk sprite, as far as the
    /// pack has them.
    pub samples: Vec<String>,
}

impl PlayerGraphicsOption {
    fn new(pack: &PlayerGraphicsPack) -> Self {
        Self {
            samples: ppr_romio::player_graphics_samples(&pack.rom_type, &pack.id)
                .iter()
                .map(|png| format!("data:image/png;base64,{}", base64(png)))
                .collect(),
            pack: pack.clone(),
        }
    }
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        for (i, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            out.push(if i <= chunk.len() {
                char::from(ALPHABET[(n >> shift & 63) as usize])
            } else {
                '='
            });
        }
    }
    out
}

/// Opens the ROM at `path` for the app: identifies it and, when its generation is ported, loads its data.
/// Files that are not a supported game fail with the `rom_unsupported` code and a reason for the user.
pub fn load_rom(path: &Path) -> Result<LoadedRom, ServiceError> {
    load_rom_with_update(path, None)
}

/// [`load_rom`] with the decrypted game update at `update` laid over the game (3DS games). An update that is
/// encrypted or for another game fails with the `rom_unsupported` code.
pub fn load_rom_with_update(path: &Path, update: Option<&Path>) -> Result<LoadedRom, ServiceError> {
    load(path, update).map(|(rom, _)| rom)
}

/// [`load_rom_with_update`], also returning the game's data, with the update laid over it, and its handler's edit
/// rules, when its generation is ported.
pub(crate) fn load(
    path: &Path,
    update: Option<&Path>,
) -> Result<(LoadedRom, Option<(GameData, EditRules)>), ServiceError> {
    let identity = match identify_file(path)? {
        RomReport::Supported { identity } => identity,
        RomReport::Unsupported { reason } => return Err(RomError::Unsupported(reason).into()),
    };
    let file_name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    match crate::randomize::open_rom(path, update) {
        Ok(handler) => {
            let game = handler.game_data();
            let rom = LoadedRom {
                identity,
                file_name,
                randomizable: true,
                starters: game.starters.clone(),
                species_count: u16::try_from(game.base_species().count()).unwrap_or(u16::MAX),
                main_game_legendaries: !game.main_game_legendaries.is_empty(),
                static_music_fix: !game.special_music_statics.is_empty(),
                tweaks: game.available_tweaks.clone(),
                // Gen 1 and Gen 2 palettes need no descriptions.
                palettes: !game.palettes.is_empty()
                    && (game.palette_files.is_some() || game.identity.generation.number() <= 2),
                player_graphics_packs: game
                    .player_graphics_packs
                    .iter()
                    .map(PlayerGraphicsOption::new)
                    .collect(),
                multiple_player_characters: handler.multiple_player_characters(),
                capabilities: handler.capabilities(),
                update_version: handler.game_update_version(),
            };
            Ok((rom, Some((game.clone(), handler.edit_rules()))))
        }
        Err(ServiceError::Rom(RomError::NotYetSupported { .. })) => Ok((
            LoadedRom {
                identity,
                file_name,
                randomizable: false,
                starters: Vec::new(),
                species_count: 0,
                main_game_legendaries: false,
                static_music_fix: false,
                tweaks: Vec::new(),
                palettes: false,
                player_graphics_packs: Vec::new(),
                multiple_player_characters: false,
                capabilities: Capabilities::default(),
                update_version: None,
            },
            None,
        )),
        Err(e) => Err(e),
    }
}

/// One supported game, for help texts and `ppr games`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub struct SupportedGame {
    /// Entry name, e.g. `Platinum (U)`.
    pub name: String,
    /// Generation of the game.
    pub generation: Generation,
    /// Console the game runs on.
    pub platform: Platform,
}

/// Every supported ROM, oldest generation first.
pub fn supported_games() -> Vec<SupportedGame> {
    ppr_romio::entries::catalogs()
        .iter()
        .flat_map(|c| {
            c.entries.iter().map(|e| SupportedGame {
                name: e.name.clone(),
                generation: c.generation,
                platform: c.generation.platform(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn base64_like_the_standard() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
        assert_eq!(base64(&[0xFF, 0xFE]), "//4=");
    }

    #[test]
    fn reports_unknown_files_and_missing_files() {
        let mut f = tempfile::NamedTempFile::new().expect("temp file");
        f.write_all(&[0u8; 64]).expect("write");
        assert!(matches!(identify_file(f.path()), Ok(RomReport::Unsupported { .. })));
        let err = identify_file(Path::new("/definitely/not/here.nds")).expect_err("missing");
        assert_eq!(err.code(), "open_failed");
    }

    #[test]
    fn loads_ported_generations_fully() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("emerald.gba");
        std::fs::write(&path, ppr_romio::testing::emerald()).expect("write");
        let rom = load_rom(&path).expect("loads");
        assert_eq!(rom.identity.name, "Emerald (U)");
        assert_eq!(rom.file_name, "emerald.gba");
        assert!(rom.randomizable);
        assert_eq!(rom.starters, [252, 255, 258]);
        assert_eq!(rom.species_count, 386);
        assert!(!rom.main_game_legendaries, "Emerald names none");
        assert!(rom.static_music_fix);
        let c = rom.capabilities;
        assert!(c.move_tutors && c.english && c.intro_pokemon && c.starter_held_items && c.shop_sizes);
        assert!(!c.map_indices && !c.mega_evolutions && !c.physical_special_split);
    }

    #[test]
    fn updates_are_for_3ds_games_only() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("emerald.gba");
        std::fs::write(&path, ppr_romio::testing::emerald()).expect("write");
        let missing = load_rom_with_update(&path, Some(&dir.path().join("none.cia"))).expect_err("no file");
        assert_eq!(missing.code(), "open_failed");
        let err = load_rom_with_update(&path, Some(&path)).expect_err("no updates");
        assert_eq!(err.code(), "rom_unsupported");
        assert_eq!(load_rom(&path).expect("loads").update_version, None);
    }

    #[test]
    fn unknown_files_fail_with_a_reason() {
        let mut f = tempfile::NamedTempFile::new().expect("temp file");
        f.write_all(&[0u8; 64]).expect("write");
        let err = load_rom(f.path()).expect_err("not a ROM");
        assert_eq!(err.code(), "rom_unsupported");
        assert!(err.to_string().contains("not a Game Boy"), "{err}");
    }

    #[test]
    fn lists_games_of_every_generation() {
        let games = supported_games();
        for g in Generation::ALL {
            assert!(games.iter().any(|x| x.generation == g), "{g} missing");
        }
    }
}
