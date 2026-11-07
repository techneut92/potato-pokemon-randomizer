//! Fixed offsets, tables and item lists of the Gen 7 games.

use ppr_model::{DexNumber, EncounterKind, EvolutionType, HeldItemPools, ItemId, MoveId, Type};

/// Sun and Moon, or Ultra Sun and Ultra Moon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Family {
    SunMoon,
    UltraSunMoon,
}

impl Family {
    /// The family of a ROM entry's `Type=`.
    pub(super) fn parse(s: &str) -> Self {
        if s.eq_ignore_ascii_case("USUM") {
            Self::UltraSunMoon
        } else {
            Self::SunMoon
        }
    }

    /// Species with a national number of their own.
    pub(super) fn pokemon_count(self) -> usize {
        match self {
            Self::SunMoon => 802,
            Self::UltraSunMoon => 807,
        }
    }

    /// Alternate formes with base stats of their own, stored after the species.
    pub(super) fn forme_count(self) -> usize {
        match self {
            Self::SunMoon => 158,
            Self::UltraSunMoon => 168,
        }
    }

    pub(super) fn move_count(self) -> usize {
        match self {
            Self::SunMoon => 719,
            Self::UltraSunMoon => 728,
        }
    }

    /// The highest ability number: Prism Armor, or Neuroforce.
    pub(super) fn highest_ability(self) -> usize {
        match self {
            Self::SunMoon => 232,
            Self::UltraSunMoon => 233,
        }
    }

    /// The bytes right before the TM moves in the executable.
    pub(super) fn tm_data_prefix(self) -> &'static str {
        match self {
            Self::SunMoon => "034003410342034303",
            Self::UltraSunMoon => "03BC03BD03BE03BF03",
        }
    }

    /// Where the shop items start in the shop module.
    pub(super) fn shop_items_offset(self) -> usize {
        match self {
            Self::SunMoon => 0x50A8,
            Self::UltraSunMoon => 0x50BC,
        }
    }

    /// The bytes before the call that turns formes back on loading a save.
    pub(super) fn save_load_forme_reversion_prefix(self) -> &'static str {
        match self {
            Self::SunMoon => "00EB040094E50C1094E5F70E80E2",
            Self::UltraSunMoon => "00EB040094E50C1094E5030B80E2EE0F80E2",
        }
    }

    /// The extra info of evolutions that only happen in Sun or Ultra Sun.
    pub(super) fn sunny_evolution_info(self) -> u8 {
        match self {
            Self::SunMoon => 0x1E,
            Self::UltraSunMoon => 0x20,
        }
    }

    /// The extra info of evolutions that only happen in Moon or Ultra Moon.
    pub(super) fn moony_evolution_info(self) -> u8 {
        match self {
            Self::SunMoon => 0x1F,
            Self::UltraSunMoon => 0x21,
        }
    }

    /// Number of files of the wild Pokémon archive.
    pub(super) fn encounter_file_count(self) -> usize {
        match self {
            Self::SunMoon => 2761,
            Self::UltraSunMoon => 3696,
        }
    }
}

// The personal (base stats) entry of a species or forme.
pub(super) const BS_HP: usize = 0;
pub(super) const BS_ATTACK: usize = 1;
pub(super) const BS_DEFENSE: usize = 2;
pub(super) const BS_SPEED: usize = 3;
pub(super) const BS_SP_ATTACK: usize = 4;
pub(super) const BS_SP_DEFENSE: usize = 5;
pub(super) const BS_PRIMARY_TYPE: usize = 6;
pub(super) const BS_SECONDARY_TYPE: usize = 7;
pub(super) const BS_CATCH_RATE: usize = 8;
pub(super) const BS_EV_YIELD: usize = 10;
pub(super) const BS_COMMON_HELD_ITEM: usize = 12;
pub(super) const BS_RARE_HELD_ITEM: usize = 14;
pub(super) const BS_EGG_CYCLES: usize = 19;
pub(super) const BS_GROWTH_CURVE: usize = 21;
pub(super) const BS_ABILITY_1: usize = 24;
pub(super) const BS_ABILITY_2: usize = 25;
pub(super) const BS_ABILITY_3: usize = 26;
pub(super) const BS_CALL_RATE: usize = 27;
pub(super) const BS_FORME: usize = 28;
pub(super) const BS_FORME_COUNT: usize = 32;
pub(super) const BS_TM_COMPAT: usize = 40;
pub(super) const BS_TUTOR_COMPAT: usize = 60;
/// Size of a personal entry.
pub(super) const BS_SIZE: usize = 0x54;

/// Bytes of TM compatibility flags, and of move tutor flags.
pub(super) const TM_COMPAT_BYTES: usize = 13;
pub(super) const TUTOR_COMPAT_BYTES: usize = 10;

pub(super) const SHEDINJA: DexNumber = 292;
pub(super) const NINCADA: DexNumber = 290;
pub(super) const ESPURR: DexNumber = 677;
pub(super) const BURMY: DexNumber = 412;
pub(super) const WORMADAM: DexNumber = 413;
pub(super) const PIKACHU: DexNumber = 25;
pub(super) const RAICHU: DexNumber = 26;
pub(super) const EXEGGCUTE: DexNumber = 102;
pub(super) const EXEGGUTOR: DexNumber = 103;
pub(super) const CUBONE: DexNumber = 104;
pub(super) const MAROWAK: DexNumber = 105;

/// The suffixes of alternate formes by base species, from forme 1; Mega Evolutions get theirs from
/// [`crate::formes::mega_suffix`]. Floette's first four formes and Minior's first six and last six are colours,
/// with no suffix; Pikachu's last forme, the Partner Cap, is in Ultra Sun and Ultra Moon only.
pub(super) const FORME_SUFFIXES: [(DexNumber, &[&str]); 58] = [
    (
        25,
        &[
            "-InACap",
            "-InACap",
            "-InACap",
            "-InACap",
            "-InACap",
            "-InACap",
            "-PartnerCap",
        ],
    ),
    (19, &["-Alolan"]),
    (20, &["-Alolan", "-Totem"]),
    (26, &["-Alolan"]),
    (27, &["-Alolan"]),
    (28, &["-Alolan"]),
    (37, &["-Alolan"]),
    (38, &["-Alolan"]),
    (50, &["-Alolan"]),
    (51, &["-Alolan"]),
    (52, &["-Alolan"]),
    (53, &["-Alolan"]),
    (74, &["-Alolan"]),
    (75, &["-Alolan"]),
    (76, &["-Alolan"]),
    (88, &["-Alolan"]),
    (89, &["-Alolan"]),
    (103, &["-Alolan"]),
    (105, &["-Alolan", "-Totem"]),
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
    (658, &["-BattleBond", "-Ash"]),
    (678, &["-F"]),
    (681, &["-Blade"]),
    (710, &["-M", "-L", "-XL"]),
    (711, &["-M", "-L", "-XL"]),
    (670, &["", "", "", "", "-Eternal"]),
    (718, &["-10p", "-PowerConstruct", "-PowerConstruct", "-Complete"]),
    (720, &["-Unbound"]),
    (741, &["-Pom-Pom", "-Pa'u", "-Sensu"]),
    (744, &["-OwnTempo"]),
    (745, &["-Midnight", "-Dusk"]),
    (746, &["-School"]),
    (774, &["", "", "", "", "", "", "-Core", "", "", "", "", "", ""]),
    (800, &["-DuskMane", "-DawnWings", "-Ultra"]),
    (801, &["-OGColors"]),
    (735, &["-Totem"]),
    (738, &["-Totem"]),
    (743, &["-Totem"]),
    (752, &["-Totem"]),
    (754, &["-Totem"]),
    (758, &["-Totem"]),
    (777, &["-Totem"]),
    (784, &["-Totem"]),
];

/// The alternate formes that only differ in looks from their base species, or that the randomizers see as
/// interchangeable with it: Cherrim's Sunshine Form, the East Sea Shellos and Gastrodon, Keldeo's Resolute Form,
/// Furfrou's trims, the sizes of Pumpkaboo and Gourgeist, Floette's colours, the Totem Pokémon, Mimikyu's Busted
/// Form, Battle Bond Greninja, Zygarde with Power Construct, Minior's colours, the original Magearna, the Pikachu
/// in caps and the Own Tempo Rockruff.
pub(super) fn is_cosmetic_forme(n: DexNumber, family: Family) -> bool {
    match family {
        Family::SunMoon => matches!(
            n,
            818..=820 | 826 | 832..=840 | 871..=880 | 910 | 925 | 928..=929 | 931..=936 | 938..=943 | 946..=960
        ),
        Family::UltraSunMoon => matches!(
            n,
            823..=825
                | 831
                | 837..=845
                | 876..=885
                | 916
                | 931..=932
                | 935..=936
                | 938..=943
                | 945..=950
                | 953..=968
                | 972..=975
        ),
    }
}

/// The formes that only last in battle or under some condition: Castform's weather formes, Darmanitan's Zen Mode,
/// Meloetta's Pirouette Forme, Kyurem's fused formes, the Mega Evolutions, Aegislash's Blade Forme, the Primal
/// Reversions, Wishiwashi's School Form, Ash-Greninja, Complete Zygarde, Minior's Core and, in Ultra Sun and Ultra
/// Moon, Necrozma's fused and Ultra formes.
pub(super) fn is_irregular_forme(n: DexNumber, family: Family) -> bool {
    match family {
        Family::SunMoon => matches!(
            n,
            815..=817 | 822..=825 | 830 | 841..=870 | 882..=897 | 899..=903 | 926 | 930 | 937
        ),
        Family::UltraSunMoon => matches!(
            n,
            820..=822 | 827..=830 | 835 | 846..=875 | 887..=902 | 904..=908 | 933 | 937 | 944 | 969..=971
        ),
    }
}

/// Alternate formes that are formes of another alternate forme in all but the data, with that forme: the Totem
/// Alolan Raticate (and Marowak in Ultra Sun and Ultra Moon), Zygarde's 10% Forme with Power Construct and the
/// coloured Cores of Minior.
pub(super) fn conceptual_base(n: DexNumber, family: Family) -> Option<DexNumber> {
    match (family, n) {
        (Family::SunMoon, 910) => Some(909),
        (Family::SunMoon, 928) => Some(927),
        (Family::SunMoon, 938..=943) => Some(937),
        (Family::UltraSunMoon, 916) => Some(915),
        (Family::UltraSunMoon, 931) => Some(930),
        (Family::UltraSunMoon, 935) => Some(934),
        (Family::UltraSunMoon, 945..=950) => Some(944),
        _ => None,
    }
}

/// Species whose cosmetic formes without data of their own random formes leave alone: Arceus, Genesect and
/// Silvally, whose formes depend on a held item, and Xerneas.
pub(super) const IGNORED_COSMETIC_FORMES: [DexNumber; 4] = [493, 649, 773, 716];

/// Evolution entries per species, of 8 bytes each.
pub(super) const EVOLUTIONS_PER_SPECIES: usize = 8;
/// Methods 1 to this one are evolutions.
pub(super) const EVOLUTION_METHOD_COUNT: u16 = 42;
/// Espurr into female Meowstic: a forme-setting method left over from Gen 6.
pub(super) const MEOWSTIC_F_EVOLUTION_METHOD: u16 = 34;
/// The methods that depend on which of the two games this is (Cosmoem, Rockruff).
pub(super) const EVOLUTION_LEVEL_GAME: u16 = 0x24;
pub(super) const EVOLUTION_LEVEL_GAME_DAY: u16 = 0x25;
pub(super) const EVOLUTION_LEVEL_GAME_NIGHT: u16 = 0x26;

/// Evolution methods 1 to 42; `None` where the method has no evolution type here.
pub(super) const EVOLUTION_TYPES: [Option<EvolutionType>; 42] = {
    use EvolutionType as E;
    [
        Some(E::Happiness),
        Some(E::HappinessDay),
        Some(E::HappinessNight),
        Some(E::Level),
        Some(E::Trade),
        Some(E::TradeItem),
        Some(E::TradeSpecial),
        Some(E::Stone),
        Some(E::LevelAttackHigher),
        Some(E::LevelAtkDefSame),
        Some(E::LevelDefenseHigher),
        Some(E::LevelLowPv),
        Some(E::LevelHighPv),
        Some(E::LevelCreateExtra),
        Some(E::LevelIsExtra),
        Some(E::HighBeauty),
        Some(E::StoneMaleOnly),
        Some(E::StoneFemaleOnly),
        Some(E::ItemDay),
        Some(E::ItemNight),
        Some(E::WithMove),
        Some(E::WithOther),
        Some(E::LevelMaleOnly),
        Some(E::LevelFemaleOnly),
        Some(E::MagneticField),
        Some(E::MossRock),
        Some(E::IceRock),
        Some(E::LevelUpsideDown),
        Some(E::FairyAffection),
        Some(E::LevelWithDark),
        Some(E::LevelRain),
        Some(E::LevelDay),
        Some(E::LevelNight),
        None,
        None,
        None,
        None,
        None,
        Some(E::Snowy),
        Some(E::LevelDusk),
        Some(E::LevelUltra),
        Some(E::StoneUltra),
    ]
};

/// The evolution method of a type.
pub(super) fn evolution_method(kind: EvolutionType) -> Option<u16> {
    use EvolutionType as E;
    if let Some(i) = EVOLUTION_TYPES.iter().position(|&t| t == Some(kind)) {
        return u16::try_from(i + 1).ok();
    }
    match kind {
        E::LevelGameThis | E::LevelGameOther => Some(EVOLUTION_LEVEL_GAME),
        E::LevelGameThisDay | E::LevelGameOtherDay => Some(EVOLUTION_LEVEL_GAME_DAY),
        E::LevelGameThisNight | E::LevelGameOtherNight => Some(EVOLUTION_LEVEL_GAME_NIGHT),
        _ => None,
    }
}

/// The evolutions whose method depends on which of the two games this is.
pub(super) fn is_game_specific(kind: EvolutionType) -> bool {
    use EvolutionType as E;
    matches!(
        kind,
        E::LevelGameThis
            | E::LevelGameOther
            | E::LevelGameThisDay
            | E::LevelGameOtherDay
            | E::LevelGameThisNight
            | E::LevelGameOtherNight
    )
}

/// Move data: physical, special or status by the category byte.
pub(super) const NO_DAMAGE_STATUS_QUALITY: u8 = 1;
pub(super) const NO_DAMAGE_STAT_CHANGE_QUALITY: u8 = 2;
pub(super) const DAMAGE_STATUS_QUALITY: u8 = 4;
pub(super) const NO_DAMAGE_STATUS_AND_STAT_CHANGE_QUALITY: u8 = 5;
pub(super) const DAMAGE_TARGET_DEBUFF_QUALITY: u8 = 6;
pub(super) const DAMAGE_USER_BUFF_QUALITY: u8 = 7;
pub(super) const DAMAGE_ABSORB_QUALITY: u8 = 8;
/// Effects of moves that trap the target.
pub(super) const TRAPPING_EFFECTS: [u16; 4] = [106, 354, 373, 384];

pub(super) const TRIPLE_KICK: MoveId = 167;
pub(super) const TOXIC: MoveId = 92;
pub(super) const POISON_FANG: MoveId = 305;

/// Sonic Boom and Dragon Rage.
pub(super) const GAME_BREAKING_MOVES: [MoveId; 2] = [49, 82];
/// Longest move name the games have room for.
pub(super) const MAX_MOVE_NAME_LENGTH: usize = 24;
pub(super) const MAX_TRAINER_NAME_LENGTH: usize = 10;
pub(super) const MAX_TRAINER_CLASS_NAME_LENGTH: usize = 15;
pub(super) const MAX_TRADE_NICKNAME_LENGTH: usize = 10;
pub(super) const MAX_TRADE_OT_NAME_LENGTH: usize = 7;

/// The 100 TMs: TM01–TM92 are items 328–419, TM93–TM95 618–620 and TM96–TM100 690–694.
pub(super) const TM_COUNT: usize = 100;
pub(super) const TM_BLOCKS: [(ItemId, usize); 3] = [(328, 92), (618, 3), (690, 5)];
/// The bytes right before the item palette table in the executable.
pub(super) const ITEM_PALETTES_PREFIX: &str = "070000000000000000010100";

/// The move tutors (Ultra Sun and Ultra Moon): their moves in the executable follow this, ending in `FFFF`.
pub(super) const TUTORS_PREFIX: &str = "5F6F6E5F6F6666FF";
/// Where the tutor moves are in the shop module, one every 4 bytes.
pub(super) const TUTORS_OFFSET: usize = 0x54DE;
pub(super) const TUTOR_MOVE_COUNT: usize = 67;

pub(super) const FASTEST_TEXT_PREFIXES: [&str; 2] = ["1080BDE80E000500F0412DE9", "34019FE50060A0E3"];
pub(super) const AFTER_BATTLE_FORME_REVERSION_PREFIX: &str = "0055E10B00001A0010A0E30700A0E1";
pub(super) const NINJASK_SPECIES_PREFIX: &str = "11FF2FE11CD08DE2F080BDE8";
pub(super) const SHEDINJA_PREFIX: &str = "A0E194FDFFEB0040A0E1";
pub(super) const BEAST_LUSAMINE_BOOSTS_PREFIX: &str = "1D14FFFF";
/// Beast Lusamine in Sun and Moon, as a position in the trainer list.
pub(super) const BEAST_LUSAMINE_TRAINER: usize = 157;
pub(super) const MINIOR_WILD_ENCOUNTER_PREFIX: &str = "032C42E2062052E2";
/// The script file of the Zygarde that is assembled on Route 16.
pub(super) const ZYGARDE_ASSEMBLY_SCRIPT_FILE: usize = 45;
pub(super) const ZYGARDE_ASSEMBLY_FORME_PREFIX: &str = "BC21CDE1B801CDE1";
pub(super) const ZYGARDE_ASSEMBLY_SPECIES_PREFIX: &str = "FBEB4CD08DE20400A0E1F08FBDE8";
/// The untouched instruction that loads the assembled Zygarde's forme.
pub(super) const ZYGARDE_FORME_LOAD: u32 = 0xE59D_0040;
pub(super) const FRIENDSHIP_FOR_EVOLUTION_LOCATOR: &str = "DC0050E3F700002A";
pub(super) const PERFECT_ODDS_BRANCH_LOCATOR: &str = "050000BA000050E3";
pub(super) const VANILLA_HAPPINESS_TO_EVOLVE: u8 = 220;
pub(super) const EASIER_HAPPINESS_TO_EVOLVE: u8 = 160;

/// The first encounter set of Route 1, whose ninth slot is also met as a static encounter (index 0 of the static
/// encounter file).
pub(super) const ROUTE_1_SET: usize = 0;
pub(super) const ROUTE_1_PIKIPEK_SLOT: usize = 8;
pub(super) const ROUTE_1_PIKIPEK_STATIC: usize = 0;

/// Sizes of the records of the static Pokémon archive's files.
pub(super) const GIFT_SIZE: usize = 0x14;
pub(super) const STATIC_SIZE: usize = 0x38;
pub(super) const TRADE_SIZE: usize = 0x34;
/// The archive's files: gifts (the starters first), static encounters and trades.
pub(super) const GIFTS_FILE: usize = 0;
pub(super) const STATICS_FILE: usize = 1;
pub(super) const TRADES_FILE: usize = 4;

/// The trade texts that name the species, by trade.
pub(super) fn hardcoded_trade_texts(family: Family, trade: usize) -> &'static [usize] {
    match trade {
        // The Route 2 trade names them in Ultra Sun and Ultra Moon only.
        0 if family == Family::UltraSunMoon => &[20, 21, 22],
        1 => &[26, 28, 30],
        2 => &[32, 33, 34, 36],
        3 => &[38, 39, 40, 42],
        4 => &[44, 45, 46, 48],
        5 => &[50, 51, 52, 54],
        6 => &[56, 57, 58, 60],
        _ => &[],
    }
}

/// The text files of the other languages, which get the trainer and class names too.
pub(super) const OTHER_LANGUAGE_TEXTS: [&str; 9] = [
    "TextStringsJaKana",
    "TextStringsJaKanji",
    "TextStringsFr",
    "TextStringsIt",
    "TextStringsDe",
    "TextStringsEs",
    "TextStringsKo",
    "TextStringsZhSimplified",
    "TextStringsZhTraditional",
];

/// The TM item palette of a type.
pub(super) fn tm_palette(t: Option<Type>) -> u16 {
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
        Some(Type::Fairy) => 555,
        Some(Type::Ghost) | None => 322,
    }
}

/// The shops in the order of the shop module.
pub(super) fn shop_names(family: Family) -> Vec<&'static str> {
    let mut names = vec![
        "Primary 0 Trials",
        "Primary 1 Trials",
        "Primary 2 Trials",
        "Primary 3 Trials",
        "Primary 4 Trials",
        "Primary 5 Trials",
        "Primary 6 Trials",
        "Primary 7 Trials",
        "Konikoni City Incenses",
        "Konikoni City Herbs",
        "Hau'oli City Secondary",
        "Route 2 Secondary",
        "Heahea City Secondary (TMs)",
        "Royal Avenue Secondary (TMs)",
        "Route 8 Secondary",
        "Paniola Town Secondary",
        "Malie City Secondary (TMs)",
        "Mount Hokulani Secondary",
        "Seafolk Village Secondary (TMs)",
        "Konikoni City TMs",
        "Konikoni City Stones",
        "Thrifty Megamart, Center-Left",
        "Thrifty Megamart, Center-Right",
        "Thrifty Megamart, Right",
    ];
    if family == Family::UltraSunMoon {
        names.extend([
            "Route 5 Secondary",
            "Konikoni City Secondary",
            "Tapu Village Secondary",
            "Mount Lanakila Secondary",
        ]);
    }
    names
}

/// The types of the Elite Four and the Champion's battles, by trainer group.
pub(super) fn gym_and_elite_themes(family: Family) -> Vec<(&'static str, Type)> {
    let mut themes = vec![
        ("ELITE1", Type::Fighting),
        ("ELITE2", Type::Rock),
        ("ELITE3", Type::Dark),
        ("ELITE4", Type::Ground),
    ];
    match family {
        Family::SunMoon => themes.extend([("ELITE5", Type::Ghost), ("ELITE6", Type::Flying)]),
        Family::UltraSunMoon => themes.extend([
            ("ELITE5", Type::Steel),
            ("ELITE6", Type::Ghost),
            ("ELITE7", Type::Flying),
        ]),
    }
    themes
}

/// Trainer tags: a tag and the trainers (by number, from 1) it applies to.
type Tags = &'static [(&'static str, &'static [usize])];

pub(super) const TRAINER_TAGS_SM: Tags = &[
    ("ELITE1", &[23, 152, 349]),
    ("ELITE2", &[90, 153, 351]),
    ("ELITE3", &[154, 403]),
    ("ELITE4", &[155, 359]),
    ("ELITE5", &[149, 350]),
    ("ELITE6", &[156, 352]),
    ("RIVAL2-0", &[129]),
    ("RIVAL2-1", &[413]),
    ("RIVAL2-2", &[414]),
    (
        "THEMED:GLADION-STRONG",
        &[79, 185, 239, 240, 415, 416, 417, 418, 419, 441],
    ),
    ("THEMED:ILIMA-STRONG", &[52, 215, 216, 396]),
    ("THEMED:LANA-STRONG", &[144]),
    ("THEMED:KIAWE-STRONG", &[398]),
    ("THEMED:MALLOW-STRONG", &[146]),
    ("THEMED:SOPHOCLES-STRONG", &[405]),
    ("THEMED:MOLAYNE-STRONG", &[167, 481]),
    ("THEMED:MINA-STRONG", &[435, 467]),
    ("THEMED:PLUMERIA-STRONG", &[89, 238, 401]),
    ("THEMED:SINA-STRONG", &[75]),
    ("THEMED:DEXIO-STRONG", &[74, 412]),
    ("THEMED:FABA-STRONG", &[132, 241, 360, 410]),
    ("THEMED:GUZMA-LEADER", &[138, 235, 236, 400]),
    ("THEMED:LUSAMINE-LEADER", &[131, 158]),
];

/// Rival tags given to three trainers in a row, `-0` to `-2`, around the middle one (number from 1).
pub(super) const RIVAL_TAGS_SM: &[(&str, usize)] = &[
    ("RIVAL3", 477),
    ("FRIEND1", 6),
    ("FRIEND2", 9),
    ("FRIEND3", 12),
    ("FRIEND4", 76),
    ("FRIEND5", 82),
    ("FRIEND6", 438),
    ("FRIEND7", 217),
    ("FRIEND8", 220),
    ("FRIEND9", 447),
    ("FRIEND10", 450),
    ("FRIEND11", 482),
    ("FRIEND12", 356),
];

pub(super) const TRAINER_TAGS_USUM: Tags = &[
    ("ELITE1", &[23, 650]),
    ("ELITE2", &[90, 153, 351]),
    ("ELITE3", &[154, 508]),
    ("ELITE4", &[359, 497]),
    ("ELITE5", &[489, 490]),
    ("ELITE6", &[149, 350]),
    ("ELITE7", &[156, 352]),
    (
        "THEMED:GLADION-STRONG",
        &[79, 185, 239, 240, 415, 416, 417, 418, 419, 441],
    ),
    ("THEMED:ILIMA-STRONG", &[52, 215, 216, 396, 502]),
    ("THEMED:LANA-STRONG", &[144, 503]),
    ("THEMED:KIAWE-STRONG", &[398, 504]),
    ("THEMED:MALLOW-STRONG", &[146, 505]),
    ("THEMED:SOPHOCLES-STRONG", &[405, 506]),
    ("THEMED:MINA-STRONG", &[507]),
    ("THEMED:PLUMERIA-STRONG", &[89, 238, 401]),
    ("THEMED:SINA-STRONG", &[75]),
    ("THEMED:DEXIO-STRONG", &[74, 412, 623]),
    ("THEMED:FABA-STRONG", &[132, 241, 410, 561]),
    ("THEMED:SOLIERA-STRONG", &[498, 499, 648, 651]),
    ("THEMED:DULSE-STRONG", &[500, 501, 649, 652]),
    ("THEMED:GUZMA-LEADER", &[138, 235, 236, 558, 647]),
    ("THEMED:LUSAMINE-LEADER", &[131, 644]),
    ("UBER", &[541, 542, 543, 580, 572, 573, 559, 560, 562, 645]),
];

pub(super) const RIVAL_TAGS_USUM: &[(&str, usize)] = &[
    ("RIVAL2", 477),
    ("FRIEND1", 491),
    ("FRIEND2", 9),
    ("FRIEND3", 12),
    ("FRIEND4", 76),
    ("FRIEND5", 82),
    ("FRIEND6", 438),
    ("FRIEND7", 217),
    ("FRIEND8", 220),
    ("FRIEND9", 447),
    ("FRIEND10", 450),
    ("FRIEND11", 494),
    ("FRIEND12", 356),
];

/// Trainers (by number, from 1) that always battle together with another: every double battle is a multi battle
/// inside.
pub(super) const MULTI_BATTLES_SM: &[usize] = &[
    92, 93, 97, 98, 134, 136, 141, 227, 241, 262, 265, 270, 278, 280, 299, 303, 307, 315, 316, 331, 332, 371, 372, 373,
    374, 375, 376, 421, 422, 425, 426, 429, 430, 442, 443, 444, 453, 454, 455, 456,
];

pub(super) const MULTI_BATTLES_USUM: &[usize] = &[
    92, 93, 97, 98, 134, 136, 141, 178, 227, 241, 262, 265, 270, 278, 280, 299, 303, 307, 315, 316, 331, 332, 371, 372,
    373, 374, 375, 376, 421, 422, 425, 426, 429, 430, 442, 443, 444, 453, 454, 455, 456, 511, 514, 515, 521, 529, 530,
    534, 544, 557, 561, 578, 586, 595, 613, 617, 618, 619, 620, 626,
];

/// Ultra Sun and Ultra Moon: where the starter stays in the rival teams (positions in the trainer list), Hau's third
/// and sixth battles and Kukui's.
pub(super) const FORCED_STARTER_POSITIONS_USUM: [(usize, u8); 9] = [
    (11, 0),
    (12, 0),
    (13, 0),
    (216, 0),
    (217, 0),
    (218, 0),
    (476, 5),
    (477, 5),
    (478, 5),
];

/// The items trainer Pokémon may be given to hold.
pub(super) fn held_item_pools() -> HeldItemPools {
    HeldItemPools {
        all: ALL_HELD_ITEMS.iter().chain(&CONSUMABLE_HELD_ITEMS).copied().collect(),
        consumable: CONSUMABLE_HELD_ITEMS.to_vec(),
        general_consumable: GENERAL_PURPOSE_CONSUMABLE_ITEMS.to_vec(),
        general: GENERAL_PURPOSE_ITEMS.to_vec(),
        // Liechi Berry, Petaya Berry, Choice Band and Muscle Band.
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

/// Items that may not be random items in Sun and Moon: key items, unused items and the like.
pub(super) const BANNED_ITEMS_SM: [u16; 354] = [
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
    746, 747, 748, 749, 750, 751, 765, 766, 771, 772, 773, 774, 775, 776, 777, 778, 779, 780, 781, 782, 783, 784, 785,
    786, 787, 788, 789, 790, 791, 792, 793, 794, 797, 798, 799, 800, 801, 802, 803, 804, 805, 806, 807, 808, 809, 810,
    811, 812, 813, 814, 815, 816, 817, 818, 819, 820, 821, 822, 823, 824, 825, 826, 827, 828, 829, 830, 831, 832, 833,
    834, 835, 836, 837, 838, 839, 840, 841, 842, 843, 845, 847, 848, 850, 857, 858, 859, 860, 861, 862, 863, 864, 865,
    866, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 885, 886, 887, 888, 889, 890, 891, 892, 893, 894,
    895, 896, 897, 898, 899, 900, 901, 902, 903,
];

/// Items that may not be random items in Ultra Sun and Ultra Moon.
pub(super) const BANNED_ITEMS_USUM: [u16; 393] = [
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
    746, 747, 748, 749, 750, 751, 765, 766, 771, 772, 773, 774, 775, 776, 777, 778, 779, 780, 781, 782, 783, 784, 785,
    786, 787, 788, 789, 790, 791, 792, 793, 794, 797, 798, 799, 800, 801, 802, 803, 804, 805, 806, 807, 808, 809, 810,
    811, 812, 813, 814, 815, 816, 817, 818, 819, 820, 821, 822, 823, 824, 825, 826, 827, 828, 829, 830, 831, 832, 833,
    834, 835, 836, 837, 838, 839, 840, 841, 842, 843, 845, 847, 848, 850, 857, 858, 859, 860, 861, 862, 863, 864, 865,
    866, 867, 868, 869, 870, 871, 872, 873, 874, 875, 876, 877, 878, 885, 886, 887, 888, 889, 890, 891, 892, 893, 894,
    895, 896, 897, 898, 899, 900, 901, 902, 903, 921, 922, 923, 924, 925, 926, 927, 928, 929, 930, 931, 932, 933, 934,
    935, 936, 937, 938, 939, 940, 941, 942, 943, 944, 945, 946, 947, 948, 949, 950, 951, 952, 953, 954, 955, 956, 957,
    958, 959,
];

/// Items useless or nearly so as held items.
pub(super) const BAD_ITEMS: [u16; 147] = [
    65, 66, 67, 68, 69, 70, 71, 95, 96, 97, 98, 111, 112, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146,
    147, 148, 155, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178,
    179, 180, 181, 182, 183, 225, 236, 256, 257, 258, 259, 260, 261, 262, 263, 264, 274, 571, 575, 584, 585, 586, 587,
    588, 589, 590, 652, 653, 654, 655, 656, 657, 658, 659, 660, 661, 662, 663, 664, 665, 666, 667, 668, 669, 670, 671,
    672, 673, 674, 675, 676, 677, 678, 679, 680, 681, 682, 683, 684, 685, 699, 704, 752, 753, 754, 755, 756, 757, 758,
    759, 760, 761, 762, 763, 764, 765, 766, 767, 768, 769, 770, 844, 904, 905, 906, 907, 908, 909, 910, 911, 912, 913,
    914, 915, 916, 917, 918, 919, 920,
];

/// Items too good to sell.
pub(super) const OP_SHOP_ITEMS: [u16; 32] = [
    42, 43, 50, 54, 86, 87, 88, 89, 90, 91, 92, 106, 206, 207, 208, 209, 210, 211, 212, 571, 580, 581, 582, 583, 584,
    585, 586, 587, 588, 589, 590, 591,
];

/// TMs only found as field items in Sun and Moon.
pub(super) const REQUIRED_FIELD_TMS_SM: [u16; 40] = [
    329, 330, 332, 333, 340, 351, 353, 355, 357, 358, 362, 363, 366, 368, 373, 376, 377, 380, 382, 384, 386, 388, 389,
    391, 399, 400, 401, 406, 407, 408, 410, 411, 412, 413, 415, 418, 618, 691, 693, 694,
];

/// TMs only found as field items in Ultra Sun and Ultra Moon.
pub(super) const REQUIRED_FIELD_TMS_USUM: [u16; 42] = [
    329, 330, 332, 333, 340, 350, 351, 353, 355, 357, 358, 359, 362, 363, 366, 368, 373, 376, 377, 380, 382, 384, 386,
    388, 389, 391, 399, 400, 401, 406, 407, 408, 410, 411, 412, 413, 415, 418, 618, 691, 693, 694,
];

/// Better prices for items whose price does not fit their use.
pub(super) const BALANCED_PRICES: [(ItemId, u32); 287] = [
    (1, 3000),
    (5, 500),
    (12, 200),
    (16, 200),
    (38, 3000),
    (39, 4500),
    (40, 15000),
    (41, 18000),
    (44, 5000),
    (53, 25000),
    (93, 5000),
    (112, 10000),
    (116, 1000),
    (117, 1000),
    (118, 1000),
    (119, 1000),
    (134, 150),
    (135, 10000),
    (136, 10000),
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
    (217, 4500),
    (218, 1000),
    (219, 1000),
    (220, 10000),
    (222, 2000),
    (223, 15000),
    (224, 1000),
    (225, 200),
    (226, 3000),
    (227, 3000),
    (230, 3000),
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
    (282, 200),
    (283, 200),
    (284, 200),
    (285, 200),
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
    (316, 1000),
    (319, 15000),
    (320, 1000),
    (321, 3000),
    (322, 3000),
    (323, 3000),
    (324, 3000),
    (325, 3000),
    (334, 20000),
    (338, 20000),
    (341, 20000),
    (342, 20000),
    (345, 20000),
    (352, 20000),
    (355, 20000),
    (364, 20000),
    (365, 20000),
    (377, 20000),
    (379, 20000),
    (386, 20000),
    (395, 20000),
    (397, 20000),
    (398, 20000),
    (492, 300),
    (493, 300),
    (494, 300),
    (495, 300),
    (496, 300),
    (497, 300),
    (498, 300),
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
    (576, 1000),
    (586, 0),
    (618, 20000),
    (619, 20000),
    (639, 2000),
    (640, 6000),
    (644, 2000),
    (645, 5000),
    (646, 3000),
    (647, 3000),
    (648, 200),
    (649, 200),
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
    (692, 20000),
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
    (851, 300),
    (880, 3000),
    (881, 1000),
    (882, 1000),
    (883, 1000),
    (884, 1000),
];

/// Held items used up in battle.
pub(super) const CONSUMABLE_HELD_ITEMS: [u16; 83] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 184, 185, 186, 187, 188, 189, 190, 191,
    192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 43, 214,
    219, 271, 275, 541, 542, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556, 557, 558, 559, 560, 561, 562,
    563, 564, 639, 648, 649, 686, 687, 688, 715, 846, 881, 882, 883, 884,
];

/// Every held item with an effect in battle, without the Z-Crystals.
pub(super) const ALL_HELD_ITEMS: [u16; 166] = [
    213, 217, 220, 221, 222, 230, 232, 233, 234, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250,
    251, 253, 254, 255, 265, 266, 267, 268, 269, 270, 272, 273, 276, 277, 278, 279, 280, 281, 282, 283, 284, 285, 286,
    287, 288, 295, 296, 297, 298, 299, 300, 301, 302, 303, 304, 305, 306, 307, 308, 309, 310, 311, 312, 313, 314, 315,
    316, 317, 318, 326, 327, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 184, 185, 186,
    187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209,
    210, 211, 212, 43, 214, 219, 271, 275, 541, 542, 545, 546, 547, 548, 549, 550, 551, 552, 553, 554, 555, 556, 557,
    558, 559, 560, 561, 562, 563, 564, 538, 539, 540, 543, 544, 639, 649, 686, 687, 688, 715, 640, 644, 650, 846, 881,
    882, 883, 884, 879, 880,
];

/// Consumable held items useful to any Pokémon.
pub(super) const GENERAL_PURPOSE_CONSUMABLE_ITEMS: [u16; 39] = [
    149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 202, 203, 205, 206, 207, 208, 209, 210, 211, 212, 43, 214, 219,
    275, 542, 545, 546, 547, 639, 648, 649, 687, 688, 159, 160, 161, 162, 163, 846,
];

/// Lasting held items useful to any Pokémon.
pub(super) const GENERAL_PURPOSE_ITEMS: [u16; 20] = [
    213, 217, 221, 230, 232, 234, 253, 255, 265, 268, 270, 276, 280, 295, 326, 327, 539, 540, 650, 880,
];

/// Held items that power up attacks of a type.
pub(super) const TYPE_BOOSTING_ITEMS: [(Type, &[ItemId]); 18] = [
    (Type::Normal, &[251]),
    (Type::Fighting, &[241, 303]),
    (Type::Flying, &[244, 306]),
    (Type::Grass, &[239, 301, 318]),
    (Type::Water, &[243, 254, 299, 317]),
    (Type::Fire, &[249, 298]),
    (Type::Rock, &[238, 309, 315]),
    (Type::Ground, &[237, 305]),
    (Type::Psychic, &[248, 307, 314]),
    (Type::Bug, &[222, 308]),
    (Type::Dragon, &[250, 311]),
    (Type::Electric, &[242, 300]),
    (Type::Ghost, &[247, 310]),
    (Type::Poison, &[245, 304]),
    (Type::Ice, &[246, 302]),
    (Type::Steel, &[233, 313]),
    (Type::Dark, &[240, 312]),
    (Type::Fairy, &[644]),
];

/// Held items with an effect for one species only.
pub(super) const SPECIES_BOOSTING_ITEMS: [(DexNumber, &[ItemId]); 11] = [
    (25, &[236]),
    (83, &[259]),
    (104, &[258]),
    (105, &[258]),
    (113, &[256]),
    (132, &[257, 274]),
    (366, &[226, 227]),
    (380, &[225]),
    (381, &[225]),
    (483, &[135]),
    (484, &[136]),
];

/// Abilities that do the same as another, by the one they vary.
pub(super) const ABILITY_VARIATIONS: [(u16, &[u16]); 13] = [
    (4, &[4, 75]),
    (13, &[13, 76]),
    (15, &[15, 72]),
    (24, &[24, 160]),
    (29, &[29, 73, 230]),
    (37, &[37, 74]),
    (104, &[104, 163, 164]),
    (111, &[111, 116, 232]),
    (136, &[136, 231]),
    (183, &[183, 221]),
    (193, &[193, 194]),
    (214, &[214, 219]),
    (222, &[222, 223]),
];

/// Abilities that do nothing for most species.
pub(super) const USELESS_ABILITIES: [u16; 11] = [59, 121, 122, 161, 176, 197, 208, 209, 210, 211, 225];

/// Items that make species evolve.
pub(super) const EVOLUTION_ITEMS: [u16; 27] = [
    80, 81, 82, 83, 84, 85, 107, 108, 109, 110, 221, 226, 227, 233, 235, 252, 321, 322, 323, 324, 325, 326, 327, 537,
    646, 647, 849,
];

/// Moves no random moveset may have: Dark Void and Hyperspace Fury.
pub(super) const BANNED_LEVEL_UP_MOVES: [MoveId; 2] = [464, 621];

/// Shops reached before the Hall of Fame in Sun and Moon.
pub(super) const MAIN_GAME_SHOPS_SM: [usize; 11] = [8, 9, 10, 11, 14, 15, 17, 20, 21, 22, 23];

/// Shops reached before the Hall of Fame in Ultra Sun and Ultra Moon.
pub(super) const MAIN_GAME_SHOPS_USUM: [usize; 15] = [8, 9, 10, 11, 14, 15, 17, 20, 21, 22, 23, 24, 25, 26, 27];

/// Moves that hit two to five times.
pub(super) const NORMAL_MULTIHIT_MOVES: [MoveId; 14] = [3, 4, 31, 42, 131, 140, 154, 198, 292, 331, 333, 350, 541, 594];

/// Moves that hit twice.
pub(super) const DOUBLE_HIT_MOVES: [MoveId; 8] = [24, 41, 155, 458, 530, 544, 742, 751];

/// The location tag of each encounter set of Sun and Moon, as runs of (tag, sets).
pub(super) const LOCATION_TAGS_SM: [(&str, usize); 90] = [
    ("ROUTE 1", 4),
    ("MELEMELE SEA", 5),
    ("ROUTE 1", 7),
    ("MELEMELE SEA", 8),
    ("UNUSED", 2),
    ("HAU'OLI CITY", 6),
    ("ROUTE 3", 1),
    ("KALA'E BAY", 1),
    ("ROUTE 3", 2),
    ("KALA'E BAY", 5),
    ("ROUTE 3", 4),
    ("KALA'E BAY", 2),
    ("ROUTE 2", 9),
    ("TEN CARAT HILL", 2),
    ("UNUSED", 1),
    ("TEN CARAT HILL", 4),
    ("HAU'OLI CEMETERY", 2),
    ("MELEMELE MEADOW", 1),
    ("SEAWARD CAVE", 4),
    ("ROUTE 2", 1),
    ("VERDANT CAVERN", 2),
    ("UNUSED", 1),
    ("HAU'OLI CITY", 4),
    ("ROUTE 4", 7),
    ("PANIOLA TOWN", 2),
    ("UNUSED", 1),
    ("ROUTE 5", 5),
    ("UNUSED", 1),
    ("ROUTE 5", 4),
    ("ROUTE 6", 3),
    ("ROUTE 7", 9),
    ("ROUTE 8", 12),
    ("ROUTE 9", 1),
    ("UNUSED", 1),
    ("HANO BEACH", 4),
    ("MEMORIAL HILL", 2),
    ("AKALA OUTSKIRTS", 7),
    ("DIGLETT'S TUNNEL", 2),
    ("WELA VOLCANO PARK", 5),
    ("BROOKLET HILL", 27),
    ("LUSH JUNGLE", 7),
    ("PANIOLA TOWN", 4),
    ("UNUSED", 1),
    ("MALIE CITY", 3),
    ("ROUTE 10", 4),
    ("UNUSED", 1),
    ("ROUTE 10", 1),
    ("UNUSED", 1),
    ("ROUTE 10", 1),
    ("UNUSED", 2),
    ("ROUTE 10", 2),
    ("ROUTE 12", 1),
    ("SECLUDED SHORE", 4),
    ("ROUTE 12", 10),
    ("ROUTE 13", 3),
    ("TAPU VILLAGE", 1),
    ("ROUTE 14", 7),
    ("ROUTE 15", 4),
    ("ROUTE 16", 1),
    ("ROUTE 15", 1),
    ("ROUTE 16", 2),
    ("ROUTE 15", 1),
    ("UNUSED", 1),
    ("ROUTE 17", 9),
    ("ROUTE 11", 6),
    ("HAINA DESERT", 18),
    ("ULA'ULA MEADOW", 4),
    ("MALIE GARDEN", 4),
    ("UNUSED", 2),
    ("MALIE GARDEN", 9),
    ("UNUSED", 1),
    ("MOUNT HOKULANI", 4),
    ("BLUSH MOUNTAIN", 2),
    ("UNUSED", 5),
    ("MOUNT LANAKILA", 4),
    ("THRIFTY MEGAMART", 1),
    ("SEAFOLK VILLAGE", 2),
    ("PONI WILDS", 19),
    ("PONI BREAKER COAST", 5),
    ("ANCIENT PONI PATH", 3),
    ("PONI GROVE", 6),
    ("UNUSED", 1),
    ("PONI PLAINS", 23),
    ("PONI COAST", 1),
    ("PONI GAUNTLET", 7),
    ("PONI MEADOW", 6),
    ("VAST PONI CANYON", 15),
    ("RESOLUTION CAVE", 2),
    ("UNUSED", 4),
    ("EXEGGUTOR ISLAND", 2),
];

/// The location tag of each encounter set of Ultra Sun and Ultra Moon, as runs of (tag, sets).
pub(super) const LOCATION_TAGS_USUM: [(&str, usize); 97] = [
    ("ROUTE 1", 3),
    ("MELEMELE SEA", 5),
    ("ROUTE 1", 2),
    ("MELEMELE SEA", 1),
    ("ROUTE 1", 6),
    ("UNUSED", 1),
    ("MELEMELE SEA", 7),
    ("UNUSED", 2),
    ("HAU'OLI CITY", 4),
    ("UNUSED", 1),
    ("HAU'OLI CITY", 1),
    ("ROUTE 3", 1),
    ("KALA'E BAY", 1),
    ("ROUTE 3", 2),
    ("KALA'E BAY", 5),
    ("ROUTE 3", 4),
    ("KALA'E BAY", 2),
    ("ROUTE 2", 9),
    ("ROUTE 1", 3),
    ("TEN CARAT HILL", 2),
    ("UNUSED", 1),
    ("TEN CARAT HILL", 4),
    ("HAU'OLI CEMETERY", 2),
    ("MELEMELE MEADOW", 1),
    ("SEAWARD CAVE", 4),
    ("ROUTE 2", 1),
    ("SANDY CAVE", 1),
    ("VERDANT CAVERN", 2),
    ("UNUSED", 1),
    ("HAU'OLI CITY", 4),
    ("ROUTE 4", 7),
    ("PANIOLA TOWN", 2),
    ("UNUSED", 1),
    ("ROUTE 5", 5),
    ("UNUSED", 1),
    ("ROUTE 5", 4),
    ("ROUTE 6", 3),
    ("ROUTE 7", 9),
    ("ROUTE 8", 12),
    ("ROUTE 9", 2),
    ("HANO BEACH", 4),
    ("DIVIDING PEAK TUNNEL", 3),
    ("MEMORIAL HILL", 2),
    ("AKALA OUTSKIRTS", 7),
    ("DIGLETT'S TUNNEL", 2),
    ("WELA VOLCANO PARK", 5),
    ("BROOKLET HILL", 26),
    ("LUSH JUNGLE", 7),
    ("PANIOLA TOWN", 4),
    ("UNUSED", 1),
    ("MALIE CITY", 3),
    ("ROUTE 10", 4),
    ("UNUSED", 1),
    ("ROUTE 10", 1),
    ("UNUSED", 1),
    ("ROUTE 10", 1),
    ("UNUSED", 2),
    ("ROUTE 10", 2),
    ("ROUTE 12", 11),
    ("ROUTE 13", 3),
    ("TAPU VILLAGE", 1),
    ("ROUTE 14", 7),
    ("ROUTE 15", 4),
    ("ROUTE 16", 1),
    ("ROUTE 15", 1),
    ("ROUTE 16", 2),
    ("ROUTE 15", 1),
    ("UNUSED", 1),
    ("ROUTE 17", 9),
    ("ROUTE 11", 6),
    ("ULA'ULA BEACH", 1),
    ("HAINA DESERT", 18),
    ("ULA'ULA MEADOW", 4),
    ("MALIE GARDEN", 4),
    ("UNUSED", 2),
    ("MALIE GARDEN", 9),
    ("UNUSED", 1),
    ("MOUNT HOKULANI", 4),
    ("BLUSH MOUNTAIN", 1),
    ("UNUSED", 4),
    ("BLUSH MOUNTAIN", 1),
    ("UNUSED", 1),
    ("MOUNT LANAKILA", 7),
    ("THRIFTY MEGAMART", 1),
    ("SEAFOLK VILLAGE", 2),
    ("PONI WILDS", 11),
    ("PONI BREAKER COAST", 8),
    ("ANCIENT PONI PATH", 3),
    ("PONI GROVE", 6),
    ("UNUSED", 1),
    ("PONI PLAINS", 23),
    ("PONI COAST", 1),
    ("PONI GAUNTLET", 7),
    ("PONI MEADOW", 6),
    ("VAST PONI CANYON", 15),
    ("RESOLUTION CAVE", 2),
    ("EXEGGUTOR ISLAND", 2),
];

/// How each encounter set of Sun and Moon is met, as runs of (kind, sets).
pub(super) const ENCOUNTER_KINDS_SM: [(EncounterKind, usize); 132] = [
    (EncounterKind::Walking, 4),
    (EncounterKind::Fishing, 5),
    (EncounterKind::Walking, 7),
    (EncounterKind::Surfing, 2),
    (EncounterKind::Fishing, 5),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Unused, 2),
    (EncounterKind::Walking, 8),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Fishing, 5),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Walking, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 3),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 3),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 8),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 10),
    (EncounterKind::Interact, 1),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 7),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 6),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Fishing, 4),
    (EncounterKind::Interact, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 5),
    (EncounterKind::Fishing, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Ambush, 2),
    (EncounterKind::Walking, 3),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 3),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 7),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 12),
    (EncounterKind::Walking, 3),
    (EncounterKind::Surfing, 3),
    (EncounterKind::Fishing, 7),
    (EncounterKind::Walking, 2),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 8),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 4),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 2),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Unused, 2),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 10),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 1),
    (EncounterKind::Fishing, 6),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 3),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 17),
    (EncounterKind::Ambush, 6),
    (EncounterKind::Walking, 4),
    (EncounterKind::Ambush, 4),
    (EncounterKind::Walking, 5),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Unused, 2),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Walking, 7),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 6),
    (EncounterKind::Unused, 5),
    (EncounterKind::Walking, 5),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Walking, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Fishing, 4),
    (EncounterKind::Interact, 3),
    (EncounterKind::Walking, 7),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Fishing, 4),
    (EncounterKind::Walking, 9),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 6),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 14),
    (EncounterKind::Ambush, 2),
    (EncounterKind::Walking, 1),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 4),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Walking, 11),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Ambush, 2),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 3),
    (EncounterKind::Unused, 4),
    (EncounterKind::Walking, 2),
];

/// How each encounter set of Ultra Sun and Ultra Moon is met, as runs of (kind, sets).
pub(super) const ENCOUNTER_KINDS_USUM: [(EncounterKind, usize); 133] = [
    (EncounterKind::Walking, 3),
    (EncounterKind::Fishing, 5),
    (EncounterKind::Walking, 2),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 6),
    (EncounterKind::Unused, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 5),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Unused, 2),
    (EncounterKind::Walking, 4),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 3),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Fishing, 5),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Walking, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 3),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 6),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 8),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Interact, 1),
    (EncounterKind::Special, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 10),
    (EncounterKind::Interact, 1),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 7),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 6),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Fishing, 4),
    (EncounterKind::Interact, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 5),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Ambush, 5),
    (EncounterKind::Walking, 3),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 3),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 7),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Fishing, 11),
    (EncounterKind::Walking, 3),
    (EncounterKind::Surfing, 3),
    (EncounterKind::Fishing, 7),
    (EncounterKind::Walking, 2),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 8),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 4),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Walking, 2),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Unused, 2),
    (EncounterKind::Ambush, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 11),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 1),
    (EncounterKind::Fishing, 6),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 3),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 13),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 4),
    (EncounterKind::Ambush, 6),
    (EncounterKind::Walking, 4),
    (EncounterKind::Ambush, 4),
    (EncounterKind::Walking, 5),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Unused, 2),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Walking, 7),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 5),
    (EncounterKind::Unused, 4),
    (EncounterKind::Walking, 1),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 8),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Walking, 1),
    (EncounterKind::Interact, 3),
    (EncounterKind::Walking, 7),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Ambush, 3),
    (EncounterKind::Fishing, 4),
    (EncounterKind::Walking, 9),
    (EncounterKind::Unused, 1),
    (EncounterKind::Walking, 1),
    (EncounterKind::Ambush, 6),
    (EncounterKind::Interact, 1),
    (EncounterKind::Walking, 14),
    (EncounterKind::Ambush, 2),
    (EncounterKind::Walking, 1),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 4),
    (EncounterKind::Fishing, 2),
    (EncounterKind::Walking, 11),
    (EncounterKind::Surfing, 1),
    (EncounterKind::Ambush, 2),
    (EncounterKind::Fishing, 3),
    (EncounterKind::Walking, 5),
];

/// Encounter sets of Sun and Moon only reached after the main story, or never.
pub(super) const POST_GAME_SETS_SM: [usize; 71] = [
    332, 333, 334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354,
    355, 356, 357, 358, 359, 360, 361, 362, 370, 371, 372, 373, 374, 375, 363, 364, 365, 366, 367, 368, 369, 391, 392,
    24, 25, 58, 73, 87, 93, 123, 182, 190, 192, 194, 195, 233, 275, 276, 286, 293, 294, 295, 296, 297, 393, 394, 395,
    396,
];

/// Encounter sets of Ultra Sun and Ultra Moon only reached after the main story, or never.
pub(super) const POST_GAME_SETS_USUM: [usize; 68] = [
    334, 335, 336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356,
    357, 358, 359, 360, 361, 362, 363, 364, 372, 373, 374, 375, 376, 377, 365, 366, 367, 368, 369, 370, 371, 393, 394,
    25, 26, 31, 32, 62, 78, 92, 98, 189, 197, 199, 201, 202, 236, 279, 280, 290, 296, 297, 298, 299, 301,
];
