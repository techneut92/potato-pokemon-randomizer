use std::io;

/// Errors from reading a binary format.
#[derive(Debug, thiserror::Error)]
pub enum FormatError {
    /// The underlying reader failed.
    #[error("I/O error")]
    Io(#[from] io::Error),
    /// The input ends before a structure that should be there.
    #[error("unexpected end of data at offset {offset:#x}")]
    Truncated {
        /// Offset of the structure that could not be read.
        offset: u64,
    },
    /// The input is not this format.
    #[error("not a {format} file: {reason}")]
    WrongFormat {
        /// Name of the expected format.
        format: &'static str,
        /// Why the input was rejected.
        reason: &'static str,
    },
    /// A container has no entry by that name or number.
    #[error("{what} not found")]
    NotFound {
        /// The entry that was asked for, e.g. `file a/0/0/2` or `overlay 12`.
        what: String,
    },
    /// The image is encrypted, so its file systems cannot be read.
    #[error("the game is encrypted; decrypt it first")]
    Encrypted,
    /// The data cannot be written in this format.
    #[error("cannot write {format} data: {reason}")]
    Unencodable {
        /// Name of the format.
        format: &'static str,
        /// Why the data does not fit.
        reason: &'static str,
    },
}
