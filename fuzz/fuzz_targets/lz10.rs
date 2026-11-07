//! LZ10: decompressing any bytes returns or fails, never panics; compressing round-trips (except empty data, see
//! `lz10::compress`).
#![no_main]

use libfuzzer_sys::fuzz_target;
use ppr_formats::lz10;

fuzz_target!(|data: &[u8]| {
    let _ = lz10::decompress(data, 0);
    if data.is_empty() {
        return;
    }
    let packed = lz10::compress(data);
    assert_eq!(lz10::decompress(&packed, 0).expect("own output decompresses"), data);
});
