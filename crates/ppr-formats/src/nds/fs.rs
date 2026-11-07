//! The `NitroFS` file name table (FNT): which file ID has which path.

use std::collections::BTreeMap;

use crate::FormatError;
use crate::io::{u16_at, u32_at};
use crate::narc::ascii;

/// Directory IDs start here; the root is `0xF000`.
const FIRST_DIR: usize = 0xF000;
/// The most directories read.
const MAX_DIRS: usize = 0x1000;

/// Reads the name table at `fnt` of `rom`: the full path of each named file, by file ID. Paths use `/` and have no
/// leading slash, e.g. `poketool/personal/pl_personal.narc`.
pub(super) fn read_names(rom: &[u8], fnt: usize) -> Result<BTreeMap<usize, String>, FormatError> {
    let dir_count = usize::from(u16_at(rom, fnt + 6)?).min(MAX_DIRS);
    let mut subtables = Vec::with_capacity(dir_count);
    for i in 0..dir_count {
        let at = fnt + i * 8;
        subtables.push((
            fnt + u32_at(rom, at)? as usize,
            usize::from(u16_at(rom, at + 4)?),
            usize::from(u16_at(rom, at + 6)?),
        ));
    }

    let mut dir_names: Vec<Option<String>> = vec![None; dir_count];
    let mut names = BTreeMap::new();
    let mut file_dirs = BTreeMap::new();
    for (dir, &(mut at, mut file_id, _)) in subtables.iter().enumerate() {
        loop {
            let control = *rom.get(at).ok_or(FormatError::Truncated { offset: at as u64 })?;
            at += 1;
            if control == 0 {
                break;
            }
            let len = usize::from(control & 0x7F);
            let name = ascii(crate::io::slice_at(rom, at, len)?);
            at += len;
            if control & 0x80 != 0 {
                let sub = usize::from(u16_at(rom, at)?);
                at += 2;
                let slot =
                    sub.checked_sub(FIRST_DIR)
                        .and_then(|i| dir_names.get_mut(i))
                        .ok_or(FormatError::WrongFormat {
                            format: "NDS",
                            reason: "a directory ID out of range",
                        })?;
                *slot = Some(name);
            } else {
                names.insert(file_id, name);
                file_dirs.insert(file_id, dir);
                file_id += 1;
            }
        }
    }

    let mut dir_paths = vec![String::new(); dir_count];
    for i in 1..dir_count {
        let mut path = String::new();
        let mut current = i;
        let mut name = dir_names[i].as_deref();
        // A malformed table can make the parents loop; no real path is deeper than the number of directories.
        let mut steps = 0;
        while let Some(n) = name.filter(|n| !n.is_empty()) {
            if !path.is_empty() {
                path.insert(0, '/');
            }
            path.insert_str(0, n);
            let parent = subtables[current].2;
            steps += 1;
            if (FIRST_DIR + 1..=0xFFFF).contains(&parent) && steps <= dir_count {
                current = parent - FIRST_DIR;
                name = dir_names.get(current).and_then(Option::as_deref);
            } else {
                break;
            }
        }
        dir_paths[i] = path;
    }

    Ok(names
        .into_iter()
        .map(|(id, name)| {
            let dir = &dir_paths[file_dirs[&id]];
            let path = if dir.is_empty() { name } else { format!("{dir}/{name}") };
            (id, path)
        })
        .collect())
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A directory for [`build_fnt`]: its parent's index and its entries (a name, and for a subdirectory its index).
    pub(crate) type Dir<'a> = (usize, Vec<(&'a str, Option<usize>)>);

    /// A name table: `dirs` lists each directory's parent index and entries, the root first. An entry is a file
    /// name or a subdirectory (name and index). File IDs start at `first_file` and run through the directories in
    /// order.
    pub(crate) fn build_fnt(dirs: &[Dir<'_>], first_file: u16) -> Vec<u8> {
        let mut table = Vec::new();
        let mut subtables = Vec::new();
        let mut next_file = first_file;
        let mut firsts = Vec::new();
        for (_, entries) in dirs {
            firsts.push(next_file);
            let mut sub = Vec::new();
            for (name, dir) in entries {
                let len = u8::try_from(name.len()).expect("short name");
                if let Some(d) = dir {
                    sub.push(len | 0x80);
                    sub.extend(name.bytes());
                    sub.extend((0xF000 + u16::try_from(*d).expect("small")).to_le_bytes());
                } else {
                    sub.push(len);
                    sub.extend(name.bytes());
                    next_file += 1;
                }
            }
            sub.push(0);
            subtables.push(sub);
        }
        let mut offset = dirs.len() * 8;
        for (i, (parent, _)) in dirs.iter().enumerate() {
            table.extend(u32::try_from(offset).expect("small").to_le_bytes());
            table.extend(firsts[i].to_le_bytes());
            let parent = if i == 0 {
                u16::try_from(dirs.len()).expect("small")
            } else {
                0xF000 + u16::try_from(*parent).expect("small")
            };
            table.extend(parent.to_le_bytes());
            offset += subtables[i].len();
        }
        for sub in subtables {
            table.extend(sub);
        }
        table
    }

    #[test]
    fn builds_full_paths() {
        let fnt = build_fnt(
            &[
                (0, vec![("a", Some(1)), ("top.bin", None)]),
                (0, vec![("0", Some(2)), ("x.narc", None)]),
                (1, vec![("2", None), ("3", None)]),
            ],
            5,
        );
        let names = read_names(&fnt, 0).expect("valid");
        let paths: Vec<(usize, &str)> = names.iter().map(|(&id, p)| (id, p.as_str())).collect();
        assert_eq!(paths, [(5, "top.bin"), (6, "a/x.narc"), (7, "a/0/2"), (8, "a/0/3")]);
    }

    #[test]
    fn rejects_broken_tables() {
        let mut fnt = build_fnt(&[(0, vec![("a", Some(7))])], 0);
        assert!(read_names(&fnt, 0).is_err());
        fnt.truncate(9);
        assert!(read_names(&fnt, 0).is_err());
    }
}
