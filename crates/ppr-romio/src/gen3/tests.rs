//! Gen 3 handler tests on the synthetic ROMs of [`super::fixtures`].

use std::fmt::Write as _;
use std::io::Cursor;

use ppr_model::{
    EncounterKind, Evolution, EvolutionType, ItemId, LevelUpMove, MoveCategory, StaticEncounter, StatusType,
    WildEncounter,
};

use crate::text::TextTable;

use super::fixtures::*;
use super::*;
use crate::detect::{Detection, detect};

const OBEY_AT: usize = 0x10_0000;
const EVO_LEVEL_AT: usize = 0x10_1000;
const EVO_STONE_AT: usize = 0x10_2000;
const DEOXYS_AT: usize = 0x10_3000;

impl Fixture {
    fn with_code(mut self, at: usize, code: &str) -> Self {
        let bytes = hex(code).expect("valid hex");
        self.rom[at..at + bytes.len()].copy_from_slice(&bytes);
        self
    }

    fn load(self) -> Gen3Handler {
        let identity = match detect(&mut Cursor::new(&self.rom)).expect("readable") {
            Detection::Supported(identity) => identity,
            Detection::Unsupported(why) => panic!("not detected: {why:?}"),
        };
        Gen3Handler::load(self.rom, identity).expect("loads")
    }
}

fn saved(h: &Gen3Handler) -> Vec<u8> {
    let mut out = Vec::new();
    h.save(&mut out).expect("saves");
    out
}

#[test]
fn loads_species_in_national_order() {
    let h = emerald().load();
    let d = h.game_data();
    assert_eq!(d.identity.name, "Emerald (U)");
    assert_eq!(d.species.len(), 386, "the 25 unused slots are not species");
    assert_eq!(d.species.first().map(|s| s.number), Some(1));
    assert_eq!(d.species.last().map(|s| s.number), Some(386));
    let treecko = d.species(252).expect("present");
    assert_eq!(treecko.name, "MON252");
    assert_eq!(treecko.primary_type, Type::Grass);
    assert_eq!(treecko.secondary_type, None, "same type twice means single-typed");
    assert_eq!(d.species(250).expect("present").secondary_type, Some(Type::Flying));
    assert_eq!(d.types.len(), 17);
}

#[test]
fn loads_evolutions_and_starters() {
    let d = emerald().load().game_data().clone();
    assert_eq!(d.starters, [252, 255, 258]);
    assert_eq!(d.stages_after(1), 2);
    assert_eq!(d.evolutions.len(), 2);
    assert_eq!(d.evolutions[0], Evolution::new(1, 2, EvolutionType::Level, 16));
}

#[test]
fn unchanged_data_writes_an_identical_rom() {
    let original = emerald().rom.clone();
    let mut h = emerald().load();
    let data = h.game_data().clone();
    h.apply(&data).expect("applies");
    let saved = saved(&h);
    assert_eq!(saved.len(), original.len());
    assert_eq!(
        saved.iter().zip(&original).position(|(a, b)| a != b),
        None,
        "first differing offset"
    );
}

#[test]
fn emerald_starters_are_written_and_read_back() {
    let fixture = emerald()
        .with_code(OBEY_AT, c::DEOXYS_OBEY_CODE)
        .with_code(OBEY_AT + 0x16, "9728");
    let offsets = fixture.starter_offsets();
    let mut h = fixture.load();
    let mut data = h.game_data().clone();
    data.starters = vec![1, 4, 386];
    h.apply(&data).expect("applies");
    let rom = saved(&h);
    let word = |at: usize| u16::from_le_bytes([rom[at], rom[at + 1]]);
    assert_eq!(
        offsets.map(word),
        [1, 4, 411],
        "internal indices; Deoxys is internal 411"
    );
    assert_eq!(
        rom[OBEY_AT..OBEY_AT + 4],
        [0x00, 0x21, 0x00, 0x21],
        "Deoxys obedience check removed"
    );
    assert_eq!(word(OBEY_AT + 0x16), 0x2800, "Mew obedience check removed");
    let reloaded = Gen3Handler::load(rom, h.game_data().identity.clone()).expect("reloads");
    assert_eq!(reloaded.game_data().starters, [1, 4, 386]);
}

#[test]
fn fire_red_writes_the_repeated_starter_slots_and_evolution_patches() {
    let fixture = fire_red()
        .with_code(EVO_LEVEL_AT, c::LEVEL_EVO_KANTO_DEX_CHECK_CODE)
        .with_code(EVO_STONE_AT, c::STONE_EVO_KANTO_DEX_CHECK_CODE);
    let base = fixture.int("StarterPokemon");
    let mut h = fixture.load();
    assert_eq!(h.game_data().starters, [252, 255, 258]);
    let mut data = h.game_data().clone();
    data.starters = vec![7, 152, 25];
    h.apply(&data).expect("applies");
    let rom = saved(&h);
    let word = |at: usize| u16::from_le_bytes([rom[at], rom[at + 1]]);
    let r = c::FRLG_STARTER_REPEAT;
    assert_eq!([word(base), word(base + r)], [7, 152]);
    assert_eq!(
        [word(base + c::FRLG_STARTER_2), word(base + c::FRLG_STARTER_2 + r)],
        [152, 25]
    );
    assert_eq!(
        [word(base + c::FRLG_STARTER_3), word(base + c::FRLG_STARTER_3 + r)],
        [25, 7]
    );
    assert_eq!([word(EVO_LEVEL_AT), word(EVO_LEVEL_AT + 2)], [0x46C0, 0xE014]);
    assert_eq!([word(EVO_STONE_AT), word(EVO_STONE_AT + 2)], [0x46C0, 0xE008]);
}

#[test]
fn deoxys_uses_the_hard_coded_stats() {
    let prefix = "190A1E0A230A280A";
    let mut fixture = emerald().with_code(DEOXYS_AT, prefix);
    let at = DEOXYS_AT + prefix.len() / 2;
    for (k, v) in [50u16, 150, 50, 150, 150, 50].into_iter().enumerate() {
        fixture.u16(at + k * 2, v);
    }
    let h = fixture.load();
    let deoxys = h.game_data().species(386).expect("present");
    let s = deoxys.stats;
    assert_eq!(
        [s.hp, s.attack, s.defense, s.speed, s.sp_attack, s.sp_defense],
        [50, 150, 50, 150, 150, 50]
    );
}

#[test]
fn base_stats_are_written_with_deoxys_hard_coded_stats() {
    let prefix = "190A1E0A230A280A";
    let mut fixture = emerald().with_code(DEOXYS_AT, prefix);
    let at = DEOXYS_AT + prefix.len() / 2;
    for (k, v) in [50u16, 150, 50, 150, 150, 50].into_iter().enumerate() {
        fixture.u16(at + k * 2, v);
    }
    let entry = fixture.entry.clone();
    let mut h = fixture.load();
    let unchanged = saved(&h);
    let mut d = h.game_data().clone();
    h.apply(&d.clone()).expect("applies");
    assert!(saved(&h) == unchanged, "nothing changed, nothing written");

    let new = BaseStats {
        hp: 1,
        attack: 2,
        defense: 3,
        speed: 4,
        sp_attack: 5,
        sp_defense: 6,
    };
    for n in [30, 386] {
        d.species.iter_mut().find(|s| s.number == n).expect("present").stats = new;
    }
    h.apply(&d).expect("applies");
    let rom = saved(&h);
    let slot = STATS_AT + internal(30) * c::BASE_STATS_ENTRY_SIZE;
    assert_eq!(rom[slot..slot + 6], [1, 2, 3, 4, 5, 6]);
    assert_eq!(rom[at..at + 12], [1, 0, 2, 0, 3, 0, 4, 0, 5, 0, 6, 0]);
    let r = Fixture { rom, entry }.load();
    assert_eq!(r.game_data().species(30).expect("present").stats, new);
    assert_eq!(r.game_data().species(386).expect("present").stats, new);
    assert_eq!(r.game_data().species(31), h.game_data().species(31));
}

#[test]
fn types_are_written_and_read_back() {
    let fixture = emerald();
    let entry = fixture.entry.clone();
    let mut h = fixture.load();
    let mut d = h.game_data().clone();
    let set = |d: &mut GameData, n, primary, secondary| {
        let sp = d.species.iter_mut().find(|s| s.number == n).expect("present");
        sp.primary_type = primary;
        sp.secondary_type = secondary;
    };
    set(&mut d, 30, Type::Dragon, Some(Type::Steel));
    set(&mut d, 31, Type::Ice, None);
    h.apply(&d).expect("applies");
    let rom = saved(&h);
    let at = |n| STATS_AT + internal(n) * c::BASE_STATS_ENTRY_SIZE + c::BS_PRIMARY_TYPE;
    assert_eq!(rom[at(30)..at(30) + 2], [0x10, 0x08]);
    assert_eq!(rom[at(31)..at(31) + 2], [0x0F, 0x0F], "one type is stored twice");
    let r = Fixture { rom, entry }.load();
    assert_eq!(r.game_data().species, d.species);

    set(&mut d, 32, Type::Fairy, None);
    assert!(h.apply(&d).is_err(), "Gen 3 has no Fairy type");
}

#[test]
fn growth_curves_are_written_and_read_back() {
    let fixture = emerald();
    let entry = fixture.entry.clone();
    let mut h = fixture.load();
    let mut d = h.game_data().clone();
    assert!(d.species.iter().all(|s| s.growth_curve == ExpCurve::MediumFast));
    d.species
        .iter_mut()
        .find(|s| s.number == 30)
        .expect("present")
        .growth_curve = ExpCurve::Slow;
    h.apply(&d).expect("applies");
    let rom = saved(&h);
    assert_eq!(
        rom[STATS_AT + internal(30) * c::BASE_STATS_ENTRY_SIZE + c::BS_GROWTH_CURVE],
        5
    );
    let r = Fixture { rom, entry }.load();
    assert_eq!(r.game_data().species, d.species);
}

#[test]
fn evolutions_are_written_and_read_back() {
    let fixture = emerald();
    let entry = fixture.entry.clone();
    let mut h = fixture.load();
    let mut d = h.game_data().clone();
    // 1 into 2 by Water Stone, a second evolution of 30 by trading with a Deep Sea Tooth; 2 into 3 no more.
    d.evolutions = vec![
        Evolution::new(1, 2, EvolutionType::Stone, 84),
        Evolution::new(30, 31, EvolutionType::Level, 20),
        Evolution::new(30, 40, EvolutionType::TradeItem, 226),
    ];
    h.apply(&d).expect("applies");
    let rom = saved(&h);
    let base = usize::try_from(entry.int("PokemonEvolutions").expect("set")).expect("fits")
        + internal(1) * c::EVOLUTION_ENTRY_SIZE;
    let word = |at: usize| u16::from_le_bytes([rom[at], rom[at + 1]]);
    assert_eq!(
        (word(base), word(base + 2)),
        (7, 97),
        "Stone, the Water Stone's Gen 3 number"
    );
    assert_eq!(word(base + 8), 0, "the other slots are cleared");
    let r = Fixture { rom, entry }.load();
    let read: Vec<_> = r
        .game_data()
        .evolutions
        .iter()
        .map(|e| (e.from, e.to, e.kind, e.parameter))
        .collect();
    let written: Vec<_> = d
        .evolutions
        .iter()
        .map(|e| (e.from, e.to, e.kind, e.parameter))
        .collect();
    assert_eq!(read, written);
}

#[test]
fn easier_happiness_evolutions_patch_the_needed_happiness() {
    const AT: usize = 0x5F_0000;
    for (mut fixture, clock) in [(emerald(), true), (fire_red(), false)] {
        fixture.rom[AT..AT + 4].copy_from_slice(&[0xDB, 0x29, 0x00, 0xD8]);
        fixture.rom[AT + 38] = 0xDB;
        fixture.rom[AT + 66] = 0xDB;
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        d.easier_happiness_evolutions = true;
        h.apply(&d).expect("applies");
        let rom = saved(&h);
        let night = if clock { 159 } else { 0xDB };
        assert_eq!([rom[AT], rom[AT + 38], rom[AT + 66]], [159, night, night]);
    }
}

#[test]
fn abilities_are_read_and_written() {
    for fixture in [emerald(), fire_red()] {
        let entry = fixture.entry.clone();
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        assert_eq!(d.ability_names.len(), 78);
        assert_eq!(d.ability_names[25], "AB25");
        assert_eq!(d.ability_variations[&13], [13, 77]);
        let sp = d.species.iter_mut().find(|s| s.number == 30).expect("present");
        assert_eq!(sp.abilities, [0, 0], "the fixture sets none");
        sp.abilities = vec![25, 0];
        d.species
            .iter_mut()
            .find(|s| s.number == 31)
            .expect("present")
            .abilities = vec![3, 4];
        h.apply(&d).expect("applies");
        let rom = saved(&h);
        let at = |n| STATS_AT + internal(n) * c::BASE_STATS_ENTRY_SIZE + c::BS_ABILITY_1;
        assert_eq!(rom[at(30)..at(30) + 2], [25, 25], "an empty second slot gets the first");
        assert_eq!(rom[at(31)..at(31) + 2], [3, 4]);
        let r = Fixture { rom, entry }.load();
        assert_eq!(r.game_data().species(30).expect("present").abilities, [25, 25]);
        assert_eq!(r.game_data().species(31).expect("present").abilities, [3, 4]);
        d.species
            .iter_mut()
            .find(|s| s.number == 32)
            .expect("present")
            .abilities = vec![78, 0];
        assert!(h.apply(&d).is_err(), "Gen 3 has no ability 78");
    }
}

#[test]
fn a_used_spare_slot_marks_a_rom_hack() {
    let mut fixture = emerald();
    let enc = encoder();
    let name_at = NAMES_AT + 252 * fixture.int("PokemonNameLength");
    for (k, ch) in "FAKEMON".chars().enumerate() {
        fixture.rom[name_at + k] = enc[&ch];
    }
    fixture.rom[name_at + 7] = 0xFF;
    fixture.rom[STATS_AT + 252 * c::BASE_STATS_ENTRY_SIZE + c::BS_HP] = 99;
    let h = fixture.load();
    let d = h.game_data();
    assert_eq!(d.species.len(), 387);
    assert_eq!(d.species(387).map(|s| s.name.as_str()), Some("FAKEMON"));
}

#[test]
fn starters_must_be_species_of_the_game() {
    let mut h = emerald().load();
    let mut data = h.game_data().clone();
    data.starters = vec![1, 2, 999];
    assert!(h.apply(&data).is_err());
    data.starters = vec![1, 2];
    assert!(h.apply(&data).is_err());
}

#[test]
fn reads_wild_encounters_with_their_tags() {
    for (fixture, tags) in [
        (emerald(), &tags::LOCATION_TAGS_EM[..]),
        (fire_red(), &tags::LOCATION_TAGS_FRLG[..]),
    ] {
        let d = fixture.load().game_data().clone();
        assert_eq!(
            d.encounters.len(),
            tags.len(),
            "map 2's shared set and map 3's rate-0 set are skipped"
        );
        let first = &d.encounters[0];
        assert_eq!(first.kind, EncounterKind::Walking);
        assert_eq!(first.name, format!("{LOCATION} Grass/Cave"));
        assert_eq!(first.location.as_deref(), Some(tags[0]));
        assert_eq!((first.map, first.rate, first.encounters.len()), (Some(1), 20, 12));
        assert_eq!(
            first.encounters[0],
            WildEncounter {
                forme: 0,
                species: 10,
                min_level: 2,
                max_level: 4
            }
        );
        let fishing = &d.encounters[1];
        assert_eq!(
            (fishing.kind, fishing.encounters.len()),
            (EncounterKind::Fishing, 10),
            "all three rods"
        );
        assert_eq!(fishing.name, format!("{LOCATION} Fishing"));
        assert_eq!(d.encounters[2].map, Some(4));
        assert!(
            d.encounters
                .iter()
                .zip(tags)
                .all(|(s, t)| s.location.as_deref() == Some(*t))
        );
        assert_eq!(d.species(10).expect("present").catch_rate, CATCH_RATE);
    }
    let frlg = fire_red().load();
    let post_game = frlg.game_data().encounters.iter().filter(|s| s.post_game).count();
    assert_eq!(post_game, tags::POST_GAME_FRLG.len());
}

#[test]
fn battle_trappers_are_banned_where_the_entry_says() {
    let mut f = fire_red();
    // MON30 gets Arena Trap.
    f.rom[STATS_AT + internal(30) * c::BASE_STATS_ENTRY_SIZE + c::BS_ABILITY_2] = 71;
    let d = f.load().game_data().clone();
    let banned: Vec<_> = d
        .encounters
        .iter()
        .enumerate()
        .filter(|(_, s)| !s.banned.is_empty())
        .map(|(i, s)| (i, s.banned.clone()))
        .collect();
    assert_eq!(banned, (55..=59).map(|i| (i, vec![30])).collect::<Vec<_>>());
    assert!(
        emerald()
            .load()
            .game_data()
            .encounters
            .iter()
            .all(|s| s.banned.is_empty())
    );
}

#[test]
fn guaranteed_catching_patches_the_ball_throw() {
    let at = 0x10_4000;
    let mut h = emerald().with_code(at, c::PERFECT_ODDS_BRANCH_LOCATOR).load();
    let mut d = h.game_data().clone();
    d.guaranteed_catching = true;
    h.apply(&d).expect("applies");
    assert_eq!(
        saved(&h)[at..at + 6],
        [0, 0, 0, 0, 0x00, 0x20],
        "cmp and bls become nops"
    );
}

#[test]
fn wild_encounters_and_catch_rates_are_written_and_read_back() {
    let mut h = fire_red().load();
    let mut d = h.game_data().clone();
    d.encounters[0].encounters[3] = WildEncounter {
        forme: 0,
        species: 300,
        min_level: 7,
        max_level: 9,
    };
    d.species
        .iter_mut()
        .find(|s| s.number == 300)
        .expect("present")
        .catch_rate = 255;
    h.apply(&d).expect("applies");

    let rom = saved(&h);
    let slot = LAND_SLOTS_AT + 3 * 4;
    assert_eq!(rom[slot..slot + 2], [7, 9]);
    assert_eq!(
        usize::from(u16::from_le_bytes([rom[slot + 2], rom[slot + 3]])),
        internal(300)
    );

    let reloaded = Fixture {
        rom,
        entry: fire_red().entry,
    }
    .load();
    let r = reloaded.game_data();
    assert_eq!(r.encounters, d.encounters);
    assert_eq!(r.species(300).expect("present").catch_rate, 255);
    assert_eq!(r.species(299).expect("present").catch_rate, CATCH_RATE);
}

#[test]
fn encounter_sets_must_match_the_loaded_ones() {
    let mut h = emerald().load();
    let mut d = h.game_data().clone();
    d.encounters[0].encounters.pop();
    assert!(h.apply(&d).is_err(), "a set can't lose slots");
    let mut d = h.game_data().clone();
    d.encounters.pop();
    assert!(h.apply(&d).is_err(), "sets can't be dropped");
}

#[test]
fn a_missing_end_of_the_wild_table_is_an_error() {
    let mut f = emerald();
    let end = f.int("WildPokemon") + 20 * (tags::LOCATION_TAGS_EM.len() + 1);
    f.rom[end..end + 2].copy_from_slice(&[0, 0]);
    let identity = match detect(&mut Cursor::new(&f.rom)).expect("readable") {
        Detection::Supported(identity) => identity,
        Detection::Unsupported(why) => panic!("not detected: {why:?}"),
    };
    // Headers with null pointers follow, up to the guard.
    assert!(Gen3Handler::load(f.rom, identity).is_err());
}

fn named(data: &mut GameData, n: DexNumber) -> &mut ppr_model::Species {
    data.species.iter_mut().find(|s| s.number == n).expect("species")
}

#[test]
fn fire_red_rewrites_the_starter_text() {
    let fixture = fire_red();
    let pointers: Vec<usize> = (0..3).map(|id| fixture.starter_text_pointer(id)).collect();
    let free = fixture.int("FreeSpace");
    let mut h = fixture.load();
    let mut data = h.game_data().clone();
    data.starters = vec![7, 152, 25];
    named(&mut data, 152).name = "MR. MIME".into();
    // A Normal type with a second type is named by the second.
    let pikachu = named(&mut data, 25);
    pikachu.primary_type = Type::Normal;
    pikachu.secondary_type = Some(Type::Electric);
    h.apply(&data).expect("applies");
    let rom = GbaRom::new(saved(&h));
    let table = TextTable::named("gba_english").expect("table");
    let text = |pointer: usize| {
        let at = rom.pointer(pointer).expect("valid");
        assert!((free..free + FREE_BYTES).contains(&at), "{at:#x} is in free space");
        assert_eq!(at % 4, 0, "long-aligned");
        table.decode(&rom.bytes()[at..], 200)
    };
    // Reference output of the starter text replacement, with two known bugs of it fixed (see `events` and
    // `TextTable::encode`).
    assert_eq!(
        text(pointers[0]),
        "I see! MON7 is your choice.\\nIt’s very easy to raise.\\pSo, \\v01, you want to go with the \
         FIRE\\nPOKéMON MON7?"
    );
    assert_eq!(
        text(pointers[1]),
        "Ah! MR. MIME is your choice.\\nYou should raise it patiently.\\pSo, \\v01, you’re claiming the \
         WATER\\nPOKéMON MR. MIME?"
    );
    assert_eq!(
        text(pointers[2]),
        "Hm! MON25 is your choice.\\nIt’s one worth raising.\\pSo, \\v01, you’ve decided on \
         the\\nELECTRIC POKéMON MON25?"
    );
    // The copy of the first pointer follows; the old texts are blanked.
    assert_eq!(
        rom.pointer(pointers[0] + 0x40).expect("valid"),
        rom.pointer(pointers[0]).expect("valid")
    );
    assert!(
        rom.bytes()[SCRIPTS_AT + 0x500..SCRIPTS_AT + 0x504]
            .iter()
            .all(|&b| b == 0xFF)
    );
}

#[test]
fn emerald_has_no_starter_text_and_changes_nothing_else() {
    let fixture = emerald();
    let free = fixture.int("FreeSpace");
    let mut h = fixture.load();
    let before = saved(&h);
    let mut data = h.game_data().clone();
    data.starters = vec![7, 152, 25];
    h.apply(&data).expect("applies");
    let after = saved(&h);
    assert_eq!(before[free..free + FREE_BYTES], after[free..free + FREE_BYTES]);
}

#[test]
fn reads_the_type_chart() {
    let h = fire_red().load();
    let chart = &h.game_data().type_chart;
    assert_eq!(chart.types().len(), 17, "no Fairy in Gen 3");
    assert_eq!(chart.get(Type::Water, Type::Fire), Some(Effectiveness::Double));
    assert_eq!(chart.get(Type::Fire, Type::Water), Some(Effectiveness::Half));
    assert_eq!(
        chart.get(Type::Normal, Type::Ghost),
        Some(Effectiveness::Zero),
        "after the Foresight marker"
    );
    assert_eq!(chart.get(Type::Fire, Type::Fire), Some(Effectiveness::Neutral));
    assert_eq!(chart.non_neutral_count(), 5);
}

/// With `PPR_TEST_ROMS` set to a folder with `FireRed` (U) 1.0: the entries' script paths lead to the game's own
/// starter texts, and rewritten texts read back.
#[test]
#[ignore = "needs a real ROM"]
fn real_fire_red_starter_text() {
    let Some(dir) = std::env::var_os("PPR_TEST_ROMS") else {
        return;
    };
    let path = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains("Fire Red") && !p.to_string_lossy().contains("randomized"))
        .expect("a FireRed ROM");
    let bytes = std::fs::read(path).expect("readable");
    let identity = match detect(&mut Cursor::new(&bytes)).expect("readable") {
        Detection::Supported(identity) => identity,
        Detection::Unsupported(why) => panic!("not detected: {why:?}"),
    };
    let mut h = Gen3Handler::load(bytes, identity).expect("loads");
    // The vanilla chart fills exactly the room the game's table has (`c::TYPE_CHART_ROOM`).
    let chart = &h.game_data().type_chart;
    assert_eq!(chart.non_neutral_count(), 110);
    assert_eq!(chart.get(Type::Ghost, Type::Psychic), Some(Effectiveness::Double));
    assert_eq!(chart.get(Type::Normal, Type::Ghost), Some(Effectiveness::Zero));
    let items = &h.game_data().items;
    let name = |id: ItemId| items.iter().find(|i| i.id == id).map(|i| i.name.as_str());
    assert_eq!(items.len(), 374);
    assert_eq!(
        (name(1), name(17), name(328)),
        (Some("MASTER BALL"), Some("POTION"), Some("TM01"))
    );
    assert_eq!(h.game_data().starter_held_items, [None]);
    // Chansey may hold a Lucky Egg, rarely.
    let chansey = h.game_data().wild_held_items[&113];
    assert_eq!((chansey.common, chansey.rare), (None, Some(ppr_model::LUCKY_EGG)));
    let table = TextTable::named("gba_english").expect("table");
    let read = |h: &Gen3Handler, t: &events::EventText| {
        let at = h.rom.pointer(t.pointer_offset().expect("resolved")).expect("valid");
        table.decode(&h.rom.bytes()[at..], 300)
    };
    let before: Vec<String> = h.texts.starter.iter().map(|t| read(&h, t)).collect();
    assert!(
        before[0].contains("BULBASAUR") && before[0].contains("GRASS"),
        "{}",
        before[0]
    );
    assert!(
        before[1].contains("CHARMANDER") && before[1].contains("FIRE"),
        "{}",
        before[1]
    );
    assert!(
        before[2].contains("SQUIRTLE") && before[2].contains("WATER"),
        "{}",
        before[2]
    );
    let mut data = h.game_data().clone();
    data.starters = vec![122, 25, 133];
    data.starter_held_items = vec![Some(17)];
    let guaranteed = ppr_model::WildHeldItems {
        guaranteed: Some(17),
        ..ppr_model::WildHeldItems::default()
    };
    data.wild_held_items.insert(113, guaranteed);
    data.wild_held_items.insert(25, ppr_model::WildHeldItems::default());
    h.apply(&data).expect("applies");
    let reloaded = Gen3Handler::load(saved(&h), h.game_data().identity.clone()).expect("reloads");
    assert_eq!(reloaded.game_data().starter_held_items, [Some(17)]);
    assert_eq!(reloaded.game_data().wild_held_items, data.wild_held_items);
    let after: Vec<String> = h.texts.starter.iter().map(|t| read(&h, t)).collect();
    assert!(
        after[0].contains("MR. MIME") && after[0].contains("PSYCHIC"),
        "{}",
        after[0]
    );
    assert!(
        after[1].contains("PIKACHU") && after[1].contains("ELECTRIC"),
        "{}",
        after[1]
    );
    assert!(
        after[2].contains("EEVEE") && after[2].contains("NORMAL"),
        "{}",
        after[2]
    );
}

#[test]
fn reads_items() {
    let h = emerald().load();
    let items = &h.game_data().items;
    assert_eq!(items.len(), 376, "ItemCount");
    assert!(items.windows(2).all(|w| w[0].id < w[1].id), "sorted by number");
    let item = |id: ItemId| items.iter().find(|i| i.id == id).expect("present");
    // Internal 13 is the Potion, standard 17.
    assert_eq!(item(17).name, "ITEM13");
    assert!(item(17).allowed && !item(17).bad && !item(17).tm);
    assert!(item(328).tm && item(377).tm, "TM01 to TM50");
    assert_eq!(items.iter().filter(|i| i.tm).count(), 50);
    assert!(!item(420).allowed, "HM01");
    assert!(!item(1052).allowed, "Gen 3 unique items are key or unused items");
    assert!(item(137).bad, "mail");
    assert!(!item(70).bad, "Shoal Salt is only bad in FireRed and LeafGreen");
    assert!(fire_red().load().game_data().items.iter().any(|i| i.id == 70 && i.bad));
}

#[test]
fn starter_held_items_are_written_and_read_back() {
    let past_ff = items::standard(300);
    for (fixture, id) in [
        (emerald(), 17),
        (emerald(), past_ff),
        (fire_red(), 17),
        (fire_red(), past_ff),
    ] {
        let family = fixture.entry.family.clone();
        let mut h = fixture.load();
        assert_eq!(h.game_data().starter_held_items, [None]);
        let mut data = h.game_data().clone();
        data.starter_held_items = vec![Some(id)];
        h.apply(&data).expect("applies");
        let rom = saved(&h);
        let reloaded = Gen3Handler::load(rom, h.game_data().identity.clone()).expect("reloads");
        assert_eq!(reloaded.game_data().starter_held_items, [Some(id)], "{family} {id}");
        data.starter_held_items = vec![Some(9999)];
        assert!(h.apply(&data).is_err(), "no such item");
    }
}

#[test]
fn rse_starter_items_use_a_movs_and_an_add_past_0xff() {
    let fixture = emerald();
    let at = fixture.int("StarterItems");
    let mut h = fixture.load();
    let mut data = h.game_data().clone();
    // Internal 300: MOVS 0xFF, then ADD 300 - 0xFF.
    data.starter_held_items = vec![Some(items::standard(300))];
    h.apply(&data).expect("applies");
    let rom = saved(&h);
    assert_eq!(rom[at], 0xFF);
    assert_eq!(rom[at + 2..at + 4], [45, 0x32]);
}

/// A real ROM from `PPR_TEST_ROMS` whose file name contains `name`, loaded.
fn real_rom(name: &str) -> Option<Gen3Handler> {
    let dir = std::env::var_os("PPR_TEST_ROMS")?;
    let path = std::fs::read_dir(dir)
        .expect("folder")
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|p| p.to_string_lossy().contains(name) && !p.to_string_lossy().contains("randomized"))?;
    let bytes = std::fs::read(path).expect("readable");
    let identity = match detect(&mut Cursor::new(&bytes)).expect("readable") {
        Detection::Supported(identity) => identity,
        Detection::Unsupported(why) => panic!("not detected: {why:?}"),
    };
    Some(Gen3Handler::load(bytes, identity).expect("loads"))
}

/// Statics as `species:level`, with `e` for eggs and `>level` per linked encounter.
fn statics_summary(statics: &[StaticEncounter]) -> String {
    statics
        .iter()
        .map(|s| {
            let mut out = format!("{}:{}", s.species, s.level);
            if s.egg {
                out.push('e');
            }
            out.extend(s.linked_levels.iter().map(|l| format!(">{l}")));
            out
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// With `PPR_TEST_ROMS` set to a folder with Ruby (U) 1.1, Emerald (U) and `FireRed` (U) 1.0: the statics are what
/// reference reader gives, and written statics, roamers included, read back.
#[test]
#[ignore = "needs real ROMs"]
fn real_statics_match_reference() {
    let expected = [
        (
            "Ruby",
            "345:20 347:20 383:45 377:40 378:40 379:40 380:50 384:70 352:30 352:30 100:25 100:25 100:25 101:30 \
             101:30 360:1e 374:5 351:25 261:2 381:40",
            vec![383],
            vec![],
        ),
        (
            "Emerald",
            "345:20 347:20 382:70 383:70 377:40 378:40 379:40 384:70 352:30 352:30 100:25 100:25 100:25 101:30 \
             101:30 185:40 381:50>40 380:50>40 386:30 151:30 250:70 249:70 360:1e 374:5 351:25 263:2",
            vec![],
            vec![151, 249, 250, 382, 383, 384, 386],
        ),
        (
            "Fire Red",
            "133:25 106:25 107:25 101:34 101:34 145:50 144:50 146:50 150:70 143:30 143:30 97:30 386:30 250:70 \
             249:70 142:5 138:5 140:5 131:25 129:5 63:9 35:8 123:25 147:18 137:26 105:30 243:50 244:50 245:50",
            vec![],
            vec![144, 145, 146, 150, 249, 250, 386],
        ),
    ];
    for (name, statics, legendaries, music) in expected {
        let Some(mut h) = real_rom(name) else { return };
        let data = h.game_data().clone();
        assert_eq!(statics_summary(&data.statics), statics, "{name}");
        assert_eq!(data.main_game_legendaries, legendaries, "{name}");
        assert_eq!(data.special_music_statics, music, "{name}");

        // Tauros (male only) everywhere, one level up; the music follows.
        let mut changed = data.clone();
        for s in &mut changed.statics {
            s.species = 128;
            // Eggs have no level to write.
            if !s.egg {
                s.level += 1;
            }
            for l in &mut s.linked_levels {
                *l += 2;
            }
        }
        changed.correct_static_music = true;
        h.apply(&changed).expect("written");
        let mut out = Vec::new();
        h.save(&mut out).expect("saved");
        let identity = data.identity.clone();
        let reloaded = Gen3Handler::load(out, identity).expect("reloads");
        assert_eq!(reloaded.game_data().statics, changed.statics, "{name}");
        {
            let probe = &reloaded.rom;
            if name == "Fire Red" {
                let gender = reloaded.entry.int("GhostMarowakGenderOffset").expect("offset");
                assert_eq!(probe.bytes()[usize::try_from(gender).expect("offset")], c::GENDER_MALE);
            }
            if !music.is_empty() {
                let pool =
                    usize::try_from(reloaded.entry.int("NewIndexToMusicPoolOffset").expect("pool")).expect("pool");
                let tauros = reloaded.dex_to_internal[128];
                for i in 0..music.len() {
                    assert_eq!(
                        probe.u16(pool + i * 4).expect("entry"),
                        tauros,
                        "{name} music entry {i}"
                    );
                }
            }
        }
    }
}

fn dex(i: usize) -> DexNumber {
    DexNumber::try_from(STATIC_SPECIES + i).expect("fits")
}

fn level(i: usize) -> u8 {
    u8::try_from(STATIC_LEVEL + i).expect("fits")
}

#[test]
fn emerald_statics_are_read() {
    let h = emerald().load();
    let statics = &h.game_data().statics;
    assert_eq!(statics.len(), 26, "25 statics and the first battle");
    assert_eq!(statics[0], StaticEncounter::new(dex(0), level(0)));
    // Wynaut's egg has no level.
    assert_eq!(
        (statics[22].species, statics[22].level, statics[22].egg),
        (dex(22), 1, true)
    );
    // The roaming Latios and Latias are the Southern Island statics.
    assert_eq!(statics[16].linked_levels, [ROAMER_LEVEL]);
    assert_eq!(statics[17].linked_levels, [ROAMER_LEVEL]);
    // The first battle's Zigzagoon, as the patch has it.
    assert_eq!(statics[25], StaticEncounter::new(263, 2));
    assert_eq!(h.game_data().special_music_statics, [151, 249, 250, 382, 383, 384, 386]);
    assert_eq!(h.game_data().main_game_legendaries, Vec::<DexNumber>::new());
}

#[test]
fn unchanged_statics_leave_the_rom_unpatched() {
    let fixture = emerald();
    let original = fixture.rom.clone();
    let mut h = fixture.load();
    let data = h.game_data().clone();
    h.apply(&data).expect("applies");
    assert!(saved(&h) == original, "nothing written");
}

#[test]
fn emerald_statics_and_roamers_are_written_and_read_back() {
    let mut h = emerald().load();
    let mut data = h.game_data().clone();
    for (i, s) in data.statics.iter_mut().enumerate() {
        s.species = DexNumber::try_from(100 + i).expect("fits");
        if !s.egg {
            s.level = 50;
        }
    }
    // Latios and Latias share their level offsets, so they share a level.
    data.statics[16].linked_levels = vec![45];
    data.statics[17].linked_levels = vec![45];
    h.apply(&data).expect("applies");
    let out = saved(&h);
    // The roamers follow Southern Island, at their own level.
    let latios = 0x16_1BB0;
    assert_eq!(u16::from_le_bytes([out[latios], out[latios + 1]]), 116);
    assert_eq!(out[0x16_1BE2], 45);
    let reloaded = Fixture {
        rom: out,
        entry: emerald().entry,
    }
    .load();
    assert_eq!(reloaded.game_data().statics, data.statics);
}

#[test]
fn fire_red_ghost_marowak_and_roamers() {
    let mut h = fire_red().load();
    let data = h.game_data().clone();
    assert_eq!(data.statics.len(), 29, "25 statics, Ghost Marowak and 3 roamers");
    assert_eq!(data.statics[25], StaticEncounter::new(105, 30));
    let roamers: Vec<DexNumber> = data.statics[26..].iter().map(|s| s.species).collect();
    assert_eq!(roamers, [243, 244, 245]);
    assert_eq!(data.statics[26].level, ROAMER_LEVEL);

    let gender = 0xA8_0004;
    let ratio_of = |n: usize| STATS_AT + internal(n) * c::BASE_STATS_ENTRY_SIZE + c::BS_GENDER_RATIO;
    for (species, ratio, expected) in [(30, 0x00, 0x00), (31, 0xFF, 0xFF), (32, 0x7F, 0xFE), (33, 0xFE, 0xFE)] {
        let mut fixture = fire_red();
        fixture.rom[ratio_of(species)] = ratio;
        let mut h = fixture.load();
        let mut changed = h.game_data().clone();
        changed.statics[25].species = DexNumber::try_from(species).expect("fits");
        h.apply(&changed).expect("applies");
        assert_eq!(saved(&h)[gender], expected, "ratio {ratio:#x}");
    }

    let mut changed = data.clone();
    changed.statics[27].species = 1;
    // The three roamers share their level offsets, so they share a level.
    for roamer in &mut changed.statics[26..] {
        roamer.level = 9;
    }
    h.apply(&changed).expect("applies");
    let reloaded = Fixture {
        rom: saved(&h),
        entry: fire_red().entry,
    }
    .load();
    assert_eq!(reloaded.game_data().statics, changed.statics);
}

#[test]
fn the_music_of_replaced_legendaries_follows_them() {
    let fixture = fire_red();
    let mewtwo = 8;
    let mut h = fixture.load();
    let mut data = h.game_data().clone();
    // Make static 8 a Mewtwo, as in the real game, then replace it.
    data.statics[mewtwo].species = 150;
    h.apply(&data).expect("applies");
    let mut h = Fixture {
        rom: saved(&h),
        entry: fire_red().entry,
    }
    .load();
    let mut data = h.game_data().clone();
    data.statics[mewtwo].species = 25;
    data.correct_static_music = true;
    h.apply(&data).expect("applies");
    let out = saved(&h);
    let pool = 0xA8_0140;
    let entries: Vec<u16> = (0..7)
        .map(|i| u16::from_le_bytes([out[pool + i * 4], out[pool + i * 4 + 1]]))
        .collect();
    assert_eq!(
        entries,
        [144, 145, 146, 25, 249, 250, 410],
        "Mewtwo's entry is Pikachu now"
    );
}

#[test]
fn patch_targets_are_never_free_space() {
    let h = fire_red().load();
    let mut free = FreedSpace::default();
    free.free(0xA7_FF00, 0x400).expect("free");
    if let Some(st) = &h.statics {
        st.reserve(&mut free);
    }
    // The roamer table (0xA80200..0xA80228), Ghost Marowak (0xA80000..0xA8002C) and the music fix
    // (0xA80100..0xA80168) are taken out.
    assert_eq!(free.find_and_unfree(0x100), Some(0xA7_FF00));
    assert_eq!(free.find_and_unfree(0xD4), Some(0xA8_002C));
    assert_eq!(free.find_and_unfree(0x98), Some(0xA8_0168));
    assert_eq!(free.find_and_unfree(0xD8), Some(0xA8_0228));
    assert_eq!(free.find_and_unfree(1), None);
}

/// With `PPR_TEST_ROMS` set: the trades are what the reference reader gives, and written trades read back.
#[test]
#[ignore = "needs real ROMs"]
fn real_trades_match_reference() {
    let expected = [
        (
            "Emerald",
            "280>273 DOTS KOBE 38726 [5, 4, 5, 4, 4, 4] Some(150)|313>311 PLUSES ROMAN 8460 [4, 4, 4, 5, 5, 4] \
             Some(141)|371>116 SEASOR SKYLAR 46285 [5, 4, 4, 4, 5, 4] Some(142)|300>52 MEOWOW ISIS 25945 \
             [4, 5, 4, 5, 4, 4] Some(148)",
        ),
        (
            "Ruby",
            "287>296 MAKIT ELYSSA 49562 [5, 5, 4, 4, 4, 4] Some(57)|25>300 SKITIT DARRELL 2259 [5, 4, 4, 5, 4, 4] \
             Some(139)|182>222 COROSO LANE 50183 [4, 4, 5, 4, 4, 5] Some(145)",
        ),
        (
            "Fire Red",
            "63>122 MIMIEN REYLEY 1985 [20, 15, 17, 24, 23, 22] None|61>124 ZYNX DONTAE 36728 [18, 17, 18, 22, 25, \
             21] Some(147)|32>29 MS. NIDO SAIGE 63184 [22, 18, 25, 19, 15, 22] Some(86)|21>83 CH’DING ELYSSA 8810 \
             [20, 25, 21, 24, 15, 20] Some(259)|33>30 NINA TURNER 13637 [22, 25, 18, 19, 22, 15] None|55>108 MARC \
             HADEN 1239 [24, 19, 21, 15, 23, 21] None|26>101 ESPHERE CLIFTON 50298 [19, 16, 18, 25, 25, 19] None|48>114 \
             TANGENY NORMA 60042 [22, 17, 25, 16, 23, 20] Some(90)|77>86 SEELOR GARETT 9853 [24, 15, 22, 16, 23, 22] \
             None",
        ),
    ];
    for (name, trades) in expected {
        let Some(mut h) = real_rom(name) else { return };
        let data = h.game_data().clone();
        let summary: Vec<String> = data
            .trades
            .iter()
            .map(|t| {
                format!(
                    "{}>{} {} {} {} {:?} {:?}",
                    t.requested.unwrap_or_default(),
                    t.given,
                    t.nickname,
                    t.ot_name,
                    t.ot_id,
                    t.ivs,
                    t.held_item
                )
            })
            .collect();
        assert_eq!(summary.join("|"), trades, "{name}");

        let mut changed = data.clone();
        for t in &mut changed.trades {
            t.given = 25;
            t.requested = Some(1);
            t.nickname = "Cuddles".into();
            t.ot_name = "Esteban".into();
            t.ot_id = 12_345;
            t.ivs = vec![31; 6];
            t.held_item = Some(1);
        }
        h.apply(&changed).expect("written");
        let mut out = Vec::new();
        h.save(&mut out).expect("saved");
        let reloaded = Gen3Handler::load(out, data.identity.clone()).expect("reloads");
        assert_eq!(reloaded.game_data().trades, changed.trades, "{name}");
    }
}

#[test]
fn trades_are_read_and_written() {
    let mut h = fire_red().load();
    let data = h.game_data().clone();
    assert_eq!(data.trades.len(), 9);
    let first = &data.trades[0];
    let given = DexNumber::try_from(TRADE_GIVEN).expect("fits");
    let requested = DexNumber::try_from(TRADE_REQUESTED).expect("fits");
    assert_eq!((first.requested, first.given), (Some(requested), given));
    assert_eq!(
        (first.nickname.as_str(), first.ot_name.as_str(), first.ot_id),
        ("NICK0", "OT0", 1000)
    );
    assert_eq!(first.ivs, [1, 2, 3, 4, 5, 6]);
    assert_eq!(first.held_item, Some(items::standard(13)));
    assert_eq!(data.trades[1].held_item, None);

    let mut changed = data.clone();
    changed.trades[1].nickname = "ABCDEFGHIJKLMNO".into();
    changed.trades[1].held_item = Some(items::standard(13));
    changed.trades[2].given = 251;
    h.apply(&changed).expect("applies");
    let out = saved(&h);
    let at = fire_red().int("TradeTableOffset") + 60;
    // Cut to the field, which then has no terminator.
    assert_eq!(&out[at + 10..at + 12], &[encoder()[&'K'], encoder()[&'L']]);
    let reloaded = Fixture {
        rom: out,
        entry: fire_red().entry,
    }
    .load();
    let mut expected = changed.trades.clone();
    expected[1].nickname = "ABCDEFGHIJKL".into();
    assert_eq!(reloaded.game_data().trades, expected);

    let mut wide = data;
    wide.trades[0].ot_id = 65_536;
    let err = h.apply(&wide).expect_err("a 32-bit trainer ID does not fit");
    assert!(err.to_string().contains("trainer ID 65536"), "{err}");
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_abilities_match_reference() {
    // A hash of "number ability1 ability2" over all species, and names, from a reference dump; the same in all three.
    const REFERENCE_HASH: u64 = 6_349_993_756_636_469_531;
    for name in ["Emerald", "Ruby", "Fire Red"] {
        let Some(h) = real_rom(name) else { return };
        let data = h.game_data();
        let hash = data.species.iter().fold(0u64, |h, sp| {
            [
                u64::from(sp.number),
                u64::from(sp.abilities[0]),
                u64::from(sp.abilities[1]),
            ]
            .into_iter()
            .fold(h, |h, v| h.wrapping_mul(31).wrapping_add(v))
        });
        assert_eq!(hash, REFERENCE_HASH, "{name}");
        assert_eq!(data.ability_names.len(), 78, "{name}");
        assert_eq!(
            (data.ability_names[25].as_str(), data.ability_names[77].as_str()),
            ("WONDER GUARD", "AIR LOCK"),
            "{name}"
        );
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_growth_curves_match_reference() {
    // A hash of number and curve byte over all species, from a reference dump; the same in all three.
    const REFERENCE_HASH: u64 = 17_558_028_907_607_063_033;
    for name in ["Emerald", "Ruby", "Fire Red"] {
        let Some(h) = real_rom(name) else { return };
        let hash = h.game_data().species.iter().fold(0u64, |h, sp| {
            h.wrapping_mul(31)
                .wrapping_add(u64::from(sp.number))
                .wrapping_mul(31)
                .wrapping_add(u64::from(sp.growth_curve.to_byte()))
        });
        assert_eq!(hash, REFERENCE_HASH, "{name}");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_evolutions_match_reference() {
    // FNV-1a of "from to type parameter estimated-level" lines, the same in all three; checked line by line
    // against a reference dump, estimated levels included.
    const REFERENCE_HASH: u64 = 15_780_667_290_645_038_090;
    for name in ["Emerald", "Ruby", "Fire Red"] {
        let Some(h) = real_rom(name) else { return };
        let mut dump = String::new();
        for e in &h.game_data().evolutions {
            writeln!(
                dump,
                "{} {} {:?} {} {}",
                e.from, e.to, e.kind, e.parameter, e.estimated_level
            )
            .expect("writes");
        }
        let hash = dump.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        assert_eq!(hash, REFERENCE_HASH, "{name}");
    }
}

/// "number|name|…" lines of the moves, as the reference harness prints them.
fn moves_dump(d: &GameData) -> String {
    let mut out = String::new();
    for m in &d.moves {
        let mut changes = String::new();
        for c in &m.stat_changes {
            write!(changes, "{:?}:{}:{},", c.stat, c.stages, c.percent_chance).expect("writes to a string");
        }
        let status = m.status.map_or_else(|| "None".to_owned(), |s| format!("{s:?}"));
        let move_type = m.move_type.map_or_else(|| "null".to_owned(), |t| format!("{t:?}"));
        writeln!(
            out,
            "{}|{}|{}|{}|{}|{}|{move_type}|{:?}|{}|{}|{}|{}|{:?}|{changes}|{:?}|{status}|{}|{:?}|{}|{}|{}|{}|{}|{}|{}|{}",
            m.number,
            m.name,
            m.effect,
            m.power,
            m.pp,
            m.accuracy,
            m.category,
            m.target,
            m.priority,
            m.hit_count_hundredths,
            m.makes_contact,
            m.stat_change_move_type,
            m.status_move_type,
            m.status_percent_chance,
            m.critical_chance,
            m.flinch_percent_chance,
            m.recoil_percent,
            m.absorb_percent,
            m.is_charge,
            m.is_recharge,
            m.is_punch,
            m.is_sound,
            m.is_trap
        )
        .expect("writes");
    }
    out
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_moves_match_reference() {
    // FNV-1a of moves_dump, checked line by line against a reference dump; FireRed's Nature Power
    // has accuracy 0 instead of 95.
    for (name, reference) in [
        ("Emerald", 13_164_873_846_529_101_842),
        ("Ruby", 13_164_873_846_529_101_842),
        ("Fire Red", 3_199_283_287_466_000_828),
    ] {
        let Some(h) = real_rom(name) else { return };
        let hash = moves_dump(h.game_data())
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        assert_eq!(hash, reference, "{name}");
    }
}

#[test]
fn moves_are_read_and_written() {
    let fixture = emerald();
    let entry = fixture.entry.clone();
    let mut h = fixture.load();
    let mut d = h.game_data().clone();
    assert_eq!(d.moves.len(), 354);
    let (one, two, three) = (&d.moves[0], &d.moves[1], &d.moves[2]);
    assert_eq!(
        (one.name.as_str(), one.power, one.pp, one.accuracy),
        ("MV1", 40, 35, 100)
    );
    assert_eq!((one.category, one.makes_contact), (MoveCategory::Physical, true));
    assert_eq!(two.category, MoveCategory::Status);
    assert_eq!(three.move_type, Some(Type::Fire));
    assert_eq!(three.category, MoveCategory::Special);
    assert_eq!(
        (three.status, three.status_percent_chance),
        (Some(StatusType::Burn), 10)
    );
    d.moves[0].power = 75;
    d.moves[0].accuracy = 90;
    d.moves[0].name = "NEWNAME".into();
    d.moves[2].move_type = Some(Type::Water);
    d.moves[2].pp = 5;
    h.apply(&d).expect("applies");
    let r = Fixture { rom: saved(&h), entry }.load();
    let back = &r.game_data().moves;
    assert_eq!(
        (back[0].power, back[0].accuracy, back[0].name.as_str()),
        (75, 90, "NEWNAME")
    );
    assert_eq!((back[2].move_type, back[2].pp), (Some(Type::Water), 5));
    assert_eq!(back[1..2], d.moves[1..2]);
}

/// "L<species>:<level>/<move>,…" and "E<species>:<move>,…" lines, as the reference harness prints them.
fn learnsets_dump(d: &GameData) -> String {
    let mut out = String::new();
    for (species, moves) in &d.learnsets {
        write!(out, "L{species}:").expect("writes to a string");
        for m in moves {
            write!(out, "{}/{},", m.level, m.move_id).expect("writes to a string");
        }
        out.push('\n');
    }
    for (species, moves) in &d.egg_moves {
        write!(out, "E{species}:").expect("writes to a string");
        for m in moves {
            write!(out, "{m},").expect("writes to a string");
        }
        out.push('\n');
    }
    out
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_learnsets_match_reference() {
    // FNV-1a of learnsets_dump, against a reference dump of level-up and egg moves.
    for (name, reference) in [
        ("Emerald", 144_799_089_184_191_888),
        ("Ruby", 9_798_246_477_592_049_132),
        ("Fire Red", 9_679_843_594_202_774_947),
    ] {
        let Some(h) = real_rom(name) else { return };
        let hash = learnsets_dump(h.game_data())
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        assert_eq!(hash, reference, "{name}");
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_learnsets_survive_growing() {
    let Some(mut h) = real_rom("Emerald") else { return };
    let mut d = h.game_data().clone();
    for moves in d.learnsets.values_mut() {
        // Four level 1 moves for everyone, as "Guaranteed Level 1 Moves" adds.
        for _ in 0..4 {
            moves.insert(0, LevelUpMove::new(1, 1));
        }
    }
    h.apply(&d).expect("applies");
    let identity = h.game_data().identity.clone();
    let back = Gen3Handler::load(saved(&h), identity).expect("loads");
    assert_eq!(back.game_data().learnsets, d.learnsets);
    assert_eq!(back.game_data().moves, d.moves);
}

#[test]
fn learnsets_and_egg_moves_are_read_and_written() {
    for fixture in [emerald(), fire_red()] {
        let entry = fixture.entry.clone();
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        assert_eq!(d.learnsets.len(), d.species.len());
        assert_eq!(
            d.learnsets[&300],
            [LevelUpMove::new(1, 33), LevelUpMove::new(10, 26)],
            "internal 325"
        );
        assert_eq!(d.egg_moves.len(), 2);
        assert_eq!(
            (d.egg_moves[&1].as_slice(), d.egg_moves[&4].as_slice()),
            (&[10, 11][..], &[12][..])
        );

        let grown: Vec<LevelUpMove> = (1..=20).map(|i| LevelUpMove::new(i, 300 + u16::from(i))).collect();
        d.learnsets.insert(1, grown.clone());
        d.learnsets.insert(2, vec![LevelUpMove::new(1, 1)]);
        d.egg_moves.insert(1, vec![20, 21]);
        d.egg_moves.insert(4, vec![22]);
        h.apply(&d).expect("applies");
        let r = Fixture {
            rom: saved(&h),
            entry: entry.clone(),
        }
        .load();
        assert_eq!(r.game_data().learnsets, d.learnsets);
        assert_eq!(r.game_data().egg_moves, d.egg_moves);

        let mut h = Fixture { rom: saved(&h), entry }.load();
        let mut more = h.game_data().clone();
        more.egg_moves.insert(4, vec![22, 23]);
        assert!(h.apply(&more).is_err(), "egg moves keep their number");
    }
}

/// The TM table, each TM item's palette pointer and its description's bytes, as hex lines.
fn tms_dump(h: &Gen3Handler) -> String {
    let rom = &h.rom;
    let entry = &h.entry;
    let images = entry.int("ItemImages").unwrap_or(0);
    let items = entry_offset(rom, entry, "ItemData").expect("items");
    let size = need(entry, "ItemEntrySize").expect("size");
    let mut out = String::new();
    for (i, m) in tms::read(rom, entry).expect("tms").iter().enumerate() {
        let item = usize::from(items::internal(328 + u16::try_from(i).expect("fits")).expect("tm"));
        let palette = if images > 0 {
            rom.pointer(usize::try_from(images).expect("offset") + item * 8 + 4)
                .expect("pointer")
        } else {
            0
        };
        let at = rom.pointer(items + item * size + 0x14).expect("description");
        let end = at + rom.bytes()[at..].iter().position(|&b| b == 0xFF).expect("end");
        write!(out, "{m}|{palette:x}|").expect("writes to a string");
        for b in &rom.bytes()[at..end] {
            write!(out, "{b:02x}").expect("writes to a string");
        }
        out.push('\n');
    }
    out
}

/// TM01 to TM50 as the reference harness sets them.
fn reference_test_tms() -> Vec<u16> {
    (0..50).map(|i| (i * 7) % 354 + 1).collect()
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_tm_writes_match_reference() {
    // FNV-1a of tms_dump after setting reference_test_tms, against reference ROMs saved with the same TMs.
    for (name, reference) in [
        ("Emerald", 15_378_305_556_512_527_599),
        ("Ruby", 6_233_369_867_740_437_335),
        ("Fire Red", 16_782_083_362_563_673_584),
    ] {
        let Some(mut h) = real_rom(name) else { return };
        let mut d = h.game_data().clone();
        d.tm_moves = reference_test_tms();
        h.apply(&d).expect("applies");
        let hash = tms_dump(&h).bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        assert_eq!(hash, reference, "{name}");
        let identity = h.game_data().identity.clone();
        let back = Gen3Handler::load(saved(&h), identity).expect("loads");
        assert_eq!(back.game_data().tm_moves, d.tm_moves, "{name}");
    }
}

#[test]
fn tms_are_read_and_written_with_their_items() {
    for fixture in [emerald(), fire_red()] {
        let entry = fixture.entry.clone();
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        assert_eq!(d.tm_moves, (1..=50).collect::<Vec<u16>>());
        d.tm_moves = (51..=100).collect();
        h.apply(&d).expect("applies");
        let r = Fixture {
            rom: saved(&h),
            entry: entry.clone(),
        }
        .load();
        assert_eq!(r.game_data().tm_moves, d.tm_moves);
        let copy = usize::try_from(entry.int("TmMovesDuplicate").expect("copy")).expect("offset");
        assert_eq!(r.rom.u16(copy).expect("reads"), 51);
        let size = need(&entry, "ItemEntrySize").expect("size");
        let item_description = r.rom.pointer(ITEMS_AT + 289 * size + 0x14).expect("pointer");
        if entry.code == "BPRE" {
            assert_eq!(
                item_description,
                MOVE_DESCRIPTIONS_AT + 51 * 16,
                "FireRed shares the move's text"
            );
        } else {
            assert_eq!(r.text.decode(&r.rom.bytes()[item_description..], 16), "DESC51");
        }
        let images = usize::try_from(entry.int("ItemImages").expect("images")).expect("offset");
        let normal = usize::try_from(entry.array("TmPals").expect("palettes")[0]).expect("offset");
        assert_eq!(r.rom.pointer(images + 289 * 8 + 4).expect("pointer"), normal);
        for t in &r.texts.tm {
            let at = r.rom.pointer(t.pointer_offset().expect("on its map")).expect("text");
            let said = r.text.decode(&r.rom.bytes()[at..], 400);
            assert!(said.contains(&format!("MV{}", 50 + t.id)), "{said}");
        }
    }
}

#[test]
fn tutors_are_read_and_written_with_their_texts() {
    for fixture in [emerald(), fire_red()] {
        let entry = fixture.entry.clone();
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        let count = need(&entry, "MoveTutorMoves").expect("count");
        assert_eq!(d.tutor_moves, (100..).take(count).collect::<Vec<u16>>());
        d.tutor_moves = (200..).take(count).collect();
        h.apply(&d).expect("applies");
        let r = Fixture {
            rom: saved(&h),
            entry: entry.clone(),
        }
        .load();
        assert_eq!(r.game_data().tutor_moves, d.tutor_moves);
        let mut named = 0;
        for t in &r.texts.tutor {
            let at = r.rom.pointer(t.pointer_offset().expect("on its map")).expect("text");
            let said = r.text.decode(&r.rom.bytes()[at..], 600);
            assert_ne!(said, OLD_TEXT, "rewritten");
            named += usize::from(said.contains(&format!("MV{}", 200 + t.id)));
        }
        assert!(named > 0, "some texts name the move");
        d.tutor_moves.pop();
        assert!(h.apply(&d).is_err(), "tutors keep their number");
    }
}

/// One line a species: its number, then a 0 or 1 a move, as compatibility flags.
fn compat_dump(compat: &std::collections::BTreeMap<DexNumber, Vec<bool>>) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    for (n, flags) in compat {
        let bits: String = flags.iter().map(|&f| if f { '1' } else { '0' }).collect();
        writeln!(out, "{n}:{bits}").expect("writes");
    }
    out
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_tm_hm_compat_matches_reference() {
    // FNV-1a of tm_hm_compat_dump, against a reference dump; all three games agree.
    for name in ["Emerald", "Ruby", "Fire Red"] {
        let Some(h) = real_rom(name) else { return };
        let hash = compat_dump(&h.game_data().tm_hm_compat)
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        assert_eq!(hash, 18_425_449_398_239_876_130, "{name}");
        assert_eq!(h.game_data().tm_hm_compat.len(), 386, "{name}");
    }
}

#[test]
fn tm_hm_compat_is_read_and_written() {
    for fixture in [emerald(), fire_red()] {
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        assert!(!d.tm_hm_compat.is_empty());
        assert!(d.tm_hm_compat.values().all(|f| f.len() == 58));
        for (i, flags) in d.tm_hm_compat.values_mut().enumerate() {
            flags[i % 58] = true;
            flags[57] = true;
        }
        h.apply(&d).expect("applies");
        let identity = h.game_data().identity.clone();
        let back = Gen3Handler::load(saved(&h), identity).expect("loads");
        assert_eq!(back.game_data().tm_hm_compat, d.tm_hm_compat);
        let early = if d.identity.name.contains("Fire") { 15 } else { 249 };
        assert_eq!(back.game_data().early_required_hm_moves, [early]);
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_tutor_compat_matches_reference() {
    // FNV-1a of compat_dump, against a reference dump of tutor compatibility.
    for (name, reference) in [
        ("Emerald", 2_428_844_707_511_757_434),
        ("Ruby", 14_695_981_039_346_656_037),
        ("Fire Red", 10_029_031_834_276_230_441),
    ] {
        let Some(h) = real_rom(name) else { return };
        let hash = compat_dump(&h.game_data().tutor_compat)
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
                (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
            });
        assert_eq!(hash, reference, "{name}");
    }
}

#[test]
fn tutor_compat_is_read_and_written() {
    for (fixture, tutors) in [(emerald(), 30), (fire_red(), 15)] {
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        assert!(d.tutor_compat.values().all(|f| f.len() == tutors));
        for (i, flags) in d.tutor_compat.values_mut().enumerate() {
            flags[i % tutors] = true;
        }
        h.apply(&d).expect("applies");
        let identity = h.game_data().identity.clone();
        let back = Gen3Handler::load(saved(&h), identity).expect("loads");
        assert_eq!(back.game_data().tutor_compat, d.tutor_compat);
        assert_eq!(
            back.game_data().tutor_moves,
            d.tutor_moves,
            "the tutor moves before the flags stay"
        );
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_field_items_match_reference() {
    // FNV-1a of one item number a line, against a reference dump of the field items.
    for (name, reference, count) in [
        ("Emerald", 14_108_016_004_576_832_760, 268),
        ("Ruby", 15_286_820_844_431_807_135, 225),
        ("Fire Red", 1_978_149_571_922_919_280, 335),
    ] {
        let Some(h) = real_rom(name) else { return };
        let items = &h.game_data().field_items;
        let dump: String = items.iter().flat_map(|i| [i.to_string(), "\n".into()]).collect();
        let hash = dump.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        assert_eq!((hash, items.len()), (reference, count), "{name}");
    }
}

fn fnv1a(data: &str) -> u64 {
    data.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_shops_and_prices_match_reference() {
    use std::fmt::Write as _;
    // FNV-1a of "name|main game|special|items,…" a shop and "item=price" a priced item, against a reference
    // dump of the shops and prices.
    for (name, reference_shops, reference_prices) in [
        ("Emerald", 5_859_389_782_234_171_442, 3_496_434_870_430_969_271),
        ("Ruby", 6_028_550_114_399_155_302, 3_496_434_870_430_969_271),
        ("Fire Red", 15_622_144_124_460_346_659, 14_839_187_197_167_225_336),
    ] {
        let Some(h) = real_rom(name) else { return };
        let d = h.game_data();
        let mut shops = String::new();
        for s in &d.shops {
            write!(shops, "{}|{}|{}|", s.name, s.main_game, s.special).expect("writes");
            for i in &s.items {
                write!(shops, "{i},").expect("writes");
            }
            shops.push('\n');
        }
        let mut prices = String::new();
        for (i, p) in d.item_prices.iter().filter(|&(_, &p)| p > 0) {
            writeln!(prices, "{i}={p}").expect("writes");
        }
        assert_eq!(
            (fnv1a(&shops), fnv1a(&prices)),
            (reference_shops, reference_prices),
            "{name}"
        );
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_shops_grow_and_prices_change() {
    let Some(mut h) = real_rom("Emerald") else { return };
    let mut d = h.game_data().clone();
    for shop in &mut d.shops {
        shop.items.push(ppr_model::RARE_CANDY);
    }
    d.item_prices.insert(ppr_model::RARE_CANDY, 10);
    d.item_prices.insert(1, 70_000);
    h.apply(&d).expect("applies");
    // Too high: written as the highest price.
    d.item_prices.insert(1, 65_535);
    let identity = h.game_data().identity.clone();
    let back = Gen3Handler::load(saved(&h), identity).expect("loads");
    assert_eq!(back.game_data().shops, d.shops);
    assert_eq!(back.game_data().item_prices, d.item_prices);
}

#[test]
fn shops_are_read_and_written() {
    for fixture in [emerald(), fire_red()] {
        let mut h = fixture.load();
        let mut d = h.game_data().clone();
        assert!(d.shops.len() > 20);
        assert!(
            d.shops.iter().all(|s| s.items.len() == 2 && s.items[0] == 17),
            "Potion first"
        );
        d.shops[0].items = vec![1, 2, 3, 4, 5];
        d.shops[1].items.clear();
        h.apply(&d).expect("applies");
        let identity = h.game_data().identity.clone();
        let back = Gen3Handler::load(saved(&h), identity).expect("loads");
        assert_eq!(back.game_data().shops, d.shops);
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_pickup_items_match_reference() {
    // A reference dump of the Pickup items.
    for (name, reference) in [
        (
            "Emerald",
            &[
                17, 18, 26, 3, 79, 78, 57, 27, 2, 25, 50, 46, 28, 45, 23, 29, 51, 41, 25, 92, 221, 23, 38, 214, 371,
                40, 328, 234, 353,
            ][..],
        ),
        ("Ruby", &[26, 27, 2, 50, 23, 28, 92, 46, 51, 221][..]),
        (
            "Fire Red",
            &[
                155, 149, 150, 151, 152, 153, 156, 337, 51, 50, 92, 179, 180, 181, 182, 183,
            ][..],
        ),
    ] {
        let Some(mut h) = real_rom(name) else { return };
        assert_eq!(h.game_data().pickup_items, reference, "{name}");
        let mut d = h.game_data().clone();
        d.pickup_items.reverse();
        h.apply(&d).expect("applies");
        // The table is found by its first items, so a changed one is not found again.
        let table = h.pickup_table;
        assert_eq!(
            pickup::read(&h.rom, &h.entry, table).expect("reads"),
            d.pickup_items,
            "{name}"
        );
    }
}

/// The type chart's bytes as the reference harness prints them: three bytes a line in hex, up to the end marker.
fn type_chart_dump(h: &Gen3Handler) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut at = need(&h.entry, "TypeEffectivenessOffset").expect("offset");
    loop {
        let b = h.rom.slice(at, 3).expect("in the ROM");
        writeln!(out, "{:02x}{:02x}{:02x}", b[0], b[1], b[2]).expect("writes");
        if b[0] == 0xFF {
            return out;
        }
        at += 3;
    }
}

#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_type_charts_are_written_like_the_reference() {
    // FNV-1a of type_chart_dump against reference ROMs after the Gen 6 type update (Ghost and Dark neutral on
    // Steel) and after inverting the chart, with no immunities added.
    for name in ["Emerald", "Fire Red"] {
        for (change, reference) in [
            ("update", 16_152_214_493_917_202_026),
            ("inverse", 13_909_836_201_588_454_436),
        ] {
            let Some(mut h) = real_rom(name) else { return };
            let mut d = h.game_data().clone();
            let chart = &mut d.type_chart;
            if change == "update" {
                chart.set(Type::Ghost, Type::Steel, Effectiveness::Neutral);
                chart.set(Type::Dark, Type::Steel, Effectiveness::Neutral);
            } else {
                let types = chart.types().to_vec();
                for &a in &types {
                    for &t in &types {
                        let inverted = match chart.get(a, t) {
                            Some(Effectiveness::Zero | Effectiveness::Half) => Effectiveness::Double,
                            Some(Effectiveness::Double) => Effectiveness::Half,
                            _ => continue,
                        };
                        chart.set(a, t, inverted);
                    }
                }
            }
            h.apply(&d).expect("applies");
            assert_eq!(fnv1a(&type_chart_dump(&h)), reference, "{name} {change}");
            let identity = h.game_data().identity.clone();
            let back = Gen3Handler::load(saved(&h), identity).expect("loads");
            assert_eq!(back.game_data().type_chart, d.type_chart, "{name} {change}");
        }
    }
}

/// Trainers as the reference harness prints them: the class names, then a line per trainer.
fn trainers_dump(data: &GameData) -> String {
    let mut out = String::new();
    for c in &data.trainer_class_names {
        let _ = writeln!(out, "class {c}");
    }
    for t in &data.trainers {
        let status = match t.multi_battle {
            ppr_model::MultiBattleStatus::Never => "NEVER",
            ppr_model::MultiBattleStatus::Potential => "POTENTIAL",
            ppr_model::MultiBattleStatus::Always => "ALWAYS",
        };
        let style = if t.battle_style == ppr_model::BattleStyle::Double {
            "D"
        } else {
            "S"
        };
        let tag = t.tag.as_deref().unwrap_or("null");
        let _ = write!(out, "{} {} {} {tag} {status} {style}", t.index, t.name, t.class);
        for p in &t.pokemon {
            let _ = write!(
                out,
                " | {} {} {} {}",
                p.species,
                p.level,
                p.ivs,
                p.held_item.unwrap_or_default()
            );
            for m in p.moves {
                let _ = write!(out, " {m}");
            }
        }
        out.push('\n');
    }
    out
}

/// With `PPR_TEST_ROMS` set: the trainers are what the reference reader gives, and changed trainers
/// read back as written.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_trainers_match_reference() {
    for (name, reference) in [
        ("Emerald", 13_063_531_116_415_784_523),
        ("Ruby", 11_871_229_326_181_164_373),
        ("Fire Red", 4_240_223_254_370_651_191),
    ] {
        let Some(mut h) = real_rom(name) else { return };
        let dump = trainers_dump(h.game_data());
        if let Some(dir) = std::env::var_os("PPR_DUMP_DIR") {
            std::fs::write(std::path::Path::new(&dir).join(format!("tr-{name}.txt")), &dump).expect("writes");
        }
        assert_eq!(fnv1a(&dump), reference, "{name}");
        let mut d = h.game_data().clone();
        for (i, t) in d.trainers.iter_mut().enumerate() {
            match i % 3 {
                // Parties stay within six: a reloaded FireRed counts its trainers as a hack does.
                0 if t.pokemon.len() < 6 => {
                    let mut extra = t.pokemon[0].clone();
                    extra.held_item = Some(17);
                    t.pokemon.push(extra);
                }
                1 => {
                    t.pokemon[0].moves = [1, 2, 3, 4];
                    t.pokemon[0].ivs = 31;
                }
                _ => t.pokemon[0].level = 77,
            }
            if i % 7 == 0 {
                t.battle_style = ppr_model::BattleStyle::Double;
                t.forced_battle_style = true;
            }
        }
        // Mossdeep Steven keeps three Pokémon.
        if name == "Emerald" {
            let steven = d.trainers.last_mut().expect("Steven");
            steven.pokemon.truncate(3);
            steven.battle_style = ppr_model::BattleStyle::Single;
            steven.forced_battle_style = false;
        }
        h.apply(&d).expect("applies");
        let reloaded = Gen3Handler::load(h.rom.bytes().to_vec(), h.loaded.identity.clone()).expect("reloads");
        let mut expected = d.trainers.clone();
        for t in &mut expected {
            t.forced_battle_style = false;
        }
        for (got, want) in reloaded.game_data().trainers.iter().zip(&expected) {
            assert_eq!(got, want, "{name}");
        }
        assert_eq!(reloaded.game_data().trainers.len(), expected.len(), "{name}");
    }
}

/// With `PPR_TEST_ROMS` set: the intro bytes are those of reference ROMs (dumped with the reference
/// harness: 8 bytes from each intro offset), and `FireRed` can't show Rayquaza, whose internal index is above 255.
#[test]
#[ignore = "needs ROM dumps in PPR_TEST_ROMS"]
fn real_intro_matches_reference() {
    for (name, dex, reference) in [
        (
            "Fire Red",
            25,
            "IntroCryOffset=1920002142f758f9 IntroImageOffset=74512308d4732308 IntroOtherOffset=192000210ef740fc",
        ),
        (
            "Emerald",
            25,
            "IntroCryOffset=19000000150b0308 IntroImageOffset=19000000ffff0000",
        ),
        (
            "Emerald",
            384,
            "IntroCryOffset=96010000150b0308 IntroImageOffset=96010000ffff0000",
        ),
        (
            "Ruby",
            25,
            "IntroCryOffset=1920c04600216af0 IntroImageOffset=34841e088c7c1e08 IntroPaletteOffset=94a61e088c4e0202 \
             IntroOtherOffset=1924c046019402f0",
        ),
        (
            "Ruby",
            384,
            "IntroCryOffset=ff20973000216af0 IntroImageOffset=1c901e088c7c1e08 IntroPaletteOffset=7cb21e088c4e0202 \
             IntroOtherOffset=ff249734019402f0",
        ),
    ] {
        let Some(mut h) = real_rom(name) else { return };
        let mut d = h.game_data().clone();
        assert!(d.intro_candidates.contains(&dex), "{name}");
        if name == "Fire Red" {
            assert!(!d.intro_candidates.contains(&384));
        }
        d.intro_pokemon = Some(dex);
        h.apply(&d).expect("applies");
        let mut dump = Vec::new();
        for key in [
            "IntroCryOffset",
            "IntroImageOffset",
            "IntroPaletteOffset",
            "IntroOtherOffset",
        ] {
            let Ok(at) = need(&h.entry, key) else { continue };
            let mut entry = format!("{key}=");
            for b in h.rom.slice(at, 8).expect("in range") {
                write!(entry, "{b:02x}").expect("writes");
            }
            dump.push(entry);
        }
        assert_eq!(dump.join(" "), reference, "{name} #{dex}");
    }
}

#[test]
fn real_palettes_round_trip_and_unown_formes_follow() {
    for name in ["Fire Red", "Emerald", "Ruby"] {
        let Some(mut h) = real_rom(name) else { return };
        let game = h.game_data().clone();
        assert_eq!(game.palettes.len(), 386, "{name}");
        // 16 colors each; Castform has 64, 16 per forme.
        let size = |n: DexNumber| if n == 351 { 64 } else { 16 };
        assert!(
            game.palettes
                .iter()
                .all(|(&n, p)| p.normal.len() == size(n) && p.shiny.len() == size(n))
        );
        assert_eq!(
            game.palette_files.as_deref(),
            Some(if name == "Fire Red" { "FRLG" } else { "E" })
        );
        let mut data = game.clone();
        let red = vec![ppr_model::Color::rgb(255, 0, 0); 16];
        for n in [1, c::UNOWN] {
            let p = data.palettes.get_mut(&n).expect("has palettes");
            p.normal.clone_from(&red);
            p.shiny = vec![ppr_model::Color::rgb(0, 0, 255); 16];
        }
        h.apply(&data).expect("applies");
        let reloaded = Gen3Handler::load(saved(&h), game.identity.clone()).expect("reloads");
        assert_eq!(reloaded.game_data().palettes, data.palettes, "{name}");
        // Unown B and on share Unown's palette.
        let table = entry_offset(&reloaded.rom, &reloaded.entry, "PokemonNormalPalettes").expect("table");
        let unown = reloaded.rom.pointer(table + 201 * 8).expect("pointer");
        let b = reloaded.rom.pointer(table + c::UNOWN_B_INDEX * 8).expect("pointer");
        assert_eq!(unown, b, "{name}");
    }
}

#[test]
fn real_player_graphics_replace_the_player() {
    use ppr_model::{PlayerCharacter, PlayerGraphicsChoice};
    for (name, pack, key) in [
        ("Emerald", "wally", "MayWalkImage"),
        ("Fire Red", "may_e", "LeafWalkImage"),
    ] {
        let Some(mut h) = real_rom(name) else { return };
        let game = h.game_data().clone();
        assert!(game.player_graphics_packs.iter().any(|p| p.id == pack), "{name}");
        let mut data = game.clone();
        data.player_graphics = Some(PlayerGraphicsChoice {
            pack: pack.into(),
            replace: PlayerCharacter::Pc2,
        });
        h.apply(&data).expect("applies");
        let graphics = crate::graphics::gen3_graphics(pack, h.family.graphics()).expect("pack");
        let walk = graphics.walk.expect("walk sprite");
        let table = need(&h.entry, "OverworldSpriteImages").expect("table");
        let first = need(&h.entry, key).expect("index");
        for i in 0..walk.frame_count() {
            let at = h.rom.pointer(table + (first + i) * 8).expect("pointer");
            assert_eq!(
                h.rom.slice(at, 256).expect("frame"),
                walk.frame(i).to_bytes(),
                "{name} frame {i}"
            );
        }
        Gen3Handler::load(saved(&h), game.identity.clone()).expect("reloads");
    }
}

#[test]
fn fire_red_hacks_are_read_through_the_code_pointers() {
    // A synthetic FireRed is not the vanilla game, so hack support detects its counts: those of the entry.
    let plain = fire_red().load();
    assert_eq!(plain.entry.int("PokemonCount"), Some(411));
    assert_eq!(plain.entry.int("TrainerCount"), Some(743));
    assert_eq!(plain.entry.int("MoveCount"), Some(354));

    // A hack moves the TM/HM compatibility table and has fewer trainers.
    let mut f = fire_red();
    let compat = f.int("PokemonTMHMCompat");
    let moved = 0xF0_0000;
    let table = f.rom[compat..compat + 412 * 8].to_vec();
    f.rom[moved..moved + table.len()].copy_from_slice(&table);
    f.rom[compat..compat + table.len()].fill(0);
    f.rom[0x4_3C68..0x4_3C6C].copy_from_slice(&(0x0800_0000 + u32::try_from(moved).expect("fits")).to_le_bytes());
    let trainers = f.int("TrainerData");
    let size = f.int("TrainerEntrySize");
    f.rom[trainers + 600 * size + size - 8] = 0;
    let h = f.load();
    assert_eq!(h.entry.int("PokemonTMHMCompat"), i64::try_from(moved).ok());
    assert_eq!(h.entry.int("TrainerCount"), Some(600));
    assert_eq!(h.game_data().trainers.len(), 599);
    assert_eq!(h.game_data().tm_hm_compat, plain.game_data().tm_hm_compat);
}

#[test]
fn real_fire_red_counts_as_its_entry_when_detected() {
    let Some(vanilla) = real_rom("Fire Red") else { return };
    assert!(!hack::is_hack(&vanilla.rom));
    // Detection on the vanilla game finds exactly the entry's tables and counts.
    let mut entry = vanilla.entry.clone();
    let mut bytes = vanilla.rom.bytes().to_vec();
    bytes.push(0);
    let rom = GbaRom::new(bytes);
    assert!(hack::adjust(&rom, &mut entry, &vanilla.text).expect("adjusts"));
    for key in [
        "PokemonCount",
        "MoveCount",
        "TrainerCount",
        "PokemonMovesets",
        "PokemonTMHMCompat",
        "PokemonEvolutions",
        "MoveDescriptions",
        "TrainerData",
    ] {
        assert_eq!(entry.int(key), vanilla.entry.int(key), "{key}");
    }
    assert_eq!(
        entry.int("MoveTutorCompatibility"),
        Some(
            vanilla.entry.int("MoveTutorData").expect("tutors")
                + vanilla.entry.int("MoveTutorMoves").expect("tutors") * 2
        )
    );
}

#[test]
fn balanced_prices_fit() {
    for (id, price) in c::BALANCED_ITEM_PRICES {
        assert_eq!(ppr_model::PriceFormat::WORD.check(price), Ok(()), "item {id}");
    }
}
