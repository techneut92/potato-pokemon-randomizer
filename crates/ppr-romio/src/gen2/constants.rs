//! Constants of Gold, Silver and Crystal.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, HeldItemPools, ItemId, MoveId, Type};

/// `Type=` in the ROM entries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    GoldSilver,
    Crystal,
}

impl Family {
    pub(super) fn parse(s: &str) -> Option<Self> {
        if s.eq_ignore_ascii_case("GS") {
            Some(Self::GoldSilver)
        } else if s.eq_ignore_ascii_case("Crystal") {
            Some(Self::Crystal)
        } else {
            None
        }
    }
}

pub(super) const SPECIES_COUNT: usize = 251;
pub(super) const MOVE_COUNT: usize = 251;
pub(super) const UNOWN: DexNumber = 201;

pub(super) const BASE_STATS_ENTRY_SIZE: usize = 0x20;
pub(super) const BS_HP: usize = 1;
pub(super) const BS_ATTACK: usize = 2;
pub(super) const BS_DEFENSE: usize = 3;
pub(super) const BS_SPEED: usize = 4;
pub(super) const BS_SP_ATK: usize = 5;
pub(super) const BS_SP_DEF: usize = 6;
pub(super) const BS_PRIMARY_TYPE: usize = 7;
pub(super) const BS_SECONDARY_TYPE: usize = 8;
pub(super) const BS_CATCH_RATE: usize = 9;
pub(super) const BS_COMMON_HELD_ITEM: usize = 11;
pub(super) const BS_RARE_HELD_ITEM: usize = 12;
pub(super) const BS_EGG_CYCLES: usize = 15;
pub(super) const BS_GROWTH_CURVE: usize = 22;
pub(super) const BS_TM_HM_COMPAT: usize = 24;
/// The last byte of the TM and HM flags; Crystal keeps the move tutor flags in its bits 1 to 3.
pub(super) const BS_TUTOR_COMPAT: usize = 31;

pub(super) const TM_COUNT: usize = 50;
pub(super) const HM_COUNT: usize = 7;
pub(super) const TUTOR_COUNT: usize = 3;
pub(super) const ODD_EGG_COUNT: usize = 14;
/// The longest trainer name, with its class name, that fits on a line.
pub(super) const MAX_TRAINER_NAME_LENGTH: usize = 17;
pub(super) const TRAINER_DATA_TERMINATOR: u8 = 0xFF;
pub(super) const EGG_MOVES_TERMINATOR: u8 = 0xFF;
pub(super) const SHOP_ITEMS_TERMINATOR: u8 = 0xFF;
/// Starts a menu of choices in a script.
pub(super) const DIALOGUE_OPTION_START: u8 = 0x80;
/// The second and third sets of fleeing data, after the first.
pub(super) const FLEEING_SET_OFFSETS: [usize; 3] = [0, 0xE, 0x17];
pub(super) const MAP_GROUP_COUNT: usize = 26;
pub(super) const MAPS_IN_LAST_GROUP: usize = 11;
pub(super) const ITEM_ATTRIBUTES_ENTRY_SIZE: usize = 7;
/// Room for pairs of types that aren't neutral against each other.
pub(super) const TYPE_CHART_ROOM: usize = 110;
pub(super) const TYPE_CHART_END: u8 = 0xFF;
/// Between the type chart and the immunities of Ghost, which Foresight skips.
pub(super) const TYPE_CHART_FORESIGHT: u8 = 0xFE;
/// What comes one byte before the happiness that evolves a species.
pub(super) const HAPPINESS_FOR_EVOLUTION_PREFIX: [u8; 3] = [0xFE, 0xDC, 0xDA];
pub(super) const VANILLA_HAPPINESS_TO_EVOLVE: u8 = 220;
pub(super) const EASIER_HAPPINESS_TO_EVOLVE: u8 = 160;

pub(super) const DIZZY_PUNCH: MoveId = 146;
pub(super) const TRIPLE_KICK: MoveId = 167;
/// Karate Chop, Razor Wind, Razor Leaf, Crabhammer, Slash, Aeroblast and Cross Chop.
pub(super) const INCREASED_CRIT_MOVES: [MoveId; 7] = [2, 13, 75, 152, 163, 177, 238];
/// Sonic Boom, Dragon Rage and the one-hit KO moves, which X Accuracy still breaks.
pub(super) const GAME_BREAKING_MOVES: [MoveId; 5] = [49, 82, 32, 90, 12];
/// Transform, for the Transform assumption glitch, and Thief, whose stolen items trainers never give back.
pub(super) const BANNED_LEVEL_UP_MOVES: [MoveId; 2] = [144, 168];
/// Cut, Fly, Surf, Strength, Flash, Dig, Teleport, Whirlpool, Waterfall, Rock Smash, Headbutt and Sweet Scent.
pub(super) const FIELD_MOVES: [MoveId; 12] = [15, 19, 57, 70, 148, 91, 100, 250, 127, 249, 29, 230];
/// Cut.
pub(super) const EARLY_REQUIRED_HM_MOVES: [MoveId; 1] = [15];

/// Effect numbers of the moves.
pub(super) mod effect {
    pub(in crate::gen2) const NO_DAMAGE_SLEEP: u16 = 1;
    pub(in crate::gen2) const DAMAGE_POISON: u16 = 2;
    pub(in crate::gen2) const DAMAGE_ABSORB: u16 = 3;
    pub(in crate::gen2) const DAMAGE_BURN: u16 = 4;
    pub(in crate::gen2) const DAMAGE_FREEZE: u16 = 5;
    pub(in crate::gen2) const DAMAGE_PARALYZE: u16 = 6;
    pub(in crate::gen2) const DREAM_EATER: u16 = 8;
    pub(in crate::gen2) const NO_DAMAGE_ATK_PLUS_ONE: u16 = 10;
    pub(in crate::gen2) const NO_DAMAGE_DEF_PLUS_ONE: u16 = 11;
    pub(in crate::gen2) const NO_DAMAGE_SP_ATK_PLUS_ONE: u16 = 13;
    pub(in crate::gen2) const NO_DAMAGE_EVASION_PLUS_ONE: u16 = 16;
    pub(in crate::gen2) const NO_DAMAGE_ATK_MINUS_ONE: u16 = 18;
    pub(in crate::gen2) const NO_DAMAGE_DEF_MINUS_ONE: u16 = 19;
    pub(in crate::gen2) const NO_DAMAGE_SPE_MINUS_ONE: u16 = 20;
    pub(in crate::gen2) const NO_DAMAGE_ACCURACY_MINUS_ONE: u16 = 23;
    pub(in crate::gen2) const NO_DAMAGE_EVASION_MINUS_ONE: u16 = 24;
    pub(in crate::gen2) const FORCE_SWITCH: u16 = 0x1C;
    pub(in crate::gen2) const FLINCH: u16 = 31;
    pub(in crate::gen2) const TOXIC: u16 = 33;
    pub(in crate::gen2) const RAZOR_WIND: u16 = 39;
    pub(in crate::gen2) const BINDING: u16 = 42;
    pub(in crate::gen2) const DAMAGE_RECOIL: u16 = 48;
    pub(in crate::gen2) const NO_DAMAGE_CONFUSION: u16 = 49;
    pub(in crate::gen2) const NO_DAMAGE_ATK_PLUS_TWO: u16 = 50;
    pub(in crate::gen2) const NO_DAMAGE_DEF_PLUS_TWO: u16 = 51;
    pub(in crate::gen2) const NO_DAMAGE_SPE_PLUS_TWO: u16 = 52;
    pub(in crate::gen2) const NO_DAMAGE_SP_DEF_PLUS_TWO: u16 = 54;
    pub(in crate::gen2) const NO_DAMAGE_ATK_MINUS_TWO: u16 = 58;
    pub(in crate::gen2) const NO_DAMAGE_DEF_MINUS_TWO: u16 = 59;
    pub(in crate::gen2) const NO_DAMAGE_SPE_MINUS_TWO: u16 = 60;
    pub(in crate::gen2) const NO_DAMAGE_SP_DEF_MINUS_TWO: u16 = 62;
    pub(in crate::gen2) const NO_DAMAGE_POISON: u16 = 66;
    pub(in crate::gen2) const NO_DAMAGE_PARALYZE: u16 = 67;
    pub(in crate::gen2) const DAMAGE_ATK_MINUS_ONE: u16 = 68;
    pub(in crate::gen2) const DAMAGE_DEF_MINUS_ONE: u16 = 69;
    pub(in crate::gen2) const DAMAGE_SPE_MINUS_ONE: u16 = 70;
    pub(in crate::gen2) const DAMAGE_SP_DEF_MINUS_ONE: u16 = 72;
    pub(in crate::gen2) const DAMAGE_ACCURACY_MINUS_ONE: u16 = 73;
    pub(in crate::gen2) const SKY_ATTACK: u16 = 75;
    pub(in crate::gen2) const DAMAGE_CONFUSION: u16 = 76;
    pub(in crate::gen2) const TWINEEDLE: u16 = 77;
    pub(in crate::gen2) const HYPER_BEAM: u16 = 80;
    pub(in crate::gen2) const COUNTER: u16 = 0x59;
    pub(in crate::gen2) const SNORE: u16 = 92;
    pub(in crate::gen2) const FLAIL_AND_REVERSAL: u16 = 102;
    pub(in crate::gen2) const PRIORITY_HIT: u16 = 0x67;
    pub(in crate::gen2) const TRAPPING: u16 = 106;
    pub(in crate::gen2) const PROTECT: u16 = 0x6F;
    pub(in crate::gen2) const ENDURE: u16 = 0x74;
    pub(in crate::gen2) const SWAGGER: u16 = 118;
    pub(in crate::gen2) const DAMAGE_BURN_AND_THAW_USER: u16 = 125;
    pub(in crate::gen2) const DAMAGE_USER_DEF_PLUS_ONE: u16 = 138;
    pub(in crate::gen2) const DAMAGE_USER_ATK_PLUS_ONE: u16 = 139;
    pub(in crate::gen2) const DAMAGE_USER_ALL_PLUS_ONE: u16 = 140;
    pub(in crate::gen2) const SKULL_BASH: u16 = 145;
    pub(in crate::gen2) const TWISTER: u16 = 146;
    pub(in crate::gen2) const FUTURE_SIGHT: u16 = 148;
    pub(in crate::gen2) const MIRROR_COAT: u16 = 0x90;
    pub(in crate::gen2) const STOMP: u16 = 150;
    pub(in crate::gen2) const SOLARBEAM: u16 = 151;
    pub(in crate::gen2) const THUNDER: u16 = 152;
    pub(in crate::gen2) const SEMI_INVULNERABLE: u16 = 155;
    pub(in crate::gen2) const DEFENSE_CURL: u16 = 156;
}

/// The type byte → type; `None` for the "???" type of Curse.
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
        0x09 => Type::Steel,
        0x14 => Type::Fire,
        0x15 => Type::Water,
        0x16 => Type::Grass,
        0x17 => Type::Electric,
        0x18 => Type::Psychic,
        0x19 => Type::Ice,
        0x1A => Type::Dragon,
        0x1B => Type::Dark,
        _ => return None,
    })
}

/// The type byte of a type; `None` for a type Gen 2 doesn't have.
pub(super) fn type_to_byte(t: Type) -> Option<u8> {
    (0..=0x1Fu8).find(|&b| type_from_byte(b) == Some(t))
}

/// The byte of the "???" type.
pub(super) const UNKNOWN_TYPE: u8 = 0x13;

/// Items only Gen 2 has are numbered from here plus their internal index.
pub(super) const UNIQUE_OFFSET: ItemId = 1000;
pub(super) const TM01: ItemId = 328;
pub(super) const HM01: ItemId = 420;

/// Internal index → standard item number.
const STANDARD: [ItemId; 256] = [
    0, 1, 2, 213, 3, 4, 1006, 1007, 81, 18, 19, 20, 21, 22, 23, 24, 25, 26, 17, 78, 79, 41, 82, 83, 84, 1025, 45, 46,
    47, 48, 256, 49, 50, 60, 85, 257, 92, 63, 27, 28, 29, 55, 76, 77, 56, 1045, 30, 31, 32, 57, 1050, 58, 59, 61, 1054,
    1055, 1056, 216, 1058, 1059, 1060, 1061, 51, 38, 39, 40, 1066, 1067, 1068, 1069, 1070, 1071, 33, 217, 151, 1075,
    237, 244, 149, 153, 152, 245, 221, 156, 150, 485, 86, 87, 222, 486, 1090, 223, 487, 488, 224, 243, 248, 490, 241,
    491, 1100, 489, 240, 473, 1104, 259, 228, 246, 242, 157, 88, 89, 229, 247, 504, 1115, 1116, 239, 258, 230, 1120,
    34, 35, 36, 37, 238, 231, 1127, 1128, 1129, 1130, 90, 91, 1133, 1134, 1135, 1136, 1137, 249, 43, 232, 1141, 1142,
    233, 250, 1145, 234, 1147, 1148, 1149, 154, 235, 1152, 1153, 1154, 1155, 44, 495, 137, 493, 494, 492, 1162, 236,
    497, 498, 496, 1167, 1168, 80, 1170, 1171, 252, 155, 158, 1175, 1176, 500, 1178, 1179, 1180, 138, 139, 140, 141,
    142, 143, 144, 145, 146, 1190, 328, 329, 330, 331, 1195, 332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342,
    343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 1220, 356, 357, 358, 359, 360, 361, 362, 363, 364,
    365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 420, 421, 422, 423, 424, 425, 426, 1250, 1251,
    1252, 1253, 1254, 1255,
];

/// The standard number of an internal item index.
pub(super) fn item_standard(internal: u8) -> ItemId {
    STANDARD[usize::from(internal)]
}

/// The internal index of a standard item number; `None` for a number Gen 2 has no item for.
pub(super) fn item_internal(id: ItemId) -> Option<u8> {
    if id >= UNIQUE_OFFSET {
        return u8::try_from(id - UNIQUE_OFFSET).ok();
    }
    STANDARD
        .iter()
        .position(|&s| s == id)
        .and_then(|i| u8::try_from(i).ok())
}

const SILVER_LEAF: ItemId = 1060;
const GOLD_LEAF: ItemId = 1075;
const PINK_BOW: ItemId = 1104;
const BERSERK_GENE: ItemId = 1152;
const NORMAL_BOX: ItemId = 1167;
const GORGEOUS_BOX: ItemId = 1168;
const POLKADOT_BOW: ItemId = 1170;
const BRICK_PIECE: ItemId = 1180;

/// Most items only Gen 2 has (key items or unused, not the leaves, bows, boxes, Brick Piece and Berserk Gene), and
/// the HMs.
pub(super) fn item_banned(id: ItemId) -> bool {
    let kept = [
        SILVER_LEAF,
        GOLD_LEAF,
        BRICK_PIECE,
        BERSERK_GENE,
        PINK_BOW,
        POLKADOT_BOW,
        NORMAL_BOX,
        GORGEOUS_BOX,
    ];
    (1006..=1255).contains(&id) && !kept.contains(&id) || (HM01..HM01 + 7).contains(&id)
}

/// Light Ball, Berry, the leaves, Brick Piece, the boxes, the items for one species, the Apricorns and the mail.
pub(super) fn item_bad(id: ItemId) -> bool {
    [236, 155, SILVER_LEAF, GOLD_LEAF, BRICK_PIECE, NORMAL_BOX, GORGEOUS_BOX].contains(&id)
        || (256..=259).contains(&id)
        || (485..=491).contains(&id)
        || (137..=146).contains(&id)
}

/// The TMs found in the field that are needed.
pub(super) const REQUIRED_FIELD_TMS: [ItemId; 12] = [331, 347, 349, 353, 355, 361, 362, 366, 367, 370, 371, 373];
/// The evolution stones: Sun, Moon, Fire, Thunder, Water and Leaf.
pub(super) const EVOLUTION_ITEMS: [ItemId; 6] = [80, 81, 82, 83, 84, 85];
/// Rare Candy, Lucky Egg and the items only worth money.
pub(super) const OP_SHOP_ITEMS: [ItemId; 11] = [50, 231, 92, 86, 87, 88, 89, 90, BRICK_PIECE, SILVER_LEAF, GOLD_LEAF];

/// Better prices for items whose price does not fit their use.
pub(super) const BALANCED_PRICES: [(ItemId, u32); 66] = [
    (213, 3000),
    (216, 6000),
    (217, 4500),
    (221, 5000),
    (223, 15000),
    (228, 1200),
    (229, 200),
    (230, 3000),
    (234, 10000),
    (224, 1000),
    (231, 10000),
    (232, 5000),
    (BERSERK_GENE, 800),
    (237, 2000),
    (244, 2000),
    (245, 2000),
    (222, 2000),
    (243, 2000),
    (248, 2000),
    (241, 2000),
    (240, 2000),
    (PINK_BOW, 2000),
    (POLKADOT_BOW, 2000),
    (246, 2000),
    (242, 2000),
    (247, 2000),
    (239, 2000),
    (238, 2000),
    (249, 2000),
    (233, 2000),
    (235, 2000),
    (256, 1200),
    (257, 1200),
    (259, 1200),
    (258, 2300),
    (236, 2300),
    (155, 50),
    (158, 500),
    (151, 100),
    (149, 200),
    (153, 250),
    (152, 250),
    (156, 200),
    (150, 250),
    (157, 500),
    (154, 3000),
    (43, 300),
    (1, 3000),
    (500, 600),
    (495, 300),
    (493, 300),
    (494, 300),
    (492, 300),
    (497, 300),
    (498, 300),
    (496, 300),
    (38, 3000),
    (39, 4500),
    (40, 15000),
    (41, 18000),
    (81, 2100),
    (50, 10000),
    (44, 10000),
    (250, 100),
    (NORMAL_BOX, 1000),
    (GORGEOUS_BOX, 1000),
];

/// The items trainer Pokémon may hold.
pub(super) fn held_item_pools() -> HeldItemPools {
    let general_consumable = vec![151, 149, 153, 152, 156, 150, 157, 154, 155, 158, 43];
    let mut consumable = general_consumable.clone();
    consumable.push(BERSERK_GENE);
    let general = vec![213, 217, 221, 228];
    let mut all = general.clone();
    all.extend([
        241,
        240,
        249,
        235,
        238,
        242,
        233,
        239,
        243,
        246,
        PINK_BOW,
        POLKADOT_BOW,
        244,
        222,
        237,
        247,
        248,
    ]);
    all.extend(&consumable);
    let type_boosting: BTreeMap<Type, Vec<ItemId>> = [
        (Type::Bug, vec![222]),
        (Type::Dark, vec![240]),
        // Not the Dragon Fang, which does nothing in Gen 2.
        (Type::Dragon, vec![235]),
        (Type::Electric, vec![242]),
        (Type::Fighting, vec![241]),
        (Type::Fire, vec![249]),
        (Type::Flying, vec![244]),
        (Type::Ghost, vec![247]),
        (Type::Grass, vec![239]),
        (Type::Ground, vec![237]),
        (Type::Ice, vec![246]),
        (Type::Normal, vec![PINK_BOW, POLKADOT_BOW]),
        (Type::Poison, vec![245]),
        (Type::Psychic, vec![248]),
        (Type::Rock, vec![238]),
        (Type::Steel, vec![233]),
        (Type::Water, vec![243]),
    ]
    .into_iter()
    .collect();
    // Pikachu, Chansey, Ditto, Cubone, Marowak and Farfetch'd.
    let species_boosting: BTreeMap<DexNumber, Vec<ItemId>> = [
        (25, vec![236]),
        (113, vec![256]),
        (132, vec![257]),
        (104, vec![258]),
        (105, vec![258]),
        (83, vec![259]),
    ]
    .into_iter()
    .collect();
    HeldItemPools {
        all,
        consumable,
        general_consumable,
        general,
        type_boosting,
        species_boosting,
        ..HeldItemPools::default()
    }
}

/// The shops, by the order of the shop table, when the ROM entry names none.
pub(super) const SHOP_NAMES: [&str; 34] = [
    "Cherrygrove Poké Mart (Before Pokédex)",
    "Cherrygrove Poké Mart (After Pokédex)",
    "Violet Poké Mart",
    "Azalea Poké Mart",
    "Cianwood Poké Mart",
    "Goldenrod Department Store 2F Upper",
    "Goldenrod Department Store 2F Lower",
    "Goldenrod Department Store 3F",
    "Goldenrod Department Store 4F",
    "Goldenrod Department Store 5F (neither TM02 nor TM08 unlocked)",
    "Goldenrod Department Store 5F (TM02 unlocked)",
    "Goldenrod Department Store 5F (TM08 unlocked)",
    "Goldenrod Department Store 5F (TM02 and TM08 unlocked)",
    "Olivine Poké Mart",
    "Ecruteak Poké Mart",
    "Mahogany Souvenir Shop (Before Team Rocket HQ)",
    "Mahogany Souvenir Shop (After Team Rocket HQ)",
    "Blackthorn Poké Mart",
    "Viridian Poké Mart",
    "Pewter Poké Mart",
    "Cerulean Poké Mart",
    "Lavender Poké Mart",
    "Vermilion Poké Mart",
    "Celadon Department Store 2F Left",
    "Celadon Department Store 2F Right",
    "Celadon Department Store 3F",
    "Celadon Department Store 4F",
    "Celadon Department Store 5F Left",
    "Celadon Department Store 5F Right",
    "Fuchsia Poké Mart",
    "Saffron Poké Mart",
    "Mt. Moon Square Shop",
    "Indigo Plateau Poké Mart",
    "Goldenrod Tunnel Herb Shop",
];
/// The shops that sell more than the usual goods, when the ROM entry names none.
pub(super) const SPECIAL_SHOPS: [usize; 7] = [7, 8, 26, 27, 28, 31, 33];
/// The shops reached before the Hall of Fame, when the ROM entry names none.
pub(super) const MAIN_GAME_SHOPS: [usize; 20] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 32, 33];

/// The type themes of the gyms and the Elite Four.
pub(super) const GYM_AND_ELITE_THEMES: [(&str, Type); 20] = [
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

/// Tags of trainers by class (from 1) and number within the class (from 0), in every game. The rival's teams are
/// stored in the order Bayleef, Quilava, Croconaw, so the tags follow the order of the starters.
pub(super) const TRAINER_TAGS: [(u8, usize, &str); 56] = [
    (1, 0, "GYM1-LEADER"),
    (3, 0, "GYM2-LEADER"),
    (2, 0, "GYM3-LEADER"),
    (4, 0, "GYM4-LEADER"),
    (7, 0, "GYM5-LEADER"),
    (6, 0, "GYM6-LEADER"),
    (5, 0, "GYM7-LEADER"),
    (8, 0, "GYM8-LEADER"),
    (17, 0, "GYM9-LEADER"),
    (18, 0, "GYM10-LEADER"),
    (19, 0, "GYM11-LEADER"),
    (21, 0, "GYM12-LEADER"),
    (26, 0, "GYM13-LEADER"),
    (35, 0, "GYM14-LEADER"),
    (46, 0, "GYM15-LEADER"),
    (64, 0, "GYM16-LEADER"),
    (11, 0, "ELITE1"),
    (15, 0, "ELITE2"),
    (13, 0, "ELITE3"),
    (14, 0, "ELITE4"),
    (16, 0, "CHAMPION"),
    (63, 0, "UBER"),
    (9, 0, "RIVAL1-1"),
    (9, 1, "RIVAL1-2"),
    (9, 2, "RIVAL1-0"),
    (9, 3, "RIVAL2-1"),
    (9, 4, "RIVAL2-2"),
    (9, 5, "RIVAL2-0"),
    (9, 6, "RIVAL3-1"),
    (9, 7, "RIVAL3-2"),
    (9, 8, "RIVAL3-0"),
    (9, 9, "RIVAL4-1"),
    (9, 10, "RIVAL4-2"),
    (9, 11, "RIVAL4-0"),
    (9, 12, "RIVAL5-1"),
    (9, 13, "RIVAL5-2"),
    (9, 14, "RIVAL5-0"),
    (42, 0, "RIVAL6-1"),
    (42, 1, "RIVAL6-2"),
    (42, 2, "RIVAL6-0"),
    (42, 3, "RIVAL7-1"),
    (42, 4, "RIVAL7-2"),
    (42, 5, "RIVAL7-0"),
    (55, 0, "THEMED:ARIANA"),
    (55, 1, "THEMED:ARIANA"),
    (51, 2, "THEMED:PETREL"),
    (51, 3, "THEMED:PETREL"),
    (51, 1, "THEMED:PROTON"),
    (31, 0, "THEMED:PROTON"),
    (56, 0, "THEMED:SPROUTTOWER"),
    (56, 1, "THEMED:SPROUTTOWER"),
    (56, 2, "THEMED:SPROUTTOWER"),
    (56, 3, "THEMED:SPROUTTOWER"),
    (56, 6, "THEMED:SPROUTTOWER"),
    (56, 7, "THEMED:SPROUTTOWER"),
    (56, 8, "THEMED:SPROUTTOWER"),
];

/// Tags of the gym trainers, the same in Gold, Silver and Crystal.
pub(super) const GYM_TRAINER_TAGS: [(u8, usize, &str); 49] = [
    (24, 0, "GYM1"),
    (24, 1, "GYM1"),
    (36, 4, "GYM2"),
    (36, 5, "GYM2"),
    (36, 6, "GYM2"),
    (61, 0, "GYM2"),
    (61, 3, "GYM2"),
    (25, 0, "GYM3"),
    (25, 1, "GYM3"),
    (29, 0, "GYM3"),
    (29, 1, "GYM3"),
    (56, 4, "GYM4"),
    (56, 5, "GYM4"),
    (57, 0, "GYM4"),
    (57, 1, "GYM4"),
    (50, 1, "GYM5"),
    (50, 3, "GYM5"),
    (50, 4, "GYM5"),
    (50, 6, "GYM5"),
    (58, 0, "GYM7"),
    (58, 1, "GYM7"),
    (58, 2, "GYM7"),
    (33, 0, "GYM7"),
    (33, 1, "GYM7"),
    (27, 2, "GYM8"),
    (27, 4, "GYM8"),
    (27, 3, "GYM8"),
    (28, 2, "GYM8"),
    (28, 3, "GYM8"),
    (54, 17, "GYM9"),
    (38, 20, "GYM10"),
    (39, 17, "GYM10"),
    (39, 18, "GYM10"),
    (49, 2, "GYM11"),
    (43, 1, "GYM11"),
    (32, 2, "GYM11"),
    (61, 4, "GYM12"),
    (61, 5, "GYM12"),
    (25, 8, "GYM12"),
    (53, 18, "GYM12"),
    (29, 13, "GYM12"),
    (25, 2, "GYM13"),
    (25, 5, "GYM13"),
    (53, 4, "GYM13"),
    (54, 4, "GYM13"),
    (57, 5, "GYM14"),
    (57, 6, "GYM14"),
    (52, 1, "GYM14"),
    (52, 10, "GYM14"),
];

/// The location tags of the Gold and Silver encounter sets, in their order.
pub(super) const TAGS_GS: [&str; 176] = [
    "SPROUT TOWER",
    "SPROUT TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "BURNED TOWER",
    "BURNED TOWER",
    "NATIONAL PARK",
    "RUINS OF ALPH",
    "RUINS OF ALPH",
    "UNION CAVE",
    "UNION CAVE",
    "UNION CAVE",
    "SLOWPOKE WELL",
    "SLOWPOKE WELL",
    "ILEX FOREST",
    "MT.MORTAR",
    "MT.MORTAR",
    "MT.MORTAR",
    "MT.MORTAR",
    "ICE PATH",
    "ICE PATH",
    "ICE PATH",
    "ICE PATH",
    "ICE PATH",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "SILVER CAVE",
    "SILVER CAVE",
    "SILVER CAVE",
    "SILVER CAVE",
    "DARK CAVE",
    "DARK CAVE",
    "ROUTE 29",
    "ROUTE 30",
    "ROUTE 31",
    "ROUTE 32",
    "ROUTE 33",
    "ROUTE 34",
    "ROUTE 35",
    "ROUTE 36",
    "ROUTE 37",
    "ROUTE 38",
    "ROUTE 39",
    "ROUTE 42",
    "ROUTE 43",
    "ROUTE 44",
    "ROUTE 45",
    "ROUTE 46",
    "SILVER CAVE",
    "RUINS OF ALPH",
    "UNION CAVE",
    "UNION CAVE",
    "UNION CAVE",
    "SLOWPOKE WELL",
    "SLOWPOKE WELL",
    "ILEX FOREST",
    "MT.MORTAR",
    "MT.MORTAR",
    "MT.MORTAR",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "SILVER CAVE",
    "DARK CAVE",
    "DARK CAVE",
    "DRAGON'S DEN",
    "ROUTE 30",
    "ROUTE 31",
    "ROUTE 32",
    "ROUTE 34",
    "ROUTE 35",
    "ROUTE 40",
    "ROUTE 41",
    "ROUTE 42",
    "ROUTE 43",
    "ROUTE 44",
    "ROUTE 45",
    "NEW BARK TOWN",
    "CHERRYGROVE CITY",
    "VIOLET CITY",
    "CIANWOOD CITY",
    "OLIVINE CITY",
    "ECRUTEAK CITY",
    "LAKE OF RAGE",
    "BLACKTHORN CITY",
    "SILVER CAVE",
    "OLIVINE CITY",
    "DIGLETT'S CAVE",
    "MT.MOON",
    "ROCK TUNNEL",
    "ROCK TUNNEL",
    "VICTORY ROAD",
    "TOHJO FALLS",
    "ROUTE 1",
    "ROUTE 2",
    "ROUTE 3",
    "ROUTE 4",
    "ROUTE 5",
    "ROUTE 6",
    "ROUTE 7",
    "ROUTE 8",
    "ROUTE 9",
    "ROUTE 10",
    "ROUTE 11",
    "ROUTE 13",
    "ROUTE 14",
    "ROUTE 15",
    "ROUTE 16",
    "ROUTE 17",
    "ROUTE 18",
    "ROUTE 21",
    "ROUTE 22",
    "ROUTE 24",
    "ROUTE 25",
    "ROUTE 26",
    "ROUTE 27",
    "ROUTE 28",
    "ROUTE 4",
    "ROUTE 6",
    "ROUTE 9",
    "ROUTE 10",
    "ROUTE 12",
    "ROUTE 13",
    "ROUTE 19",
    "ROUTE 20",
    "ROUTE 21",
    "ROUTE 22",
    "ROUTE 24",
    "ROUTE 25",
    "ROUTE 26",
    "ROUTE 27",
    "TOHJO FALLS",
    "ROUTE 28",
    "PALLET TOWN",
    "VIRIDIAN CITY",
    "CERULEAN CITY",
    "VERMILION CITY",
    "CELADON CITY",
    "FUCHSIA CITY",
    "CINNABAR ISLAND",
    "VERMILION CITY",
    "ROUTE 35",
    "ROUTE 38",
    "DARK CAVE",
    "MT.MORTAR",
    "MT.MORTAR",
    "FISHING SHORE",
    "FISHING OCEAN",
    "FISHING LAKE",
    "FISHING POND",
    "FISHING DRATINI 1",
    "FISHING QWILFISH",
    "FISHING REMORAID",
    "FISHING GYARADOS",
    "FISHING DRATINI 2",
    "FISHING WHIRL ISLANDS",
    "FISHING QWILFISH",
    "FISHING REMORAID",
    "HEADBUTT FOREST GS",
    "HEADBUTT FOREST GS",
    "HEADBUTT CANYON GS",
    "HEADBUTT CANYON GS",
    "ROCK SMASH",
    "BUG CATCHING CONTEST",
];

/// The location tags of the Crystal encounter sets, in their order.
pub(super) const TAGS_CRYSTAL: [&str; 181] = [
    "SPROUT TOWER",
    "SPROUT TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "TIN TOWER",
    "BURNED TOWER",
    "BURNED TOWER",
    "NATIONAL PARK",
    "RUINS OF ALPH",
    "RUINS OF ALPH",
    "UNION CAVE",
    "UNION CAVE",
    "UNION CAVE",
    "SLOWPOKE WELL",
    "SLOWPOKE WELL",
    "ILEX FOREST",
    "MT.MORTAR",
    "MT.MORTAR",
    "MT.MORTAR",
    "MT.MORTAR",
    "ICE PATH",
    "ICE PATH",
    "ICE PATH",
    "ICE PATH",
    "ICE PATH",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "SILVER CAVE",
    "SILVER CAVE",
    "SILVER CAVE",
    "SILVER CAVE",
    "DARK CAVE",
    "DARK CAVE",
    "ROUTE 29",
    "ROUTE 30",
    "ROUTE 31",
    "ROUTE 32",
    "ROUTE 33",
    "ROUTE 34",
    "ROUTE 35",
    "ROUTE 36",
    "ROUTE 37",
    "ROUTE 38",
    "ROUTE 39",
    "ROUTE 42",
    "ROUTE 43",
    "ROUTE 44",
    "ROUTE 45",
    "ROUTE 46",
    "SILVER CAVE",
    "RUINS OF ALPH",
    "UNION CAVE",
    "UNION CAVE",
    "UNION CAVE",
    "SLOWPOKE WELL",
    "SLOWPOKE WELL",
    "ILEX FOREST",
    "MT.MORTAR",
    "MT.MORTAR",
    "MT.MORTAR",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "WHIRL ISLANDS",
    "SILVER CAVE",
    "DARK CAVE",
    "DARK CAVE",
    "DRAGON'S DEN",
    "OLIVINE CITY",
    "ROUTE 30",
    "ROUTE 31",
    "ROUTE 32",
    "ROUTE 34",
    "ROUTE 35",
    "ROUTE 40",
    "ROUTE 41",
    "ROUTE 42",
    "ROUTE 43",
    "ROUTE 44",
    "ROUTE 45",
    "NEW BARK TOWN",
    "CHERRYGROVE CITY",
    "VIOLET CITY",
    "CIANWOOD CITY",
    "OLIVINE CITY",
    "ECRUTEAK CITY",
    "LAKE OF RAGE",
    "BLACKTHORN CITY",
    "SILVER CAVE",
    "DIGLETT'S CAVE",
    "MT.MOON",
    "ROCK TUNNEL",
    "ROCK TUNNEL",
    "VICTORY ROAD",
    "TOHJO FALLS",
    "ROUTE 1",
    "ROUTE 2",
    "ROUTE 3",
    "ROUTE 4",
    "ROUTE 5",
    "ROUTE 6",
    "ROUTE 7",
    "ROUTE 8",
    "ROUTE 9",
    "ROUTE 10",
    "ROUTE 11",
    "ROUTE 13",
    "ROUTE 14",
    "ROUTE 15",
    "ROUTE 16",
    "ROUTE 17",
    "ROUTE 18",
    "ROUTE 21",
    "ROUTE 22",
    "ROUTE 24",
    "ROUTE 25",
    "ROUTE 26",
    "ROUTE 27",
    "ROUTE 28",
    "TOHJO FALLS",
    "VERMILION CITY",
    "ROUTE 4",
    "ROUTE 6",
    "ROUTE 9",
    "ROUTE 10",
    "ROUTE 12",
    "ROUTE 13",
    "ROUTE 19",
    "ROUTE 20",
    "ROUTE 21",
    "ROUTE 22",
    "ROUTE 24",
    "ROUTE 25",
    "ROUTE 26",
    "ROUTE 27",
    "ROUTE 28",
    "PALLET TOWN",
    "VIRIDIAN CITY",
    "CERULEAN CITY",
    "VERMILION CITY",
    "CELADON CITY",
    "FUCHSIA CITY",
    "CINNABAR ISLAND",
    "DARK CAVE",
    "ROUTE 35",
    "FISHING SHORE",
    "FISHING OCEAN",
    "FISHING LAKE",
    "FISHING POND",
    "FISHING DRATINI 1",
    "FISHING QWILFISH",
    "FISHING REMORAID",
    "FISHING GYARADOS",
    "FISHING DRATINI 2",
    "FISHING WHIRL ISLANDS",
    "FISHING QWILFISH",
    "FISHING REMORAID",
    "HEADBUTT CANYON C",
    "HEADBUTT CANYON C",
    "HEADBUTT TOWN",
    "HEADBUTT TOWN",
    "HEADBUTT ROUTE",
    "HEADBUTT ROUTE",
    "HEADBUTT KANTO",
    "HEADBUTT KANTO",
    "HEADBUTT LAKE",
    "HEADBUTT LAKE",
    "HEADBUTT FOREST C",
    "HEADBUTT FOREST C",
    "ROCK SMASH",
    "BUG CATCHING CONTEST",
];

/// The post-game encounter sets of Gold and Silver.
pub(super) const POST_GAME_GS: [usize; 54] = [
    145, 146, 147, 148, 152, 149, 150, 105, 106, 107, 108, 129, 109, 110, 130, 111, 112, 113, 131, 114, 132, 115, 133,
    116, 134, 117, 118, 119, 120, 121, 135, 136, 122, 137, 123, 138, 124, 139, 125, 140, 151, 99, 101, 102, 100, 128,
    144, 38, 39, 40, 41, 60, 74, 97,
];

/// The post-game encounter sets of Gold and Silver, by time of day.
pub(super) const POST_GAME_GS_TIMED: [usize; 116] = [
    327, 328, 329, 330, 334, 331, 332, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 311, 251, 252, 253,
    254, 255, 256, 312, 257, 258, 259, 260, 261, 262, 263, 264, 265, 313, 266, 267, 268, 314, 269, 270, 271, 315, 272,
    273, 274, 316, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 317, 318, 290, 291, 292,
    319, 293, 294, 295, 320, 296, 297, 298, 321, 299, 300, 301, 322, 333, 221, 222, 223, 227, 228, 229, 230, 231, 232,
    224, 225, 226, 308, 309, 310, 326, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 180, 181, 182, 196,
    219,
];

/// The post-game encounter sets of Crystal.
pub(super) const POST_GAME_CRYSTAL: [usize; 54] = [
    146, 147, 148, 130, 149, 150, 151, 105, 106, 107, 108, 131, 109, 110, 132, 111, 112, 113, 133, 114, 134, 115, 135,
    116, 136, 117, 118, 119, 120, 121, 137, 138, 122, 139, 123, 140, 124, 141, 125, 142, 152, 99, 101, 102, 100, 128,
    145, 38, 39, 40, 41, 60, 74, 98,
];

/// The post-game encounter sets of Crystal, by time of day.
pub(super) const POST_GAME_CRYSTAL_TIMED: [usize; 116] = [
    328, 329, 330, 312, 331, 332, 333, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 313, 251, 252, 253,
    254, 255, 256, 314, 257, 258, 259, 260, 261, 262, 263, 264, 265, 315, 266, 267, 268, 316, 269, 270, 271, 317, 272,
    273, 274, 318, 275, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286, 287, 288, 289, 319, 320, 290, 291, 292,
    321, 293, 294, 295, 322, 296, 297, 298, 323, 299, 300, 301, 324, 334, 221, 222, 223, 227, 228, 229, 230, 231, 232,
    224, 225, 226, 308, 309, 310, 327, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 180, 181, 182, 196,
    220,
];

/// The fishing sets of Gold and Silver whose Super Rod slots are post-game.
pub(super) const PARTLY_POST_GAME_GS: [usize; 12] = [158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169];

/// The fishing sets of Gold and Silver, time-specific ones included, whose Super Rod slots are post-game.
pub(super) const PARTLY_POST_GAME_GS_TIMED: [usize; 23] = [
    348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370,
];

/// The fishing sets of Crystal whose Super Rod slots are post-game.
pub(super) const PARTLY_POST_GAME_CRYSTAL: [usize; 12] = [155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166];

/// The fishing sets of Crystal, time-specific ones included, whose Super Rod slots are post-game.
pub(super) const PARTLY_POST_GAME_CRYSTAL_TIMED: [usize; 23] = [
    341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358, 359, 360, 361, 362, 363,
];

/// The first post-game slot of a fishing set: the Super Rod's.
pub(super) const SUPER_ROD_FROM: usize = 7;
/// The first post-game slot of a fishing set by time of day, and of a time-specific one.
pub(super) const SUPER_ROD_FROM_TIMED: [usize; 2] = [6, 2];
/// The location tags of the time-specific fishing sets.
pub(super) const TIME_SPECIFIC_FISHING_TAGS: [&str; 11] = [
    "FISHING SHORE",
    "FISHING OCEAN",
    "FISHING LAKE",
    "FISHING POND",
    "FISHING DRATINI 1",
    "FISHING QWILFISH",
    "FISHING REMORAID",
    "FISHING GYARADOS",
    "FISHING DRATINI 2",
    "FISHING WHIRL ISLANDS",
    "FISHING QWILFISH",
];

pub(super) const LAND_SLOTS: usize = 7;
pub(super) const SEA_SLOTS: usize = 3;
pub(super) const FISHING_AREA_COUNT: usize = 12;
pub(super) const FISHING_SLOTS: usize = 11;
pub(super) const FISHING_SLOT_SIZE: usize = 3;
pub(super) const TIME_SPECIFIC_FISHING_AREA_COUNT: usize = 11;
pub(super) const TIME_SPECIFIC_FISHING_SLOTS: usize = 4;
/// The fishing areas Crystal never uses.
pub(super) const CRYSTAL_UNUSED_FISHING_AREAS: [usize; 2] = [6, 11];
pub(super) const FISHING_AREA_NAMES: [&str; 12] = [
    "Shore",
    "Ocean",
    "Lake",
    "Pond",
    "Dratini 1 (Ice Path, Dragon's Den)",
    "Qwilfish Swarm (Route 32)",
    "Remoraid Swarm (Route 44)",
    "Gyarados (Lake of Rage, Fuchsia City)",
    "Dratini 2 (Route 45)",
    "Whirl Islands",
    "Qwilfish (Routes 32, 12, 13)",
    "Remoraid (Route 44)",
];
pub(super) const HEADBUTT_AREA_NAMES_GS: [&str; 5] = [
    "Headbutt (Routes 34-39, Azalea Town, Ilex Forest, Routes 26-27) (common)",
    "Headbutt (Routes 34-39, Azalea Town, Ilex Forest, Routes 26-27) (rare)",
    "Headbutt (Routes 29-33, 42-46) (common)",
    "Headbutt (Routes 29-33, 42-46) (rare)",
    "Rock Smash",
];
pub(super) const HEADBUTT_AREA_NAMES_CRYSTAL: [&str; 13] = [
    "Headbutt (Routes 44, 45, 46) (common)",
    "Headbutt (Routes 44, 45, 46) (rare)",
    "Headbutt (Azalea Town, Route 42) (common)",
    "Headbutt (Azalea Town, Route 42) (rare)",
    "Headbutt (Routes 29-31, 34-39) (common)",
    "Headbutt (Routes 29-31, 34-39) (rare)",
    "Headbutt (Routes 32, 26, 27) (common)",
    "Headbutt (Routes 32, 26, 27) (rare)",
    "Headbutt (Route 43) (common)",
    "Headbutt (Route 43) (rare)",
    "Headbutt (Ilex Forest) (common)",
    "Headbutt (Ilex Forest) (rare)",
    "Rock Smash",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_numbers_round_trip() {
        for internal in 1..=255u8 {
            assert_eq!(item_internal(item_standard(internal)), Some(internal), "{internal}");
        }
        assert_eq!(item_standard(191), TM01);
        assert_eq!(item_standard(243), HM01);
        assert!(item_banned(HM01) && item_banned(1058) && !item_banned(PINK_BOW) && !item_banned(1));
        assert_eq!(type_to_byte(Type::Dark), Some(0x1B));
        assert_eq!(type_from_byte(UNKNOWN_TYPE), None);
    }
}
