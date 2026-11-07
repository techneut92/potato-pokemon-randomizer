//! Fixed data of the Gen 6 games: sizes and layouts, code patterns to find tables by, item and move lists.

use ppr_model::{DexNumber, EvolutionType, HeldItemPools, ItemId, MoveId, Type};

/// The two game families.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// X and Y.
    Xy,
    /// Omega Ruby and Alpha Sapphire.
    Oras,
}

impl Family {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "XY" => Some(Self::Xy),
            "ORAS" => Some(Self::Oras),
            _ => None,
        }
    }

    /// The size of one species' personal data.
    pub(crate) fn personal_size(self) -> usize {
        match self {
            Self::Xy => 0x40,
            Self::Oras => 0x50,
        }
    }

    /// The number of alternate formes with personal data of their own.
    pub(crate) fn forme_count(self) -> usize {
        match self {
            Self::Xy => 77,
            Self::Oras => 104,
        }
    }

    pub(crate) fn move_count(self) -> usize {
        match self {
            Self::Xy => 617,
            Self::Oras => 621,
        }
    }

    pub(crate) fn hm_count(self) -> usize {
        match self {
            Self::Xy => 5,
            Self::Oras => 7,
        }
    }

    /// Where TM93 starts, in words after the start of the TM table.
    pub(crate) fn tm_block_two_start(self) -> usize {
        match self {
            Self::Xy => 97,
            Self::Oras => 98,
        }
    }

    pub(crate) fn static_count(self) -> usize {
        match self {
            Self::Xy => 0xD,
            Self::Oras => 0x3B,
        }
    }

    pub(crate) fn gift_count(self) -> usize {
        match self {
            Self::Xy => 0x13,
            Self::Oras => 0x25,
        }
    }

    pub(crate) fn gift_size(self) -> usize {
        match self {
            Self::Xy => 0x18,
            Self::Oras => 0x24,
        }
    }

    pub(crate) fn shop_items_locator(self) -> &'static str {
        match self {
            Self::Xy => "0400110004000300",
            Self::Oras => "04001100120004000300",
        }
    }

    pub(crate) fn trades_prefix(self) -> &'static str {
        match self {
            Self::Xy => "BA0A02015E000100BC0A150069000100",
            Self::Oras => "810B7A0097000A00000047006B000A00",
        }
    }

    pub(crate) fn forme_reversion_prefix(self) -> &'static str {
        match self {
            Self::Xy => "09EB000094E5141094E54A0B80E2",
            Self::Oras => "09EB000094E5141094E5120A80E2",
        }
    }

    /// The highest ability number: Aura Break in X and Y, Delta Stream in Omega Ruby and Alpha Sapphire.
    pub(crate) fn highest_ability(self) -> usize {
        match self {
            Self::Xy => 188,
            Self::Oras => 191,
        }
    }

    /// The Pokédex area data per species.
    pub(crate) fn area_data_length(self) -> usize {
        match self {
            Self::Xy => 0xE8,
            Self::Oras => 0x2A0,
        }
    }
}

/// The national Pokédex size.
pub(crate) const SPECIES_COUNT: usize = 721;

/// Types by their number in the game.
pub(crate) const TYPES: [Type; 18] = Type::ALL;

/// The game's number of a type.
pub(crate) fn type_byte(t: Type) -> u8 {
    u8::try_from(TYPES.iter().position(|&x| x == t).unwrap_or(0)).unwrap_or(0)
}

/// The type a byte names; bytes past the table read as Normal.
pub(crate) fn type_of(b: u8) -> Option<Type> {
    TYPES.get(usize::from(b)).copied()
}

// Offsets into a species' personal data.
pub(crate) const BS_HP: usize = 0;
pub(crate) const BS_ATTACK: usize = 1;
pub(crate) const BS_DEFENSE: usize = 2;
pub(crate) const BS_SPEED: usize = 3;
pub(crate) const BS_SP_ATK: usize = 4;
pub(crate) const BS_SP_DEF: usize = 5;
pub(crate) const BS_TYPE1: usize = 6;
pub(crate) const BS_TYPE2: usize = 7;
pub(crate) const BS_CATCH_RATE: usize = 8;
pub(crate) const BS_EV_YIELD: usize = 10;
pub(crate) const BS_COMMON_ITEM: usize = 12;
pub(crate) const BS_RARE_ITEM: usize = 14;
pub(crate) const BS_GENDER: usize = 18;
pub(crate) const BS_EGG_CYCLES: usize = 19;
pub(crate) const BS_GROWTH: usize = 21;
pub(crate) const BS_ABILITY1: usize = 24;
pub(crate) const BS_ABILITY2: usize = 25;
pub(crate) const BS_ABILITY3: usize = 26;
pub(crate) const BS_FORME_OFFSET: usize = 28;
pub(crate) const BS_FORME_COUNT: usize = 32;
pub(crate) const BS_TM_COMPAT: usize = 40;
pub(crate) const BS_TUTOR_COMPAT: usize = 64;

/// The evolution methods by number, from 1. Method 34 is Espurr's evolution into the female Meowstic.
pub(crate) const EVOLUTION_METHODS: [EvolutionType; 33] = [
    EvolutionType::Happiness,
    EvolutionType::HappinessDay,
    EvolutionType::HappinessNight,
    EvolutionType::Level,
    EvolutionType::Trade,
    EvolutionType::TradeItem,
    EvolutionType::TradeSpecial,
    EvolutionType::Stone,
    EvolutionType::LevelAttackHigher,
    EvolutionType::LevelAtkDefSame,
    EvolutionType::LevelDefenseHigher,
    EvolutionType::LevelLowPv,
    EvolutionType::LevelHighPv,
    EvolutionType::LevelCreateExtra,
    EvolutionType::LevelIsExtra,
    EvolutionType::HighBeauty,
    EvolutionType::StoneMaleOnly,
    EvolutionType::StoneFemaleOnly,
    EvolutionType::ItemDay,
    EvolutionType::ItemNight,
    EvolutionType::WithMove,
    EvolutionType::WithOther,
    EvolutionType::LevelMaleOnly,
    EvolutionType::LevelFemaleOnly,
    EvolutionType::MagneticField,
    EvolutionType::MossRock,
    EvolutionType::IceRock,
    EvolutionType::LevelUpsideDown,
    EvolutionType::FairyAffection,
    EvolutionType::LevelWithDark,
    EvolutionType::LevelRain,
    EvolutionType::LevelDay,
    EvolutionType::LevelNight,
];
pub(crate) const MEOWSTIC_F_METHOD: u16 = 34;
pub(crate) const EVOLUTION_METHOD_COUNT: u16 = 34;

// Species and forme numbers.
pub(crate) const NINCADA: DexNumber = 290;
pub(crate) const SHEDINJA: DexNumber = 292;
pub(crate) const FEEBAS: DexNumber = 349;
pub(crate) const ESPURR: DexNumber = 677;
pub(crate) const MEOWSTIC: DexNumber = 678;
pub(crate) const MEOWSTIC_F: u16 = 748;
pub(crate) const FEEBAS_BEAUTY: u16 = 170;

/// The suffixes of alternate formes by base species, from forme 1; Mega Evolutions get theirs from
/// [`crate::formes::mega_suffix`]. Floette's first four formes are colours, with no suffix.
pub(crate) const FORME_SUFFIXES: [(DexNumber, &[&str]); 23] = [
    (
        25,
        &["-Cosplay", "-Cosplay", "-Cosplay", "-Cosplay", "-Cosplay", "-Cosplay"],
    ),
    (351, &["-Sunny", "-Rainy", "-Snowy"]),
    (382, &["-Primal"]),
    (383, &["-Primal"]),
    (386, &["-Attack", "-Defense", "-Speed"]),
    (413, &["-Sandy", "-Trash"]),
    (479, &["-Heat", "-Wash", "-Frost", "-Fan", "-Mow"]),
    (487, &["-Origin"]),
    (492, &["-Sky"]),
    (550, &["-Blue"]),
    (555, &["-Zen"]),
    (641, &["-Therian"]),
    (642, &["-Therian"]),
    (645, &["-Therian"]),
    (646, &["-White", "-Black"]),
    (647, &["-Resolute"]),
    (648, &["-Pirouette"]),
    (678, &["-F"]),
    (681, &["-Blade"]),
    (710, &["-M", "-L", "-XL"]),
    (711, &["-M", "-L", "-XL"]),
    (670, &["", "", "", "", "-Eternal"]),
    (720, &["-Unbound"]),
];

/// The alternate formes that only differ in looks from their base species, or that the randomizers see as
/// interchangeable with it: Cherrim's Sunshine Form, Keldeo's Resolute Form, Furfrou's trims, the sizes of
/// Pumpkaboo and Gourgeist, Floette's colours and the Cosplay Pikachu.
pub(crate) fn is_cosmetic_forme(n: DexNumber) -> bool {
    matches!(n, 737 | 743 | 749..=757 | 788..=797 | 815..=820)
}

/// The formes that only last in battle or under some condition: Castform's weather formes, Darmanitan's Zen Mode,
/// Meloetta's Pirouette Forme, Kyurem's fused formes, Aegislash's Blade Forme, the Mega Evolutions and, in Omega
/// Ruby and Alpha Sapphire, the Primal Reversions.
pub(crate) fn is_irregular_forme(n: DexNumber, family: Family) -> bool {
    match n {
        734..=736 | 739..=742 | 747 | 758..=787 => true,
        799..=814 | 822..=825 => family == Family::Oras,
        _ => false,
    }
}

/// Species whose cosmetic formes without data of their own random formes leave alone: Arceus and Genesect, whose
/// formes depend on a held item, and Xerneas.
pub(crate) const IGNORED_COSMETIC_FORMES: [DexNumber; 3] = [493, 649, 716];
/// Burmy, which may become any of Wormadam's formes.
pub(crate) const BURMY: DexNumber = 412;
pub(crate) const WORMADAM_FORMES: [DexNumber; 2] = [725, 726];

// Moves.
pub(crate) const TOXIC: MoveId = 92;
pub(crate) const POISON_FANG: MoveId = 305;
pub(crate) const TRIPLE_KICK: MoveId = 167;
pub(crate) const HYPERSPACE_FURY: MoveId = 621;
pub(crate) const GAME_BREAKING_MOVES: [MoveId; 2] = [49, 82];
/// Move effects of moves that trap.
pub(crate) const TRAPPING_EFFECTS: [u16; 3] = [106, 354, 373];

// Move qualities.
pub(crate) const Q_NO_DAMAGE_STATUS: i8 = 1;
pub(crate) const Q_NO_DAMAGE_STAT_CHANGE: i8 = 2;
pub(crate) const Q_DAMAGE_STATUS: i8 = 4;
pub(crate) const Q_NO_DAMAGE_STATUS_AND_STAT_CHANGE: i8 = 5;
pub(crate) const Q_DAMAGE_TARGET_DEBUFF: i8 = 6;
pub(crate) const Q_DAMAGE_USER_BUFF: i8 = 7;
pub(crate) const Q_DAMAGE_ABSORB: i8 = 8;

// Items.
pub(crate) const TM01: ItemId = 328;
pub(crate) const TM92: ItemId = 419;
pub(crate) const TM93: ItemId = 618;
pub(crate) const TM95: ItemId = 620;
pub(crate) const TM96: ItemId = 690;
pub(crate) const TM100: ItemId = 694;
pub(crate) const TM_COUNT: usize = 100;
pub(crate) const TM_BLOCK_ONE: usize = 92;
pub(crate) const TM_BLOCK_TWO: usize = 3;
pub(crate) const HM_BLOCK_ONE: usize = 5;
pub(crate) const ROCK_SMASH_OFFSET_ORAS: usize = 10;
pub(crate) const DIVE_OFFSET_ORAS: usize = 28;

// Code patterns.
pub(crate) const TM_DATA_PREFIX: &str = "D400AE02AF02B002";
pub(crate) const HIDDEN_ITEMS_PREFIX_ORAS: &str = "A100A200A300A400A5001400010053004A0084000900";
pub(crate) const HIDDEN_ITEM_COUNT_ORAS: usize = 170;
pub(crate) const ITEM_PALETTES_PREFIX: &str = "6F7461746500FF920A063F";
pub(crate) const TUTORS_LOCATOR: &str = "C2015701A20012024401BA01";
pub(crate) const TUTORS_SHOP_PREFIX: &str = "8A02000030000000";
pub(crate) const TUTOR_MOVE_COUNT: usize = 60;
pub(crate) const TUTOR_SIZES: [usize; 4] = [15, 17, 16, 15];
pub(crate) const TRADE_SIZE: usize = 0x24;
pub(crate) const FRIENDSHIP_FOR_EVO_LOCATOR: &str = "DC0050E3BC00002A";
pub(crate) const PERFECT_ODDS_LOCATOR: &str = "050000BA000050E3";
pub(crate) const FASTEST_TEXT_PREFIXES: [&str; 2] =
    ["1080BDE80000A0E31080BDE8F0412DE9", "485080E59C4040E24C50C0E5EC009FE5"];
pub(crate) const AFTER_BATTLE_FORME_REVERSION_PREFIX: &str = "E4FFFFEA0000000000000000";
pub(crate) const NINJASK_SPECIES_PREFIX: &str = "241094E5B810D1E1";
pub(crate) const SHEDINJA_SPECIES_PREFIX: &str = "C2FFFFEB0040A0E10020A0E3";
pub(crate) const BOX_LEGENDARY_FUNCTION_PREFIX_XY: &str = "14D08DE20900A0E1";
pub(crate) const BOX_LEGENDARY_ENCOUNTER_FILE_XY: usize = 341;
pub(crate) const BOX_LEGENDARY_LOCAL_SCRIPT_OFFSET_XY: usize = 0x6E0;
pub(crate) const BOX_LEGENDARY_CODE_OFFSETS_XY: [usize; 3] = [144, 300, 584];
pub(crate) const SEA_SPIRITS_DEN_ENCOUNTER_FILE_XY: usize = 351;
pub(crate) const SEA_SPIRITS_DEN_LOCAL_SCRIPT_OFFSET_XY: usize = 0x1C0;
pub(crate) const SEA_SPIRITS_DEN_SCRIPT_OFFSETS_XY: [usize; 3] = [0x500, 0x508, 0x510];
pub(crate) const RAYQUAZA_FUNCTION_PREFIX_ORAS: &str = "0900A0E1F08FBDE8";
pub(crate) const RAYQUAZA_SCRIPT_OFFSETS_ORAS: [usize; 2] = [3334, 14734];
pub(crate) const RAYQUAZA_CODE_OFFSETS_ORAS: [usize; 3] = [136, 292, 576];
pub(crate) const NATIONAL_DEX_FUNCTION_LOCATOR: &str = "080094E5010000E21080BDE8170F122F";
pub(crate) const XY_GET_DEX_FLAG_FUNCTION_LOCATOR: &str = "000055E30100A0030A00000A";
pub(crate) const ORAS_GET_HOENN_DEX_CAUGHT_FUNCTION_PREFIX: &str = "170F122F1CC15800";
pub(crate) const MEGA_STONE_TABLE_START_ORAS: usize = 0xABA;
pub(crate) const MEGA_STONE_TABLE_ENTRY_SIZE_ORAS: usize = 0x20;
pub(crate) const MEGA_STONE_TABLE_LENGTH_ORAS: usize = 27;
pub(crate) const PICKUP_TABLE_LOCATOR: &str = "110012001A00";
pub(crate) const PICKUP_ITEM_COUNT: usize = 29;
pub(crate) const XY_ROAMER_FREE_SPACE_POSTFIX: &str = "540095E50220A0E30810A0E1";
pub(crate) const XY_ROAMER_SPECIES_LOCATOR: &str = "9040A0030400000A";
pub(crate) const XY_ROAMER_LEVEL_PREFIX: &str = "B020DDE13F3BC1E3";
pub(crate) const XY_TRASH_ENCOUNTERS_PREFIX: &str = "4028100000";
pub(crate) const XY_TRASH_ENCOUNTER_LENGTH: usize = 16;
/// The trash can encounters of X and Y in groups: first, length, and whether the group counts as one encounter.
pub(crate) const XY_TRASH_GROUPS: [(usize, usize, bool); 5] =
    [(0, 6, true), (6, 6, true), (12, 3, true), (15, 3, true), (18, 6, false)];
pub(crate) const CRIES_TABLE_PREFIX_XY: &str = "60000A006B000A0082000A003D010A00";
pub(crate) const INTRO_MODEL_PREFIX_XY: &str = "01000400020002000200000003000000";
pub(crate) const INTRO_CRY_PREFIXES_XY: [&str; 3] = [
    "3AFEFFEB000055E31400D40507005001",
    "0800A0E110FEFFEB000057E31550C405",
    "0020E0E30310A0E1E4FDFFEB0000A0E3",
];
pub(crate) const INTRO_REPEATED_CRY_PREFIX_XY: &str = "1080BDE800002041000000008D001000";

// Field encounters of X and Y in the field module.
pub(crate) const FALLING_ENCOUNTER_OFFSET: usize = 0xF4270;
pub(crate) const FALLING_ENCOUNTER_COUNT: usize = 55;
pub(crate) const RUSTLING_BUSH_ENCOUNTER_OFFSET: usize = 0xF40CC;
pub(crate) const RUSTLING_BUSH_ENCOUNTER_COUNT: usize = 7;
pub(crate) const FIELD_ENCOUNTER_SIZE: usize = 0x3C;
/// The names of the falling encounters, by the first encounter of each place.
pub(crate) const FALLING_ENCOUNTER_NAMES: [(usize, &str); 8] = [
    (0, "Glittering Cave Ceiling Encounter"),
    (4, "Reflection Cave Ceiling Encounter"),
    (20, "Victory Road Outside 2 Sky Encounter"),
    (24, "Victory Road Inside 2 Ceiling Encounter"),
    (28, "Victory Road Outside 3 Sky Encounter"),
    (32, "Victory Road Inside 3 Ceiling Encounter"),
    (36, "Victory Road Outside 4 Sky Encounter"),
    (46, "Terminus Cave Ceiling Encounter"),
];
pub(crate) const RUSTLING_BUSH_ENCOUNTER_NAMES: [(usize, &str); 2] = [
    (0, "Route 6 Rustling Bush Encounter"),
    (3, "Route 18 Rustling Bush Encounter"),
];

/// X and Y's trades whose given species is named in story text, and the text files.
pub(crate) const XY_HARDCODED_TRADES: [(usize, usize); 2] = [(1, 129), (8, 349)];

pub(crate) const MAIN_GAME_SHOPS_XY: [usize; 10] = [10, 11, 12, 13, 16, 17, 20, 21, 24, 25];
pub(crate) const MAIN_GAME_SHOPS_ORAS: [usize; 10] = [10, 11, 13, 14, 16, 17, 18, 19, 20, 21];
pub(crate) const SHOP_NAMES_XY: [&str; 26] = [
    "Primary 0 Badges",
    "Primary 1 Badges",
    "Primary 2 Badges",
    "Primary 3 Badges",
    "Primary 4 Badges",
    "Primary 5 Badges",
    "Primary 6 Badges",
    "Primary 7 Badges",
    "Primary 8 Badges",
    "Unused",
    "Lumiose Herboriste",
    "Lumiose Poké Ball Boutique",
    "Lumiose Stone Emporium",
    "Coumarine Incenses",
    "Aquacorde Poké Ball",
    "Aquacorde Potion",
    "Lumiose North Secondary",
    "Cyllage Secondary",
    "Shalour Secondary (TMs)",
    "Lumiose South Secondary (TMs)",
    "Laverre Secondary",
    "Snowbelle Secondary",
    "Kiloude Secondary (TMs)",
    "Anistar Secondary (TMs)",
    "Santalune Secondary",
    "Coumarine Secondary",
];
pub(crate) const SHOP_NAMES_ORAS: [&str; 24] = [
    "Primary 0 Badges (After Pokédex)",
    "Primary 1 Badges",
    "Primary 2 Badges",
    "Primary 3 Badges",
    "Primary 4 Badges",
    "Primary 5 Badges",
    "Primary 6 Badges",
    "Primary 7 Badges",
    "Primary 8 Badges",
    "Primary 0 Badges (Before Pokédex)",
    "Slateport Incenses",
    "Slateport Vitamins",
    "Slateport TMs",
    "Rustboro Secondary",
    "Slateport Secondary",
    "Mauville Secondary (TMs)",
    "Verdanturf Secondary",
    "Fallarbor Secondary",
    "Lavaridge Herbs",
    "Lilycove Dept. Store 2F Left",
    "Lilycove Dept. Store 3F Left",
    "Lilycove Dept. Store 3F Right",
    "Lilycove Dept. Store 4F Left (TMs)",
    "Lilycove Dept. Store 4F Right (TMs)",
];

/// The item palette of a TM of each type.
pub(crate) fn tm_palette(t: Option<Type>) -> u16 {
    match t {
        Some(Type::Dark) => 309,
        Some(Type::Dragon) => 310,
        Some(Type::Psychic) => 311,
        Some(Type::Normal) => 312,
        Some(Type::Poison) => 313,
        Some(Type::Ice) => 314,
        Some(Type::Fighting) => 315,
        Some(Type::Fire) => 316,
        Some(Type::Water) => 317,
        Some(Type::Flying) => 323,
        Some(Type::Grass) => 318,
        Some(Type::Rock) => 319,
        Some(Type::Electric) => 320,
        Some(Type::Ground) => 321,
        Some(Type::Steel) => 324,
        Some(Type::Bug) => 325,
        Some(Type::Fairy) => 546,
        _ => 322,
    }
}

/// Gym and Elite Four type themes.
pub(crate) fn themes(family: Family) -> &'static [(&'static str, Type)] {
    match family {
        Family::Xy => &[
            ("ELITE1", Type::Fire),
            ("ELITE2", Type::Water),
            ("ELITE3", Type::Steel),
            ("ELITE4", Type::Dragon),
            ("GYM1", Type::Bug),
            ("GYM2", Type::Rock),
            ("GYM3", Type::Fighting),
            ("GYM4", Type::Grass),
            ("GYM5", Type::Electric),
            ("GYM6", Type::Fairy),
            ("GYM7", Type::Psychic),
            ("GYM8", Type::Ice),
        ],
        Family::Oras => &[
            ("CHAMPION", Type::Steel),
            ("ELITE1", Type::Dark),
            ("ELITE2", Type::Ghost),
            ("ELITE3", Type::Ice),
            ("ELITE4", Type::Dragon),
            ("GYM1", Type::Rock),
            ("GYM2", Type::Fighting),
            ("GYM3", Type::Electric),
            ("GYM4", Type::Fire),
            ("GYM5", Type::Normal),
            ("GYM6", Type::Flying),
            ("GYM7", Type::Psychic),
            ("GYM8", Type::Water),
        ],
    }
}

/// The held items trainer Pokémon may get. The pools of physical and special attackers are the closest Gen 6 has:
/// the Liechi and Petaya Berries, the Choice Band and the Muscle Band.
pub(crate) fn held_item_pools() -> HeldItemPools {
    HeldItemPools {
        all: ALL_HELD_ITEMS.to_vec(),
        consumable: CONSUMABLE_HELD_ITEMS.to_vec(),
        general_consumable: GENERAL_PURPOSE_CONSUMABLE_ITEMS.to_vec(),
        general: GENERAL_PURPOSE_ITEMS.to_vec(),
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

/// Key items, version exclusives, unused items and HMs of X and Y.
pub(crate) const BANNED_ITEMS_XY: [ItemId; 202] = [
    113, 114, 115, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 216, 420, 421, 422, 423, 424,
    425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 447,
    448, 449, 450, 451, 452, 453, 454, 455, 456, 457, 458, 459, 460, 461, 462, 463, 464, 465, 466, 467, 468, 469, 470,
    471, 472, 473, 474, 475, 476, 477, 478, 479, 480, 481, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493,
    494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 505, 506, 507, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517,
    518, 519, 520, 521, 522, 523, 524, 525, 526, 527, 528, 529, 530, 531, 532, 533, 534, 535, 536, 574, 578, 579, 592,
    593, 594, 595, 596, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615,
    616, 617, 621, 622, 623, 624, 625, 626, 627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 638, 641, 642, 643,
    651, 689, 695, 696, 697, 698, 700, 701, 702, 703, 705, 706, 707, 712, 713, 714, 716, 717,
];

/// The banned items of X and Y, and the key items of Omega Ruby and Alpha Sapphire.
pub(crate) const BANNED_ITEMS_ORAS: [ItemId; 243] = [
    113, 114, 115, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 216, 420, 421, 422, 423, 424,
    425, 426, 427, 428, 429, 430, 431, 432, 433, 434, 435, 436, 437, 438, 439, 440, 441, 442, 443, 444, 445, 446, 447,
    448, 449, 450, 451, 452, 453, 454, 455, 456, 457, 458, 459, 460, 461, 462, 463, 464, 465, 466, 467, 468, 469, 470,
    471, 472, 473, 474, 475, 476, 477, 478, 479, 480, 481, 482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493,
    494, 495, 496, 497, 498, 499, 500, 501, 502, 503, 505, 506, 507, 508, 509, 510, 511, 512, 513, 514, 515, 516, 517,
    518, 519, 520, 521, 522, 523, 524, 525, 526, 527, 528, 529, 530, 531, 532, 533, 534, 535, 536, 574, 578, 579, 592,
    593, 594, 595, 596, 597, 598, 599, 600, 601, 602, 603, 604, 605, 606, 607, 608, 609, 610, 611, 612, 613, 614, 615,
    616, 617, 621, 622, 623, 624, 625, 626, 627, 628, 629, 630, 631, 632, 633, 634, 635, 636, 637, 638, 641, 642, 643,
    651, 689, 695, 696, 697, 698, 700, 701, 702, 703, 705, 706, 707, 712, 713, 714, 716, 717, 718, 719, 720, 721, 722,
    723, 724, 725, 726, 727, 728, 729, 730, 731, 732, 733, 734, 735, 736, 737, 738, 739, 740, 741, 742, 743, 744, 745,
    746, 747, 748, 749, 750, 751, 765, 766, 771, 772, 773, 774, 775,
];

/// Items of little use: mulch, mail, flavour berries, items for one species, scarves and relics.
pub(crate) const BAD_ITEMS_ORAS: [ItemId; 73] = [
    95, 96, 97, 98, 111, 112, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 155, 159, 160, 161,
    162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 225,
    236, 256, 257, 258, 259, 260, 261, 262, 263, 264, 274, 571, 575, 584, 585, 586, 587, 588, 589, 590, 652, 653, 654,
    655, 699, 704,
];

/// The bad items of Omega Ruby and Alpha Sapphire, and the shoal items and shards.
pub(crate) const BAD_ITEMS_XY: [ItemId; 79] = [
    70, 71, 72, 73, 74, 75, 95, 96, 97, 98, 111, 112, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147,
    148, 155, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179,
    180, 181, 182, 183, 225, 236, 256, 257, 258, 259, 260, 261, 262, 263, 264, 274, 571, 575, 584, 585, 586, 587, 588,
    589, 590, 652, 653, 654, 655, 699, 704,
];

/// Items too good to sell.
pub(crate) const OP_SHOP_ITEMS: [ItemId; 40] = [
    42, 43, 50, 54, 65, 66, 67, 68, 69, 70, 71, 86, 87, 88, 89, 90, 91, 92, 106, 206, 207, 208, 209, 210, 211, 212,
    231, 571, 580, 581, 582, 583, 584, 585, 586, 587, 588, 589, 590, 591,
];

/// Items that make species evolve.
pub(crate) const EVOLUTION_ITEMS: [ItemId; 26] = [
    80, 81, 82, 83, 84, 85, 107, 108, 109, 110, 221, 226, 227, 233, 235, 252, 321, 322, 323, 324, 325, 326, 327, 537,
    646, 647,
];

/// TMs only found on the field in X and Y.
pub(crate) const REQUIRED_FIELD_TMS_XY: [ItemId; 25] = [
    328, 336, 367, 346, 392, 400, 396, 401, 408, 384, 388, 691, 620, 398, 406, 357, 358, 363, 380, 356, 349, 330, 329,
    407, 353,
];

/// TMs only found on the field in Omega Ruby and Alpha Sapphire.
pub(crate) const REQUIRED_FIELD_TMS_ORAS: [ItemId; 36] = [
    364, 359, 389, 338, 413, 356, 386, 370, 380, 396, 333, 329, 340, 345, 349, 388, 357, 691, 334, 417, 353, 382, 361,
    362, 391, 392, 393, 401, 406, 407, 408, 411, 416, 418, 618, 620,
];

/// Moves used outside battle in X and Y.
pub(crate) const FIELD_MOVES_XY: [MoveId; 10] = [15, 19, 57, 70, 148, 91, 100, 127, 230, 249];

/// Moves used outside battle in Omega Ruby and Alpha Sapphire.
pub(crate) const FIELD_MOVES_ORAS: [MoveId; 12] = [15, 19, 57, 70, 148, 91, 100, 127, 230, 249, 290, 291];

/// Held items that are used up.
pub(crate) const CONSUMABLE_HELD_ITEMS: [ItemId; 78] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 184, 185, 186, 187, 188, 189, 190, 191,
    192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 43, 214,
    219, 271, 275, 541, 542, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562,
    563, 564, 639, 648, 649, 686, 687, 688, 715,
];

/// Every item worth holding, the consumable ones included. Deliberate choice: the Luminous Moss (648) is here too,
/// as it is among the consumable and general-purpose items.
pub(crate) const ALL_HELD_ITEMS: [ItemId; 160] = [
    213, 217, 220, 221, 222, 230, 232, 233, 234, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250,
    251, 253, 254, 255, 265, 266, 267, 268, 269, 270, 272, 273, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286,
    287, 288, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315,
    316, 317, 318, 326, 327, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 184, 185, 186,
    187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209,
    210, 211, 212, 43, 214, 219, 271, 275, 541, 542, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556, 557,
    558, 559, 560, 561, 562, 563, 564, 538, 539, 540, 543, 544, 639, 649, 686, 687, 688, 715, 640, 644, 650, 648,
];

/// Consumable items good for any Pokémon.
pub(crate) const GENERAL_PURPOSE_CONSUMABLE_ITEMS: [ItemId; 33] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 202, 203, 205, 206, 207, 208, 209, 210, 211, 212, 43, 214, 219,
    275, 542, 545, 546, 547, 639, 648, 649, 687, 688,
];

/// Lasting items good for any Pokémon.
pub(crate) const GENERAL_PURPOSE_ITEMS: [ItemId; 19] = [
    213, 217, 221, 230, 232, 234, 253, 255, 265, 268, 270, 276, 280, 295, 326, 327, 539, 540, 650,
];

/// Items that strengthen moves of a type.
pub(crate) const TYPE_BOOSTING_ITEMS: [(Type, &[ItemId]); 18] = [
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
    (Type::Fairy, &[644]),
];

/// Items that strengthen one species.
pub(crate) const SPECIES_BOOSTING_ITEMS: [(DexNumber, &[ItemId]); 11] = [
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

/// Abilities that do the same as another, by the one they vary, itself included.
pub(crate) const ABILITY_VARIATIONS: [(u16, &[u16]); 8] = [
    (15, &[15, 72]),
    (29, &[29, 73]),
    (37, &[37, 74]),
    (4, &[4, 75]),
    (13, &[13, 76]),
    (111, &[111, 116]),
    (24, &[24, 160]),
    (104, &[104, 163, 164]),
];

/// Abilities that do nothing for most species.
pub(crate) const USELESS_ABILITIES: [u16; 5] = [59, 121, 122, 161, 176];

/// Better prices for items whose price does not fit their use.
pub(crate) const BALANCED_PRICES: [(ItemId, u32); 311] = [
    (1, 3000),
    (5, 500),
    (30, 400),
    (31, 600),
    (32, 700),
    (33, 800),
    (34, 400),
    (35, 1100),
    (38, 3000),
    (39, 4500),
    (40, 15000),
    (41, 18000),
    (42, 450),
    (44, 10000),
    (50, 10000),
    (53, 24900),
    (54, 450),
    (72, 400),
    (73, 400),
    (74, 400),
    (75, 400),
    (80, 3000),
    (81, 3000),
    (82, 3000),
    (83, 3000),
    (84, 3000),
    (85, 3000),
    (93, 5000),
    (94, 500),
    (99, 5000),
    (100, 5000),
    (101, 5000),
    (102, 5000),
    (103, 8000),
    (104, 5000),
    (105, 5000),
    (107, 3000),
    (108, 3000),
    (109, 3000),
    (110, 3000),
    (134, 150),
    (149, 200),
    (150, 250),
    (151, 100),
    (152, 250),
    (153, 250),
    (154, 3000),
    (155, 50),
    (156, 200),
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
    (214, 1000),
    (216, 0),
    (217, 4500),
    (218, 1000),
    (219, 1000),
    (220, 10000),
    (221, 5000),
    (222, 2000),
    (223, 15000),
    (224, 1000),
    (226, 3000),
    (227, 3000),
    (230, 3000),
    (231, 10000),
    (232, 5000),
    (233, 3000),
    (234, 10000),
    (235, 3000),
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
    (265, 1500),
    (266, 2000),
    (267, 2000),
    (268, 6000),
    (269, 1500),
    (270, 10000),
    (271, 1000),
    (272, 1500),
    (273, 1500),
    (275, 2000),
    (276, 1500),
    (277, 3000),
    (278, 1000),
    (279, 1000),
    (280, 1500),
    (281, 5000),
    (286, 1500),
    (287, 10000),
    (288, 1500),
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
    (321, 3000),
    (322, 3000),
    (323, 3000),
    (324, 3000),
    (325, 3000),
    (326, 5000),
    (327, 5000),
    (328, 10000),
    (334, 20000),
    (338, 20000),
    (341, 20000),
    (342, 20000),
    (343, 20000),
    (345, 20000),
    (347, 20000),
    (352, 20000),
    (360, 20000),
    (364, 20000),
    (365, 20000),
    (377, 10000),
    (379, 10000),
    (395, 20000),
    (397, 10000),
    (398, 10000),
    (400, 10000),
    (403, 10000),
    (504, 150),
    (537, 3000),
    (538, 10000),
    (539, 1000),
    (540, 6000),
    (541, 1000),
    (542, 1000),
    (543, 1000),
    (544, 2000),
    (545, 1000),
    (546, 1000),
    (547, 1000),
    (548, 1000),
    (549, 1000),
    (550, 1000),
    (551, 1000),
    (552, 1000),
    (553, 1000),
    (554, 1000),
    (555, 1000),
    (556, 1000),
    (557, 1000),
    (558, 1000),
    (559, 1000),
    (560, 1000),
    (561, 1000),
    (562, 1000),
    (563, 1000),
    (564, 1000),
    (572, 5000),
    (573, 5000),
    (576, 1000),
    (586, 0),
    (591, 450),
    (618, 10000),
    (639, 2000),
    (640, 6000),
    (644, 2000),
    (645, 5000),
    (646, 3000),
    (647, 3000),
    (650, 3000),
    (656, 10000),
    (657, 10000),
    (658, 10000),
    (659, 10000),
    (660, 10000),
    (661, 10000),
    (662, 20000),
    (663, 20000),
    (664, 10000),
    (665, 5000),
    (666, 10000),
    (667, 10000),
    (668, 5000),
    (669, 20000),
    (670, 10000),
    (671, 10000),
    (672, 10000),
    (673, 10000),
    (674, 5000),
    (675, 5000),
    (676, 10000),
    (677, 5000),
    (678, 10000),
    (679, 10000),
    (680, 10000),
    (681, 3000),
    (682, 5000),
    (683, 20000),
    (684, 20000),
    (685, 20000),
    (686, 1000),
    (687, 1000),
    (688, 1000),
    (708, 450),
    (709, 450),
    (710, 5000),
    (711, 5000),
    (715, 1000),
    (752, 10000),
    (753, 10000),
    (754, 3000),
    (755, 5000),
    (756, 10000),
    (757, 5000),
    (758, 20000),
    (759, 5000),
    (760, 5000),
    (761, 10000),
    (762, 5000),
    (763, 5000),
    (764, 20000),
    (767, 5000),
    (768, 5000),
    (769, 20000),
    (770, 3000),
];

/// The Pokédex area of each wild area of X and Y with encounters, in file order, then the falling and rustling bush encounters.
pub(crate) const XY_MAP_TO_DEX_AREA: [i16; 115] = [
    7, 8, 13, 14, 16, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 44, 45, 46,
    46, 47, 47, 47, 47, 49, 49, 49, 49, 49, 50, 51, 51, 51, 51, 51, 52, 54, 54, 54, 54, 54, 55, 43, 46, 46, 46, 46, 47,
    47, 47, 47, 47, 47, 47, 47, 47, 47, 47, 47, 47, 47, 47, 47, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51,
    51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 51, 54, 54, 54, 54, 54, 54, 54, 54, 54, 26, 26, 26, 38, 38, 38, 38,
];
/// The Pokédex area of each wild area of Omega Ruby and Alpha Sapphire with encounters, in file order; -1 for none.
pub(crate) const ORAS_MAP_TO_DEX_AREA: [i16; 150] = [
    2, 6, 7, 8, 12, 13, 14, 15, 17, 18, 19, 20, 21, 22, 23, 24, 26, 27, 28, 30, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41,
    41, 43, 43, 45, 46, 47, 48, 50, 51, 53, 55, 57, 59, 61, 62, 63, 64, 25, 49, 52, 56, 58, 60, 69, 69, 69, 69, 73, 74,
    74, 74, 78, 80, 81, 82, 82, 82, 82, 82, 82, -1, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 88, 102, 102, 102, 102,
    102, 114, 114, 114, 114, 119, 119, 119, 119, 119, 119, 119, 130, 136, 136, 136, 136, -1, -1, -1, -1, -1, -1, 150,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 159, 65, 65, 65, 65, 102, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
];
/// The location of each wild area of X and Y, as (number of areas, location) in order.
pub(crate) const XY_LOCATION_TAGS: [(usize, &str); 43] = [
    (4, "COURIWAY TOWN"),
    (5, "AMBRETTE TOWN"),
    (5, "CYLLAGE CITY"),
    (4, "SHALOUR CITY"),
    (3, "LAVERRE CITY"),
    (1, "ROUTE 2"),
    (5, "ROUTE 3"),
    (2, "ROUTE 4"),
    (5, "ROUTE 5"),
    (1, "ROUTE 6"),
    (6, "ROUTE 7"),
    (10, "ROUTE 8"),
    (1, "ROUTE 9"),
    (5, "ROUTE 10"),
    (4, "ROUTE 11"),
    (10, "ROUTE 12"),
    (2, "ROUTE 13"),
    (9, "ROUTE 14"),
    (9, "ROUTE 15"),
    (9, "ROUTE 16"),
    (1, "ROUTE 17"),
    (6, "ROUTE 18"),
    (10, "ROUTE 19"),
    (5, "ROUTE 20"),
    (9, "ROUTE 21"),
    (6, "ROUTE 22"),
    (1, "SANTALUNE FOREST"),
    (3, "PARFUM PALACE"),
    (2, "GLITTERING CAVE"),
    (16, "REFLECTION CAVE"),
    (24, "FROST CAVERN"),
    (9, "POKEMON VILLAGE"),
    (30, "VICTORY ROAD"),
    (4, "CONNECTING CAVE"),
    (23, "TERMINUS CAVE"),
    (1, "LOST HOTEL"),
    (9, "AZURE BAY"),
    (4, "GLITTERING CAVE"),
    (16, "REFLECTION CAVE"),
    (26, "VICTORY ROAD"),
    (9, "TERMINUS CAVE"),
    (3, "ROUTE 6"),
    (4, "ROUTE 18"),
];
/// The location of each wild area of Omega Ruby and Alpha Sapphire, as (number of areas, location) in order.
pub(crate) const ORAS_LOCATION_TAGS: [(usize, &str); 76] = [
    (4, "DEWFORD TOWN"),
    (4, "PACIFIDLOG TOWN"),
    (4, "PETALBURG CITY"),
    (4, "SLATEPORT CITY"),
    (5, "LILYCOVE CITY"),
    (4, "MOSSDEEP CITY"),
    (4, "SOOTOPOLIS CITY"),
    (4, "EVER GRANDE CITY"),
    (5, "ROUTE 101"),
    (9, "ROUTE 102"),
    (9, "ROUTE 103"),
    (18, "ROUTE 104"),
    (5, "ROUTE 105"),
    (5, "ROUTE 106"),
    (5, "ROUTE 107"),
    (5, "ROUTE 108"),
    (5, "ROUTE 109"),
    (9, "ROUTE 110"),
    (10, "ROUTE 111"),
    (10, "ROUTE 112"),
    (5, "ROUTE 113"),
    (10, "ROUTE 114"),
    (9, "ROUTE 115"),
    (5, "ROUTE 116"),
    (9, "ROUTE 117"),
    (10, "ROUTE 118"),
    (16, "ROUTE 119"),
    (16, "ROUTE 120"),
    (6, "ROUTE 121"),
    (5, "ROUTE 122"),
    (8, "ROUTE 123"),
    (5, "ROUTE 124"),
    (5, "ROUTE 125"),
    (5, "ROUTE 126"),
    (5, "ROUTE 127"),
    (5, "ROUTE 128"),
    (5, "ROUTE 129"),
    (5, "ROUTE 130"),
    (5, "ROUTE 131"),
    (5, "ROUTE 132"),
    (5, "ROUTE 133"),
    (5, "ROUTE 134"),
    (1, "ROUTE 107"),
    (1, "ROUTE 124"),
    (1, "ROUTE 126"),
    (1, "ROUTE 128"),
    (1, "ROUTE 129"),
    (1, "ROUTE 130"),
    (36, "METEOR FALLS"),
    (5, "RUSTURF TUNNEL"),
    (16, "GRANITE CAVE"),
    (5, "PETALBURG WOODS"),
    (5, "JAGGED PASS"),
    (5, "FIERY PATH"),
    (26, "MT. PYRE"),
    (4, "TEAM AQUA HIDEOUT"),
    (55, "SEAFLOOR CAVERN"),
    (20, "CAVE OF ORIGIN"),
    (28, "VICTORY ROAD"),
    (43, "SHOAL CAVE"),
    (4, "NEW MAUVILLE"),
    (16, "SEA MAUVILLE"),
    (4, "SEALED CHAMBER"),
    (21, "SCORCHED SLAB"),
    (4, "TEAM MAGMA HIDEOUT"),
    (4, "SKY PILLAR"),
    (9, "MIRAGE FOREST"),
    (10, "MIRAGE ISLAND"),
    (1, "MIRAGE MOUNTAIN"),
    (4, "BATTLE RESORT"),
    (40, "SAFARI ZONE"),
    (4, "CAVE OF ORIGIN"),
    (9, "MIRAGE MOUNTAIN"),
    (12, "MIRAGE CAVE"),
    (5, "MT. PYRE"),
    (4, "SOOTOPOLIS CITY"),
];
/// The wild areas of Omega Ruby and Alpha Sapphire that do not count as local: Sky Pillar, the Battle Resort, the
/// Mirage spots and the `DexNav` foreign encounters.
pub(crate) const ORAS_POST_GAME_AREAS: [usize; 105] = [
    585, 586, 587, 588, 609, 610, 611, 612, 589, 590, 591, 592, 593, 594, 595, 596, 597, 598, 599, 600, 601, 602, 603,
    604, 605, 606, 607, 608, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671, 672, 673, 674,
    675, 676, 677, 34, 39, 48, 57, 66, 74, 79, 84, 89, 94, 100, 109, 119, 124, 129, 134, 144, 153, 158, 168, 210, 214,
    227, 232, 237, 242, 247, 252, 257, 262, 267, 272, 277, 289, 298, 307, 316, 330, 335, 340, 346, 351, 356, 377, 382,
    494, 499, 504, 510, 515, 524, 615, 625, 635, 645, 679,
];
