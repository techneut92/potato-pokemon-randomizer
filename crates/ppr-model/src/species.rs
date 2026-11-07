use serde::{Deserialize, Serialize};

use crate::{ExpCurve, ItemId, Type};

/// National Pokédex number. 0 never names a species.
pub type DexNumber = u16;

/// Shedinja's national number; its HP is always 1, which the BST accounts for.
const SHEDINJA: DexNumber = 292;
const PIKACHU: DexNumber = 25;

/// The six base stats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[allow(missing_docs)]
pub struct BaseStats {
    pub hp: u8,
    pub attack: u8,
    pub defense: u8,
    pub speed: u8,
    pub sp_attack: u8,
    pub sp_defense: u8,
}

impl BaseStats {
    /// Sum of all six stats.
    pub fn total(&self) -> u16 {
        [
            self.hp,
            self.attack,
            self.defense,
            self.speed,
            self.sp_attack,
            self.sp_defense,
        ]
        .iter()
        .map(|&s| u16::from(s))
        .sum()
    }
}

/// One species as the randomizers see it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Species {
    /// National Pokédex number.
    pub number: DexNumber,
    /// Name as stored in the ROM.
    pub name: String,
    /// First type.
    pub primary_type: Type,
    /// Second type; `None` when the ROM stores the same type twice.
    pub secondary_type: Option<Type>,
    /// Base stats.
    pub stats: BaseStats,
    /// Catch rate, 3 (hardest) to 255 (easiest).
    pub catch_rate: u8,
    /// Ability numbers, one per slot the game has (two in Gen 3); 0 for an empty slot. Empty before Gen 3.
    pub abilities: Vec<u16>,
    /// How much experience it needs per level.
    pub growth_curve: ExpCurve,
    /// What makes it a forme: its base species, forme number and flags; the default for a species without formes.
    #[serde(default)]
    pub forme: Forme,
}

/// How a species relates to its formes. Alternate formes with data of their own (Deoxys's, Rotom's appliances) are
/// species entries of their own, numbered after the last national number as the game data orders them, and linked
/// to their base species here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Forme {
    /// For an alternate forme, the species it is a forme of; `None` for a base species.
    pub base: Option<DexNumber>,
    /// Its forme number, as trainer, wild and static Pokémon store it; 0 for a base species.
    pub number: u8,
    /// What the full name adds to the name, e.g. `-Origin`; base species of some formed species have one too
    /// (`-Plant`).
    pub suffix: String,
    /// An alternate forme that only differs in looks from its base species (Keldeo's Resolute Forme): it copies what
    /// its base species gets and is never chosen as itself.
    pub cosmetic: bool,
    /// An alternate forme that only lasts in battle or under some condition (Castform's weather formes, Darmanitan's
    /// Zen Mode, Kyurem's fused formes): "No Irregular Alt Formes" leaves it out.
    pub irregular: bool,
    /// For a base species, the forme numbers of its cosmetic formes without data of their own (Unown's letters,
    /// Shellos's East Sea) that random formes may pick; empty when there are none.
    pub cosmetic_formes: Vec<u8>,
    /// A Mega Evolution, which its base species turns into in battle.
    #[serde(default)]
    pub mega: bool,
    /// The Mega Stone a Mega Evolution needs; `None` when it needs none (Mega Rayquaza) or is no Mega Evolution.
    #[serde(default)]
    pub mega_item: Option<ItemId>,
    /// For an alternate forme that is a forme of another alternate forme in all but the data, that forme (the
    /// Totem Alolan Raticate's is the Alolan Raticate): what it copies from instead of its base species.
    #[serde(default)]
    pub conceptual_base: Option<DexNumber>,
}

impl Species {
    /// Base stat total as used for power levels. Shedinja ignores its HP and counts 6/5 of the rest, because its 1 HP
    /// makes it much stronger than the plain sum suggests.
    pub fn bst(&self) -> u16 {
        if self.number == SHEDINJA {
            (self.stats.total() - u16::from(self.stats.hp)) * 6 / 5
        } else {
            self.stats.total()
        }
    }

    /// True when the species has `t` as either type.
    pub fn has_type(&self, t: Type) -> bool {
        self.primary_type == t || self.secondary_type == Some(t)
    }

    /// True for legendary and mythical species, by a fixed list for Gen 1–7; an alternate forme is one when its
    /// base species is.
    pub fn is_legendary(&self) -> bool {
        is_legendary(self.base_number())
    }

    /// True for the strongest legendaries, alternate formes by their base species.
    pub fn is_strong_legendary(&self) -> bool {
        is_strong_legendary(self.base_number())
    }

    /// The generation that introduced it. An alternate forme's is its base species', except for those a later
    /// generation added: Alolan formes and the like (7), Mega Evolutions, Primal Reversions and Pikachu's formes (6).
    pub fn generation(&self) -> u8 {
        let suffix = self.forme.suffix.as_str();
        if matches!(suffix, "-Alolan" | "-Ash" | "-10p" | "-Complete") {
            7
        } else if self.forme.mega
            || suffix.starts_with("-Mega")
            || suffix == "-Primal"
            || (!self.is_base_forme() && self.base_number() == PIKACHU)
        {
            6
        } else {
            generation_of(self.base_number())
        }
    }

    /// A Mega Evolution.
    pub fn is_mega(&self) -> bool {
        self.forme.mega
    }

    /// True for a base species, false for an alternate forme.
    pub fn is_base_forme(&self) -> bool {
        self.forme.base.is_none()
    }

    /// The number of its base species: its own for a base species.
    pub fn base_number(&self) -> DexNumber {
        self.forme.base.unwrap_or(self.number)
    }

    /// For an alternate forme, the species it copies what it gets from: its conceptual base forme, else its base
    /// species; `None` for a base species.
    pub fn conceptual_base(&self) -> Option<DexNumber> {
        self.forme.conceptual_base.or(self.forme.base)
    }

    /// An alternate forme that differs in more than looks.
    pub fn is_functional_forme(&self) -> bool {
        !self.is_base_forme() && !self.forme.cosmetic
    }

    /// The name and forme suffix, which tells formes apart: `GIRATINA-Origin`.
    pub fn full_name(&self) -> String {
        format!("{}{}", self.name, self.forme.suffix)
    }
}

/// The legendary and mythical species, Gen 1–7.
const LEGENDARIES: [DexNumber; 68] = [
    144, 145, 146, 150, 151, 243, 244, 245, 249, 250, 251, 377, 378, 379, 380, 381, 382, 383, 384, 385, 386, 480, 481,
    482, 483, 484, 485, 486, 487, 488, 489, 490, 491, 492, 493, 494, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647,
    648, 649, 716, 717, 718, 719, 720, 721, 772, 773, 785, 786, 787, 788, 789, 790, 791, 792, 800, 801, 802, 807,
];

/// The last national number of each generation, 1 to 7.
const GENERATION_ENDS: [DexNumber; 7] = [151, 251, 386, 493, 649, 721, 809];

/// The generation that introduced the species with this national number (of its base forme); 0 for numbers past
/// generation 7.
pub fn generation_of(number: DexNumber) -> u8 {
    GENERATION_ENDS
        .iter()
        .position(|&end| number <= end)
        .and_then(|i| u8::try_from(i + 1).ok())
        .unwrap_or(0)
}

/// True when `number` is a legendary or mythical species.
pub fn is_legendary(number: DexNumber) -> bool {
    LEGENDARIES.binary_search(&number).is_ok()
}

/// The cover legendaries and the like, by national number.
const STRONG_LEGENDARIES: [DexNumber; 20] = [
    150, 249, 250, 382, 383, 384, 483, 484, 486, 487, 493, 643, 644, 646, 716, 717, 789, 790, 791, 792,
];

/// True for the strongest legendaries, like Mewtwo and Rayquaza.
pub fn is_strong_legendary(number: DexNumber) -> bool {
    STRONG_LEGENDARIES.binary_search(&number).is_ok()
}

/// The Ultra Beasts, by national number.
const ULTRA_BEASTS: [DexNumber; 11] = [793, 794, 795, 796, 797, 798, 799, 803, 804, 805, 806];

/// True for the Ultra Beasts, like Nihilego and Poipole. Only for base species: an alternate forme's own number can
/// be the national number of an Ultra Beast in a game without it (Sun and Moon).
pub fn is_ultra_beast(number: DexNumber) -> bool {
    ULTRA_BEASTS.binary_search(&number).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn species(number: DexNumber, stats: [u8; 6]) -> Species {
        let [hp, attack, defense, speed, sp_attack, sp_defense] = stats;
        Species {
            number,
            name: String::new(),
            primary_type: Type::Bug,
            secondary_type: Some(Type::Ghost),
            stats: BaseStats {
                hp,
                attack,
                defense,
                speed,
                sp_attack,
                sp_defense,
            },
            catch_rate: 45,
            abilities: Vec::new(),
            growth_curve: crate::ExpCurve::MediumFast,
            forme: Forme::default(),
        }
    }

    #[test]
    fn formes_count_as_their_base_species() {
        let mut origin = species(507, [150, 120, 100, 90, 120, 100]);
        origin.forme = Forme {
            base: Some(487),
            number: 1,
            suffix: "-Origin".into(),
            ..Forme::default()
        };
        origin.name = "GIRATINA".into();
        assert!(origin.is_legendary() && origin.is_strong_legendary());
        assert_eq!((origin.generation(), origin.base_number()), (4, 487));
        assert!(origin.is_functional_forme() && !origin.is_base_forme());
        assert_eq!(origin.full_name(), "GIRATINA-Origin");
    }

    #[test]
    fn formes_of_later_generations_count_as_theirs() {
        let mut mega = species(760, [80, 100, 123, 80, 122, 120]);
        mega.forme = Forme {
            base: Some(3),
            number: 1,
            suffix: "-Mega".into(),
            mega: true,
            mega_item: Some(659),
            ..Forme::default()
        };
        assert!(mega.is_mega());
        assert_eq!(mega.generation(), 6);
        mega.forme.suffix = "-Alolan".into();
        assert_eq!(mega.generation(), 7);
        let mut cosplay = species(815, [45, 80, 50, 120, 75, 60]);
        cosplay.forme.base = Some(25);
        assert_eq!(cosplay.generation(), 6);
    }

    #[test]
    fn bst_with_the_shedinja_rule() {
        assert_eq!(species(1, [45, 49, 49, 45, 65, 65]).bst(), 318);
        // Shedinja: 90 + 45 + 40 + 30 + 30 = 235, times 6/5 = 282.
        assert_eq!(species(SHEDINJA, [1, 90, 45, 40, 30, 30]).bst(), 282);
    }

    #[test]
    fn legendary_list_is_sorted_and_complete() {
        assert!(LEGENDARIES.is_sorted());
        assert!(STRONG_LEGENDARIES.is_sorted());
        assert!(STRONG_LEGENDARIES.iter().all(|&n| is_legendary(n)));
        assert!(is_strong_legendary(384) && !is_strong_legendary(380));
        assert!(is_legendary(151) && is_legendary(386) && is_legendary(807));
        assert!(!is_legendary(152) && !is_legendary(0));
    }

    #[test]
    fn ultra_beasts_are_sorted_and_not_legendary() {
        assert!(ULTRA_BEASTS.is_sorted());
        assert!(ULTRA_BEASTS.iter().all(|&n| !is_legendary(n)));
        assert!(is_ultra_beast(793) && is_ultra_beast(799) && is_ultra_beast(803) && is_ultra_beast(806));
        assert!(!is_ultra_beast(792) && !is_ultra_beast(800) && !is_ultra_beast(807));
    }
}
