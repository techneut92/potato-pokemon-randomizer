//! Gen 3 tables: a synthetic Emerald with fuzzed bytes written over it loads, writes back and saves, or is refused
//! with an error. The first four input bytes pick where the rest goes.
#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;

fn emerald() -> &'static [u8] {
    static ROM: OnceLock<Vec<u8>> = OnceLock::new();
    ROM.get_or_init(ppr_romio::testing::emerald)
}

fuzz_target!(|data: &[u8]| {
    let Some((at, patch)) = data.split_first_chunk::<4>() else {
        return;
    };
    let mut rom = emerald().to_vec();
    // Past the header, so the game is still recognised.
    let start = 0xC0 + usize::try_from(u32::from_le_bytes(*at)).unwrap_or(0) % (rom.len() - 0xC0);
    let end = (start + patch.len()).min(rom.len());
    rom[start..end].copy_from_slice(&patch[..end - start]);
    let Ok(mut handler) = ppr_romio::open_bytes(rom) else {
        return;
    };
    let data = handler.game_data().clone();
    if handler.apply(&data).is_ok() {
        let _ = handler.save(&mut std::io::sink());
    }
});
