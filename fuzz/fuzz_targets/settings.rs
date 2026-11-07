//! Settings strings and `.rnqs` files: any input is read or refused, and what is read writes back readably.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok((settings, extras)) = ppr_settings::from_settings_file(data) {
        let bytes = ppr_settings::to_settings_file(&settings, &extras);
        ppr_settings::from_settings_file(&bytes).expect("own settings file reads back");
    }
    if let Ok(text) = std::str::from_utf8(data)
        && let Ok((settings, extras)) = ppr_settings::from_settings_string(text)
    {
        let text = ppr_settings::to_settings_string(&settings, &extras);
        ppr_settings::from_settings_string(&text).expect("own settings string reads back");
    }
});
