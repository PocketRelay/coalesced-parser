//! Error type used when decoding packets [`DecodeError`] and result
//! type alias [`DecodeResult`]

use std::{error::Error, fmt::Display, str::Utf8Error};

/// Error type for errors that can occur while decoding a value
/// using the tdf decode
#[derive(Debug)]
pub enum DecodeError {
    /// Encountered an unknown tag type
    UnknownType {
        /// The tag type value
        ty: u8,
    },

    /// Reached the end of the available bytes before
    /// a value could be obtained
    UnexpectedEof {
        /// The current reader cusor position
        cursor: usize,
        /// The number of bytes attempted to read
        wanted: usize,
        /// The remaining bytes in the reader slice
        remaining: usize,
    },

    /// Attempted to decode a str slice but the content wasn't valid utf-8
    InvalidUtf8Value(Utf8Error),

    /// Other error type with custom message
    Other(&'static str),
}

/// Type alias for result which could result in a Decode Error
pub type DecodeResult<T> = Result<T, DecodeError>;

/// Error implementation
impl Error for DecodeError {}

/// Display formatting implementation
impl Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnknownType { ty } => {
                write!(f, "Unknown tag type: {}", ty)
            }
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
            DecodeError::InvalidUtf8Value(err) => err.fmt(f),
            DecodeError::Other(err) => f.write_str(err),
        }
    }
}
