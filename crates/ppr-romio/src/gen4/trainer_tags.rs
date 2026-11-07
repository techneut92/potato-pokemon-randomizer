//! Which trainers are gym trainers, leaders, the Elite Four, rivals, friends and other notable trainers, and
//! which battle alongside another trainer, by trainer number.

use ppr_model::{MultiBattleStatus, Trainer};

use super::constants::Family;

/// One tagging step, applied in order, later ones winning.
enum Tag {
    /// The tag for each of these trainers.
    Plain(&'static str, &'static [u16]),
    /// A rival battle for each starter, the trainers after `fire`: `fire` gets `-0`, the next `-1`, the one before
    /// `-2`.
    Rival(&'static str, u16),
    /// A friend's tag battle: `grass` gets `-1`, the next `-2`, the one after `-0`.
    Friend(&'static str, u16),
    /// A friend's tag battle: `water` gets `-0`, the next `-1`, the one after `-2`.
    Friend2(&'static str, u16),
}

use Tag::{Friend, Friend2, Plain, Rival};

const DP: &[Tag] = &[
    Plain("GYM1", &[0xf4, 0xf5]),
    Plain("GYM2", &[0x144, 0x103, 0x104, 0x15C]),
    Plain("GYM3", &[0x135, 0x136, 0x137, 0x138]),
    Plain("GYM4", &[0x1f1, 0x1f2, 0x191, 0x153, 0x125, 0x1E3]),
    Plain("GYM5", &[0x165, 0x145, 0x10a, 0x14a, 0x154, 0x157, 0x118, 0x11c]),
    Plain("GYM6", &[0x13a, 0x100, 0x101, 0x117, 0x16f, 0xe8, 0x11b]),
    Plain("GYM7", &[0x10c, 0x10d, 0x10e, 0x10f, 0x33b, 0x33c]),
    Plain("GYM8", &[0x158, 0x155, 0x12d, 0x12e, 0x12f, 0x11d, 0x119]),
    Plain("GYM1-LEADER", &[0xf6]),
    Plain("GYM2-LEADER", &[0x13b]),
    Plain("GYM3-LEADER", &[0x13d]),
    Plain("GYM4-LEADER", &[0x13c]),
    Plain("GYM5-LEADER", &[0x13e]),
    Plain("GYM6-LEADER", &[0xfa]),
    Plain("GYM7-LEADER", &[0x13f]),
    Plain("GYM8-LEADER", &[0x140]),
    Plain("ELITE1", &[0x105]),
    Plain("ELITE2", &[0x106]),
    Plain("ELITE3", &[0x107]),
    Plain("ELITE4", &[0x108]),
    Plain("CHAMPION", &[0x10b]),
    Rival("RIVAL2", 0xf8),
    Rival("RIVAL3", 0x1d7),
    Rival("RIVAL4", 0x1da),
    Rival("RIVAL5", 0x1dd),
    // The tag battle doesn't follow the usual order.
    Plain("RIVAL6-0", &[0x26b]),
    Plain("RIVAL6-1", &[0x26c]),
    Plain("RIVAL6-2", &[0x25f]),
    Rival("RIVAL7", 0x1e0),
    Rival("RIVAL8", 0x346),
    Rival("RIVAL9", 0x349),
    Plain("THEMED:CYRUS-LEADER", &[0x193, 0x194]),
    Plain("THEMED:MARS-STRONG", &[0x127, 0x195, 0x210]),
    Plain("THEMED:JUPITER-STRONG", &[0x196, 0x197]),
    Plain("THEMED:SATURN-STRONG", &[0x198, 0x199]),
    Friend("FRIEND1", 0x265),
    Friend("FRIEND1", 0x268),
    Friend2("FRIEND2", 0x26D),
    Friend2("FRIEND2", 0x270),
];

const PT: &[Tag] = &[
    Plain("GYM1", &[0xf4, 0xf5]),
    Plain("GYM2", &[0x144, 0x103, 0x104, 0x15C]),
    Plain("GYM3", &[0x165, 0x145, 0x154, 0x157, 0x118, 0x11c]),
    Plain("GYM4", &[0x135, 0x136, 0x137, 0x138]),
    Plain("GYM5", &[0x1f1, 0x1f2, 0x191, 0x153, 0x125, 0x1E3]),
    Plain("GYM6", &[0x13a, 0x100, 0x101, 0x117, 0x16f, 0xe8, 0x11b]),
    Plain("GYM7", &[0x10c, 0x10d, 0x10e, 0x10f, 0x33b, 0x33c]),
    Plain("GYM8", &[0x158, 0x155, 0x12d, 0x12e, 0x12f, 0x11d, 0x119, 0x14b]),
    Plain("GYM1-LEADER", &[0xf6]),
    Plain("GYM2-LEADER", &[0x13b]),
    Plain("GYM3-LEADER", &[0x13e]),
    Plain("GYM4-LEADER", &[0x13d]),
    Plain("GYM5-LEADER", &[0x13c]),
    Plain("GYM6-LEADER", &[0xfa]),
    Plain("GYM7-LEADER", &[0x13f]),
    Plain("GYM8-LEADER", &[0x140]),
    Plain("ELITE1", &[0x105]),
    Plain("ELITE2", &[0x106]),
    Plain("ELITE3", &[0x107]),
    Plain("ELITE4", &[0x108]),
    Plain("CHAMPION", &[0x10b]),
    Rival("RIVAL1", 0x353),
    Rival("RIVAL2", 0xf8),
    Rival("RIVAL3", 0x1d7),
    Rival("RIVAL4", 0x1da),
    Rival("RIVAL5", 0x1dd),
    Plain("RIVAL6-0", &[0x26b]),
    Plain("RIVAL6-1", &[0x26c]),
    Plain("RIVAL6-2", &[0x25f]),
    Rival("RIVAL7", 0x1e0),
    Rival("RIVAL8", 0x346),
    Rival("RIVAL9", 0x349),
    Rival("RIVAL10", 0x368),
    // The gym leaders at the Battleground.
    Plain("GYM1", &[0x35A]),
    Plain("GYM2", &[0x359]),
    Plain("GYM3", &[0x35C]),
    Plain("GYM4", &[0x356]),
    Plain("GYM5", &[0x35B]),
    Plain("GYM6", &[0x358]),
    Plain("GYM7", &[0x355]),
    Plain("GYM8", &[0x357]),
    // Volkner and Flint at the Battle Frontier.
    Plain("GYM8", &[0x399]),
    Plain("ELITE3", &[0x39A]),
    // The Elite Four rematches.
    Plain("ELITE1", &[0x362]),
    Plain("ELITE2", &[0x363]),
    Plain("ELITE3", &[0x364]),
    Plain("ELITE4", &[0x365]),
    Plain("CHAMPION", &[0x366]),
    Plain("THEMED:CYRUS-LEADER", &[0x391, 0x193, 0x194]),
    Plain("THEMED:MARS-STRONG", &[0x127, 0x195, 0x210, 0x39e]),
    Plain("THEMED:JUPITER-STRONG", &[0x196, 0x197, 0x39f]),
    Plain("THEMED:SATURN-STRONG", &[0x198, 0x199]),
    Friend("FRIEND1", 0x265),
    Friend("FRIEND1", 0x268),
    Friend2("FRIEND2", 0x26D),
    Friend2("FRIEND2", 0x270),
];

const HGSS: &[Tag] = &[
    Plain("GYM1", &[0x32, 0x1D]),
    Plain("GYM2", &[0x43, 0x44, 0x45, 0x0a]),
    Plain("GYM3", &[0x05, 0x46, 0x47, 0x16]),
    Plain("GYM4", &[0x1ed, 0x1ee, 0x59, 0x2e]),
    Plain("GYM5", &[0x9c, 0x9d, 0x9f, 0xfb]),
    Plain("GYM7", &[0x1e0, 0x1e1, 0x1e2, 0x1e3, 0x1e4]),
    Plain("GYM8", &[0x6e, 0x6f, 0x70, 0x75, 0x77]),
    Plain("GYM9", &[0x134, 0x2ad]),
    Plain("GYM10", &[0x2a4, 0x2a5, 0x2a6, 0x129, 0x12a]),
    Plain("GYM11", &[0x18c, 0xe8, 0x151]),
    Plain("GYM12", &[0x150, 0x146, 0x164, 0x15a]),
    Plain("GYM13", &[0x53, 0x54, 0xb7, 0x88]),
    Plain("GYM14", &[0x170, 0x171, 0xe6, 0x19f]),
    Plain("GYM15", &[0x2b1, 0x2b2, 0x2b3, 0x2b4, 0x2b5, 0x2b6]),
    Plain("GYM16", &[0x2a9, 0x2aa, 0x2ab, 0x2ac]),
    Plain("GYM1-LEADER", &[0x14]),
    Plain("GYM2-LEADER", &[0x15]),
    Plain("GYM3-LEADER", &[0x1e]),
    Plain("GYM4-LEADER", &[0x1f]),
    Plain("GYM5-LEADER", &[0x22]),
    Plain("GYM6-LEADER", &[0x21]),
    Plain("GYM7-LEADER", &[0x20]),
    Plain("GYM8-LEADER", &[0x23]),
    Plain("GYM9-LEADER", &[0xFD]),
    Plain("GYM10-LEADER", &[0xFE]),
    Plain("GYM11-LEADER", &[0xFF]),
    Plain("GYM12-LEADER", &[0x100]),
    Plain("GYM13-LEADER", &[0x101]),
    Plain("GYM14-LEADER", &[0x102]),
    Plain("GYM15-LEADER", &[0x103]),
    Plain("GYM16-LEADER", &[0x105]),
    Plain("ELITE1", &[0xf5]),
    Plain("ELITE2", &[0xf7]),
    Plain("ELITE3", &[0x1a2]),
    Plain("ELITE4", &[0xf6]),
    Plain("CHAMPION", &[0xf4]),
    // Red.
    Plain("UBER", &[0x104]),
    // The gym leader rematches.
    Plain("GYM1-LEADER", &[0x2c8]),
    Plain("GYM2-LEADER", &[0x2c9]),
    Plain("GYM3-LEADER", &[0x2ca]),
    Plain("GYM4-LEADER", &[0x2cb]),
    Plain("GYM5-LEADER", &[0x2ce]),
    Plain("GYM6-LEADER", &[0x2cd]),
    Plain("GYM7-LEADER", &[0x2cc]),
    Plain("GYM8-LEADER", &[0x2cf]),
    Plain("GYM9-LEADER", &[0x2d0]),
    Plain("GYM10-LEADER", &[0x2d1]),
    Plain("GYM11-LEADER", &[0x2d2]),
    Plain("GYM12-LEADER", &[0x2d3]),
    Plain("GYM13-LEADER", &[0x2d4]),
    Plain("GYM14-LEADER", &[0x2d5]),
    Plain("GYM15-LEADER", &[0x2d6]),
    Plain("GYM16-LEADER", &[0x2d7]),
    // The Elite Four rematches.
    Plain("ELITE1", &[0x2be]),
    Plain("ELITE2", &[0x2bf]),
    Plain("ELITE3", &[0x2c0]),
    Plain("ELITE4", &[0x2c1]),
    Plain("CHAMPION", &[0x2bd]),
    Rival("RIVAL1", 0x1F0),
    Plain("RIVAL2-0", &[0x10a]),
    Plain("RIVAL2-1", &[0x10d]),
    Plain("RIVAL2-2", &[0x1]),
    Plain("RIVAL3-0", &[0x10B]),
    Plain("RIVAL3-1", &[0x10E]),
    Plain("RIVAL3-2", &[0x107]),
    Plain("RIVAL4-0", &[0x121]),
    Plain("RIVAL4-1", &[0x10f]),
    Plain("RIVAL4-2", &[0x120]),
    Plain("RIVAL5-0", &[0x10C]),
    Plain("RIVAL5-1", &[0x110]),
    Plain("RIVAL5-2", &[0x108]),
    Rival("RIVAL6", 0x11e),
    // The tag battle in the Dragon's Den.
    Rival("RIVAL7", 0x2e0),
    Rival("RIVAL8", 0x1EA),
    // Clair and Lance in the Dragon's Den.
    Plain("GYM8", &[0x2DE]),
    Plain("CHAMPION", &[0x2DD]),
    Plain("KIMONO1-STRONG", &[0xa0]),
    Plain("KIMONO2-STRONG", &[0xa1]),
    Plain("KIMONO3-STRONG", &[0xa2]),
    Plain("KIMONO4-STRONG", &[0xa3]),
    Plain("KIMONO5-STRONG", &[0xa4]),
    Plain("THEMED:ARIANA-STRONG", &[0x1df, 0x1de]),
    Plain("THEMED:PETREL-STRONG", &[0x1e8, 0x1e7]),
    Plain("THEMED:PROTON-STRONG", &[0x1e6, 0x2c2]),
    Plain("THEMED:SPROUTTOWER", &[0x2b, 0x33, 0x34, 0x35, 0x36, 0x37, 0x122]),
    // Archer.
    Plain("LEADER", &[485]),
];

/// Trainers who always battle alongside another, in Diamond and Pearl; Platinum adds Flint and Volkner at the
/// Fight Area.
const DP_ALWAYS_MULTI: &[u16] = &[
    414, 415, 419, 420, 426, 427, 521, 527, 528, 561, 590, 835, 836, 848, 849,
];
const PT_ALWAYS_MULTI: &[u16] = &[921, 922];
/// Trainers who battle alongside another when the player walks into both, in Diamond, Pearl and Platinum.
const DPPT_POTENTIAL_MULTI: &[u16] = &[
    34, 35, 82, 83, 84, 85, 95, 96, 104, 106, 160, 186, 191, 201, 202, 203, 204, 205, 206, 278, 287, 337, 358, 359,
    360, 372, 373, 379, 383, 386, 388, 389, 390, 392, 393, 394, 395, 396, 397, 398, 399, 400, 443, 445, 446, 447, 450,
    452, 453, 454, 455, 459, 494, 496, 499, 500, 505, 506, 554, 555, 556, 560, 562, 566, 569, 570, 571, 572, 575, 579,
    580, 581, 582, 584, 585, 586, 589, 591, 594, 596, 599, 602, 606,
];
const HGSS_ALWAYS_MULTI: &[u16] = &[120, 354, 355, 417, 479, 499, 679, 680, 733, 734];
/// Pokéfan Georgia, Jeremy's partner, has more than three Pokémon, so she is left out.
const HGSS_POTENTIAL_MULTI: &[u16] = &[147, 151, 423, 564, 567, 575, 576, 577, 579];

fn set_tag(trainers: &mut [Trainer], number: u16, tag: &str) {
    if let Some(t) = usize::from(number).checked_sub(1).and_then(|i| trainers.get_mut(i)) {
        t.tag = Some(tag.to_owned());
    }
}

/// Tags the trainers of the game family and sets which battle alongside another.
pub(super) fn tag(trainers: &mut [Trainer], family: Family) {
    let tags = match family {
        Family::DiamondPearl => DP,
        Family::Platinum => PT,
        Family::HeartGoldSoulSilver => HGSS,
    };
    for t in tags {
        match *t {
            Plain(tag, numbers) => {
                for &n in numbers {
                    set_tag(trainers, n, tag);
                }
            }
            Rival(tag, fire) => {
                set_tag(trainers, fire, &format!("{tag}-0"));
                set_tag(trainers, fire + 1, &format!("{tag}-1"));
                set_tag(trainers, fire - 1, &format!("{tag}-2"));
            }
            Friend(tag, grass) => {
                set_tag(trainers, grass, &format!("{tag}-1"));
                set_tag(trainers, grass + 1, &format!("{tag}-2"));
                set_tag(trainers, grass + 2, &format!("{tag}-0"));
            }
            Friend2(tag, water) => {
                set_tag(trainers, water, &format!("{tag}-0"));
                set_tag(trainers, water + 1, &format!("{tag}-1"));
                set_tag(trainers, water + 2, &format!("{tag}-2"));
            }
        }
    }
    let (always, extra, potential): (&[u16], &[u16], &[u16]) = match family {
        Family::DiamondPearl => (DP_ALWAYS_MULTI, &[], DPPT_POTENTIAL_MULTI),
        Family::Platinum => (DP_ALWAYS_MULTI, PT_ALWAYS_MULTI, DPPT_POTENTIAL_MULTI),
        Family::HeartGoldSoulSilver => (HGSS_ALWAYS_MULTI, &[], HGSS_POTENTIAL_MULTI),
    };
    for (status, numbers) in [
        (MultiBattleStatus::Always, always),
        (MultiBattleStatus::Always, extra),
        (MultiBattleStatus::Potential, potential),
    ] {
        for &n in numbers {
            if let Some(t) = usize::from(n).checked_sub(1).and_then(|i| trainers.get_mut(i)) {
                t.multi_battle = status;
            }
        }
    }
}
