//! IPS patches, and the ones shipped for Gen 3.

use crate::RomError;

/// One change: `data` written at `offset`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    pub(crate) offset: usize,
    pub(crate) data: Vec<u8>,
}

/// A parsed IPS patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Patch {
    records: Vec<Record>,
}

fn malformed(message: &str) -> RomError {
    RomError::Malformed(format!("IPS patch: {message}"))
}

impl Patch {
    /// Parses `PATCH`, then records of a 3-byte offset and a 2-byte size (0 for a run of one repeated byte), up to
    /// `EOF`. Anything after `EOF` (a truncation length) is ignored, as the ROM is never resized.
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, RomError> {
        let mut rest = bytes
            .strip_prefix(b"PATCH")
            .ok_or_else(|| malformed("no PATCH header"))?;
        let mut records = Vec::new();
        loop {
            if rest.starts_with(b"EOF") {
                return Ok(Self { records });
            }
            let (head, tail) = rest.split_at_checked(5).ok_or_else(|| malformed("record cut short"))?;
            let offset = usize::from(head[0]) << 16 | usize::from(head[1]) << 8 | usize::from(head[2]);
            let size = usize::from(u16::from_be_bytes([head[3], head[4]]));
            let (data, tail) = if size == 0 {
                let (run, tail) = tail.split_at_checked(3).ok_or_else(|| malformed("run cut short"))?;
                (vec![run[2]; usize::from(u16::from_be_bytes([run[0], run[1]]))], tail)
            } else {
                let (data, tail) = tail.split_at_checked(size).ok_or_else(|| malformed("data cut short"))?;
                (data.to_vec(), tail)
            };
            records.push(Record { offset, data });
            rest = tail;
        }
    }

    pub(crate) fn records(&self) -> &[Record] {
        &self.records
    }

    /// Writes every record into `rom`; nothing is written when a record would pass the end.
    pub(crate) fn apply(&self, rom: &mut [u8]) -> Result<(), RomError> {
        if let Some(r) = self.records.iter().find(|r| r.offset + r.data.len() > rom.len()) {
            return Err(malformed(&format!(
                "record at {:#x} passes the end of the ROM",
                r.offset
            )));
        }
        for r in &self.records {
            rom[r.offset..r.offset + r.data.len()].copy_from_slice(&r.data);
        }
        Ok(())
    }
}

macro_rules! patches {
    ($($name:literal),* $(,)?) => {
        /// The bundled patches, by the name the ROM entries use (`StaticFirstBattleTweak=hardcoded_statics/…`).
        const PATCHES: &[(&str, &[u8])] = &[
            $(($name, include_bytes!(concat!("../data/patches/", $name, ".ips")))),*
        ];
    };
}

patches!(
    "bwexp/rb_en_bwxp",
    "bwexp/yellow_en_bwxp",
    "bwexp/gs_en_bwxp",
    "bwexp/crystal_en_bwxp",
    "rb_en_critrate",
    "rb_en_xaccnerf",
    "yellow_en_critrate",
    "yellow_en_xaccnerf",
    "hardcoded_statics/rs_firstbattle",
    "hardcoded_statics/em_firstbattle",
    "hardcoded_statics/fr_marowak_10",
    "hardcoded_statics/fr_marowak_11",
    "hardcoded_statics/lg_marowak_10",
    "hardcoded_statics/lg_marowak_11",
    "hardcoded_statics/roamers/fr_roamers_10",
    "hardcoded_statics/roamers/fr_roamers_11",
    "hardcoded_statics/roamers/lg_roamers_10",
    "hardcoded_statics/roamers/lg_roamers_11",
    "musicfix/em_musicfix",
    "musicfix/fr_musicfix_10",
    "musicfix/fr_musicfix_11",
    "musicfix/lg_musicfix_10",
    "musicfix/lg_musicfix_11",
    "instant_text/em_instant_text",
    "instant_text/fr_10_instant_text",
    "instant_text/fr_11_instant_text",
    "instant_text/lg_10_instant_text",
    "instant_text/lg_11_instant_text",
    "instant_text/ruby_10_instant_text",
    "instant_text/ruby_11_instant_text",
    "instant_text/ruby_12_instant_text",
    "instant_text/sapphire_10_instant_text",
    "instant_text/sapphire_11_instant_text",
    "instant_text/sapphire_12_instant_text",
    "pt_fast_distortion_world",
    "hgss_catching_tutorialfix",
    "national_dex/dp_national_dex",
    "national_dex/plat_national_dex",
    "national_dex/hgss_national_dex",
    "musicfix/diamond_musicfix",
    "musicfix/plat_musicfix",
    "instant_text/dp_instant_text",
    "instant_text/plat_instant_text",
    "instant_text/hgss_instant_text",
    "hardcoded_statics/roamers/plat_roamers",
    "hardcoded_statics/roamers/hgss_roamers",
    "instant_text/b1_instant_text",
    "instant_text/w1_instant_text",
    "instant_text/b2_instant_text",
    "instant_text/w2_instant_text",
    "musicfix/black_musicfix",
    "musicfix/black_ovl21_musicfix",
    "musicfix/white_musicfix",
    "musicfix/white_ovl21_musicfix",
    "musicfix/black2_musicfix",
    "musicfix/black2_ovl36_musicfix",
    "musicfix/white2_musicfix",
    "musicfix/white2_ovl36_musicfix",
    "national_dex/bw1_national_dex",
    "national_dex/bw2_national_dex",
    "shedinja/black_shedinja",
    "shedinja/black_ovl195_shedinja",
    "shedinja/white_shedinja",
    "shedinja/white_ovl195_shedinja",
    "shedinja/black2_shedinja",
    "shedinja/black2_ovl284_shedinja",
    "shedinja/white2_shedinja",
    "shedinja/white2_ovl284_shedinja",
);

/// The embedded patch with this name.
pub(crate) fn named(name: &str) -> Result<Patch, RomError> {
    let (_, bytes) = PATCHES
        .iter()
        .find(|(n, _)| *n == name)
        .ok_or_else(|| malformed(&format!("no patch named {name}")))?;
    Patch::parse(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_records_and_runs() {
        let patch =
            Patch::parse(b"PATCH\x00\x00\x02\x00\x02ab\x00\x00\x06\x00\x00\x00\x03zEOF\x00\x00\x10").expect("valid");
        let mut rom = vec![0u8; 10];
        patch.apply(&mut rom).expect("fits");
        assert_eq!(rom, b"\0\0ab\0\0zzz\0");
    }

    #[test]
    fn rejects_broken_patches() {
        for bytes in [
            &b"PATCX"[..],
            b"PATCH",
            b"PATCH\x00\x00\x01\x00\x05ab",
            b"PATCH\x00\x00\x01\x00\x00\x00",
        ] {
            assert!(Patch::parse(bytes).is_err(), "{bytes:?}");
        }
        let patch = Patch::parse(b"PATCH\x00\x00\x09\x00\x02abEOF").expect("valid");
        let mut rom = vec![0u8; 10];
        assert!(patch.apply(&mut rom).is_err());
        assert_eq!(rom, [0; 10], "nothing written");
    }

    #[test]
    fn every_embedded_patch_parses() {
        for (name, _) in PATCHES {
            let patch = named(name).expect(name);
            assert!(!patch.records().is_empty(), "{name}");
        }
        assert!(named("nope").is_err());
    }
}
