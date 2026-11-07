//! BLZ: decompressing any bytes returns or fails, never panics; compressing round-trips.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ppr_formats::blz;

fuzz_target!(|data: &[u8]| {
    let _ = blz::decompress(data);
    let packed = blz::compress(data, 0, blz::Mode::Normal);
    let unpacked = blz::decompress(&packed).expect("own output decompresses");
    // Data stored uncompressed comes back with its footer.
    assert_eq!(&unpacked[..data.len()], data);
});
