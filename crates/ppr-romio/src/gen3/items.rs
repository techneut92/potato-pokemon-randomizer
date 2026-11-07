//! Gen 3 item numbers: the standard number of each internal item, and the banned and bad items. Generated
//! tables; see the module doc of [`super`].

use ppr_model::ItemId;

/// Items only Gen 3 has are numbered from here (plus their internal index).
pub(super) const UNIQUE_OFFSET: ItemId = 1000;
/// The standard number of TM01; Gen 3 has 50 TMs.
pub(super) const TM01: ItemId = 328;
pub(super) const TM_COUNT: ItemId = 50;
/// The standard number of HM01.
pub(super) const HM01: ItemId = 420;
/// HM01 to HM08, banned like every item only Gen 3 has.
pub(super) const HMS: std::ops::RangeInclusive<ItemId> = 420..=427;
pub(super) const UNIQUE_BANNED: std::ops::RangeInclusive<ItemId> = 1052..=1376;

/// Internal index → standard item number. Internal indices past the table are
/// `UNIQUE_OFFSET` + index.
const STANDARD: [ItemId; 347] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34,
    35, 36, 37, 38, 39, 40, 41, 42, 65, 66, 67, 68, 69, 43, 44, 70, 71, 72, 73, 74, 75, 1052, 1053, 1054, 1055, 1056,
    1057, 1058, 1059, 1060, 1061, 1062, 45, 46, 47, 48, 49, 50, 51, 52, 53, 1072, 55, 56, 57, 58, 59, 60, 61, 63, 64,
    1082, 76, 77, 78, 79, 1087, 1088, 1089, 1090, 1091, 1092, 80, 81, 82, 83, 84, 85, 1099, 1100, 1101, 1102, 86, 87,
    1105, 88, 89, 90, 91, 92, 93, 1112, 1113, 1114, 1115, 1116, 1117, 1118, 1119, 1120, 137, 138, 139, 140, 141, 142,
    143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165,
    166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 201, 202, 203, 204, 205,
    206, 207, 208, 1176, 1177, 1178, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228,
    229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251,
    252, 253, 254, 255, 256, 257, 258, 259, 1226, 1227, 1228, 1229, 1230, 1231, 1232, 1233, 1234, 1235, 1236, 1237,
    1238, 1239, 1240, 1241, 1242, 1243, 1244, 1245, 1246, 1247, 1248, 1249, 1250, 1251, 1252, 1253, 260, 261, 262, 263,
    264, 1259, 1260, 1261, 1262, 1263, 1264, 1265, 1266, 1267, 1268, 1269, 1270, 1271, 1272, 1273, 1274, 1275, 1276,
    1277, 1278, 1279, 1280, 1281, 1282, 1283, 1284, 1285, 1286, 1287, 1288, 328, 329, 330, 331, 332, 333, 334, 335,
    336, 337, 338, 339, 340, 341, 342, 343, 344, 345, 346, 347, 348, 349, 350, 351, 352, 353, 354, 355, 356, 357, 358,
    359, 360, 361, 362, 363, 364, 365, 366, 367, 368, 369, 370, 371, 372, 373, 374, 375, 376, 377, 420, 421, 422, 423,
    424, 425, 426, 427,
];

/// Items no starter or wild Pokémon should hold in Ruby, Sapphire and Emerald: mail, berries, species-specific items, scarves.
pub(super) const BAD_RSE: [ItemId; 57] = [
    137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 155, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168,
    169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 201, 202, 203, 204, 205, 206, 207, 208,
    225, 236, 256, 257, 258, 259, 260, 261, 262, 263, 264,
];

/// `FireRed` and `LeafGreen` add the Shoal items and Shards, which do nothing there.
pub(super) const BAD_FRLG_EXTRA: [ItemId; 6] = [70, 71, 72, 73, 74, 75];

/// The standard item number of an internal index.
pub(super) fn standard(internal: u16) -> ItemId {
    STANDARD
        .get(usize::from(internal))
        .copied()
        .unwrap_or(UNIQUE_OFFSET + internal)
}

/// The internal index of a standard item number; `None` for a number Gen 3 has no item for.
pub(super) fn internal(id: ItemId) -> Option<u16> {
    if id >= UNIQUE_OFFSET {
        return Some(id - UNIQUE_OFFSET);
    }
    STANDARD
        .iter()
        .position(|&s| s == id)
        .and_then(|i| u16::try_from(i).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_round_trip() {
        for i in 0..=400 {
            assert_eq!(internal(standard(i)), Some(i), "{i}");
        }
        assert_eq!(standard(1), 1, "Master Ball");
        assert_eq!(standard(13), 17, "Potion");
        assert_eq!(standard(400), 1400);
    }
}
