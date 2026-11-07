//! Opening a ROM for randomization (ADR 0002: load → randomize → write).

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Write};
use std::path::{Path, PathBuf};

use ppr_model::{Capabilities, EditRules, GameData, RomIdentity};

use crate::RomError;
use crate::detect::{Detection, Unsupported, detect};
use crate::gen1::Gen1Handler;
use crate::gen2::Gen2Handler;
use crate::gen3::Gen3Handler;
use crate::gen4::Gen4Handler;
use crate::gen5::Gen5Handler;
use crate::gen6::Gen6Handler;
use crate::gen7::Gen7Handler;

/// A loaded ROM. Randomizers never see this; they work on [`GameData`].
pub trait GameHandler: std::fmt::Debug + Send {
    /// The game data as loaded from the ROM.
    fn game_data(&self) -> &GameData;
    /// What the game supports, for the options the UI offers.
    fn capabilities(&self) -> Capabilities;
    /// What manual edits may do in this game (ADR 0008), from the game as loaded.
    fn edit_rules(&self) -> EditRules;
    /// Writes every difference between `data` and [`game_data`](Self::game_data) into the in-memory ROM.
    fn apply(&mut self, data: &GameData) -> Result<(), RomError>;
    /// Makes the game data's wild encounter sets those by time of day (`on`), or those without it, and has
    /// [`apply`](Self::apply) write that view. Choose the view before changing the game data. Nothing for games
    /// whose time of day handling is not ported.
    fn use_time_of_day(&mut self, _on: bool) -> Result<(), RomError> {
        Ok(())
    }
    /// For each wild encounter set of the game data, the set it comes from in the view without time of day (its
    /// index there): by time of day several sets come from one. Without time of day, each set is its own.
    fn encounter_set_origins(&self) -> Vec<usize> {
        (0..self.game_data().encounters.len()).collect()
    }
    /// Writes the ROM.
    fn save(&self, out: &mut dyn Write) -> Result<(), RomError>;
    /// Marks the game with the seed it was randomized with, where it has room for that (the title description of
    /// 3DS games). Nothing by default.
    fn set_seed(&mut self, _seed: u64) {}
    /// Writes the changes as a LayeredFS folder below `dir` and returns the folder it made (3DS games).
    fn save_layered_fs(&self, _dir: &Path) -> Result<PathBuf, RomError> {
        Err(RomError::Unsupported(
            "this game can't be saved as a LayeredFS folder".into(),
        ))
    }
    /// Loads a decrypted game update, whose files then take the place of the game's (3DS games).
    fn load_game_update(&mut self, _path: &Path) -> Result<(), RomError> {
        Err(RomError::Unsupported("this game has no game updates".into()))
    }
    /// What the title screen of the loaded game update shows, e.g. `Ver. 1.5`; `None` without an update.
    fn game_update_version(&self) -> Option<String> {
        None
    }
    /// Whether the player can be either of two characters, so custom player graphics can replace the second.
    fn multiple_player_characters(&self) -> bool {
        true
    }
}

/// Identifies the ROM at `path` and loads it with its generation's handler.
pub fn open(path: &Path) -> Result<Box<dyn GameHandler>, RomError> {
    let identity = identify(detect(&mut BufReader::new(File::open(path)?))?)?;
    match identity.generation.number() {
        // 3DS images are read from the file as needed, not loaded whole.
        6 => {
            let file = BufReader::with_capacity(1 << 16, File::open(path)?);
            return Ok(Box::new(Gen6Handler::open(Box::new(file), identity)?));
        }
        7 => {
            let file = BufReader::with_capacity(1 << 16, File::open(path)?);
            return Ok(Box::new(Gen7Handler::open(Box::new(file), identity)?));
        }
        _ => {}
    }
    let load = || {
        let mut bytes = Vec::new();
        File::open(path)?.read_to_end(&mut bytes)?;
        Ok(bytes)
    };
    load_with_handler(identity, load)
}

/// [`open`] for a ROM already in memory.
pub fn open_bytes(bytes: Vec<u8>) -> Result<Box<dyn GameHandler>, RomError> {
    let identity = identify(detect(&mut Cursor::new(bytes.as_slice()))?)?;
    match identity.generation.number() {
        6 => return Ok(Box::new(Gen6Handler::open(Box::new(Cursor::new(bytes)), identity)?)),
        7 => return Ok(Box::new(Gen7Handler::open(Box::new(Cursor::new(bytes)), identity)?)),
        _ => {}
    }
    load_with_handler(identity, || Ok(bytes))
}

fn identify(detection: Detection) -> Result<RomIdentity, RomError> {
    match detection {
        Detection::Supported(identity) => Ok(identity),
        Detection::Unsupported(Unsupported::UnknownFile) => Err(RomError::Unsupported("not a supported ROM".into())),
        Detection::Unsupported(Unsupported::UnknownGame { platform, code }) => Err(RomError::Unsupported(format!(
            "unsupported {} game {code}",
            platform.display_name()
        ))),
    }
}

/// Loads the ROM with its generation's handler; `bytes` is only read for a supported generation.
fn load_with_handler(
    identity: RomIdentity,
    bytes: impl FnOnce() -> Result<Vec<u8>, RomError>,
) -> Result<Box<dyn GameHandler>, RomError> {
    match identity.generation.number() {
        1 => Ok(Box::new(Gen1Handler::load(bytes()?, identity)?)),
        2 => Ok(Box::new(Gen2Handler::load(bytes()?, identity)?)),
        3 => Ok(Box::new(Gen3Handler::load(bytes()?, identity)?)),
        4 => Ok(Box::new(Gen4Handler::load(bytes()?, identity)?)),
        5 => Ok(Box::new(Gen5Handler::load(bytes()?, identity)?)),
        _ => Err(RomError::NotYetSupported {
            generation: identity.generation,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_bytes_loads_like_open() {
        let rom = crate::gen3::fixtures::emerald().rom;
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("emerald.gba");
        std::fs::write(&path, &rom).expect("write");
        let from_file = open(&path).expect("opens");
        let from_bytes = open_bytes(rom).expect("opens");
        assert_eq!(from_file.game_data(), from_bytes.game_data());
    }

    #[test]
    fn open_bytes_refuses_what_it_does_not_know() {
        assert!(matches!(open_bytes(vec![0; 64]), Err(RomError::Unsupported(_))));
    }
}
