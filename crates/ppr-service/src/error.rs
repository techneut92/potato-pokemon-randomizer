use std::path::PathBuf;

use serde::Serialize;
use ts_rs::TS;

/// Errors from a use case, with a stable `code` for the UI (ADR 0006).
#[derive(Debug, thiserror::Error)]
pub enum ServiceError {
    /// The file could not be opened.
    #[error("could not open {}", path.display())]
    Open {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// Reading the ROM failed.
    #[error(transparent)]
    Rom(#[from] ppr_romio::RomError),
    /// The settings contradict each other.
    #[error(transparent)]
    Settings(#[from] ppr_settings::InvalidSettings),
    /// A settings string or file can't be read.
    #[error(transparent)]
    SettingsString(#[from] ppr_settings::SettingsStringError),
    /// The randomization stopped.
    #[error(transparent)]
    Randomize(#[from] ppr_random::RandomizeError),
    /// An edit list in a format this build can't read, or for another game.
    #[error(transparent)]
    EditList(#[from] ppr_model::EditError),
    /// Manual edits that don't fit the game; nothing was written.
    #[error("{} of the manual edits can't be applied: {}", .0.len(), .0.iter().map(|i| i.message.as_str()).collect::<Vec<_>>().join("; "))]
    EditsRejected(Vec<crate::EditIssue>),
    /// The output could not be written.
    #[error("could not write {}", path.display())]
    Write {
        /// The output file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// An edits file can't be loaded for the loaded game.
    #[error(transparent)]
    EditFile(#[from] crate::EditFileError),
}

/// Serializable form of [`ServiceError`] for front ends.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ApiError {
    /// Stable machine-readable code.
    pub code: &'static str,
    /// Message for the user, including causes.
    pub message: String,
}

impl ServiceError {
    /// Stable machine-readable code.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Open { .. } => "open_failed",
            Self::Rom(ppr_romio::RomError::NotYetSupported { .. }) => "generation_not_supported",
            Self::Rom(ppr_romio::RomError::Unsupported(_)) => "rom_unsupported",
            Self::Rom(_) => "rom_unreadable",
            Self::Settings(_) => "invalid_settings",
            Self::SettingsString(ppr_settings::SettingsStringError::Malformed) => "settings_invalid",
            Self::SettingsString(ppr_settings::SettingsStringError::OldVersion(_)) => "settings_too_old",
            Self::SettingsString(ppr_settings::SettingsStringError::NewerVersion(_)) => "settings_too_new",
            Self::Randomize(ppr_random::RandomizeError::Cancelled) => "cancelled",
            Self::Randomize(_) => "randomization_failed",
            Self::EditList(ppr_model::EditError::UnsupportedVersion { .. }) => "edits_unsupported_version",
            Self::EditList(ppr_model::EditError::WrongGame { .. }) => "edits_wrong_game",
            Self::EditList(_) | Self::EditsRejected(_) => "edits_rejected",
            Self::Write { .. } => "write_failed",
            Self::EditFile(e) => e.code(),
        }
    }
}

impl From<&ServiceError> for ApiError {
    fn from(e: &ServiceError) -> Self {
        let mut message = e.to_string();
        let mut source = std::error::Error::source(e);
        while let Some(s) = source {
            message.push_str(": ");
            message.push_str(&s.to_string());
            source = s.source();
        }
        Self {
            code: e.code(),
            message,
        }
    }
}
