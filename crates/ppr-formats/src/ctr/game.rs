//! A 3DS game as the console runs it: the game partition of the base image and, when an update is installed, the
//! update's partition laid over it.
//!
//! An update ships a complete new executable and the RomFS files it changes; every other file still comes from the
//! base game. [`Game`] reads the executable and each file from the update when the update has it and from the base
//! game otherwise, and a change goes to the partition the file was read from, so each partition tracks against its
//! own original which of its files really changed.
//!
//! A game with an update is written as a LayeredFS folder ([`Game::write_layered_fs`]): the console applies it over
//! the installed base game and update, so it needs only the changed files, by the base game's title ID, and the
//! executable is the update's with the changes. A rebuilt image would lose the update, so [`Game::write_cxi`]
//! refuses a game with an update.
//!
//! Deliberate choices:
//! - A file only the update has is written to the LayeredFS folder when it changed, like any other changed file.
//! - A file the update has is changed in the update only, not also in the base game, as the base game's copy is
//!   never written: an image cannot be saved with an update loaded.
//! - Files and an executable written back unchanged are left out of the folder, including `code.bin`.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::smdh::Smdh;
use super::{CtrHeader, Ncch, layered_fs};
use crate::{FormatError, RomSource};

/// The base game's partition and, optionally, an update's over it.
#[derive(Debug)]
pub struct Game<R> {
    base: Ncch<R>,
    update: Option<Ncch<R>>,
}

impl<R: RomSource> Game<R> {
    /// A game without an update.
    pub fn new(base: Ncch<R>) -> Self {
        Self { base, update: None }
    }

    /// A game with `update` laid over `base`. That the update belongs to the game is the caller's to check (see
    /// [`update_title_id`](super::update_title_id)).
    pub fn with_update(base: Ncch<R>, update: Ncch<R>) -> Self {
        Self {
            base,
            update: Some(update),
        }
    }

    /// Lays `update` over the base game instead of the update loaded before, if any, whose changes are dropped.
    /// Changes made to the base game stay. That the update belongs to the game is the caller's to check.
    pub fn set_update(&mut self, update: Option<Ncch<R>>) {
        self.update = update;
    }

    /// The base game's partition.
    pub fn base(&self) -> &Ncch<R> {
        &self.base
    }

    /// The update's partition, if one is loaded.
    pub fn update(&self) -> Option<&Ncch<R>> {
        self.update.as_ref()
    }

    /// Where the base game's partition is and what it is.
    pub fn identity(&self) -> &CtrHeader {
        self.base.identity()
    }

    /// The partition the executable comes from: the update's when there is one.
    fn code_partition(&mut self) -> &mut Ncch<R> {
        match &mut self.update {
            Some(update) => update,
            None => &mut self.base,
        }
    }

    /// The partition `path` is read from and written to: the update's when it has the file.
    fn file_partition(&mut self, path: &str) -> &mut Ncch<R> {
        match &mut self.update {
            Some(update) if update.has_file(path) => update,
            _ => &mut self.base,
        }
    }

    /// The executable (`.code`), decompressed: the update's when there is one, with the changes.
    ///
    /// # Errors
    /// When there is no executable or it does not decompress.
    pub fn code(&mut self) -> Result<&[u8], FormatError> {
        self.code_partition().code()
    }

    /// Replaces the executable (decompressed). Setting the original contents undoes the change.
    ///
    /// # Errors
    /// When there is no executable to replace.
    pub fn set_code(&mut self, code: Vec<u8>) -> Result<(), FormatError> {
        self.code_partition().set_code(code)
    }

    /// Whether the executable was replaced.
    pub fn is_code_changed(&self) -> bool {
        self.update.as_ref().unwrap_or(&self.base).is_code_changed()
    }

    /// Whether the base game or the update has a RomFS file at `path`.
    pub fn has_file(&self, path: &str) -> bool {
        self.base.has_file(path) || self.update.as_ref().is_some_and(|u| u.has_file(path))
    }

    /// The paths of all RomFS files of the base game and the update, sorted.
    pub fn file_names(&self) -> impl Iterator<Item = &str> {
        let mut names: BTreeSet<&str> = self.base.file_names().collect();
        if let Some(update) = &self.update {
            names.extend(update.file_names());
        }
        names.into_iter()
    }

    /// The RomFS file at `path`, from the update when it has the file, with changes.
    ///
    /// # Errors
    /// When neither has the file or it cannot be read.
    pub fn read_file(&mut self, path: &str) -> Result<Vec<u8>, FormatError> {
        self.file_partition(path).read_file(path)
    }

    /// Replaces the RomFS file at `path` in the partition it is read from. Replacing a file with what that
    /// partition holds undoes the change.
    ///
    /// # Errors
    /// When neither has the file or the original cannot be read for the comparison.
    pub fn replace_file(&mut self, path: &str, data: Vec<u8>) -> Result<(), FormatError> {
        self.file_partition(path).replace_file(path, data)
    }

    /// The RomFS files that were replaced, with their new contents, by path: the update's first, then the base
    /// game's. No path is in both, as a file the update has is only changed there.
    pub fn changed_files(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.update
            .iter()
            .flat_map(Ncch::changed_files)
            .chain(self.base.changed_files())
    }

    /// The base game's icon file, if its ExeFS has one.
    pub fn smdh(&self) -> Option<&Smdh> {
        self.base.smdh()
    }

    /// The base game's icon file for editing; the edit is written by [`write_cxi`](Self::write_cxi).
    pub fn smdh_mut(&mut self) -> Option<&mut Smdh> {
        self.base.smdh_mut()
    }

    /// Whether anything differs from the base game and the update as loaded.
    pub fn is_modified(&self) -> bool {
        self.base.is_modified() || self.update.as_ref().is_some_and(Ncch::is_modified)
    }

    /// Writes the base game's partition as a decrypted CXI with all changes, and returns its length.
    ///
    /// # Errors
    /// [`FormatError::Unencodable`] for a game with an update, which is written as LayeredFS; otherwise as
    /// [`Ncch::write_cxi`].
    pub fn write_cxi(&mut self, out: &mut (impl Write + ?Sized)) -> Result<u64, FormatError> {
        if self.update.is_some() {
            return Err(FormatError::Unencodable {
                format: "CXI",
                reason: "a game with an update can only be saved as a LayeredFS folder",
            });
        }
        self.base.write_cxi(out)
    }

    /// Writes the changed executable and RomFS files below `dir/<title id of the base game>/`, which is emptied
    /// first: `code.bin` and `romfs/<path>`. Returns the `<title id>` folder.
    ///
    /// # Errors
    /// When writing fails or a changed file has a path that cannot be a relative file path.
    pub fn write_layered_fs(&mut self, dir: &Path) -> Result<PathBuf, FormatError> {
        let code = if self.is_code_changed() {
            Some(self.code()?.to_vec())
        } else {
            None
        };
        layered_fs::write(dir, &self.identity().title_id, code.as_deref(), self.changed_files())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Cursor;

    use super::*;
    use crate::ctr::testing::{Cxi, cci, cia, cxi};
    use crate::ctr::update_title_id;

    const BASE_ID: u64 = 0x0004_0000_0005_5D00;
    const UPDATE_ID: u64 = 0x0004_000E_0005_5D00;

    fn base_image() -> Vec<u8> {
        cci(&cxi(&Cxi {
            product_code: "CTR-P-EKJA",
            title_id: BASE_ID,
            code: &[0xB0; 4000],
            compress_code: true,
            files: &[
                ("a/0/0/0", b"base only"),
                ("a/0/0/1", b"old in base"),
                ("DllBattle.cro", b"base battle"),
            ],
            decrypted: true,
        }))
    }

    fn update_image() -> Vec<u8> {
        cia(
            &cxi(&Cxi {
                product_code: "CTR-P-EKJA",
                title_id: UPDATE_ID,
                code: &[0xC1; 5000],
                compress_code: true,
                files: &[
                    ("a/0/0/1", b"new in update"),
                    ("a/0/0/9", b"update only"),
                    ("DllBattle.cro", b"update battle"),
                ],
                decrypted: true,
            }),
            5232,
        )
    }

    fn game<'a>(base: &'a [u8], update: &'a [u8]) -> Game<Cursor<&'a [u8]>> {
        Game::with_update(
            Ncch::open(Cursor::new(base)).expect("base opens"),
            Ncch::open(Cursor::new(update)).expect("update opens"),
        )
    }

    #[test]
    fn reads_the_update_over_the_base_game() {
        let (base, update) = (base_image(), update_image());
        let mut g = game(&base, &update);
        assert_eq!(g.update().expect("loaded").title_version(), 5232);
        assert_eq!(g.base().title_version(), 0, "a CCI has no version");
        assert_eq!(
            g.update().expect("loaded").identity().title_id,
            update_title_id(&g.identity().title_id)
        );
        assert_eq!(g.identity().title_id, "0004000000055D00");
        assert_eq!(g.code().expect("code"), [0xC1; 5000], "the update's executable");
        assert_eq!(g.read_file("a/0/0/0").expect("exists"), b"base only");
        assert_eq!(g.read_file("a/0/0/1").expect("exists"), b"new in update");
        assert_eq!(g.read_file("a/0/0/9").expect("exists"), b"update only");
        assert!(g.has_file("a/0/0/9") && g.has_file("a/0/0/0") && !g.has_file("a/0/0/2"));
        assert_eq!(
            g.file_names().collect::<Vec<_>>(),
            ["DllBattle.cro", "a/0/0/0", "a/0/0/1", "a/0/0/9"]
        );
        assert!(g.read_file("a/0/0/2").is_err());
        assert!(!g.is_modified());
    }

    #[test]
    fn changes_are_tracked_against_the_file_they_replace() {
        let (base, update) = (base_image(), update_image());
        let mut g = game(&base, &update);
        // Writing back what was read is no change, also for a file the update replaces.
        g.replace_file("a/0/0/1", b"new in update".to_vec()).expect("exists");
        g.set_code(vec![0xC1; 5000]).expect("has code");
        assert!(!g.is_modified() && !g.is_code_changed());
        g.replace_file("a/0/0/1", b"randomized".to_vec()).expect("exists");
        g.replace_file("a/0/0/0", b"randomized base".to_vec()).expect("exists");
        g.replace_file("a/0/0/9", b"randomized update".to_vec())
            .expect("exists");
        assert_eq!(g.read_file("a/0/0/1").expect("exists"), b"randomized");
        assert!(
            g.base().changed_files().all(|(p, _)| p == "a/0/0/0"),
            "not changed in the base game"
        );
        let mut changed: Vec<_> = g.changed_files().map(|(p, _)| p).collect();
        changed.sort_unstable();
        assert_eq!(changed, ["a/0/0/0", "a/0/0/1", "a/0/0/9"]);
        assert!(g.is_modified());
    }

    #[test]
    fn writes_only_changes_as_layered_fs() {
        let (base, update) = (base_image(), update_image());
        let mut g = game(&base, &update);
        let dir = tempfile::tempdir().expect("temp dir");
        let root = g.write_layered_fs(dir.path()).expect("writes");
        assert_eq!(root, dir.path().join("0004000000055D00"), "named after the base game");
        assert_eq!(
            fs::read_dir(&root).expect("exists").count(),
            1,
            "only an empty romfs folder"
        );
        assert_eq!(fs::read_dir(root.join("romfs")).expect("exists").count(), 0);

        let mut code = g.code().expect("code").to_vec();
        code[100] = 0x42;
        g.set_code(code.clone()).expect("has code");
        g.replace_file("a/0/0/1", b"randomized".to_vec()).expect("exists");
        g.replace_file("a/0/0/9", b"randomized update".to_vec())
            .expect("exists");
        g.replace_file("a/0/0/0", b"randomized base".to_vec()).expect("exists");
        let root = g.write_layered_fs(dir.path()).expect("writes");
        assert_eq!(fs::read(root.join("code.bin")).expect("written"), code, "decompressed");
        assert_eq!(fs::read(root.join("romfs/a/0/0/1")).expect("written"), b"randomized");
        assert_eq!(
            fs::read(root.join("romfs/a/0/0/9")).expect("written"),
            b"randomized update"
        );
        assert_eq!(
            fs::read(root.join("romfs/a/0/0/0")).expect("written"),
            b"randomized base"
        );
        assert!(!root.join("romfs/DllBattle.cro").exists());
        let mut all = Vec::new();
        let mut stack = vec![root.clone()];
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(d).expect("readable") {
                let p = e.expect("entry").path();
                if p.is_dir() {
                    stack.push(p);
                } else {
                    all.push(
                        p.strip_prefix(&root)
                            .expect("inside")
                            .to_string_lossy()
                            .replace('\\', "/"),
                    );
                }
            }
        }
        all.sort();
        assert_eq!(all, ["code.bin", "romfs/a/0/0/0", "romfs/a/0/0/1", "romfs/a/0/0/9"]);
    }

    #[test]
    fn a_game_with_an_update_is_no_image() {
        let (base, update) = (base_image(), update_image());
        let mut g = game(&base, &update);
        let err = g.write_cxi(&mut Vec::new()).expect_err("refused");
        assert!(matches!(err, FormatError::Unencodable { .. }));
        let mut plain = Game::new(Ncch::open(Cursor::new(base.as_slice())).expect("opens"));
        assert_eq!(plain.code().expect("code"), [0xB0; 4000]);
        let mut out = Vec::new();
        plain.write_cxi(&mut out).expect("writes");
        assert_eq!(out, base[0x4000..], "unchanged partition");
    }
}
