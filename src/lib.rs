mod crc32;
mod huffman;

pub mod de;
pub mod error;
pub mod ser;
pub mod shared;

pub use de::{deserialize_coalesced, deserialize_tlk};
pub use ser::{serialize_coalesced, serialize_tlk};
pub use shared::*;
