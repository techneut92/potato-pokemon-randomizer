//! In-game trades: a file per trade (given species, IVs, OT ID, held item, requested species), the nicknames and
//! OT names in a text file, and what the traders say. The trades that are static encounters (in `HeartGold` and
//! `SoulSilver`) are left to [`super::statics`].
//!
//! Deliberate choices:
//! - Only the trades that changed are written, and only their traders' texts are rewritten, by the species names
//!   as loaded.

use std::collections::BTreeMap;

use ppr_model::{DexNumber, GameData, InGameTrade};

use super::Rom;
use super::constants::Family;
use super::files::{long, put_long, put_word, word};
use crate::RomError;

/// The longest nickname and OT name a trade can have.
pub(super) const MAX_NICKNAME_LENGTH: usize = 10;
pub(super) const MAX_OT_NAME_LENGTH: usize = 7;

/// Where the parts of a trade are in its file.
const GIVEN: usize = 0;
const IVS: usize = 4;
const OT_ID: usize = 0x20;
const HELD_ITEM: usize = 0x3C;
const REQUESTED: usize = 0x4C;
const GENDER: usize = 0x50;
/// The `HeartGold` and `SoulSilver` trade whose trader's text is in two files.
const HGSS_TWO_TEXTS_TRADE: usize = 6;

/// The trade files that are trades and not static encounters, and how many files there are.
fn trade_files(rom: &mut Rom) -> Result<(Vec<usize>, usize), RomError> {
    let path = rom.entry.file("InGameTrades")?;
    let count = rom.files.narc(&path)?.files.len();
    let statics = rom.entry.array("StaticPokemonTrades");
    Ok(((0..count).filter(|i| !statics.contains(i)).collect(), count))
}

fn dex(value: u32) -> Result<DexNumber, RomError> {
    DexNumber::try_from(value).map_err(|_| RomError::Malformed(format!("species {value}")))
}

/// The trades.
pub(super) fn read(rom: &mut Rom) -> Result<Vec<InGameTrade>, RomError> {
    let (files, count) = trade_files(rom)?;
    let strings = rom.strings_of("IngameTradesTextOffset")?;
    let path = rom.entry.file("InGameTrades")?;
    files
        .into_iter()
        .map(|i| {
            let f = rom.files.narc_file(&path, i)?;
            let ivs = (0..6)
                .map(|iv| long(f, IVS + iv * 4).map(|v| u8::try_from(v).unwrap_or(u8::MAX)))
                .collect::<Result<Vec<_>, _>>()?;
            let item = long(f, HELD_ITEM)?;
            Ok(InGameTrade {
                requested: Some(dex(long(f, REQUESTED)?)?),
                given: dex(long(f, GIVEN)?)?,
                nickname: strings.get(i).cloned().unwrap_or_default(),
                ot_name: strings.get(i + count).cloned().unwrap_or_default(),
                ot_id: word(f, OT_ID)?.into(),
                ivs,
                held_item: (item != 0).then(|| u16::try_from(item).unwrap_or_default()),
            })
        })
        .collect()
}

/// Writes the trades that changed, and what their traders say about the species.
pub(super) fn write(rom: &mut Rom, loaded: &GameData, data: &GameData) -> Result<(), RomError> {
    if data.trades == loaded.trades {
        return Ok(());
    }
    let (files, count) = trade_files(rom)?;
    if data.trades.len() != files.len() {
        return Err(RomError::Malformed(format!(
            "this game has {} trades, got {}",
            files.len(),
            data.trades.len()
        )));
    }
    let path = rom.entry.file("InGameTrades")?;
    let strings_index = rom.entry.need("IngameTradesTextOffset")?;
    let mut strings = rom.files.strings(strings_index)?;
    for ((&i, old), new) in files.iter().zip(&loaded.trades).zip(&data.trades) {
        if old == new {
            continue;
        }
        if let Some(s) = strings.get_mut(i) {
            s.clone_from(&new.nickname);
        }
        if let Some(s) = strings.get_mut(i + count) {
            s.clone_from(&new.ot_name);
        }
        let f = rom.files.narc_file(&path, i)?;
        put_long(f, GIVEN, u32::from(new.given))?;
        for (iv, &v) in new.ivs.iter().take(6).enumerate() {
            put_long(f, IVS + iv * 4, u32::from(v))?;
        }
        put_word(f, OT_ID, RomError::short_ot_id(new.ot_id)?)?;
        put_long(f, HELD_ITEM, u32::from(new.held_item.unwrap_or(0)))?;
        put_long(f, REQUESTED, u32::from(new.requested.unwrap_or(0)))?;
        if f.len() > GENDER {
            // Any gender.
            put_long(f, GENDER, 0)?;
        }
    }
    if strings != rom.files.strings(strings_index)? {
        rom.files.set_strings(strings_index, strings)?;
    }

    let name =
        |d: &GameData, n: Option<DexNumber>| n.and_then(|n| d.species(n)).map(|s| s.name.clone()).unwrap_or_default();
    let texts = rom.entry.array("IngameTradePersonTextOffsets");
    let line = rom.chars_per_line();
    for (t, &text) in texts.iter().enumerate() {
        let (Some(old), Some(new)) = (loaded.trades.get(t), data.trades.get(t)) else {
            break;
        };
        if text == 0 || (old.given == new.given && old.requested == new.requested) {
            continue;
        }
        let mut replacements = BTreeMap::from([(
            name(loaded, Some(old.given)).to_uppercase(),
            name(data, Some(new.given)),
        )]);
        if old.requested != new.requested {
            replacements.insert(name(loaded, old.requested).to_uppercase(), name(data, new.requested));
        }
        rom.files.replace_names(text, &replacements, line)?;
        if rom.family() == Family::HeartGoldSoulSilver && t == HGSS_TWO_TEXTS_TRADE {
            rom.files.replace_names(text + 1, &replacements, line)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::GameHandler;
    use crate::gen4::tests::{load, real_rom, save};

    /// With `PPR_TEST_ROMS`: trades written to Platinum read back.
    #[test]
    #[ignore = "needs a real ROM"]
    fn real_platinum_trades_read_back() {
        let Some((_, bytes)) = real_rom("Platinum") else {
            return;
        };
        let mut h = load(bytes);
        let mut data = h.game_data().clone();
        assert_eq!(data.trades.len(), 4);
        let t = &mut data.trades[0];
        t.given = 150;
        t.requested = Some(1);
        t.nickname = "Mewtwoey".into();
        t.ot_name = "Ann".into();
        t.ot_id = 12345;
        t.ivs = vec![31, 30, 29, 28, 27, 26];
        t.held_item = None;
        h.apply(&data).expect("applies");
        let again = load(save(&h));
        assert_eq!(again.game_data().trades, data.trades);
    }
}
