use ppr_model::Generation;

/// Errors from reading or writing a ROM.
#[derive(Debug, thiserror::Error)]
pub enum RomError {
    /// The ROM could not be read or written.
    #[error("could not read or write the ROM")]
    Io(#[from] std::io::Error),
    /// The ROM's header data does not have the expected layout.
    #[error("the ROM data is malformed")]
    Format(#[from] ppr_formats::FormatError),
    /// The file is not a supported game.
    #[error("{0}")]
    Unsupported(String),
    /// The game is recognized, but its generation is not supported yet.
    #[error("{generation} games can be identified, but not randomized yet")]
    NotYetSupported {
        /// Generation of the game.
        generation: Generation,
    },
    /// The game data does not look like the entry for this game says it should.
    #[error("unexpected ROM data: {0}")]
    Malformed(String),
}

impl RomError {
    /// A trade's original trainer ID for a game that stores 16 bits of it. A larger ID fails instead of losing its
    /// high bits.
    pub(crate) fn short_ot_id(id: u32) -> Result<u16, Self> {
        u16::try_from(id).map_err(|_| Self::Malformed(format!("trainer ID {id} does not fit the game's 16 bits")))
    }
}

#[cfg(test)]
mod tests {
    use super::RomError;

    #[test]
    fn a_trainer_id_over_16_bits_fails_instead_of_being_cut() {
        assert_eq!(RomError::short_ot_id(65_535).ok(), Some(65_535));
        assert!(matches!(RomError::short_ot_id(65_536), Err(RomError::Malformed(_))));
    }
}
