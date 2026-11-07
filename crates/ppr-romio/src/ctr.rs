//! Opening Gen 6 and Gen 7 games: the decrypted game partition of a 3DS image and, optionally, of an update for
//! it, as a [`Game`] the handlers read and write.

use ppr_formats::RomSource;
use ppr_formats::ctr::{Game, Ncch, update_title_id};

use crate::RomError;
use crate::entries::RomEntry;

/// Files every Gen 6 and Gen 7 game has; finding one by name means the file system was decrypted.
const BATTLE_CODE: [&str; 2] = ["DllBattle.cro", "Battle.cro"];

const ENCRYPTED: &str = "this game is encrypted; decrypt it first, then open the decrypted file";

/// Opens the game partition of a CCI, CXI or CIA for reading and writing.
///
/// Images whose header does not mark them as decrypted are accepted when their file system reads and has the
/// battle code module, as some decryption tools leave the mark unset.
///
/// # Errors
/// [`RomError::Unsupported`] for an encrypted image; other errors when the image cannot be read.
pub fn open<R: RomSource>(src: R) -> Result<Ncch<R>, RomError> {
    let ncch = Ncch::open(src).map_err(|e| match e {
        ppr_formats::FormatError::Encrypted => RomError::Unsupported(ENCRYPTED.into()),
        other => other.into(),
    })?;
    if ncch.is_marked_decrypted() || BATTLE_CODE.iter().any(|f| ncch.has_file(f)) {
        Ok(ncch)
    } else {
        Err(RomError::Unsupported(ENCRYPTED.into()))
    }
}

/// Opens a game update (a CIA, or the update's CXI) for the game `base` and checks that it belongs to it: its
/// title ID is the game's with the update type.
///
/// # Errors
/// [`RomError::Unsupported`] for an encrypted update or one for another game; other errors when it cannot be read.
pub fn open_update<R: RomSource>(base: &Ncch<R>, src: R) -> Result<Ncch<R>, RomError> {
    let update = open(src)?;
    let expected = update_title_id(&base.identity().title_id);
    let actual = &update.identity().title_id;
    if *actual != expected {
        return Err(RomError::Unsupported(format!(
            "this is not an update for this game: its title ID is {actual}, the game's updates have {expected}"
        )));
    }
    Ok(update)
}

/// Opens the game in `base` and, when given, the update in `update` laid over it.
///
/// # Errors
/// As [`open`] and [`open_update`].
pub fn open_game<R: RomSource>(base: R, update: Option<R>) -> Result<Game<R>, RomError> {
    let base = open(base)?;
    Ok(match update {
        Some(src) => {
            let update = open_update(&base, src)?;
            Game::with_update(base, update)
        }
        None => Game::new(base),
    })
}

/// Logs a warning when `update` is not the version the ROM entry describes (its `FullyUpdatedVersionNumber`): the
/// game is still loaded, but data the entry locates may have moved.
pub(crate) fn check_update_version<R: RomSource>(update: &Ncch<R>, entry: &RomEntry) {
    let version = update.title_version();
    if let Some(expected) = entry.int("FullyUpdatedVersionNumber")
        && i64::from(version) != expected
    {
        tracing::warn!(
            version,
            expected,
            game = entry.name,
            "game update of an unexpected version"
        );
    }
}

/// The version the title screen shows, e.g. `Ver. 1.5`: line `UpdateStringOffset` of text file
/// `TitleScreenTextOffset`, read by `texts`; `Unpatched` when there is no such line.
pub(crate) fn title_screen_version(
    entry: &RomEntry,
    texts: impl FnOnce(usize) -> Result<Vec<String>, RomError>,
) -> Result<String, RomError> {
    let index = |key: &str| {
        entry
            .int(key)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| RomError::Malformed(format!("ROM entry [{}] has no {key}", entry.name)))
    };
    let line = index("UpdateStringOffset")?;
    Ok(texts(index("TitleScreenTextOffset")?)?
        .get(line)
        .cloned()
        .unwrap_or_else(|| "Unpatched".to_owned()))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use ppr_formats::ctr::testing::{Cxi, cci, cia, cxi};

    use super::*;

    fn image(files: &[(&str, &[u8])], decrypted: bool) -> Vec<u8> {
        image_of(0x0004_0000_0005_5D00, files, decrypted)
    }

    fn image_of(title_id: u64, files: &[(&str, &[u8])], decrypted: bool) -> Vec<u8> {
        cci(&cxi(&Cxi {
            product_code: "CTR-P-EKJA",
            title_id,
            code: &[0xE5; 64],
            compress_code: false,
            files,
            decrypted,
        }))
    }

    fn update_of(title_id: u64, decrypted: bool) -> Vec<u8> {
        cia(
            &cxi(&Cxi {
                product_code: "CTR-P-EKJA",
                title_id,
                code: &[0xE6; 64],
                compress_code: false,
                files: &[("DllBattle.cro", b"new code")],
                decrypted,
            }),
            5232,
        )
    }

    #[test]
    fn opens_an_update_for_the_game() {
        let base = image(&[("DllBattle.cro", b"code")], true);
        let update = update_of(0x0004_000E_0005_5D00, true);
        let mut game = open_game(Cursor::new(base.as_slice()), Some(Cursor::new(update.as_slice()))).expect("opens");
        assert_eq!(game.update().expect("loaded").title_version(), 5232);
        assert_eq!(game.read_file("DllBattle.cro").expect("exists"), b"new code");
        // Updates that decryption tools left unmarked are recognized like base games.
        let unmarked = update_of(0x0004_000E_0005_5D00, false);
        assert!(open_game(Cursor::new(base.as_slice()), Some(Cursor::new(unmarked.as_slice()))).is_ok());
        let plain = open_game(Cursor::new(base.as_slice()), None).expect("opens");
        assert!(plain.update().is_none());
    }

    #[test]
    fn refuses_updates_of_other_games() {
        let base = image(&[("DllBattle.cro", b"code")], true);
        // Y's update, and X itself rather than its update.
        for other in [
            update_of(0x0004_000E_0005_5E00, true),
            update_of(0x0004_0000_0005_5D00, true),
        ] {
            let err =
                open_game(Cursor::new(base.as_slice()), Some(Cursor::new(other.as_slice()))).expect_err("refused");
            assert!(
                matches!(&err, RomError::Unsupported(m) if m.contains("0004000E00055D00")),
                "{err}"
            );
        }
        let other_base = image_of(0x0004_0000_0005_5E00, &[("DllBattle.cro", b"code")], true);
        assert!(open_game(Cursor::new(other_base.as_slice()), None).is_ok());
    }

    #[test]
    fn opens_decrypted_images() {
        let img = image(&[("a/0/0/0", b"data")], true);
        let mut ncch = open(Cursor::new(&img)).expect("opens");
        assert_eq!(ncch.read_file("a/0/0/0").expect("exists"), b"data");
    }

    #[test]
    fn accepts_unmarked_images_with_readable_files() {
        let img = image(&[("DllBattle.cro", b"code")], false);
        assert!(open(Cursor::new(&img)).is_ok());
    }

    #[test]
    fn refuses_encrypted_images() {
        let unmarked = image(&[("a/0/0/0", b"data")], false);
        let err = open(Cursor::new(&unmarked)).expect_err("no proof of decryption");
        assert!(
            matches!(&err, RomError::Unsupported(m) if m.contains("decrypt")),
            "{err}"
        );
        let mut scrambled = image(&[("DllBattle.cro", b"code")], false);
        for (i, b) in scrambled.iter_mut().enumerate().skip(0x4000 + 0xC00) {
            *b ^= (i % 251).to_le_bytes()[0] | 0x80;
        }
        assert!(matches!(open(Cursor::new(&scrambled)), Err(RomError::Unsupported(_))));
    }
}
