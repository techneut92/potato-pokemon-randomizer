//! NARC: reading any bytes returns or fails, never panics; what reads rebuilds into an archive that reads the same.
#![no_main]

use libfuzzer_sys::fuzz_target;
use ppr_formats::narc::Narc;

fuzz_target!(|data: &[u8]| {
    if let Ok(narc) = Narc::parse(data) {
        let again = Narc::parse(&narc.to_bytes()).expect("own output reads");
        assert_eq!(again.files, narc.files);
    }
});
