//! `.rndp` premade seeds: any bytes are read or refused, never a panic.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(premade) = ppr_service::read_premade_seed(data) {
        let bytes = ppr_service::premade_seed_bytes(&premade);
        ppr_service::read_premade_seed(&bytes).expect("own seed file reads back");
    }
});
