use std::{error::Error, fmt::Display};

#[derive(Debug)]
pub enum CoalescedError {
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
pub type CoalResult<T> = Result<T, CoalescedError>;

/// Error implementation
impl Error for CoalescedError {}

/// Display formatting implementation
impl Display for CoalescedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoalescedError::UnexpectedEof {
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
            CoalescedError::UnknownFileMagic => f.write_str("Unexpected file magic bytes"),
            CoalescedError::StringTableHashMismatch => {
                f.write_str("String table hash didn't match")
            }
            CoalescedError::StringTableSizeMismatch => {
                f.write_str("String table size didn't match")
            }
            CoalescedError::InvalidNameOffset => f.write_str("Invalid name offset"),
            CoalescedError::UnknownValueType => f.write_str("Unknown value type"),
            CoalescedError::MalformedDecompressionNodes => {
                f.write_str("Decompression nodes are malformed")
            }
        }
    }
}
