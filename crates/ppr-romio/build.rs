//! Embeds the custom player graphics packs (`data/player_graphics`) as a list of files.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<PathBuf> = fs::read_dir(dir)
        .expect("readable pack directory")
        .map(|e| e.expect("readable entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("data/player_graphics");
    println!("cargo::rerun-if-changed={}", root.display());
    let mut paths = Vec::new();
    files(&root, &mut paths);
    let mut code = String::from("&[\n");
    for path in &paths {
        println!("cargo::rerun-if-changed={}", path.display());
        let name = path
            .strip_prefix(&root)
            .expect("under the root")
            .to_string_lossy()
            .replace('\\', "/");
        writeln!(
            code,
            "    ({name:?}, include_bytes!({:?})),",
            path.display().to_string()
        )
        .expect("writes");
    }
    code.push(']');
    let out = Path::new(&env::var("OUT_DIR").expect("set by cargo")).join("player_graphics.rs");
    fs::write(out, code).expect("writable OUT_DIR");
}
