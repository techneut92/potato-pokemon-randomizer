//! Synthetic Gen 3 ROMs laid out like the real games: the same entry offsets, internal order (25 unused slots
//! between Celebi and Treecko) and table formats. No copyrighted data.

use std::collections::HashMap;

use super::constants as c;
use crate::entries::{RomEntry, catalog};
use ppr_model::Generation;

pub(crate) const NAMES_AT: usize = 0x40_0000;
pub(crate) const STATS_AT: usize = 0x41_0000;
/// Free space for the wild encounter data.
pub(crate) const WILD_AT: usize = 0x50_0000;
/// Land slots of the fixture's first map, shared with its second map.
pub(crate) const LAND_SLOTS_AT: usize = WILD_AT;
/// Fishing slots of the fixture's first map.
pub(crate) const FISHING_SLOTS_AT: usize = WILD_AT + 0x200;
/// Catch rate of every fixture species.
pub(crate) const CATCH_RATE: u8 = 45;
/// Map name of every fixture area.
pub(crate) const LOCATION: &str = "ROUTE1";
/// Scripts and texts of the starter text entries (`FireRed` and `LeafGreen`).
pub(crate) const SCRIPTS_AT: usize = 0x52_0000;
/// Bytes of free space the fixture makes at the entry's `FreeSpace` offset.
pub(crate) const FREE_BYTES: usize = 0x2000;
/// Item data (names first), pointed to from pointer block 2.
pub(crate) const ITEMS_AT: usize = 0x53_0000;
/// Where the ability names go when the entry has a pointer to them.
pub(crate) const ABILITIES_AT: usize = 0x54_0000;
/// Move data and names, where pointer blocks 2 and 1 lead.
pub(crate) const MOVES_AT: usize = 0x56_0000;
pub(crate) const MOVE_NAMES_AT: usize = 0x57_0000;
/// Where the learnsets are: 8 bytes for each internal index.
pub(crate) const LEARNSETS_AT: usize = 0x58_0000;
/// Move `m`'s description `DESC<m>`, 16 bytes each.
pub(crate) const MOVE_DESCRIPTIONS_AT: usize = 0x59_0000;
/// TM `i`'s item description `TMDESC`, 16 bytes each.
pub(crate) const TM_DESCRIPTIONS_AT: usize = 0x5A_0000;
/// Where the maps, people and scripts of the TM texts go.
const TM_TEXT_MAPS_AT: usize = 0x70_0000;
/// Shop item lists: shop `i` sells Potion and item 30 + `i`, at `SHOPS_AT + 0x10 * i`.
pub(crate) const SHOPS_AT: usize = 0x5B_0000;
/// Trainer `i`'s team, 8 bytes: one #(`i` % 200 + 1) at level 5 + `i` % 50, at `TRAINER_TEAMS_AT + 8 * i`.
pub(crate) const TRAINER_TEAMS_AT: usize = 0x5C_0000;
/// The egg moves: #1 with moves 10 and 11, #4 with move 12.
pub(crate) const EGG_MOVES: [(usize, &[u16]); 2] = [(1, &[10, 11]), (4, &[12])];
/// What the starter texts say before they are rewritten.
pub(crate) const OLD_TEXT: &str = "OLD";

/// Species of the first static encounter; the next ones count up.
pub(crate) const STATIC_SPECIES: usize = 10;
/// Level of the first static encounter; the next ones count up.
pub(crate) const STATIC_LEVEL: usize = 5;
/// Species the first in-game trade gives; the next ones count up.
pub(crate) const TRADE_GIVEN: usize = 120;
/// Species the first in-game trade asks for; the next ones count up.
pub(crate) const TRADE_REQUESTED: usize = 130;
/// Level of every roamer.
pub(crate) const ROAMER_LEVEL: u8 = 40;

/// The species and level offsets of a `StaticPokemon{}` value.
fn spot_offsets(spot: &str) -> (Vec<usize>, Vec<usize>) {
    let list = |key: &str| -> Vec<usize> {
        spot.split(key)
            .nth(1)
            .and_then(|rest| rest.split(']').next())
            .map(|l| {
                l.split(',')
                    .map(|o| usize::from_str_radix(o.trim().trim_start_matches("0x"), 16).expect("hex"))
                    .collect()
            })
            .unwrap_or_default()
    };
    (list("Species=["), list("Level=["))
}

/// Internal index of a national number in the real Gen 3 order.
pub(crate) fn internal(dex: usize) -> usize {
    if dex <= 251 { dex } else { dex + 25 }
}

/// A synthetic ROM and the entry it is laid out after.
pub(crate) struct Fixture {
    pub(crate) rom: Vec<u8>,
    pub(crate) entry: RomEntry,
}

pub(crate) fn encoder() -> HashMap<char, u8> {
    include_str!("../../data/texttables/gba_english.tbl")
        .lines()
        .filter_map(|l| l.split_once('='))
        .filter_map(|(k, v)| Some((v.chars().next()?, u8::from_str_radix(k, 16).ok()?)))
        .filter(|(c, _)| c.is_ascii_alphanumeric() || *c == '?')
        .collect()
}

impl Fixture {
    pub(crate) fn new(entry_name: &str, code: [u8; 4]) -> Self {
        let entry = catalog(Generation::new(3).expect("gen 3"))
            .entries
            .iter()
            .find(|e| e.name == entry_name)
            .cloned()
            .expect("entry exists");
        let mut f = Self {
            rom: vec![0u8; 0x100_0000],
            entry,
        };
        f.rom[0xAC..0xB0].copy_from_slice(&code);
        f.rom[0xBD] = f.rom[0xA0..0xBD]
            .iter()
            .fold(0u8, |c, &b| c.wrapping_sub(b))
            .wrapping_sub(0x19);
        f.u32(
            c::POKEMON_NAMES_POINTER,
            0x0800_0000 + u32::try_from(NAMES_AT).expect("fits"),
        );
        f.u32(
            c::POKEMON_STATS_POINTER,
            0x0800_0000 + u32::try_from(STATS_AT).expect("fits"),
        );
        f.species_tables();
        // Bulbasaur → Ivysaur → Venusaur by level, internal indices as targets.
        f.evolution(1, 0, 4, 16, 2);
        f.evolution(2, 0, 4, 32, 3);
        f.write_starters([252, 255, 258]);
        f.wild();
        f.type_chart();
        f.items();
        f.ability_names();
        f.moves();
        f.learnsets();
        f.tms();
        f.shops();
        f.trainers();
        let free = f.int("FreeSpace");
        f.rom[free..free + FREE_BYTES].fill(0xFF);
        f.starter_text_scripts();
        f.tm_text_scripts();
        f.statics();
        f.trades();
        f.hack_pointers();
        f
    }

    /// The pointers `FireRed` 1.0's code reads its tables through, which hack support follows (a synthetic ROM
    /// is never the vanilla game).
    fn hack_pointers(&mut self) {
        if self.entry.code != "BPRE" || self.entry.version != 0 {
            return;
        }
        let tutor_compat = self.int("MoveTutorData") + self.int("MoveTutorMoves") * 2;
        for (at, key) in [
            (0x4_3C68, "PokemonTMHMCompat"),
            (0x4_2F6C, "PokemonEvolutions"),
            (0xE_5440, "MoveDescriptions"),
            (0xFC00, "TrainerData"),
        ] {
            let table = self.int(key);
            self.pointer(at, table);
        }
        self.pointer(0x12_0C30, tutor_compat);
    }

    /// Trade `i` asks for species `TRADE_REQUESTED + i` and gives `TRADE_GIVEN + i` called `NICK<i>`, from
    /// `OT<i>` (ID 1000 + i), with IVs 1–6; the first holds item 13.
    fn trades(&mut self) {
        let enc = encoder();
        let table = self.int("TradeTableOffset");
        for i in 0..self.int("TradeTableSize") {
            let at = table + i * 60;
            for (offset, name) in [(0, format!("NICK{i}")), (43, format!("OT{i}"))] {
                for (k, ch) in name.chars().enumerate() {
                    self.rom[at + offset + k] = enc[&ch];
                }
                self.rom[at + offset + name.len()] = 0xFF;
            }
            self.u16(at + 12, u16::try_from(internal(TRADE_GIVEN + i)).expect("fits"));
            self.rom[at + 14..at + 20].copy_from_slice(&[1, 2, 3, 4, 5, 6]);
            self.u16(at + 24, u16::try_from(1000 + i).expect("fits"));
            self.u16(at + 40, if i == 0 { 13 } else { 0 });
            self.u16(at + 56, u16::try_from(internal(TRADE_REQUESTED + i)).expect("fits"));
        }
    }

    /// Static `i` is species `STATIC_SPECIES + i` at level `STATIC_LEVEL + i`, and the hardcoded encounters and
    /// roamers are left unpatched, as in the real games.
    fn statics(&mut self) {
        let spots: Vec<String> = self.entry.list("StaticPokemon{}").to_vec();
        for (i, spot) in spots.iter().enumerate() {
            let (species, levels) = spot_offsets(spot);
            for at in species {
                self.u16(at, u16::try_from(internal(STATIC_SPECIES + i)).expect("fits"));
            }
            for at in levels {
                self.rom[at] = u8::try_from(STATIC_LEVEL + i).expect("fits");
            }
        }
        if let Some(at) = self.entry.int("StaticFirstBattleSpeciesOffset") {
            self.u16(usize::try_from(at).expect("offset"), 0xFFFF);
        }
        if let Some(at) = self
            .entry
            .array("GhostMarowakSpeciesOffsets")
            .and_then(|a| a.first().copied())
        {
            self.u16(usize::try_from(at).expect("offset"), 0xFFFF);
        }
        let roamers: Vec<String> = self.entry.list("RoamingPokemon{}").to_vec();
        for (i, roamer) in roamers.iter().enumerate() {
            let (species, levels) = spot_offsets(roamer);
            match (self.entry.family.as_str(), i) {
                // Unpatched: free space in FireRed and LeafGreen, a pointer in Emerald (only Latias's is a constant).
                ("FRLG", _) => self.u16(species[0], 0xFFFF),
                ("Em", 0) => self.u16(species[0], 0x5D8C),
                _ => self.u16(species[0], u16::try_from(internal(380)).expect("fits")),
            }
            for at in levels {
                self.rom[at] = ROAMER_LEVEL;
            }
        }
    }

    /// Every item is called `ITEM<internal index>`.
    /// Abilities 0–77 called `AB<i>`, where the handler looks for them.
    fn ability_names(&mut self) {
        let at = if self.entry.int("HasPointerBlock2") == Some(1) {
            self.pointer(c::ABILITY_NAMES_POINTER, ABILITIES_AT);
            ABILITIES_AT
        } else {
            self.int("AbilityNames")
        };
        let len = self.int("AbilityNameLength");
        let enc = encoder();
        for i in 0..=77 {
            let name = format!("AB{i}");
            for (k, ch) in name.chars().enumerate() {
                self.rom[at + i * len + k] = enc[&ch];
            }
            self.rom[at + i * len + name.len()] = 0xFF;
        }
    }

    /// Moves 1 to `MoveCount` called `MV<i>`: Normal, 40 power, 100% accurate, 35 PP; move 2 a 0-power
    /// status move and move 3 a Fire move with a 10% burn.
    fn moves(&mut self) {
        let data = if self.entry.int("HasPointerBlock2") == Some(1) {
            self.pointer(c::MOVE_DATA_POINTER, MOVES_AT);
            MOVES_AT
        } else {
            self.int("MoveData")
        };
        let names = if self.entry.int("HasPointerBlock1") == Some(1) {
            self.pointer(c::MOVE_NAMES_POINTER, MOVE_NAMES_AT);
            MOVE_NAMES_AT
        } else {
            self.int("MoveNames")
        };
        let len = self.int("MoveNameLength");
        let enc = encoder();
        for i in 1..=self.int("MoveCount") {
            let at = data + i * 0xC;
            self.rom[at..at + 9].copy_from_slice(&[0, 40, 0, 100, 35, 0, 0, 0, 1]);
            let name = format!("MV{i}");
            for (k, ch) in name.chars().enumerate() {
                self.rom[names + i * len + k] = enc[&ch];
            }
            self.rom[names + i * len + name.len()] = 0xFF;
        }
        self.rom[data + 2 * 0xC + 1] = 0;
        // Effect 4 (damage with a burn chance), Fire, 10%.
        self.rom[data + 3 * 0xC..data + 3 * 0xC + 6].copy_from_slice(&[4, 90, 10, 100, 15, 10]);
    }

    /// Every internal species learns move 33 at level 1 and move `index % 300 + 1` at level 10.
    fn learnsets(&mut self) {
        let table = self.int("PokemonMovesets");
        if self.entry.code == "BPRE" && self.entry.version == 0 {
            self.pointer(0x3_EA7C, table);
        }
        for i in 1..=411 {
            let at = LEARNSETS_AT + i * 8;
            self.pointer(table + i * 4, at);
            let [low, high] = u16::try_from(i % 300 + 1).expect("fits").to_le_bytes();
            self.rom[at..at + 6].copy_from_slice(&[33, 1 << 1, low, (10 << 1) | high, 0xFF, 0xFF]);
        }
        let mut at = self.int("EggMoves");
        for (dex, moves) in EGG_MOVES {
            self.u16(at, u16::try_from(internal(dex)).expect("fits") + 20_000);
            at += 2;
            for &m in moves {
                self.u16(at, m);
                at += 2;
            }
        }
        self.u16(at, 0xFFFF);
    }

    /// TM `i` teaches move `i` and tutor `i` (from 0) move `100 + i`; moves and TM items have descriptions.
    fn tms(&mut self) {
        if let Some(tutors) = self.entry.int("MoveTutorData") {
            let tutors = usize::try_from(tutors).expect("offset");
            for i in 0..self.int("MoveTutorMoves") {
                self.u16(tutors + i * 2, u16::try_from(100 + i).expect("fits"));
            }
        }
        let table = self.int("TmMoves");
        for i in 1..=50 {
            self.u16(table + (i - 1) * 2, u16::try_from(i).expect("fits"));
        }
        let enc = encoder();
        let text = |rom: &mut Vec<u8>, at: usize, s: &str| {
            for (k, ch) in s.chars().enumerate() {
                rom[at + k] = enc[&ch];
            }
            rom[at + s.len()] = 0xFF;
        };
        let descriptions = self.int("MoveDescriptions");
        for m in 1..=self.int("MoveCount") {
            let at = MOVE_DESCRIPTIONS_AT + m * 16;
            text(&mut self.rom, at, &format!("DESC{m}"));
            self.pointer(descriptions + (m - 1) * 4, at);
        }
        let size = self.int("ItemEntrySize");
        for i in 0..50 {
            let at = TM_DESCRIPTIONS_AT + i * 16;
            text(&mut self.rom, at, "TMDESC");
            self.pointer(ITEMS_AT + (289 + i) * size + 0x14, at);
        }
    }

    fn items(&mut self) {
        self.pointer(c::ITEM_DATA_POINTER, ITEMS_AT);
        let size = self.int("ItemEntrySize");
        let enc = encoder();
        for internal in 1..=self.int("ItemCount") {
            let at = ITEMS_AT + size * internal;
            let name = format!("ITEM{internal}");
            for (k, ch) in name.chars().enumerate() {
                self.rom[at + k] = enc[&ch];
            }
            self.rom[at + name.len()] = 0xFF;
        }
    }

    /// Water, Grass and Fire beat each other, Fire resists Water, and Normal can't hit Ghost (after Foresight's
    /// marker), like the real chart's format.
    fn type_chart(&mut self) {
        let at = self.int("TypeEffectivenessOffset");
        let entries: [[u8; 3]; 6] = [
            [0x0B, 0x0A, 20],
            [0x0C, 0x0B, 20],
            [0x0A, 0x0C, 20],
            [0x0A, 0x0B, 5],
            [0xFE, 0xFE, 0],
            [0x00, 0x07, 0],
        ];
        for (k, e) in entries.iter().enumerate() {
            self.rom[at + 3 * k..at + 3 * k + 3].copy_from_slice(e);
        }
        self.rom[at + 18..at + 21].copy_from_slice(&[0xFF, 0xFF, 0]);
    }

    /// For each `StarterText[]` entry, the path from the map headers through the person's script to a pointer to
    /// [`OLD_TEXT`]. The first entry's pointer has a copy 0x40 bytes further on, like a second script would.
    fn starter_text_scripts(&mut self) {
        let texts = super::events::parse(self.entry.list("StarterText[]"));
        if texts.is_empty() {
            return;
        }
        let maps = SCRIPTS_AT;
        let header = SCRIPTS_AT + 0x100;
        let events = SCRIPTS_AT + 0x200;
        let people = SCRIPTS_AT + 0x300;
        let groups = self.int("MapHeaders");
        let (bank, map) = texts[0].location();
        self.pointer(groups + 4 * bank, maps);
        self.pointer(maps + 4 * map, header);
        self.pointer(header + 4, events);
        self.rom[events] = 8;
        self.pointer(events + 4, people);
        let enc = encoder();
        for (n, t) in texts.iter().enumerate() {
            let text = SCRIPTS_AT + 0x500 + 0x10 * n;
            for (k, ch) in OLD_TEXT.chars().enumerate() {
                self.rom[text + k] = enc[&ch];
            }
            self.rom[text + OLD_TEXT.len()] = 0xFF;
            assert_eq!(t.location(), (bank, map), "all on one map");
            let mut at = SCRIPTS_AT + 0x1000 * (n + 1);
            self.pointer(people + t.person() * 24 + 16, at);
            let (last, path) = t.relative().split_last().expect("a path");
            for (k, step) in path.iter().enumerate() {
                let next = SCRIPTS_AT + 0x1000 * (n + 1) + 0x100 * (k + 1);
                self.pointer(at + step, next);
                at = next;
            }
            self.pointer(at + last, text);
            if n == 0 {
                self.pointer(at + last + 0x40, text);
            }
        }
    }

    /// Each `TMText[]` and `MoveTutorText[]` entry's map, person and script path, ending at the text `OLD`; maps the starter texts set up
    /// are shared.
    fn tm_text_scripts(&mut self) {
        const PEOPLE: usize = 64;
        let mut texts = super::events::parse(self.entry.list("TMText[]"));
        texts.extend(super::events::parse(self.entry.list("MoveTutorText[]")));
        let groups = self.int("MapHeaders");
        let mut next = TM_TEXT_MAPS_AT;
        let mut alloc = |size: usize| {
            let at = next;
            next += size;
            at
        };
        let enc = encoder();
        for t in &texts {
            let (bank, map) = t.location();
            let maps = self.pointer_or(groups + 4 * bank, || alloc(0x400));
            let header = self.pointer_or(maps + 4 * map, || alloc(0x100));
            let events = self.pointer_or(header + 4, || alloc(0x100));
            if self.rom[events] == 0 {
                self.rom[events] = u8::try_from(PEOPLE).expect("fits");
            }
            let people = self.pointer_or(events + 4, || alloc(PEOPLE * 24));
            // Texts of one person share the script.
            let mut at = self.pointer_or(people + t.person() * 24 + 16, || alloc(0x1000));
            let (last, path) = t.relative().split_last().expect("a path");
            for step in path {
                at = self.pointer_or(at + step, || alloc(0x1000));
            }
            let text = alloc(0x10);
            for (k, ch) in OLD_TEXT.chars().enumerate() {
                self.rom[text + k] = enc[&ch];
            }
            self.rom[text + OLD_TEXT.len()] = 0xFF;
            self.pointer(at + last, text);
        }
    }

    /// The offset the pointer at `at` points to, after pointing it to `new()` if it pointed nowhere.
    fn pointer_or(&mut self, at: usize, new: impl FnOnce() -> usize) -> usize {
        let raw = u32::from_le_bytes(self.rom[at..at + 4].try_into().expect("4 bytes"));
        if raw == 0 {
            let to = new();
            self.pointer(at, to);
            to
        } else {
            usize::try_from(raw - 0x0800_0000).expect("fits")
        }
    }

    /// Where the pointer to a starter text is.
    #[cfg(test)]
    pub(crate) fn starter_text_pointer(&self, id: usize) -> usize {
        let texts = super::events::parse(self.entry.list("StarterText[]"));
        let t = &texts[id];
        let mut at = SCRIPTS_AT + 0x1000 * (id + 1);
        let (last, path) = t.relative().split_last().expect("a path");
        for k in 0..path.len() {
            at = SCRIPTS_AT + 0x1000 * (id + 1) + 0x100 * (k + 1);
        }
        at + last
    }

    pub(crate) fn int(&self, key: &str) -> usize {
        usize::try_from(self.entry.int(key).expect(key)).expect("positive")
    }

    pub(crate) fn u16(&mut self, at: usize, v: u16) {
        self.rom[at..at + 2].copy_from_slice(&v.to_le_bytes());
    }

    fn u32(&mut self, at: usize, v: u32) {
        self.rom[at..at + 4].copy_from_slice(&v.to_le_bytes());
    }

    fn species_tables(&mut self) {
        let enc = encoder();
        let order = self.int("PokedexOrder");
        let name_len = self.int("PokemonNameLength");
        for i in 1..=411 {
            let (dex, name) = match i {
                1..=251 => (i, format!("MON{i}")),
                252..=276 => (i + 135, "?".to_owned()),
                _ => (i - 25, format!("MON{}", i - 25)),
            };
            self.u16(order + (i - 1) * 2, u16::try_from(dex).expect("fits"));
            let at = NAMES_AT + i * name_len;
            for (k, ch) in name.chars().enumerate() {
                self.rom[at + k] = enc[&ch];
            }
            self.rom[at + name.len()] = 0xFF;
            let stats = STATS_AT + i * c::BASE_STATS_ENTRY_SIZE;
            if (252..=276).contains(&i) {
                self.rom[stats..stats + 28].copy_from_slice(&c::EMPTY_POKEMON_SIG);
            } else {
                // Grass (0x0C), Fire (0x0A) and Water (0x0B) in turn; every 10th is Water/Flying.
                let primary = [0x0C, 0x0A, 0x0B][dex % 3];
                let secondary = if dex % 10 == 0 { 0x02 } else { primary };
                let b = u8::try_from(dex % 200 + 10).expect("fits");
                self.rom[stats..stats + 9].copy_from_slice(&[b, b, b, b, b, b, primary, secondary, CATCH_RATE]);
            }
        }
    }

    /// Every shop of the entry, each with two items and the terminator the shop reader looks for.
    fn shops(&mut self) {
        let pointers = self.entry.array("ShopPointerOffsets").unwrap_or_default();
        for (i, &pointer) in pointers.iter().enumerate() {
            let at = SHOPS_AT + 0x10 * i;
            self.u16(at, 13);
            self.u16(at + 2, u16::try_from(30 + i).expect("fits"));
            self.rom[at + 4..at + 8].copy_from_slice(&[0, 0, 0x6C, 0x02]);
            self.pointer(usize::try_from(pointer).expect("offset"), at);
        }
    }

    /// Every trainer of the entry with one Pokémon (see [`TRAINER_TEAMS_AT`]), named `TR`; Emerald's Mossdeep
    /// Steven with three #1 at level 42.
    fn trainers(&mut self) {
        let enc = encoder();
        let (table, size) = (self.int("TrainerData"), self.int("TrainerEntrySize"));
        for i in 1..self.int("TrainerCount") {
            let at = table + i * size;
            self.rom[at + 4..at + 7].copy_from_slice(&[enc[&'T'], enc[&'R'], 0xFF]);
            self.rom[at + size - 8] = 1;
            let team = TRAINER_TEAMS_AT + 8 * i;
            self.pointer(at + size - 4, team);
            self.u16(team + 2, u16::try_from(5 + i % 50).expect("fits"));
            self.u16(team + 4, u16::try_from(internal(i % 200 + 1)).expect("fits"));
        }
        if let Some(steven) = self.entry.int("MossdeepStevenTeamOffset") {
            let steven = usize::try_from(steven).expect("offset");
            for k in 0..3 {
                self.u16(steven + k * 20, u16::try_from(internal(1)).expect("fits"));
                self.rom[steven + k * 20 + 3] = 42;
            }
        }
    }

    fn pointer(&mut self, at: usize, to: usize) {
        self.u32(at, 0x0800_0000 + u32::try_from(to).expect("fits"));
    }

    /// As many encounter areas as the real game, all in map bank 0 and on one region map section:
    /// - map 1: walking (Pokémon #10–#21, levels 2–4) and fishing (#100–#109, levels 5–14);
    /// - map 2: map 1's walking slots again (read once);
    /// - map 3: walking with rate 0 (skipped);
    /// - maps 4 and on: walking with #30 and up at level 5, one area each.
    fn wild(&mut self) {
        let areas = super::tags_for(&self.entry.family).len();
        let land_info = WILD_AT + 0x100;
        let fishing_info = WILD_AT + 0x300;
        let header = WILD_AT + 0x400;
        let name = WILD_AT + 0x600;
        let maps = WILD_AT + 0x800;
        let more = WILD_AT + 0x1000;
        for i in 0..12 {
            let at = LAND_SLOTS_AT + i * 4;
            self.rom[at..at + 2].copy_from_slice(&[2, 4]);
            self.u16(at + 2, u16::try_from(internal(10 + i)).expect("fits"));
        }
        for i in 0..10 {
            let at = FISHING_SLOTS_AT + i * 4;
            let level = u8::try_from(5 + i).expect("fits");
            self.rom[at..at + 2].copy_from_slice(&[level, level]);
            self.u16(at + 2, u16::try_from(internal(100 + i)).expect("fits"));
        }
        self.rom[land_info] = 20;
        self.pointer(land_info + 4, LAND_SLOTS_AT);
        self.rom[fishing_info] = 10;
        self.pointer(fishing_info + 4, FISHING_SLOTS_AT);

        let table = self.int("WildPokemon");
        let mut headers: Vec<(usize, usize)> = vec![(4, land_info), (16, fishing_info)];
        self.wild_header(table, 1, &headers);
        self.wild_header(table + 20, 2, &[(4, land_info)]);
        let silent = more;
        self.pointer(silent + 4, LAND_SLOTS_AT + 0x40);
        self.wild_header(table + 40, 3, &[(4, silent)]);
        // Two areas so far; one more map per remaining area.
        for k in 0..areas - 2 {
            let info = more + 0x40 * (k + 1);
            let slots = info + 8;
            self.rom[info] = 20;
            self.pointer(info + 4, slots);
            for i in 0..12 {
                let at = slots + i * 4;
                self.rom[at..at + 2].copy_from_slice(&[5, 5]);
                self.u16(at + 2, u16::try_from(internal(30 + k % 50)).expect("fits"));
            }
            headers = vec![(4, info)];
            self.wild_header(table + 60 + 20 * k, 4 + k, &headers);
        }
        let end = table + 60 + 20 * (areas - 2);
        self.rom[end..end + 2].copy_from_slice(&[0xFF, 0xFF]);

        // Every map uses one header, on region map section 0x60.
        let section = 0x60;
        self.rom[header + 0x14] = section;
        for number in 1..areas + 2 {
            self.pointer(maps + 4 * number, header);
        }
        let groups = self.int("MapHeaders");
        self.pointer(groups, maps);
        let labels = self.int("MapLabels");
        let label = if self.entry.family == "FRLG" {
            labels + 4 * (usize::from(section) - 0x58)
        } else {
            labels + 8 * usize::from(section) + 4
        };
        self.pointer(label, name);
        let enc = encoder();
        for (k, ch) in LOCATION.chars().enumerate() {
            self.rom[name + k] = enc[&ch];
        }
        self.rom[name + LOCATION.len()] = 0xFF;
    }

    fn wild_header(&mut self, at: usize, map: usize, infos: &[(usize, usize)]) {
        self.rom[at..at + 2].copy_from_slice(&[0, u8::try_from(map).expect("fits")]);
        for &(field, info) in infos {
            self.pointer(at + field, info);
        }
    }

    fn evolution(&mut self, dex: usize, slot: usize, method: u16, parameter: u16, to_dex: usize) {
        let at = self.int("PokemonEvolutions") + internal(dex) * c::EVOLUTION_ENTRY_SIZE + slot * 8;
        self.u16(at, method);
        self.u16(at + 2, parameter);
        self.u16(at + 4, u16::try_from(internal(to_dex)).expect("fits"));
    }

    pub(crate) fn starter_offsets(&self) -> [usize; 3] {
        let base = self.int("StarterPokemon");
        if self.entry.family == "FRLG" {
            [base, base + c::FRLG_STARTER_2, base + c::FRLG_STARTER_3]
        } else {
            [base, base + c::RSE_STARTER_2, base + c::RSE_STARTER_3]
        }
    }

    fn write_starters(&mut self, dex: [usize; 3]) {
        for (at, d) in self.starter_offsets().into_iter().zip(dex) {
            self.u16(at, u16::try_from(internal(d)).expect("fits"));
        }
    }
}

/// Synthetic Emerald (U).
pub(crate) fn emerald() -> Fixture {
    Fixture::new("Emerald (U)", *b"BPEE")
}

/// Synthetic `FireRed` (U) 1.0.
pub(crate) fn fire_red() -> Fixture {
    Fixture::new("Fire Red (U) 1.0", *b"BPRE")
}
