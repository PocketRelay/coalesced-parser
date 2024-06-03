pub mod crc32;
pub mod de;
pub mod error;
pub mod huffman;
pub mod ser;
pub mod shared;

pub use de::deserialize_coalesced;
pub use ser::serialize_coalesced;
pub use shared::*;
