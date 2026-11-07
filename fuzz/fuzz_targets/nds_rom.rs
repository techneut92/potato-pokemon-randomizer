//! DS ROMs: opening any bytes returns or fails, never panics, and so does reading every file, overlay and the
//! ARM9 of what opens, and saving it with a replaced file.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ppr_formats::nds::NdsRom;

fuzz_target!(|data: &[u8]| {
    let Ok(mut rom) = NdsRom::from_bytes(data.to_vec()) else {
        return;
    };
    let paths: Vec<String> = rom.paths().map(str::to_owned).collect();
    for path in &paths {
        let _ = rom.narc(path);
    }
    for n in 0..rom.overlays().len() {
        let _ = rom.overlay(n);
    }
    let _ = rom.arm9();
    if let Some(path) = paths.first() {
        rom.replace_file(path, vec![1, 2, 3]).expect("exists");
        let _ = rom.save();
    }
});
