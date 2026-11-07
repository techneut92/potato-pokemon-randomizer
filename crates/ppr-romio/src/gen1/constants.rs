//! Constants of Red, Green, Blue and Yellow.

use ppr_model::{ItemId, MoveId, Type};

/// `Type_RB` and `Type_Yellow` (`Type=` in the ROM entries).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    RedBlue,
    Yellow,
}

impl Family {
    pub(super) fn parse(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("RB") {
            Some(Self::RedBlue)
        } else if s.eq_ignore_ascii_case("Yellow") {
            Some(Self::Yellow)
        } else {
            None
        }
    }
}

pub(super) const BASE_STATS_ENTRY_SIZE: usize = 0x1C;
pub(super) const BS_HP: usize = 1;
pub(super) const BS_ATTACK: usize = 2;
pub(super) const BS_DEFENSE: usize = 3;
pub(super) const BS_SPEED: usize = 4;
pub(super) const BS_SPECIAL: usize = 5;
pub(super) const BS_PRIMARY_TYPE: usize = 6;
pub(super) const BS_SECONDARY_TYPE: usize = 7;
pub(super) const BS_CATCH_RATE: usize = 8;
pub(super) const BS_LEVEL_1_MOVES: usize = 15;
pub(super) const BS_GROWTH_CURVE: usize = 19;
pub(super) const BS_TM_HM_COMPAT: usize = 20;

pub(super) const ENCOUNTER_TABLE_END: u16 = 0xFFFF;
pub(super) const ENCOUNTER_TABLE_SIZE: usize = 10;
pub(super) const YELLOW_SUPER_ROD_TABLE_SIZE: usize = 4;
/// The maps of the Pokémon Tower, where the ghost Marowak must not be met in the wild.
pub(super) const TOWER_MAPS: std::ops::RangeInclusive<usize> = 0x90..=0x94;

pub(super) const TRAINER_CLASS_COUNT: usize = 47;
/// How far before the gym leader moves table the jump to the champion's custom moves is.
pub(super) const CHAMP_RIVAL_OFFSET_FROM_GYM_LEADER_MOVES: usize = 0x44;
/// The TMs (from 1) whose moves the gym leaders of Red and Blue use, gym by gym.
pub(super) const GYM_LEADER_TMS: [usize; 8] = [34, 11, 24, 21, 6, 46, 38, 27];
/// Trainer classes named after a person: the gym leaders, Giovanni and the Elite Four.
pub(super) const PERSONAL_TRAINER_CLASSES: [u8; 12] = [28, 32, 33, 34, 35, 36, 37, 38, 39, 43, 45, 46];
/// The trainer classes (from 1) that also have an abbreviated in-battle name.
pub(super) const ABBREVIATED_TRAINER_CLASSES: [usize; 21] = [
    1, 2, 3, 5, 6, 7, 8, 11, 12, 13, 15, 18, 20, 21, 24, 26, 27, 28, 30, 31, 32,
];

pub(super) const TM_COUNT: usize = 50;
pub(super) const HM_COUNT: usize = 5;

pub(super) const MEW: u16 = 151;
pub(super) const MAROWAK: u16 = 105;
pub(super) const PIKACHU: u16 = 25;

/// Sonic Boom, Dragon Rage, Spore and the one-hit KO moves.
pub(super) const GAME_BREAKING_MOVES: [MoveId; 6] = [49, 82, 147, 32, 90, 12];
/// Transform, for the Transform assumption glitch.
pub(super) const BANNED_LEVEL_UP_MOVES: [MoveId; 1] = [144];
/// Cut, Fly, Surf, Strength, Flash, Dig and Teleport.
pub(super) const FIELD_MOVES: [MoveId; 7] = [15, 19, 57, 70, 148, 91, 100];
/// Cut.
pub(super) const EARLY_REQUIRED_HM_MOVES: [MoveId; 1] = [15];
/// Karate Chop, Razor Leaf, Crabhammer and Slash.
pub(super) const INCREASED_CRIT_MOVES: [MoveId; 4] = [2, 75, 152, 163];
pub(super) const TOXIC: MoveId = 92;

pub(super) use crate::gb::{DOUBLE_HIT, MULTIHIT, NO_POWER_NON_STATUS};

/// The TMs found in the field that are needed.
pub(super) const REQUIRED_FIELD_TMS: [ItemId; 18] = [
    330, 331, 335, 337, 339, 341, 343, 346, 347, 349, 352, 353, 357, 367, 370, 371, 372, 374,
];
/// The evolution stones: Moon, Fire, Thunder, Water and Leaf.
pub(super) const EVOLUTION_ITEMS: [ItemId; 5] = [81, 82, 83, 84, 85];
/// Rare Candy and Nugget.
pub(super) const OP_SHOP_ITEMS: [ItemId; 2] = [50, 92];
/// Balanced shop prices, by item.
pub(super) const BALANCED_PRICES: [(ItemId, u32); 8] = [
    (1, 3000),
    (5, 1200),
    (81, 2100),
    (51, 9800),
    (38, 3000),
    (39, 4500),
    (40, 15000),
    (41, 18000),
];

pub(super) const SHOP_ITEMS_SCRIPT: u8 = 0xFE;
pub(super) const SHOP_ITEMS_TERMINATOR: u8 = 0xFF;
pub(super) const SHOP_NAMES: [&str; 14] = [
    "Viridian Poké Mart",
    "Pewter Poké Mart",
    "Cerulean Poké Mart",
    "Vermilion Poké Mart",
    "Lavender Poké Mart",
    "Celadon Department Store 2F Left",
    "Celadon Department Store 2F Right",
    "Celadon Department Store 4F",
    "Celadon Department Store 5F Left",
    "Celadon Department Store 5F Right",
    "Fuchsia Poké Mart",
    "Cinnabar Poké Mart",
    "Saffron Poké Mart",
    "Indigo Plateau Lobby",
];
/// The Celadon 5F shops.
pub(super) const SPECIAL_SHOPS: [usize; 2] = [8, 9];

pub(super) const HIDDEN_OBJECT_MAPS_TERMINATOR: u8 = 0xFF;
pub(super) const HIDDEN_OBJECTS_TERMINATOR: u8 = 0xFF;
pub(super) const GUARANTEED_CATCH_PREFIX: [u8; 4] = [0xCF, 0x7E, 0xFE, 0x01];

/// Items only Gen 1 has are numbered from here plus their internal index.
pub(super) const UNIQUE_OFFSET: ItemId = 1000;
pub(super) const TM01: ItemId = 328;
pub(super) const HM01: ItemId = 420;
pub(super) const SAFARI_BALL: ItemId = 5;
pub(super) const MAX_ELIXIR: ItemId = 41;

/// Internal index → standard item number.
const STANDARD: [ItemId; 256] = [
    0, 1, 2, 3, 4, 1005, 1006, 1007, 5, 1009, 81, 18, 19, 20, 21, 22, 23, 24, 25, 26, 17, 1021, 1022, 1023, 1024, 1025,
    1026, 1027, 1028, 78, 79, 1031, 82, 83, 84, 45, 46, 47, 48, 49, 50, 1041, 1042, 1043, 1044, 1045, 60, 85, 1048, 92,
    1050, 63, 27, 28, 29, 55, 76, 77, 56, 1059, 30, 31, 32, 1063, 1064, 57, 58, 59, 61, 1069, 1070, 1071, 1072, 1073,
    1074, 1075, 1076, 1077, 1078, 51, 38, 39, 40, 41, 1084, 1085, 1086, 1087, 1088, 1089, 1090, 1091, 1092, 1093, 1094,
    1095, 1096, 1097, 1098, 1099, 1100, 1101, 1102, 1103, 1104, 1105, 1106, 1107, 1108, 1109, 1110, 1111, 1112, 1113,
    1114, 1115, 1116, 1117, 1118, 1119, 1120, 1121, 1122, 1123, 1124, 1125, 1126, 1127, 1128, 1129, 1130, 1131, 1132,
    1133, 1134, 1135, 1136, 1137, 1138, 1139, 1140, 1141, 1142, 1143, 1144, 1145, 1146, 1147, 1148, 1149, 1150, 1151,
    1152, 1153, 1154, 1155, 1156, 1157, 1158, 1159, 1160, 1161, 1162, 1163, 1164, 1165, 1166, 1167, 1168, 1169, 1170,
    1171, 1172, 1173, 1174, 1175, 1176, 1177, 1178, 1179, 1180, 1181, 1182, 1183, 1184, 1185, 1186, 1187, 1188, 1189,
    1190, 1191, 1192, 1193, 1194, 1195, 420, 421, 422, 423, 424, 328, 329, 330, 331, 332, 333, 334, 335, 336, 337, 338,
    339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361,
    362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 1251, 1252, 1253, 1254, 1255,
];

/// The standard number of an internal item index.
pub(super) fn item_standard(internal: u8) -> ItemId {
    STANDARD[usize::from(internal)]
}

/// The internal index of a standard item number; `None` for a number Gen 1 has no item for.
pub(super) fn item_internal(id: ItemId) -> Option<u8> {
    if id >= UNIQUE_OFFSET {
        return u8::try_from(id - UNIQUE_OFFSET).ok();
    }
    STANDARD
        .iter()
        .position(|&s| s == id)
        .and_then(|i| u8::try_from(i).ok())
}

/// Every Gen 1 unique item (all key items or unused), the HMs and the Safari Ball.
pub(super) fn item_banned(id: ItemId) -> bool {
    id >= UNIQUE_OFFSET || (HM01..HM01 + 5).contains(&id) || id == SAFARI_BALL
}

/// The type byte → type.
pub(super) fn type_from_byte(b: u8) -> Option<Type> {
    Some(match b {
        0x00 => Type::Normal,
        0x01 => Type::Fighting,
        0x02 => Type::Flying,
        0x03 => Type::Poison,
        0x04 => Type::Ground,
        0x05 => Type::Rock,
        0x07 => Type::Bug,
        0x08 => Type::Ghost,
        0x14 => Type::Fire,
        0x15 => Type::Water,
        0x16 => Type::Grass,
        0x17 => Type::Electric,
        0x18 => Type::Psychic,
        0x19 => Type::Ice,
        0x1A => Type::Dragon,
        _ => return None,
    })
}

/// The type byte of a type; `None` for a type Gen 1 doesn't have.
pub(super) fn type_to_byte(t: Type) -> Option<u8> {
    (0..=0x1Fu8).find(|&b| type_from_byte(b) == Some(t))
}

/// The type themes of the gyms and the Elite Four.
pub(super) const GYM_AND_ELITE_THEMES: [(&str, Type); 12] = [
    ("ELITE1", Type::Ice),
    ("ELITE2", Type::Fighting),
    ("ELITE3", Type::Ghost),
    ("ELITE4", Type::Dragon),
    ("GYM1", Type::Rock),
    ("GYM2", Type::Water),
    ("GYM3", Type::Electric),
    ("GYM4", Type::Grass),
    ("GYM5", Type::Poison),
    ("GYM6", Type::Psychic),
    ("GYM7", Type::Fire),
    ("GYM8", Type::Ground),
];

/// The tags every Gen 1 game shares: (class from 1, number within the class, tag).
const TAGS_UNIVERSAL: [(u8, usize, &str); 14] = [
    (34, 0, "GYM1-LEADER"),
    (35, 0, "GYM2-LEADER"),
    (36, 0, "GYM3-LEADER"),
    (37, 0, "GYM4-LEADER"),
    (38, 0, "GYM5-LEADER"),
    (40, 0, "GYM6-LEADER"),
    (39, 0, "GYM7-LEADER"),
    (29, 2, "GYM8-LEADER"),
    (29, 0, "GIO1"),
    (29, 1, "GIO2"),
    (44, 0, "ELITE1"),
    (33, 0, "ELITE2"),
    (46, 0, "ELITE3"),
    (47, 0, "ELITE4"),
];

/// The gym trainers, the same in every game.
const TAGS_GYMS: [(u8, usize, &str); 41] = [
    (5, 0, "GYM1"),
    (15, 0, "GYM2"),
    (6, 0, "GYM2"),
    (4, 7, "GYM3"),
    (20, 0, "GYM3"),
    (41, 2, "GYM3"),
    (3, 16, "GYM4"),
    (3, 17, "GYM4"),
    (6, 10, "GYM4"),
    (18, 0, "GYM4"),
    (18, 1, "GYM4"),
    (18, 2, "GYM4"),
    (32, 0, "GYM4"),
    (21, 2, "GYM5"),
    (21, 3, "GYM5"),
    (21, 6, "GYM5"),
    (21, 7, "GYM5"),
    (22, 0, "GYM5"),
    (22, 1, "GYM5"),
    (19, 0, "GYM6"),
    (19, 1, "GYM6"),
    (19, 2, "GYM6"),
    (19, 3, "GYM6"),
    (45, 21, "GYM6"),
    (45, 22, "GYM6"),
    (45, 23, "GYM6"),
    (8, 8, "GYM7"),
    (8, 9, "GYM7"),
    (8, 10, "GYM7"),
    (8, 11, "GYM7"),
    (11, 3, "GYM7"),
    (11, 4, "GYM7"),
    (11, 5, "GYM7"),
    (22, 2, "GYM8"),
    (22, 3, "GYM8"),
    (24, 5, "GYM8"),
    (24, 6, "GYM8"),
    (24, 7, "GYM8"),
    (31, 0, "GYM8"),
    (31, 8, "GYM8"),
    (31, 9, "GYM8"),
];

/// The rival's battles, three teams each for the three starters.
const TAGS_RIVAL_RB: [(u8, usize, &str); 24] = [
    (25, 0, "RIVAL1-0"),
    (25, 1, "RIVAL1-1"),
    (25, 2, "RIVAL1-2"),
    (25, 3, "RIVAL2-0"),
    (25, 4, "RIVAL2-1"),
    (25, 5, "RIVAL2-2"),
    (25, 6, "RIVAL3-0"),
    (25, 7, "RIVAL3-1"),
    (25, 8, "RIVAL3-2"),
    (42, 0, "RIVAL4-0"),
    (42, 1, "RIVAL4-1"),
    (42, 2, "RIVAL4-2"),
    (42, 3, "RIVAL5-0"),
    (42, 4, "RIVAL5-1"),
    (42, 5, "RIVAL5-2"),
    (42, 6, "RIVAL6-0"),
    (42, 7, "RIVAL6-1"),
    (42, 8, "RIVAL6-2"),
    (42, 9, "RIVAL7-0"),
    (42, 10, "RIVAL7-1"),
    (42, 11, "RIVAL7-2"),
    (43, 0, "RIVAL8-0"),
    (43, 1, "RIVAL8-1"),
    (43, 2, "RIVAL8-2"),
];

/// The rival's battles, and Jessie and James.
const TAGS_RIVAL_YELLOW: [(u8, usize, &str); 20] = [
    (25, 0, "RIVAL1-0"),
    (25, 1, "RIVAL2-0"),
    (25, 2, "RIVAL3-0"),
    (42, 0, "RIVAL4-0"),
    (42, 1, "RIVAL5-0"),
    (42, 2, "RIVAL5-1"),
    (42, 3, "RIVAL5-2"),
    (42, 4, "RIVAL6-0"),
    (42, 5, "RIVAL6-1"),
    (42, 6, "RIVAL6-2"),
    (42, 7, "RIVAL7-0"),
    (42, 8, "RIVAL7-1"),
    (42, 9, "RIVAL7-2"),
    (43, 0, "RIVAL8-0"),
    (43, 1, "RIVAL8-1"),
    (43, 2, "RIVAL8-2"),
    (30, 41, "THEMED:JESSIE&JAMES"),
    (30, 42, "THEMED:JESSIE&JAMES"),
    (30, 43, "THEMED:JESSIE&JAMES"),
    (30, 44, "THEMED:JESSIE&JAMES"),
];

/// The trainer tags, in order: universal tags, then the game's rivals (and Jessie and James), then the
/// gym trainers.
pub(super) fn trainer_tags(family: Family) -> Vec<(u8, usize, &'static str)> {
    let rivals: &[(u8, usize, &str)] = match family {
        Family::RedBlue => &TAGS_RIVAL_RB,
        Family::Yellow => &TAGS_RIVAL_YELLOW,
    };
    TAGS_UNIVERSAL.iter().chain(rivals).chain(&TAGS_GYMS).copied().collect()
}

/// Starters that are a trainer's Pokémon, not a byte of their own: (starter, class from 1, number within the class,
/// position in the team). Yellow's second starter is the rival's Eevee in the first battle, which its
/// `StarterOffsets2` points to inside the trainer data; that data moves when the rival's class grows.
pub(super) fn trainer_starters(family: Family) -> &'static [(usize, u8, usize, usize)] {
    match family {
        Family::RedBlue => &[],
        Family::Yellow => &[(1, 25, 0, 0)],
    }
}

/// Where the rival's starter must stay in a team: (class from 1, number within the class, position of the starter).
pub(super) fn forced_starter_positions(family: Family) -> &'static [(u8, usize, u8)] {
    match family {
        Family::RedBlue => &[
            (25, 3, 1),
            (25, 4, 1),
            (25, 5, 1),
            (25, 6, 3),
            (25, 7, 3),
            (25, 8, 3),
            (42, 3, 4),
            (42, 4, 4),
            (42, 5, 4),
        ],
        Family::Yellow => &[(25, 1, 1), (25, 2, 3), (42, 1, 4), (42, 2, 4), (42, 3, 4)],
    }
}

/// The Cerulean Cave areas, by family (Japanese Blue counts Route 19 and 20 apart).
pub(super) const POST_GAME_RBG: [usize; 4] = [53, 54, 55, 67];
pub(super) const POST_GAME_JAPANESE_BLUE: [usize; 4] = [54, 55, 56, 68];
pub(super) const POST_GAME_YELLOW: [usize; 5] = [59, 60, 61, 94, 95];

/// The location tags of the encounter areas, in load order. Super Rod areas shared by several maps of Red and
/// Blue are numbered.
pub(super) fn location_tags(family: Family, japanese_blue: bool) -> Vec<&'static str> {
    let routes_rb: &[&str] = if japanese_blue {
        &["ROUTE 19", "ROUTE 20"]
    } else {
        &["ROUTE 19/20"]
    };
    let dungeons_rb: [&str; 33] = [
        "VIRIDIAN FOREST",
        "MT.MOON",
        "MT.MOON",
        "MT.MOON",
        "ROCK TUNNEL",
        "POWER PLANT",
        "VICTORY ROAD",
        "POKEMON TOWER",
        "POKEMON TOWER",
        "POKEMON TOWER",
        "POKEMON TOWER",
        "POKEMON TOWER",
        "SEAFOAM ISLANDS",
        "SEAFOAM ISLANDS",
        "SEAFOAM ISLANDS",
        "SEAFOAM ISLANDS",
        "POKEMON MANSION",
        "SEAFOAM ISLANDS",
        "VICTORY ROAD",
        "DIGLETT'S CAVE",
        "VICTORY ROAD",
        "POKEMON MANSION",
        "POKEMON MANSION",
        "POKEMON MANSION",
        "SAFARI ZONE",
        "SAFARI ZONE",
        "SAFARI ZONE",
        "SAFARI ZONE",
        "CERULEAN CAVE",
        "CERULEAN CAVE",
        "CERULEAN CAVE",
        "ROCK TUNNEL",
        "OLD ROD",
    ];
    let mut tags: Vec<&str> = Vec::new();
    match family {
        Family::RedBlue => {
            tags.extend((1..=18).map(route));
            tags.extend(routes_rb);
            tags.extend(["ROUTE 21", "ROUTE 21", "ROUTE 22", "ROUTE 23", "ROUTE 24", "ROUTE 25"]);
            tags.extend(dungeons_rb);
            tags.push("GOOD ROD");
            tags.extend(SUPER_RODS);
        }
        Family::Yellow => {
            tags.extend(YELLOW);
        }
    }
    tags
}

const SUPER_RODS: [&str; 10] = [
    "SUPER ROD 1",
    "SUPER ROD 2",
    "SUPER ROD 3",
    "SUPER ROD 4",
    "SUPER ROD 5",
    "SUPER ROD 6",
    "SUPER ROD 7",
    "SUPER ROD 8",
    "SUPER ROD 9",
    "SUPER ROD 10",
];

fn route(n: usize) -> &'static str {
    const ROUTES: [&str; 18] = [
        "ROUTE 1", "ROUTE 2", "ROUTE 3", "ROUTE 4", "ROUTE 5", "ROUTE 6", "ROUTE 7", "ROUTE 8", "ROUTE 9", "ROUTE 10",
        "ROUTE 11", "ROUTE 12", "ROUTE 13", "ROUTE 14", "ROUTE 15", "ROUTE 16", "ROUTE 17", "ROUTE 18",
    ];
    ROUTES[n - 1]
}

/// Yellow has a Super Rod area per map.
const YELLOW: [&str; 96] = [
    "ROUTE 1",
    "ROUTE 2",
    "ROUTE 3",
    "ROUTE 4",
    "ROUTE 5",
    "ROUTE 6",
    "ROUTE 6",
    "ROUTE 7",
    "ROUTE 8",
    "ROUTE 9",
    "ROUTE 10",
    "ROUTE 11",
    "ROUTE 12",
    "ROUTE 12",
    "ROUTE 13",
    "ROUTE 13",
    "ROUTE 14",
    "ROUTE 15",
    "ROUTE 16",
    "ROUTE 17",
    "ROUTE 18",
    "ROUTE 19",
    "ROUTE 20",
    "ROUTE 21",
    "ROUTE 21",
    "ROUTE 22",
    "ROUTE 23",
    "ROUTE 24",
    "ROUTE 25",
    "VIRIDIAN FOREST",
    "MT.MOON",
    "MT.MOON",
    "MT.MOON",
    "ROCK TUNNEL",
    "POWER PLANT",
    "VICTORY ROAD",
    "POKEMON TOWER",
    "POKEMON TOWER",
    "POKEMON TOWER",
    "POKEMON TOWER",
    "POKEMON TOWER",
    "SEAFOAM ISLANDS",
    "SEAFOAM ISLANDS",
    "SEAFOAM ISLANDS",
    "SEAFOAM ISLANDS",
    "SEAFOAM ISLANDS",
    "SEAFOAM ISLANDS",
    "POKEMON MANSION",
    "SEAFOAM ISLANDS",
    "VICTORY ROAD",
    "DIGLETT'S CAVE",
    "VICTORY ROAD",
    "POKEMON MANSION",
    "POKEMON MANSION",
    "POKEMON MANSION",
    "SAFARI ZONE",
    "SAFARI ZONE",
    "SAFARI ZONE",
    "SAFARI ZONE",
    "CERULEAN CAVE",
    "CERULEAN CAVE",
    "CERULEAN CAVE",
    "ROCK TUNNEL",
    "OLD ROD",
    "GOOD ROD",
    "PALLET TOWN",
    "VIRIDIAN CITY",
    "CERULEAN CITY",
    "VERMILION CITY",
    "CELADON CITY",
    "FUCHSIA CITY",
    "CINNABAR ISLAND",
    "ROUTE 4",
    "ROUTE 6",
    "ROUTE 24",
    "ROUTE 25",
    "ROUTE 10",
    "ROUTE 11",
    "ROUTE 12",
    "ROUTE 13",
    "ROUTE 17",
    "ROUTE 18",
    "ROUTE 19",
    "ROUTE 20",
    "ROUTE 21",
    "ROUTE 22",
    "ROUTE 23",
    "VERMILION CITY",
    "SAFARI ZONE",
    "SAFARI ZONE",
    "SAFARI ZONE",
    "SAFARI ZONE",
    "SEAFOAM ISLANDS",
    "SEAFOAM ISLANDS",
    "CERULEAN CAVE",
    "CERULEAN CAVE",
];

/// Move effect numbers.
pub(super) mod effect {
    pub(crate) const DAMAGE_POISON_20: u16 = 2;
    pub(crate) const DAMAGE_ABSORB: u16 = 3;
    pub(crate) const DAMAGE_BURN_10: u16 = 4;
    pub(crate) const DAMAGE_FREEZE_10: u16 = 5;
    pub(crate) const DAMAGE_PARALYZE_10: u16 = 6;
    pub(crate) const DREAM_EATER: u16 = 8;
    pub(crate) const NO_DAMAGE_ATK_PLUS_ONE: u16 = 10;
    pub(crate) const NO_DAMAGE_DEF_PLUS_ONE: u16 = 11;
    pub(crate) const NO_DAMAGE_SPECIAL_PLUS_ONE: u16 = 13;
    pub(crate) const NO_DAMAGE_EVASION_PLUS_ONE: u16 = 15;
    pub(crate) const NO_DAMAGE_ATK_MINUS_ONE: u16 = 18;
    pub(crate) const NO_DAMAGE_DEF_MINUS_ONE: u16 = 19;
    pub(crate) const NO_DAMAGE_SPE_MINUS_ONE: u16 = 20;
    pub(crate) const NO_DAMAGE_ACCURACY_MINUS_ONE: u16 = 22;
    pub(crate) const FLINCH_10: u16 = 31;
    pub(crate) const NO_DAMAGE_SLEEP: u16 = 32;
    pub(crate) const DAMAGE_POISON_40: u16 = 33;
    pub(crate) const DAMAGE_BURN_30: u16 = 34;
    pub(crate) const DAMAGE_FREEZE_30: u16 = 35;
    pub(crate) const DAMAGE_PARALYZE_30: u16 = 36;
    pub(crate) const FLINCH_30: u16 = 37;
    pub(crate) const CHARGE: u16 = 39;
    pub(crate) const FLY: u16 = 43;
    pub(crate) const DAMAGE_RECOIL: u16 = 48;
    pub(crate) const NO_DAMAGE_CONFUSION: u16 = 49;
    pub(crate) const NO_DAMAGE_ATK_PLUS_TWO: u16 = 50;
    pub(crate) const NO_DAMAGE_DEF_PLUS_TWO: u16 = 51;
    pub(crate) const NO_DAMAGE_SPE_PLUS_TWO: u16 = 52;
    pub(crate) const NO_DAMAGE_SPECIAL_PLUS_TWO: u16 = 53;
    pub(crate) const NO_DAMAGE_DEF_MINUS_TWO: u16 = 59;
    pub(crate) const NO_DAMAGE_POISON: u16 = 66;
    pub(crate) const NO_DAMAGE_PARALYZE: u16 = 67;
    pub(crate) const DAMAGE_ATK_MINUS_ONE: u16 = 68;
    pub(crate) const DAMAGE_DEF_MINUS_ONE: u16 = 69;
    pub(crate) const DAMAGE_SPE_MINUS_ONE: u16 = 70;
    pub(crate) const DAMAGE_SPECIAL_MINUS_ONE: u16 = 71;
    pub(crate) const DAMAGE_CONFUSION: u16 = 76;
    pub(crate) const TWINEEDLE: u16 = 77;
    pub(crate) const HYPER_BEAM: u16 = 80;
}
