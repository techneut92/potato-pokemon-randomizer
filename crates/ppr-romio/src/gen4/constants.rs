//! Constants of Diamond, Pearl, Platinum, `HeartGold` and `SoulSilver`.

use ppr_model::{DexNumber, HeldItemPools, ItemId, MoveCategory, MoveId, Type};

/// `Type=` in the ROM entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    DiamondPearl,
    Platinum,
    HeartGoldSoulSilver,
}

impl Family {
    pub(crate) fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "dp" => Some(Self::DiamondPearl),
            "plat" => Some(Self::Platinum),
            "hgss" => Some(Self::HeartGoldSoulSilver),
            _ => None,
        }
    }

    /// Diamond, Pearl or Platinum.
    pub(crate) fn is_dppt(self) -> bool {
        self != Self::HeartGoldSoulSilver
    }

    /// The number of alternate formes with data of their own: Deoxys and Wormadam in Diamond and Pearl, and
    /// Giratina, Shaymin and Rotom too from Platinum on.
    pub(crate) fn forme_count(self) -> usize {
        if self == Self::DiamondPearl { 5 } else { 12 }
    }
}

pub(crate) const SPECIES_COUNT: usize = 493;
pub(crate) const MOVE_COUNT: usize = 467;
/// Alternate formes' data comes this many files after the last species (an egg and a bad egg come between).
pub(crate) const FORME_OFFSET: usize = 2;

/// The alternate formes with data of their own, in the order of their files, numbered from 494: base species,
/// forme number and the full name's suffix. Diamond and Pearl have the first five.
pub(crate) const FORMES: [(DexNumber, u8, &str); 12] = [
    (386, 1, "-Attack"),
    (386, 2, "-Defense"),
    (386, 3, "-Speed"),
    (413, 1, "-Sandy"),
    (413, 2, "-Trash"),
    (487, 1, "-Origin"),
    (492, 1, "-Sky"),
    (479, 1, "-Heat"),
    (479, 2, "-Wash"),
    (479, 3, "-Frost"),
    (479, 4, "-Fan"),
    (479, 5, "-Mow"),
];
/// Base species whose full name has a suffix too: Wormadam's Plant Cloak.
pub(crate) const BASE_FORME_SUFFIXES: [(DexNumber, &str); 1] = [(413, "-Plant")];
/// Species with cosmetic formes that have no data of their own, from Platinum on, with their number of formes:
/// Unown, Burmy, Shellos and Gastrodon.
pub(crate) const COSMETIC_FORMES: [(DexNumber, u8); 4] = [(201, 28), (412, 3), (422, 2), (423, 2)];
/// Giratina's Origin Forme, which trainers can't use from Platinum on: without the Griseous Orb it reverts.
pub(crate) const GIRATINA_ORIGIN: DexNumber = 499;
/// Burmy, and Wormadam's alternate formes, which Burmy's evolution can take too.
pub(crate) const BURMY: DexNumber = 412;
pub(crate) const WORMADAM_FORMES: [DexNumber; 2] = [497, 498];

/// The species number of forme `forme` of `base`: an alternate forme with data of its own counts from 494, other
/// formes (cosmetic ones, and forme 0) are `base`.
pub(crate) fn forme_species(family: Family, base: DexNumber, forme: u8) -> DexNumber {
    FORMES[..family.forme_count()]
        .iter()
        .position(|&(b, f, _)| b == base && f == forme && forme != 0)
        .and_then(|i| DexNumber::try_from(SPECIES_COUNT + 1 + i).ok())
        .unwrap_or(base)
}

/// The base species of `number`: itself unless it is an alternate forme of [`FORMES`].
pub(crate) fn base_species(number: DexNumber) -> DexNumber {
    usize::from(number)
        .checked_sub(SPECIES_COUNT + 1)
        .and_then(|i| FORMES.get(i))
        .map_or(number, |&(base, _, _)| base)
}

/// The file of species or alternate forme `number` in the archives kept per species (stats, learnsets).
pub(crate) fn species_file(number: DexNumber) -> usize {
    let n = usize::from(number);
    if n > SPECIES_COUNT { n + FORME_OFFSET } else { n }
}

pub(crate) const BS_HP: usize = 0;
pub(crate) const BS_ATTACK: usize = 1;
pub(crate) const BS_DEFENSE: usize = 2;
pub(crate) const BS_SPEED: usize = 3;
pub(crate) const BS_SP_ATK: usize = 4;
pub(crate) const BS_SP_DEF: usize = 5;
pub(crate) const BS_PRIMARY_TYPE: usize = 6;
pub(crate) const BS_SECONDARY_TYPE: usize = 7;
pub(crate) const BS_CATCH_RATE: usize = 8;
pub(crate) const BS_EV_YIELD: usize = 10;
pub(crate) const BS_COMMON_HELD_ITEM: usize = 12;
pub(crate) const BS_RARE_HELD_ITEM: usize = 14;
pub(crate) const BS_GENDER_RATIO: usize = 16;
pub(crate) const BS_EGG_CYCLES: usize = 17;
pub(crate) const BS_GROWTH_CURVE: usize = 19;
pub(crate) const BS_ABILITY_1: usize = 22;
pub(crate) const BS_ABILITY_2: usize = 23;
pub(crate) const BS_TM_HM_COMPAT: usize = 28;
/// Bytes of TM and HM flags.
pub(crate) const TM_HM_COMPAT_BYTES: usize = 13;

pub(crate) const TM_COUNT: usize = 92;
pub(crate) const HM_COUNT: usize = 8;
pub(crate) const TM01: ItemId = 328;
pub(crate) const DPPT_TM_DATA_PREFIX: &str = "D100D200D300D400";
pub(crate) const HGSS_TM_DATA_PREFIX: &str = "1E003200";
pub(crate) const DP_ITEM_PALETTES_PREFIX: &str = "8D018E01210132018D018F0122013301";
pub(crate) const PTHGSS_ITEM_PALETTES_PREFIX: &str = "8D018E01210133018D018F0122013401";
pub(crate) const DPPT_TEXT_CHARS_PER_LINE: usize = 38;
pub(crate) const HGSS_TEXT_CHARS_PER_LINE: usize = 36;

pub(crate) const EVOLUTIONS_PER_SPECIES: usize = 7;
pub(crate) const EVOLUTION_ENTRY_SIZE: usize = 6;
pub(crate) const SHEDINJA_SPECIES_LOCATOR: &str = "492080000090281C0521";
pub(crate) const FRIENDSHIP_VALUE_FOR_EVO_LOCATOR: &str = "DC286AD3";
pub(crate) const VANILLA_HAPPINESS_TO_EVOLVE: u8 = 220;
pub(crate) const EASIER_HAPPINESS_TO_EVOLVE: u8 = 160;
pub(crate) const PERFECT_ODDS_BRANCH_LOCATOR: &str = "FF2901D30425";
pub(crate) const DPPT_EGG_MOVE_TABLE_PREFIX: &str = "40016601";
/// Egg move lists start with the species number plus this.
pub(crate) const EGG_MOVES_SPECIES_BASE: u16 = 20000;
pub(crate) const EGG_MOVES_END: u16 = 0xFFFF;

pub(crate) const TYPE_CHART_END: u8 = 0xFF;
pub(crate) const TYPE_CHART_FORESIGHT: u8 = 0xFE;
/// The pairs that aren't neutral that the type chart has room for.
pub(crate) const TYPE_CHART_ROOM: usize = 110;
/// The "???" type of Curse.
pub(crate) const UNKNOWN_TYPE: u8 = 0x09;

pub(crate) const NINCADA: DexNumber = 290;
pub(crate) const MARILL: DexNumber = 183;
pub(crate) const CHIKORITA: DexNumber = 152;
pub(crate) const CYNDAQUIL: DexNumber = 155;
pub(crate) const TOTODILE: DexNumber = 158;
pub(crate) const TURTWIG: DexNumber = 387;
pub(crate) const HEADBUTT: MoveId = 29;
pub(crate) const ROAR: MoveId = 46;
pub(crate) const CHATTER: MoveId = 448;
pub(crate) const TRIPLE_KICK: MoveId = 167;
pub(crate) const ADAMANT_ORB: ItemId = 135;

/// The type of a type byte; `None` for unused bytes and the "???" type.
pub(crate) fn type_from_byte(b: u8) -> Option<Type> {
    Some(match b {
        0x00 => Type::Normal,
        0x01 => Type::Fighting,
        0x02 => Type::Flying,
        0x03 => Type::Poison,
        0x04 => Type::Ground,
        0x05 => Type::Rock,
        0x06 => Type::Bug,
        0x07 => Type::Ghost,
        0x08 => Type::Steel,
        0x0A => Type::Fire,
        0x0B => Type::Water,
        0x0C => Type::Grass,
        0x0D => Type::Electric,
        0x0E => Type::Psychic,
        0x0F => Type::Ice,
        0x10 => Type::Dragon,
        0x11 => Type::Dark,
        _ => return None,
    })
}

/// The type byte of a type; Fairy, which Gen 4 lacks, is Normal.
pub(crate) fn type_to_byte(t: Type) -> u8 {
    match t {
        Type::Normal | Type::Fairy => 0x00,
        Type::Fighting => 0x01,
        Type::Flying => 0x02,
        Type::Poison => 0x03,
        Type::Ground => 0x04,
        Type::Rock => 0x05,
        Type::Bug => 0x06,
        Type::Ghost => 0x07,
        Type::Steel => 0x08,
        Type::Fire => 0x0A,
        Type::Water => 0x0B,
        Type::Grass => 0x0C,
        Type::Electric => 0x0D,
        Type::Psychic => 0x0E,
        Type::Ice => 0x0F,
        Type::Dragon => 0x10,
        Type::Dark => 0x11,
    }
}

pub(crate) fn category_from_byte(b: u8) -> MoveCategory {
    match b {
        0 => MoveCategory::Physical,
        1 => MoveCategory::Special,
        _ => MoveCategory::Status,
    }
}

pub(crate) fn category_to_byte(c: MoveCategory) -> u8 {
    match c {
        MoveCategory::Physical => 0,
        MoveCategory::Special => 1,
        MoveCategory::Status => 2,
    }
}

/// Moves that hit two to five times (three on average).
pub(crate) const MULTIHIT_MOVES: [MoveId; 14] = [292, 140, 198, 331, 4, 3, 31, 154, 333, 42, 350, 131, 541, 594];
/// Moves that hit twice.
pub(crate) const DOUBLE_HIT_MOVES: [MoveId; 8] = [155, 458, 742, 24, 751, 530, 544, 41];
pub(crate) const SOUND_MOVES: [MoveId; 14] = [45, 46, 47, 48, 103, 173, 253, 319, 320, 304, 405, 448, 195, 215];
pub(crate) const PUNCH_MOVES: [MoveId; 15] = [8, 7, 9, 183, 264, 146, 223, 359, 5, 4, 309, 325, 409, 418, 327];

/// Sonic Boom and Dragon Rage.
pub(crate) const GAME_BREAKING_MOVES: [MoveId; 2] = [49, 82];
pub(crate) const DP_REQUIRED_FIELD_TMS: [ItemId; 24] = [
    329, 330, 332, 336, 339, 346, 350, 355, 361, 366, 368, 370, 373, 374, 376, 377, 389, 396, 406, 407, 409, 411, 412,
    414,
];
pub(crate) const PT_REQUIRED_FIELD_TMS: [ItemId; 28] = [
    329, 330, 332, 334, 336, 338, 339, 345, 346, 350, 355, 361, 364, 366, 368, 370, 373, 374, 376, 377, 389, 396, 406,
    407, 409, 411, 412, 414,
];
pub(crate) const DPPT_FIELD_MOVES: [MoveId; 12] = [15, 19, 57, 70, 148, 91, 100, 127, 249, 230, 432, 431];
pub(crate) const HGSS_FIELD_MOVES: [MoveId; 13] = [15, 19, 57, 70, 148, 91, 100, 250, 127, 249, 29, 230, 431];
pub(crate) const DPPT_EARLY_REQUIRED_HM_MOVES: [MoveId; 2] = [249, 15];
pub(crate) const HGSS_EARLY_REQUIRED_HM_MOVES: [MoveId; 1] = [15];

pub(crate) const EVOLUTION_ITEMS: [ItemId; 23] = [
    80, 81, 82, 83, 84, 85, 107, 108, 109, 110, 221, 226, 227, 233, 235, 252, 321, 322, 323, 324, 325, 326, 327,
];

/// Insomnia and Vital Spirit, Clear Body and White Smoke, Huge Power and Pure Power, Battle Armor and Shell Armor,
/// Cloud Nine and Air Lock, Filter and Solid Rock.
pub(crate) const ABILITY_VARIATIONS: [(u16, [u16; 2]); 6] = [
    (15, [15, 72]),
    (29, [29, 73]),
    (37, [37, 74]),
    (4, [4, 75]),
    (13, [13, 76]),
    (111, [111, 116]),
];
/// Forecast, Multitype and Flower Gift (which does something in Gen 4, but is listed as in later games).
pub(crate) const USELESS_ABILITIES: [u16; 3] = [59, 121, 122];

/// Items that may not be used: key items, version exclusives, unknown items and HMs.
pub(crate) fn item_banned(id: ItemId) -> bool {
    (428..=536).contains(&id) || (112..=134).contains(&id) || (420..=427).contains(&id)
}

/// Items of little use as held items: for one species, mulch, mail, berries without a battle effect, contest
/// scarves.
pub(crate) fn item_bad(id: ItemId) -> bool {
    [111, 112, 225, 236, 155, 274, 70, 71, 135, 136].contains(&id)
        || (95..=98).contains(&id)
        || (137..=148).contains(&id)
        || (159..=183).contains(&id)
        || (256..=259).contains(&id)
        || (260..=264).contains(&id)
}

/// Rare Candy, the vitamins, PP Up, PP Max and the Lucky Egg.
pub(crate) const OP_SHOP_ITEMS: [ItemId; 10] = [50, 86, 87, 88, 89, 90, 91, 92, 106, 231];

const CONSUMABLE_HELD_ITEMS: [ItemId; 49] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 184, 185, 186, 187, 188, 189, 190, 191,
    192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 43, 214,
    219, 271, 275,
];
const LASTING_HELD_ITEMS: [ItemId; 74] = [
    213, 217, 220, 221, 222, 230, 232, 233, 234, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250,
    251, 253, 254, 255, 265, 266, 267, 268, 269, 270, 272, 273, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286,
    287, 288, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315,
    316, 317, 318, 326, 327,
];
const GENERAL_PURPOSE_CONSUMABLE_ITEMS: [ItemId; 24] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 202, 203, 205, 206, 207, 208, 209, 210, 211, 212, 43, 214, 219,
    275,
];
const GENERAL_PURPOSE_ITEMS: [ItemId; 16] = [
    213, 217, 221, 230, 232, 234, 253, 255, 265, 268, 270, 276, 280, 295, 326, 327,
];
const TYPE_BOOSTING_ITEMS: [(Type, &[ItemId]); 17] = [
    (Type::Bug, &[222, 308]),
    (Type::Dark, &[240, 312]),
    (Type::Dragon, &[250, 311]),
    (Type::Electric, &[242, 300]),
    (Type::Fighting, &[241, 303]),
    (Type::Fire, &[249, 298]),
    (Type::Flying, &[244, 306]),
    (Type::Ghost, &[247, 310]),
    (Type::Grass, &[239, 301, 318]),
    (Type::Ground, &[237, 305]),
    (Type::Ice, &[246, 302]),
    (Type::Normal, &[251]),
    (Type::Poison, &[245, 304]),
    (Type::Psychic, &[248, 307, 314]),
    (Type::Rock, &[238, 309, 315]),
    (Type::Steel, &[233, 313]),
    (Type::Water, &[243, 254, 299, 317]),
];
const SPECIES_BOOSTING_ITEMS: [(DexNumber, &[ItemId]); 11] = [
    (483, &[135]),
    (484, &[136]),
    (380, &[225]),
    (381, &[225]),
    (366, &[226, 227]),
    (25, &[236]),
    (113, &[256]),
    (132, &[257, 274]),
    (104, &[258]),
    (105, &[258]),
    (83, &[259]),
];

/// The held item pools: every held item, the consumable ones, and those sensible for most species.
pub(crate) fn held_item_pools() -> HeldItemPools {
    let consumable = CONSUMABLE_HELD_ITEMS.to_vec();
    HeldItemPools {
        all: LASTING_HELD_ITEMS
            .iter()
            .copied()
            .chain(consumable.iter().copied())
            .collect(),
        consumable,
        general_consumable: GENERAL_PURPOSE_CONSUMABLE_ITEMS.to_vec(),
        general: GENERAL_PURPOSE_ITEMS.to_vec(),
        // Liechi Berry, Petaya Berry; Choice Band and Muscle Band.
        physical_consumable: vec![201],
        special_consumable: vec![204],
        physical: vec![220, 266],
        type_boosting: TYPE_BOOSTING_ITEMS
            .iter()
            .map(|&(t, items)| (t, items.to_vec()))
            .collect(),
        species_boosting: SPECIES_BOOSTING_ITEMS
            .iter()
            .map(|&(n, items)| (n, items.to_vec()))
            .collect(),
    }
}

pub(crate) const DP_GYM_AND_ELITE_THEMES: [(&str, Type); 12] = [
    ("ELITE1", Type::Bug),
    ("ELITE2", Type::Ground),
    ("ELITE3", Type::Fire),
    ("ELITE4", Type::Psychic),
    ("GYM1", Type::Rock),
    ("GYM2", Type::Grass),
    ("GYM3", Type::Fighting),
    ("GYM4", Type::Water),
    ("GYM5", Type::Ghost),
    ("GYM6", Type::Steel),
    ("GYM7", Type::Ice),
    ("GYM8", Type::Electric),
];
pub(crate) const PT_GYM_AND_ELITE_THEMES: [(&str, Type); 12] = [
    ("ELITE1", Type::Bug),
    ("ELITE2", Type::Ground),
    ("ELITE3", Type::Fire),
    ("ELITE4", Type::Psychic),
    ("GYM1", Type::Rock),
    ("GYM2", Type::Grass),
    ("GYM3", Type::Ghost),
    ("GYM4", Type::Fighting),
    ("GYM5", Type::Water),
    ("GYM6", Type::Steel),
    ("GYM7", Type::Ice),
    ("GYM8", Type::Electric),
];
pub(crate) const HGSS_GYM_AND_ELITE_THEMES: [(&str, Type); 20] = [
    ("CHAMPION", Type::Dragon),
    ("ELITE1", Type::Psychic),
    ("ELITE2", Type::Poison),
    ("ELITE3", Type::Fighting),
    ("ELITE4", Type::Dark),
    ("GYM1", Type::Flying),
    ("GYM2", Type::Bug),
    ("GYM3", Type::Normal),
    ("GYM4", Type::Ghost),
    ("GYM5", Type::Fighting),
    ("GYM6", Type::Steel),
    ("GYM7", Type::Ice),
    ("GYM8", Type::Dragon),
    ("GYM9", Type::Rock),
    ("GYM10", Type::Water),
    ("GYM11", Type::Electric),
    ("GYM12", Type::Grass),
    ("GYM13", Type::Poison),
    ("GYM14", Type::Psychic),
    ("GYM15", Type::Fire),
];

/// Better prices for items whose price does not fit their use, by item.
pub(crate) const BALANCED_ITEM_PRICES: [(ItemId, u32); 404] = [
    (1, 3000),
    (2, 1200),
    (3, 600),
    (4, 200),
    (5, 500),
    (6, 1000),
    (7, 1000),
    (8, 1000),
    (9, 1000),
    (10, 1000),
    (11, 1000),
    (12, 200),
    (13, 1000),
    (14, 300),
    (15, 1000),
    (16, 200),
    (492, 300),
    (493, 300),
    (494, 300),
    (495, 300),
    (496, 300),
    (497, 300),
    (498, 300),
    (499, 300),
    (485, 20),
    (487, 20),
    (486, 20),
    (488, 20),
    (489, 20),
    (490, 20),
    (491, 20),
    (17, 300),
    (18, 100),
    (19, 250),
    (20, 250),
    (21, 250),
    (22, 200),
    (23, 3000),
    (24, 2500),
    (25, 1200),
    (26, 700),
    (27, 600),
    (28, 1500),
    (29, 4000),
    (30, 400),
    (31, 600),
    (32, 700),
    (33, 800),
    (34, 400),
    (35, 1100),
    (36, 450),
    (37, 2800),
    (38, 3000),
    (39, 4500),
    (40, 15000),
    (41, 18000),
    (42, 450),
    (43, 100),
    (44, 10000),
    (54, 450),
    (45, 9800),
    (46, 9800),
    (47, 9800),
    (48, 9800),
    (49, 9800),
    (50, 10000),
    (51, 9800),
    (52, 9800),
    (53, 24900),
    (55, 700),
    (56, 650),
    (57, 500),
    (58, 550),
    (59, 350),
    (60, 950),
    (61, 350),
    (62, 350),
    (63, 1000),
    (64, 1000),
    (65, 20),
    (66, 20),
    (67, 20),
    (68, 20),
    (69, 20),
    (70, 20),
    (71, 20),
    (72, 400),
    (73, 400),
    (74, 400),
    (75, 400),
    (76, 500),
    (77, 700),
    (78, 550),
    (79, 350),
    (80, 3000),
    (81, 3000),
    (82, 3000),
    (83, 3000),
    (84, 3000),
    (85, 3000),
    (107, 3000),
    (108, 3000),
    (109, 3000),
    (110, 3000),
    (321, 3000),
    (322, 3000),
    (323, 3000),
    (324, 3000),
    (325, 3000),
    (326, 5000),
    (327, 5000),
    (86, 500),
    (87, 5000),
    (88, 1400),
    (89, 7500),
    (90, 2000),
    (91, 9800),
    (92, 10000),
    (93, 5000),
    (106, 10000),
    (94, 500),
    (95, 200),
    (96, 200),
    (97, 200),
    (98, 200),
    (99, 5000),
    (100, 5000),
    (101, 5000),
    (102, 5000),
    (103, 8000),
    (104, 5000),
    (105, 5000),
    (111, 2100),
    (112, 10000),
    (135, 10000),
    (136, 10000),
    (137, 50),
    (138, 50),
    (139, 50),
    (140, 50),
    (141, 50),
    (142, 50),
    (143, 50),
    (144, 50),
    (145, 50),
    (146, 50),
    (147, 50),
    (148, 50),
    (150, 250),
    (151, 100),
    (152, 250),
    (153, 250),
    (154, 3000),
    (155, 50),
    (157, 500),
    (158, 500),
    (159, 100),
    (160, 100),
    (161, 100),
    (162, 100),
    (163, 100),
    (164, 500),
    (165, 500),
    (166, 500),
    (167, 500),
    (168, 500),
    (169, 500),
    (170, 500),
    (171, 500),
    (172, 500),
    (173, 500),
    (174, 500),
    (175, 500),
    (176, 500),
    (177, 500),
    (178, 500),
    (179, 500),
    (180, 500),
    (181, 500),
    (182, 500),
    (183, 500),
    (184, 1000),
    (185, 1000),
    (186, 1000),
    (187, 1000),
    (188, 1000),
    (189, 1000),
    (190, 1000),
    (191, 1000),
    (192, 1000),
    (193, 1000),
    (194, 1000),
    (195, 1000),
    (196, 1000),
    (197, 1000),
    (198, 1000),
    (199, 1000),
    (200, 1000),
    (201, 1000),
    (202, 1000),
    (203, 1000),
    (204, 1000),
    (205, 1000),
    (206, 1000),
    (207, 1000),
    (208, 1000),
    (209, 1000),
    (210, 1000),
    (211, 1000),
    (212, 1000),
    (213, 3000),
    (215, 3000),
    (216, 6000),
    (217, 4500),
    (220, 10000),
    (221, 5000),
    (222, 2000),
    (223, 15000),
    (224, 1000),
    (225, 200),
    (226, 3000),
    (227, 3000),
    (228, 200),
    (229, 200),
    (230, 3000),
    (231, 10000),
    (232, 5000),
    (233, 3000),
    (234, 10000),
    (235, 3000),
    (236, 100),
    (237, 2000),
    (238, 2000),
    (239, 2000),
    (240, 2000),
    (241, 2000),
    (242, 2000),
    (243, 2000),
    (244, 2000),
    (245, 2000),
    (246, 2000),
    (247, 2000),
    (248, 2000),
    (249, 2000),
    (250, 2000),
    (251, 2000),
    (252, 3000),
    (253, 6000),
    (254, 2000),
    (255, 3000),
    (256, 10),
    (257, 10),
    (258, 500),
    (259, 200),
    (260, 100),
    (261, 100),
    (262, 100),
    (263, 100),
    (264, 100),
    (265, 1500),
    (268, 6000),
    (269, 1500),
    (270, 10000),
    (272, 1500),
    (273, 1500),
    (274, 10),
    (276, 1500),
    (277, 3000),
    (278, 1000),
    (279, 1000),
    (280, 1500),
    (281, 5000),
    (282, 200),
    (283, 200),
    (284, 200),
    (285, 200),
    (286, 1500),
    (287, 10000),
    (288, 1500),
    (289, 3000),
    (290, 3000),
    (291, 3000),
    (292, 3000),
    (293, 3000),
    (294, 3000),
    (295, 500),
    (296, 1500),
    (297, 10000),
    (298, 2000),
    (299, 2000),
    (300, 2000),
    (301, 2000),
    (302, 2000),
    (303, 2000),
    (304, 2000),
    (305, 2000),
    (306, 2000),
    (307, 2000),
    (308, 2000),
    (309, 2000),
    (310, 2000),
    (311, 2000),
    (312, 2000),
    (313, 2000),
    (314, 2000),
    (315, 2000),
    (316, 1000),
    (317, 2000),
    (318, 2000),
    (319, 15000),
    (320, 1000),
    (328, 3000),
    (329, 3000),
    (330, 3000),
    (331, 1500),
    (332, 1000),
    (333, 3000),
    (334, 2000),
    (335, 1500),
    (336, 2000),
    (337, 2000),
    (338, 2000),
    (339, 1500),
    (340, 3000),
    (341, 5500),
    (342, 7500),
    (343, 2000),
    (344, 2000),
    (345, 2000),
    (346, 3000),
    (347, 2000),
    (348, 1000),
    (349, 3000),
    (350, 3000),
    (351, 3000),
    (352, 5500),
    (353, 3000),
    (354, 1000),
    (355, 2000),
    (356, 3000),
    (357, 3000),
    (358, 3000),
    (359, 1000),
    (360, 2000),
    (361, 3000),
    (362, 3000),
    (363, 3000),
    (364, 2000),
    (365, 5500),
    (366, 2000),
    (367, 3000),
    (368, 1500),
    (369, 3000),
    (370, 2000),
    (371, 3000),
    (372, 3000),
    (373, 2000),
    (374, 3000),
    (375, 3000),
    (376, 1500),
    (377, 5500),
    (378, 2000),
    (379, 5500),
    (380, 3000),
    (381, 2000),
    (382, 3000),
    (383, 2000),
    (384, 3000),
    (385, 2000),
    (386, 3000),
    (387, 3000),
    (388, 2000),
    (389, 3000),
    (390, 2000),
    (391, 7500),
    (392, 3000),
    (393, 3000),
    (394, 1000),
    (395, 7500),
    (396, 1500),
    (397, 1000),
    (398, 3000),
    (399, 3000),
    (400, 2000),
    (401, 3000),
    (402, 1500),
    (403, 2000),
    (404, 1500),
    (405, 1500),
    (406, 3000),
    (407, 3000),
    (408, 3000),
    (409, 1000),
    (410, 2000),
    (411, 3000),
    (412, 3000),
    (413, 3000),
    (414, 1500),
    (415, 3000),
    (416, 3000),
    (417, 2000),
    (418, 3000),
    (419, 5500),
    (484, 0),
];
