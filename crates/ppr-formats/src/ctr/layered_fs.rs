//! LayeredFS output: the changed files of a game as a folder a mod loader lays over the installed game, so the
//! console or emulator runs the changes without a rebuilt image.
//!
//! The folder is named after the title ID of the base game and holds `code.bin` (the decompressed executable) when
//! the executable changed, and `romfs/<path>` for every changed RomFS file. Unchanged files are left out.

use std::fs;
use std::path::{Path, PathBuf};

use crate::FormatError;

/// Writes the changes below `dir/<title_id>/`, which is emptied first, and returns that folder.
///
/// # Errors
/// When writing fails or a file has a path that cannot be a relative file path.
pub(super) fn write<'a>(
    dir: &Path,
    title_id: &str,
    code: Option<&[u8]>,
    files: impl IntoIterator<Item = (&'a str, &'a [u8])>,
) -> Result<PathBuf, FormatError> {
    let root = dir.join(title_id);
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    let romfs = root.join("romfs");
    fs::create_dir_all(&romfs)?;
    if let Some(code) = code {
        fs::write(root.join("code.bin"), code)?;
    }
    for (path, data) in files {
        let mut target = romfs.clone();
        for part in path.split('/') {
            if part.is_empty() || part == "." || part == ".." || part.contains(['\\', ':']) {
                return Err(FormatError::Unencodable {
                    format: "LayeredFS",
                    reason: "a RomFS path that is not a plain relative path",
                });
            }
            target.push(part);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(target, data)?;
    }
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_paths_that_leave_the_folder() {
        let dir = tempfile::tempdir().expect("temp dir");
        for bad in ["../x", "a//b", "a/./b", "c:/x", "a\\b"] {
            let err = write(dir.path(), "T", None, [(bad, &b"x"[..])]).expect_err(bad);
            assert!(matches!(err, FormatError::Unencodable { .. }), "{bad}");
        }
        assert!(!dir.path().join("x").exists());
    }

    #[test]
    fn replaces_an_earlier_output() {
        let dir = tempfile::tempdir().expect("temp dir");
        write(dir.path(), "T", Some(b"code"), [("a/b", &b"old"[..])]).expect("writes");
        let root = write(dir.path(), "T", None, [("c", &b"new"[..])]).expect("writes");
        assert!(!root.join("code.bin").exists());
        assert!(!root.join("romfs/a").exists());
        assert_eq!(fs::read(root.join("romfs/c")).expect("written"), b"new");
    }
}
