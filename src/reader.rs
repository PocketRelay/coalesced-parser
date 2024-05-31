use std::borrow::Cow;

use crate::error::{DecodeError, DecodeResult};

/// [Deserializer] provides functions for reading tag values from a buffer.
/// See [module documentation](crate::reader) for usage
pub struct Deserializer<'de> {
    /// Buffer storing the bytes to be deserialized
    pub(crate) buffer: &'de [u8],
    /// Cursor representing the current offset within the buffer
    pub(crate) cursor: usize,
}

impl<'de> Deserializer<'de> {
    /// Creates a new [Deserializer] from the provided buffer
    pub fn new(buffer: &'de [u8]) -> Self {
        Self { buffer, cursor: 0 }
    }

    /// Obtains the remaining length in bytes left of
    /// the buffer after the cursor
    pub fn remaining(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    /// Returns whether there is no remaining bytes in the buffer
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Internal function used to read single byte from the buffer
    pub(crate) fn read_byte(&mut self) -> DecodeResult<u8> {
        if self.cursor == self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: 1,
                remaining: 0,
            });
        }

        let byte: u8 = self.buffer[self.cursor];
        self.cursor += 1;
        Ok(byte)
    }

    /// Keeps reading until a null terminator is reached
    pub fn until_terminator(&mut self) -> DecodeResult<()> {
        while self.cursor < self.buffer.len() {
            let byte: u8 = self.buffer[self.cursor];
            self.cursor += 1;

            // Reached the null terminator
            if byte == 0 {
                return Ok(());
            }
        }

        // Reached end of buffer without terminator
        Err(DecodeError::UnexpectedEof {
            cursor: self.cursor,
            wanted: 1,
            remaining: 0,
        })
    }

    /// Internal function used to read a slice of bytes from the buffer
    pub(crate) fn read_bytes(&mut self, length: usize) -> DecodeResult<&'de [u8]> {
        if self.cursor + length > self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: length,
                remaining: self.remaining(),
            });
        }

        let slice: &[u8] = &self.buffer[self.cursor..self.cursor + length];
        self.cursor += length;
        Ok(slice)
    }

    /// Internal function used to move the cursor back 1 position
    pub(crate) fn step_back(&mut self) {
        self.cursor -= 1;
    }

    pub(crate) fn seek(&mut self, cursor: usize) -> DecodeResult<()> {
        if cursor >= self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: cursor,
                remaining: self.remaining(),
            });
        }

        self.cursor = cursor;

        Ok(())
    }

    /// Internal function for reading a fixed length array from the buffer
    pub(crate) fn read_fixed<const S: usize>(&mut self) -> DecodeResult<[u8; S]> {
        let slice = self.read_bytes(S)?;

        // Copy the bytes into the new fixed size array
        let mut bytes: [u8; S] = [0u8; S];
        bytes.copy_from_slice(slice);

        Ok(bytes)
    }

    /// Internal function for skipping a length of bytes
    pub(crate) fn skip_length(&mut self, length: usize) -> DecodeResult<()> {
        if self.cursor + length > self.buffer.len() {
            return Err(DecodeError::UnexpectedEof {
                cursor: self.cursor,
                wanted: length,
                remaining: self.remaining(),
            });
        }

        self.cursor += length;
        Ok(())
    }

    pub fn take_slice(&mut self, length: usize) -> DecodeResult<Deserializer<'de>> {
        Ok(Self::new(self.read_bytes(length)?))
    }
}

pub trait Deserialize<'de>: Sized {
    /// Deserialize this value from the provided deserializer
    fn deserialize(r: &mut Deserializer<'de>) -> DecodeResult<Self>;
}

pub trait DeserializeOwned: Sized {
    /// Deserialize this value from the provided deserializer
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self>;
}

/// All types that implement [TdfDeserializeOwned] also implement [TdfDeserialize]
impl<T> Deserialize<'_> for T
where
    T: DeserializeOwned,
{
    #[inline]
    fn deserialize(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        Self::deserialize_owned(r)
    }
}

impl DeserializeOwned for u32 {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let bytes = r.read_fixed::<4>()?;
        Ok(u32::from_le_bytes(bytes))
    }
}

impl DeserializeOwned for u16 {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let bytes = r.read_fixed::<2>()?;
        Ok(u16::from_le_bytes(bytes))
    }
}

impl DeserializeOwned for i32 {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let bytes = r.read_fixed::<4>()?;
        Ok(i32::from_le_bytes(bytes))
    }
}

impl DeserializeOwned for i16 {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let bytes = r.read_fixed::<2>()?;
        Ok(i16::from_le_bytes(bytes))
    }
}

#[derive(Debug)]
pub struct HeaderBlock {
    pub magic: u32,
    pub version: u32,
    pub max_field_name_length: u32,
    pub max_value_length: u32,
    pub string_table_size: u32,
    pub huffman_size: u32,
    pub index_size: u32,
    pub data_size: u32,
}

impl DeserializeOwned for HeaderBlock {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let magic = u32::deserialize(r)?;
        let version = u32::deserialize(r)?;
        let max_field_name_length = u32::deserialize(r)?;
        let max_value_length = u32::deserialize(r)?;
        let string_table_size = u32::deserialize(r)?;
        let huffman_size = u32::deserialize(r)?;
        let index_size = u32::deserialize(r)?;
        let data_size = u32::deserialize(r)?;
        Ok(Self {
            magic,
            version,
            max_field_name_length,
            max_value_length,
            string_table_size,
            huffman_size,
            index_size,
            data_size,
        })
    }
}

#[derive(Debug)]
pub struct StringTable {
    pub values: Vec<String>,
}

impl DeserializeOwned for StringTable {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let local_size = u32::deserialize_owned(r)?;
        let count = u32::deserialize_owned(r)?;

        let mut offsets = Vec::new();

        for _ in 0..count {
            let hash = u32::deserialize_owned(r)?;
            let offset = u32::deserialize_owned(r)?;
            offsets.push((offset, hash))
        }

        let mut values = Vec::new();
        for (offset, hash) in offsets {
            r.seek((8 + offset) as usize)?;

            let length = u16::deserialize_owned(r)?;
            let bytes = r.read_bytes(length as usize)?;
            let text: Cow<str> = String::from_utf8_lossy(bytes);
            let text: String = text.to_string();
            values.push(text);

            // TODO : Compare hash
        }

        Ok(Self { values })
    }
}

#[derive(Debug)]
pub struct HuffmanTree(pub Vec<(i32, i32)>);

impl DeserializeOwned for HuffmanTree {
    fn deserialize_owned(r: &mut Deserializer<'_>) -> DecodeResult<Self> {
        let count = u16::deserialize_owned(r)?;
        let mut values = Vec::new();
        for _ in 0..count {
            let left = i32::deserialize_owned(r)?;
            let right = i32::deserialize_owned(r)?;
            values.push((left, right))
        }
        Ok(HuffmanTree(values))
    }
}

fn huffman_decode(tree: &HuffmanTree, data: &[u8], offset: usize, max_length: usize) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    let start = tree.0.len() - 1;

    todo!()
}
