//! Identifies a ROM from its header: game code, version and, where needed, title and checksum (ADR 0003).

use ppr_formats::ctr::CtrHeader;
use ppr_formats::gb::GbHeader;
use ppr_formats::gba::GbaHeader;
use ppr_formats::nds::NdsHeader;
use ppr_formats::{FormatError, RomSource, ascii_field};
use ppr_model::{Generation, Platform, RomIdentity};

use crate::RomError;
use crate::entries::{RomEntry, catalog};

/// Result of [`detect`]. Not recognizing a file is an expected outcome, not an error (ADR 0006).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Detection {
    /// A supported game.
    Supported(RomIdentity),
    /// Not a game we support.
    Unsupported(Unsupported),
}

/// Why a file is not supported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unsupported {
    /// Not a ROM of any platform we know.
    UnknownFile,
    /// A valid ROM for this platform, but not one of the supported games or revisions.
    UnknownGame {
        /// Platform the header belongs to.
        platform: Platform,
        /// Code or title read from the header.
        code: String,
    },
}

/// Identifies the game in `src`. Only the header is read.
pub fn detect<S: RomSource>(src: &mut S) -> Result<Detection, RomError> {
    type Probe = fn(&mut dyn RomSource) -> Result<Option<Detection>, FormatError>;
    // Cheap, size-checked formats first; the DS header has no size check, so it goes last.
    const PROBES: [Probe; 4] = [probe_gb, probe_gba, probe_ctr, probe_nds];
    let src: &mut dyn RomSource = src;
    for probe in PROBES {
        match probe(src) {
            Ok(Some(found)) => return Ok(found),
            Err(FormatError::Io(e)) => return Err(e.into()),
            Ok(None) | Err(_) => {}
        }
    }
    Ok(Detection::Unsupported(Unsupported::UnknownFile))
}

fn generation(n: u8) -> Generation {
    Generation::new(n).unwrap_or(Generation::ALL[0])
}

fn identity(entry: &RomEntry, generation: Generation, version: Option<u8>) -> RomIdentity {
    RomIdentity {
        name: entry.name.clone(),
        code: entry.code.clone(),
        version,
        generation,
        platform: generation.platform(),
        family: entry.family.clone(),
    }
}

fn unknown(platform: Platform, code: String) -> Detection {
    Detection::Unsupported(Unsupported::UnknownGame { platform, code })
}

/// Whether a Game Boy header carries an entry's `Game=` signature.
type SignatureRule = fn(&GbHeader, &RomEntry) -> bool;

/// Gen 1 matches the title, Gen 2 the game code; both prefer an entry with this exact header checksum
/// (`CRCInHeader`) over one without.
fn probe_gb(src: &mut dyn RomSource) -> Result<Option<Detection>, FormatError> {
    let h = GbHeader::read(src)?;
    if !h.checksum_ok {
        return Ok(None);
    }
    let rules: [(u8, SignatureRule); 2] = [
        (1, |h, e| h.title_starts_with(&e.code)),
        (2, |h, e| h.game_code() == e.code.as_bytes()),
    ];
    for (n, sig_matches) in rules {
        let candidates = || {
            catalog(generation(n)).entries.iter().filter(|e| {
                sig_matches(&h, e) && e.version == i64::from(h.version) && e.non_japanese == i64::from(h.destination)
            })
        };
        let crc = i64::from(h.global_checksum);
        let found = candidates()
            .find(|e| e.int("CRCInHeader") == Some(crc))
            .or_else(|| candidates().find(|e| e.int("CRCInHeader").is_none_or(|c| c == -1)));
        if let Some(e) = found {
            return Ok(Some(Detection::Supported(identity(e, generation(n), Some(h.version)))));
        }
    }
    let code = if h.is_cgb() {
        ascii_field(h.game_code())
    } else {
        ascii_field(&h.title_area)
    };
    Ok(Some(unknown(Platform::GameBoy, code)))
}

/// The unofficial English translation of Emerald has no game code; it is treated as `BPET`.
const UNOFFICIAL_EMERALD_TITLE: &str = "YJencrypted";

fn probe_gba(src: &mut dyn RomSource) -> Result<Option<Detection>, FormatError> {
    let h = GbaHeader::read(src)?;
    let translated = h.title_starts_with(UNOFFICIAL_EMERALD_TITLE);
    if !h.checksum_ok && !translated {
        return Ok(None);
    }
    let code = if translated {
        "BPET".to_owned()
    } else {
        ascii_field(&h.game_code)
    };
    let g3 = generation(3);
    Ok(Some(
        catalog(g3)
            .entries
            .iter()
            .find(|e| e.code == code && e.version == i64::from(h.version))
            .map_or_else(
                || unknown(Platform::GameBoyAdvance, code.clone()),
                |e| Detection::Supported(identity(e, g3, Some(h.version))),
            ),
    ))
}

fn probe_nds(src: &mut dyn RomSource) -> Result<Option<Detection>, FormatError> {
    let h = NdsHeader::read(src)?;
    if !h.checksum_ok {
        return Ok(None);
    }
    let code = ascii_field(&h.game_code);
    for g in [generation(4), generation(5)] {
        if let Some(e) = catalog(g)
            .entries
            .iter()
            .find(|e| e.code == code && e.version == i64::from(h.version))
        {
            return Ok(Some(Detection::Supported(identity(e, g, Some(h.version)))));
        }
    }
    Ok(Some(unknown(Platform::NintendoDs, code)))
}

fn probe_ctr(src: &mut dyn RomSource) -> Result<Option<Detection>, FormatError> {
    let h = CtrHeader::read(src)?;
    for g in [generation(6), generation(7)] {
        let found = catalog(g).entries.iter().find(|e| {
            e.code == h.product_code
                && e.title_id
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case(&h.title_id))
        });
        if let Some(e) = found {
            return Ok(Some(Detection::Supported(identity(e, g, None))));
        }
    }
    Ok(Some(unknown(Platform::Nintendo3ds, h.product_code)))
}
