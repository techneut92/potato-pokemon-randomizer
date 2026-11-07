//! Fixed offsets, tables and item lists of the Gen 3 games.

use ppr_model::{DexNumber, HeldItemPools, Type};

/// Pointer to the species front images, in pointer block 1.
pub(super) const POKEMON_FRONT_IMAGES_POINTER: usize = 0x128;
/// Pointer to the species normal palettes, in pointer block 1.
pub(super) const POKEMON_NORMAL_PALETTES_POINTER: usize = 0x130;
/// Pointer to the species shiny palettes, in pointer block 1.
pub(super) const POKEMON_SHINY_PALETTES_POINTER: usize = 0x134;
/// Pointer to the species names in ROMs with pointer block 1.
pub(super) const POKEMON_NAMES_POINTER: usize = 0x144;
/// Pointer to the base stats in ROMs with pointer block 2.
pub(super) const POKEMON_STATS_POINTER: usize = 0x1BC;
/// Pointer to the item data, in pointer block 2.
pub(super) const ITEM_DATA_POINTER: usize = 0x1C8;
pub(super) const ABILITY_NAMES_POINTER: usize = 0x1C0;
/// Pointer to the move names, in pointer block 1.
pub(super) const MOVE_NAMES_POINTER: usize = 0x148;
/// Pointer to the move data, in pointer block 2.
pub(super) const MOVE_DATA_POINTER: usize = 0x1CC;
/// FireRed/LeafGreen: the starters' held item, a word this far after `StarterPokemon`.
pub(super) const FRLG_STARTER_ITEMS: usize = 218;
/// The Thumb `ADD r2, #imm` opcode byte, which follows the item in Ruby, Sapphire and Emerald.
pub(super) const GBA_ADD_R2: u8 = 0x30 | 2;
/// Ruby/Sapphire without pointer block 1: the names pointer sits right before this code.
pub(super) const RS_POKEMON_NAMES_POINTER_SUFFIX: &str = "30B50025084CC8F7";

pub(super) const BASE_STATS_ENTRY_SIZE: usize = 0x1C;
pub(super) const BS_HP: usize = 0;
pub(super) const BS_ATTACK: usize = 1;
pub(super) const BS_DEFENSE: usize = 2;
pub(super) const BS_SPEED: usize = 3;
pub(super) const BS_SP_ATTACK: usize = 4;
pub(super) const BS_SP_DEFENSE: usize = 5;
pub(super) const BS_PRIMARY_TYPE: usize = 6;
pub(super) const BS_SECONDARY_TYPE: usize = 7;
pub(super) const BS_CATCH_RATE: usize = 8;
pub(super) const BS_COMMON_HELD_ITEM: usize = 12;
pub(super) const BS_RARE_HELD_ITEM: usize = 14;
pub(super) const BS_GENDER_RATIO: usize = 16;
pub(super) const BS_GROWTH_CURVE: usize = 19;
pub(super) const BS_ABILITY_1: usize = 22;
pub(super) const BS_ABILITY_2: usize = 23;

pub(super) const UNOWN: DexNumber = 201;
/// Unown's 28 formes, B and on at internal index 413 and up.
pub(super) const UNOWN_FORME_COUNT: usize = 28;
pub(super) const UNOWN_B_INDEX: usize = 413;

/// Shadow Tag, Magnet Pull and Arena Trap.
pub(super) const BATTLE_TRAPPING_ABILITIES: [u8; 3] = [23, 42, 71];

/// `cmp r6, #0xFE; bls …` in the ball-throw command, the branch the perfect-odds patch removes.
pub(super) const PERFECT_ODDS_BRANCH_LOCATOR: &str = "FE2E2FD90020";

/// Base stats of an unused ("?") species slot.
pub(super) const EMPTY_POKEMON_SIG: [u8; 28] = [
    0x32, 0x96, 0x32, 0x96, 0x96, 0x32, 0x00, 0x00, 0x03, 0x01, 0xAA, 0x0A, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x78, 0x00,
    0x00, 0x0F, 0x0F, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
];

/// Highest national number in an unmodified game, counting the 25 unused slots.
pub(super) const UNHACKED_MAX_POKEDEX: usize = 411;
/// Real species in an unmodified game.
pub(super) const UNHACKED_REAL_POKEDEX: usize = 386;
/// Internal index of the first unused slot.
pub(super) const HOENN_POKES_START: usize = 252;

pub(super) const EVOLUTION_ENTRY_SIZE: usize = 0x28;
pub(super) const EVOLUTIONS_PER_SPECIES: usize = 5;
pub(super) const EVOLUTION_METHOD_COUNT: u16 = 15;

pub(super) const RSE_STARTER_2: usize = 2;
pub(super) const RSE_STARTER_3: usize = 4;
pub(super) const FRLG_STARTER_2: usize = 515;
pub(super) const FRLG_STARTER_3: usize = 461;
pub(super) const FRLG_STARTER_REPEAT: usize = 5;

pub(super) const DEOXYS: DexNumber = 386;
pub(super) const MEW: u16 = 151;

/// Thumb `MOVS R1, 0x19A; CMP R0, R1; BEQ` in the obedience check.
pub(super) const DEOXYS_OBEY_CODE: &str = "CD21490088420FD0";
pub(super) const MEW_OBEY_OFFSET_FROM_DEOXYS_OBEY: usize = 0x16;
pub(super) const LEVEL_EVO_KANTO_DEX_CHECK_CODE: &str = "972814DD";
pub(super) const STONE_EVO_KANTO_DEX_CHECK_CODE: &str = "972808D9";
pub(super) const LEVEL_EVO_KANTO_DEX_JUMP_AMOUNT: u16 = 0x14;
pub(super) const STONE_EVO_KANTO_DEX_JUMP_AMOUNT: u16 = 0x08;

pub(super) const GBA_SET_RX_OPCODE: u8 = 0x20;
pub(super) const GBA_CMP_RX_OPCODE: u16 = 0x28;
pub(super) const GBA_UNCONDITIONAL_JUMP_OPCODE: u16 = 0xE0;
pub(super) const GBA_NOP_OPCODE: u16 = 0x46C0;
pub(super) const GBA_R0: u16 = 0;
pub(super) const GBA_R1: u8 = 1;

/// The marker that ends the type chart.
pub(super) const TYPE_CHART_END: u8 = 0xFF;
/// The type chart marker that separates the immunities that Foresight removes.
pub(super) const TYPE_CHART_FORESIGHT: u8 = 0xFE;
pub(super) const TYPE_CHART_ENTRY_SIZE: usize = 3;
/// The non-neutral pairs the game's type chart has room for.
pub(super) const TYPE_CHART_ROOM: usize = 110;
pub(super) use crate::type_order::TYPE_ORDER;

/// The byte of the unused "???" type.
pub(super) const UNKNOWN_TYPE: u8 = 0x09;

/// Byte → type. 0x09 is the unused "???" type.
pub(super) fn type_from_byte(b: u8) -> Option<Type> {
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

/// Type → byte; `None` for types Gen 3 doesn't have.
pub(super) fn type_to_byte(t: Type) -> Option<u8> {
    (0..=0x11).find(|&b| type_from_byte(b) == Some(t))
}

/// Latios, whose species Ruby and Emerald compute instead of storing; the roamer patches store it.
pub(super) const LATIOS: DexNumber = 381;

/// Gender ratios that are also the genders `CreateMonWithGenderNatureLetter` takes (pokefirered).
pub(super) const GENDER_MALE: u8 = 0x00;
pub(super) const GENDER_FEMALE: u8 = 0xFE;
pub(super) const GENDER_UNKNOWN: u8 = 0xFF;

/// One entry of the static music fix's table: the species, then two bytes the fix's code uses.
pub(super) const MUSIC_POOL_ENTRY_SIZE: usize = 4;

/// The HM moves: Cut, Fly, Surf, Strength, Flash, Rock Smash, Waterfall and Dive.
pub(super) const HM_MOVES: [u16; 8] = [15, 19, 57, 70, 148, 249, 127, 291];
/// HM moves needed early: Rock Smash in Ruby, Sapphire and Emerald, Cut in FRLG.
pub(super) const RSE_EARLY_REQUIRED_HM_MOVES: [u16; 1] = [249];
pub(super) const FRLG_EARLY_REQUIRED_HM_MOVES: [u16; 1] = [15];
/// TMs, per game family, that random
/// field items keep, as no other place gives them.
pub(super) const RS_REQUIRED_FIELD_TMS: [u16; 12] = [328, 329, 333, 334, 338, 345, 349, 350, 353, 357, 364, 375];
pub(super) const E_REQUIRED_FIELD_TMS: [u16; 10] = [329, 333, 334, 338, 345, 349, 350, 357, 364, 375];
pub(super) const FRLG_REQUIRED_FIELD_TMS: [u16; 24] = [
    328, 329, 334, 335, 336, 338, 339, 341, 344, 345, 348, 349, 352, 359, 363, 364, 367, 368, 371, 373, 374, 375, 376,
    377,
];
/// Moves that break the game early: Sonic Boom and Dragon Rage.
pub(super) const GAME_BREAKING_MOVES: [u16; 2] = [49, 82];
/// Field moves of Ruby, Sapphire and Emerald: Cut, Fly, Surf, Strength, Flash, Dig, Teleport, Waterfall, Rock Smash,
/// Sweet Scent, Dive and Secret Power.
pub(super) const RSE_FIELD_MOVES: [u16; 12] = [15, 19, 57, 70, 148, 91, 100, 127, 249, 230, 291, 290];
/// Field moves of FRLG: the same without Dive and Secret Power.
pub(super) const FRLG_FIELD_MOVES: [u16; 10] = [15, 19, 57, 70, 148, 91, 100, 127, 249, 230];

/// Items too strong to sell in shops: Rare Candy, the items that sell for a lot (Tiny Mushroom to Nugget) and
/// Lucky Egg.
pub(super) const OP_SHOP_ITEMS: [u16; 9] = [50, 86, 87, 88, 89, 90, 91, 92, 231];
/// The evolution items: the six evolution stones.
pub(super) const EVOLUTION_ITEMS: [u16; 6] = [80, 81, 82, 83, 84, 85];
/// The end of a shop's items, and the script's `release` and `end`.
pub(super) const SHOP_TERMINATOR: [u8; 4] = [0x00, 0x00, 0x6C, 0x02];
/// Balanced item prices, by standard item number.
pub(super) const BALANCED_ITEM_PRICES: [(u16, u32); 133] = [
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
    (65, 20),
    (66, 20),
    (67, 20),
    (68, 20),
    (69, 20),
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
    (148, 50),
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
    (201, 1000),
    (202, 1000),
    (203, 1000),
    (204, 1000),
    (205, 1000),
    (206, 1000),
    (207, 1000),
    (208, 1000),
    (213, 3000),
    (214, 1000),
    (216, 6000),
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
    (331, 1500),
    (334, 2000),
    (335, 1500),
    (336, 2000),
    (337, 2000),
    (339, 1500),
    (343, 2000),
    (344, 2000),
    (347, 2000),
    (356, 3000),
    (359, 1000),
    (360, 2000),
    (363, 3000),
    (366, 2000),
    (368, 1500),
    (370, 2000),
    (373, 2000),
    (376, 1500),
    (377, 5500),
];
/// Shop names for RS.
pub(super) const RS_SHOP_NAMES: [&str; 24] = [
    "Slateport Vitamins",
    "Slateport TMs",
    "Oldale Poké Mart (Before Pokédex)",
    "Oldale Poké Mart (After Pokédex)",
    "Lavaridge Herbs",
    "Lavaridge Poké Mart",
    "Fallarbor Poké Mart",
    "Verdanturf Poké Mart",
    "Petalburg Poké Mart (Before 4 Badges)",
    "Petalburg Poké Mart (After 4 Badges)",
    "Slateport Poké Mart",
    "Mauville Poké Mart",
    "Rustboro Poké Mart (Before Delivering Devon Goods)",
    "Rustboro Poké Mart (After Delivering Devon Goods)",
    "Fortree Poké Mart",
    "Lilycove Department Store 2F Left",
    "Lilycove Department Store 2F Right",
    "Lilycove Department Store 3F Left",
    "Lilycove Department Store 3F Right",
    "Lilycove Department Store 4F Left (TMs)",
    "Lilycove Department Store 4F Right (TMs)",
    "Mossdeep Poké Mart",
    "Sootopolis Poké Mart",
    "Pokémon League Poké Mart",
];
/// Shop names for E.
pub(super) const E_SHOP_NAMES: [&str; 27] = [
    "Slateport Vitamins",
    "Slateport TMs",
    "Oldale Poké Mart (Before Pokédex)",
    "Oldale Poké Mart (After Pokédex)",
    "Lavaridge Herbs",
    "Lavaridge Poké Mart",
    "Fallarbor Poké Mart",
    "Verdanturf Poké Mart",
    "Petalburg Poké Mart (Before 4 Badges)",
    "Petalburg Poké Mart (After 4 Badges)",
    "Slateport Poké Mart",
    "Mauville Poké Mart",
    "Rustboro Poké Mart (Before Delivering Devon Goods)",
    "Rustboro Poké Mart (After Delivering Devon Goods)",
    "Fortree Poké Mart",
    "Lilycove Department Store 2F Left",
    "Lilycove Department Store 2F Right",
    "Lilycove Department Store 3F Left",
    "Lilycove Department Store 3F Right",
    "Lilycove Department Store 4F Left (TMs)",
    "Lilycove Department Store 4F Right (TMs)",
    "Mossdeep Poké Mart",
    "Sootopolis Poké Mart",
    "Pokémon League Poké Mart",
    "Battle Frontier Poké Mart",
    "Trainer Hill Poké Mart (Before Hall of Fame)",
    "Trainer Hill Poké Mart (After Hall of Fame)",
];
/// Shop names for FRLG.
pub(super) const FRLG_SHOP_NAMES: [&str; 23] = [
    "Trainer Tower Poké Mart",
    "Two Island Market Stall (Initial)",
    "Two Island Market Stall (After Saving Lostelle)",
    "Two Island Market Stall (After Hall of Fame)",
    "Two Island Market Stall (After Ruby/Sapphire Quest)",
    "Viridian Poké Mart",
    "Pewter Poké Mart",
    "Cerulean Poké Mart",
    "Lavender Poké Mart",
    "Vermillion Poké Mart",
    "Celadon Department 2F South",
    "Celadon Department 2F North (TMs)",
    "Celadon Department 4F",
    "Celadon Department 5F South",
    "Celadon Department 5F North",
    "Fuchsia Poké Mart",
    "Cinnabar Poké Mart",
    "Indigo Plateau Poké Mart",
    "Saffron Poké Mart",
    "Seven Island Poké Mart",
    "Three Island Poké Mart",
    "Four Island Poké Mart",
    "Six Island Poké Mart",
];

/// Held items used up in battle: the battle berries, Berry Juice, White Herb and Mental Herb.
const CONSUMABLE_HELD_ITEMS: [u16; 25] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 201, 202, 203, 204, 205, 206, 207, 43,
    214, 219,
];
/// The held items that are not used up.
const LASTING_HELD_ITEMS: [u16; 27] = [
    213, 217, 220, 221, 222, 230, 232, 233, 234, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250,
    251, 253, 254, 255,
];
/// Consumable held items any species can use: no flavour berries, which may confuse the holder.
const GENERAL_PURPOSE_CONSUMABLE_ITEMS: [u16; 18] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 202, 203, 205, 206, 207, 43, 214, 219,
];
/// Lasting held items any species can use: Bright Powder, Quick Claw, King's Rock, Focus Band, Scope Lens,
/// Leftovers, Shell Bell and Lax Incense.
const GENERAL_PURPOSE_ITEMS: [u16; 8] = [213, 217, 221, 230, 232, 234, 253, 255];
/// Held items that boost a type's moves.
const TYPE_BOOSTING_ITEMS: [(Type, &[u16]); 17] = [
    (Type::Bug, &[222]),
    (Type::Dark, &[240]),
    (Type::Dragon, &[250]),
    (Type::Electric, &[242]),
    (Type::Fighting, &[241]),
    (Type::Fire, &[249]),
    (Type::Flying, &[244]),
    (Type::Ghost, &[247]),
    (Type::Grass, &[239]),
    (Type::Ground, &[237]),
    (Type::Ice, &[246]),
    (Type::Normal, &[251]),
    (Type::Poison, &[245]),
    (Type::Psychic, &[248]),
    (Type::Rock, &[238]),
    (Type::Steel, &[233]),
    (Type::Water, &[243, 254]),
];
/// Held items that boost particular species: Soul Dew, Deep Sea Tooth and Scale, Light Ball, Lucky Punch,
/// Metal Powder, Thick Club and Leek.
const SPECIES_BOOSTING_ITEMS: [(DexNumber, &[u16]); 9] = [
    (380, &[225]),
    (381, &[225]),
    (366, &[226, 227]),
    (25, &[236]),
    (113, &[256]),
    (132, &[257]),
    (104, &[258]),
    (105, &[258]),
    (83, &[259]),
];

/// The items Gen 3 trainer Pokémon may hold.
pub(super) fn held_item_pools() -> HeldItemPools {
    HeldItemPools {
        all: LASTING_HELD_ITEMS
            .iter()
            .chain(&CONSUMABLE_HELD_ITEMS)
            .copied()
            .collect(),
        consumable: CONSUMABLE_HELD_ITEMS.to_vec(),
        general_consumable: GENERAL_PURPOSE_CONSUMABLE_ITEMS.to_vec(),
        general: GENERAL_PURPOSE_ITEMS.to_vec(),
        // Liechi Berry, Petaya Berry and Choice Band.
        physical_consumable: vec![201],
        special_consumable: vec![204],
        physical: vec![220],
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
