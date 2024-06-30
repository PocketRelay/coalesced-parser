use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum DecodeError {
    /// Reached the end of the available bytes before
    /// a value could be obtained
    UnexpectedEof {
        /// The current reader cursor position
        cursor: usize,
        /// The number of bytes attempted to read
        wanted: usize,
        /// The remaining bytes in the reader slice
        remaining: usize,
    },

    UnknownFileMagic,
    StringTableHashMismatch,
    StringTableSizeMismatch,
    InvalidNameOffset,
    UnknownValueType,
    MalformedDecompressionNodes,
}

/// Type alias for result which could result in a Coalesced Error
pub type DecodeResult<T> = Result<T, DecodeError>;

/// Error implementation
impl Error for DecodeError {}

/// Display formatting implementation
impl Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnexpectedEof {
                cursor,
                wanted,
                remaining,
            } => {
                write!(
                    f,
                    "Unexpected end of file (cursor: {}, wanted: {}, remaining: {})",
                    cursor, wanted, remaining
                )
            }
            DecodeError::UnknownFileMagic => f.write_str("Unexpected file magic bytes"),
            DecodeError::StringTableHashMismatch => f.write_str("String table hash didn't match"),
            DecodeError::StringTableSizeMismatch => f.write_str("String table size didn't match"),
            DecodeError::InvalidNameOffset => f.write_str("Invalid name offset"),
            DecodeError::UnknownValueType => f.write_str("Unknown value type"),
            DecodeError::MalformedDecompressionNodes => {
                f.write_str("Decompression nodes are malformed")
            }
        }
    }
}
