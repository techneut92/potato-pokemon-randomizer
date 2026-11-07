//! The game's files: the executable and the RomFS of the game partition, with a game update laid over them by the
//! shared [`Game`]: reads take the update's copy of a file when it has one, and a change goes to the partition the
//! file was read from.

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::{Read, Seek};

use ppr_formats::ctr::{Decompression, Game, Garc, Ncch};

use crate::RomError;

/// A readable image of a game partition.
pub(crate) trait Source: Read + Seek + Send + Debug {}

impl<T: Read + Seek + Send + Debug> Source for T {}

pub(super) type Image = Ncch<Box<dyn Source>>;

/// A parsed archive, kept while the handler lives, and whether it changed since it was last written back.
#[derive(Debug)]
struct Cached {
    garc: Garc,
    dirty: bool,
}

/// The game with its optional update, and the executable.
#[derive(Debug)]
pub(super) struct Files {
    pub(super) game: Game<Box<dyn Source>>,
    /// The executable with the changes made so far (from the update when there is one).
    pub(super) code: Vec<u8>,
    garcs: BTreeMap<String, Cached>,
}

impl Files {
    pub(super) fn new(base: Image, update: Option<Image>) -> Result<Self, RomError> {
        let mut game = match update {
            Some(update) => Game::with_update(base, update),
            None => Game::new(base),
        };
        let code = game.code()?.to_vec();
        Ok(Self {
            game,
            code,
            garcs: BTreeMap::new(),
        })
    }

    /// Lays an update over the base game (or removes it); what was read before is read again.
    pub(super) fn set_update(&mut self, update: Option<Image>) -> Result<(), RomError> {
        self.game.set_update(update);
        self.garcs.clear();
        self.code = self.game.code()?.to_vec();
        Ok(())
    }

    /// The RomFS file at `path`, from the update when it has it.
    pub(super) fn read(&mut self, path: &str) -> Result<Vec<u8>, RomError> {
        Ok(self.game.read_file(path)?)
    }

    /// Replaces the RomFS file at `path`, in the update when it has it.
    pub(super) fn write(&mut self, path: &str, data: Vec<u8>) -> Result<(), RomError> {
        Ok(self.game.replace_file(path, data)?)
    }

    fn cached(&mut self, path: &str, decompression: &Decompression) -> Result<&mut Cached, RomError> {
        if !self.garcs.contains_key(path) {
            let garc = Garc::parse(self.read(path)?, decompression)?;
            self.garcs.insert(path.to_owned(), Cached { garc, dirty: false });
        }
        self.garcs
            .get_mut(path)
            .ok_or_else(|| RomError::Malformed(format!("archive {path}")))
    }

    /// The archive at `path`, for reading. It is parsed once; `decompression` applies to that first parse.
    pub(super) fn garc(&mut self, path: &str, decompression: &Decompression) -> Result<&Garc, RomError> {
        Ok(&self.cached(path, decompression)?.garc)
    }

    /// The archive at `path`, for changing; [`flush`](Self::flush) writes it back.
    pub(super) fn garc_mut(&mut self, path: &str, decompression: &Decompression) -> Result<&mut Garc, RomError> {
        let cached = self.cached(path, decompression)?;
        cached.dirty = true;
        Ok(&mut cached.garc)
    }

    /// Writes the changed archives and the executable back into the images.
    pub(super) fn flush(&mut self) -> Result<(), RomError> {
        let dirty: Vec<String> = self
            .garcs
            .iter()
            .filter(|(_, c)| c.dirty)
            .map(|(p, _)| p.clone())
            .collect();
        for path in dirty {
            let bytes = match self.garcs.get_mut(&path) {
                Some(c) => {
                    c.dirty = false;
                    c.garc.to_bytes()?
                }
                None => continue,
            };
            self.write(&path, bytes)?;
        }
        // Setting the executable as loaded undoes earlier changes.
        self.game.set_code(self.code.clone())?;
        Ok(())
    }
}
