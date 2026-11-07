use super::*;

/// Reference strings of random settings that only use options this app models.
const REFERENCE_STRINGS: &str = include_str!("reference-strings.txt");

#[test]
fn reference_strings_survive_a_round_trip_exactly() {
    for (n, line) in REFERENCE_STRINGS.lines().enumerate() {
        let (settings, extras) = from_settings_string(line).unwrap_or_else(|e| panic!("string {n}: {e}"));
        assert_eq!(to_settings_string(&settings, &extras), line, "string {n}");
    }
}

#[test]
fn reference_strings_carry_the_rom_name() {
    let names: Vec<String> = REFERENCE_STRINGS
        .lines()
        .take(3)
        .map(|l| from_settings_string(l).expect("reads").1.rom_name)
        .collect();
    assert_eq!(
        names,
        ["Pokemon Emerald (U)", "Pokemon FireRed (U) 1.0", "Pokemon Ruby (U) 1.1"]
    );
}

#[test]
fn default_settings_survive_a_round_trip() {
    let extras = SettingsExtras {
        rom_name: "Pokemon Emerald (U)".into(),
        race_mode: true,
    };
    let text = to_settings_string(&Settings::default(), &extras);
    assert!(text.starts_with("428"));
    assert_eq!(from_settings_string(&text), Ok((Settings::default(), extras)));
}

#[test]
fn time_of_day_is_the_first_bit_of_byte_18() {
    let extras = SettingsExtras {
        rom_name: "Pokemon Ultra Sun".into(),
        race_mode: false,
    };
    let mut settings = Settings::default();
    let without = base64::decode(&to_settings_string(&settings, &extras)[3..]).expect("Base64");
    settings.wild.time_of_day = true;
    let text = to_settings_string(&settings, &extras);
    let with = base64::decode(&text[3..]).expect("Base64");
    assert_eq!((without[18] & 1, with[18] & 1), (0, 1));
    assert_eq!(with[..18], without[..18]);
    assert_eq!(from_settings_string(&text), Ok((settings, extras)));
}

#[test]
fn settings_files_hold_the_version_length_and_string() {
    let line = REFERENCE_STRINGS.lines().next().expect("a string");
    let (settings, extras) = from_settings_string(line).expect("reads");
    let file = to_settings_file(&settings, &extras);
    assert_eq!(file[..4], SETTINGS_VERSION.to_be_bytes());
    assert_eq!(
        usize::try_from(u32::from_be_bytes([file[4], file[5], file[6], file[7]])),
        Ok(line.len() - 3)
    );
    assert_eq!(&file[8..], &line.as_bytes()[3..]);
    assert_eq!(from_settings_file(&file), Ok((settings, extras)));
}

#[test]
fn damaged_and_foreign_strings_are_refused() {
    let line = REFERENCE_STRINGS.lines().next().expect("a string");
    assert_eq!(from_settings_string(""), Err(Malformed));
    assert_eq!(from_settings_string("hello"), Err(Malformed));
    assert_eq!(from_settings_string("428!!!"), Err(Malformed));
    assert_eq!(from_settings_string(&line[..60]), Err(Malformed));
    // A changed byte breaks the checksum.
    let mut bytes = base64::decode(&line[3..]).expect("Base64");
    bytes[0] ^= 1;
    assert_eq!(
        from_settings_string(&format!("428{}", base64::encode(&bytes))),
        Err(Malformed)
    );
    assert_eq!(
        from_settings_string(&format!("427{}", &line[3..])),
        Err(SettingsStringError::OldVersion(427))
    );
    assert_eq!(
        from_settings_string(&format!("429{}", &line[3..])),
        Err(SettingsStringError::NewerVersion(429))
    );

    let mut file = to_settings_file(&Settings::default(), &SettingsExtras::default());
    assert_eq!(from_settings_file(&file[..6]), Err(Malformed));
    file[0] = 100;
    assert!(matches!(
        from_settings_file(&file),
        Err(SettingsStringError::OldVersion(_))
    ));
}

#[test]
fn random_shiny_trainer_pokemon_is_bit_6_of_byte_29() {
    let mut settings = Settings::default();
    settings.trainers.random_shiny = true;
    let extras = SettingsExtras::default();
    let text = to_settings_string(&settings, &extras);
    let bytes = base64::decode(&text[3..]).expect("Base64");
    let plain = base64::decode(&to_settings_string(&Settings::default(), &extras)[3..]).expect("Base64");
    assert_eq!(bytes[29], plain[29] | 1 << 6);
    assert_eq!(from_settings_string(&text), Ok((settings, extras)));
}

#[test]
fn following_mega_evolutions_is_bit_6_of_bytes_1_and_2_and_bit_7_of_byte_3() {
    type Case = (fn(&mut Settings), usize, u8);
    let extras = SettingsExtras::default();
    let plain = base64::decode(&to_settings_string(&Settings::default(), &extras)[3..]).expect("Base64");
    let cases: [Case; 3] = [
        (|s| s.base_stats.follow_megas = true, 1, 6),
        (|s| s.types.follow_megas = true, 2, 6),
        (|s| s.abilities.follow_megas = true, 3, 7),
    ];
    for (set, byte, bit) in cases {
        let mut settings = Settings::default();
        set(&mut settings);
        let text = to_settings_string(&settings, &extras);
        let bytes = base64::decode(&text[3..]).expect("Base64");
        assert_eq!(bytes[byte], plain[byte] | 1 << bit, "byte {byte}");
        assert_eq!(from_settings_string(&text), Ok((settings, extras.clone())));
    }
}
