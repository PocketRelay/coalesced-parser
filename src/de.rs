use crate::{
    crc32::hash_crc32,
    error::{CoalResult, CoalescedError},
    huffman::Huffman,
    huffman_utf16::HuffmanUtf16,
    invert_huffman_tree,
    shared::{CoalFile, Coalesced, Property, Section, Value, ValueType, ME3_MAGIC},
    Tlk, TlkString, TLK_MAGIC,
};
use std::borrow::Cow;

/// Seekable read buffer
pub struct ReadBuffer<'de> {
    /// Buffer storing the bytes to be deserialized
    buffer: &'de [u8],
    /// Cursor representing the current offset within the buffer
    cursor: usize,
}

impl<'de> ReadBuffer<'de> {
    /// Creates a new [Deserializer] from the provided buffer
    pub fn new(buffer: &'de [u8]) -> Self {
        Self { buffer, cursor: 0 }
    }

    /// Obtains the remaining length in bytes left of
    /// the buffer after the cursor
    pub fn remaining(&self) -> usize {
        self.buffer.len() - self.cursor
    }

    /// Internal function used to read a slice of bytes from the buffer
    pub(crate) fn read_bytes(&mut self, length: usize) -> CoalResult<&'de [u8]> {
        if self.cursor + length > self.buffer.len() {
            return Err(CoalescedError::UnexpectedEof {
                cursor: self.cursor,
                wanted: length,
                remaining: self.remaining(),
            });
        }

        let slice: &[u8] = &self.buffer[self.cursor..self.cursor + length];
        self.cursor += length;
        Ok(slice)
    }

    pub(crate) fn seek(&mut self, cursor: usize) -> CoalResult<()> {
        if cursor >= self.buffer.len() {
            return Err(CoalescedError::UnexpectedEof {
                cursor: self.cursor,
                wanted: cursor,
                remaining: self.remaining(),
            });
        }

        self.cursor = cursor;

        Ok(())
    }

    /// Internal function for reading a fixed length array from the buffer
    pub(crate) fn read_fixed<const S: usize>(&mut self) -> CoalResult<[u8; S]> {
        let slice = self.read_bytes(S)?;

        // Copy the bytes into the new fixed size array
        let mut bytes: [u8; S] = [0u8; S];
        bytes.copy_from_slice(slice);

        Ok(bytes)
    }

    pub fn take_slice(&mut self, length: usize) -> CoalResult<ReadBuffer<'de>> {
        Ok(Self::new(self.read_bytes(length)?))
    }

    pub fn read_u32(&mut self) -> CoalResult<u32> {
        let bytes = self.read_fixed::<4>()?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_u16(&mut self) -> CoalResult<u16> {
        let bytes = self.read_fixed::<2>()?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub fn read_i32(&mut self) -> CoalResult<i32> {
        let bytes = self.read_fixed::<4>()?;
        Ok(i32::from_le_bytes(bytes))
    }
}

pub fn deserialize_coalesced(input: &[u8]) -> CoalResult<Coalesced> {
    let mut r = ReadBuffer::new(input);
    // Read the file header
    let magic = r.read_u32()?;

    if magic != ME3_MAGIC {
        return Err(CoalescedError::UnknownFileMagic);
    }

    let version = r.read_u32()?;
    let _max_field_name_length = r.read_u32()?;
    let max_value_length = r.read_u32()?;
    let string_table_size = r.read_u32()?;
    let huffman_size = r.read_u32()?;
    let index_size = r.read_u32()?;
    let data_size = r.read_u32()?;

    // Read the string lookup table
    let string_table: Vec<String> = {
        let mut string_table_block = r.take_slice(string_table_size as usize)?;

        let local_size = string_table_block.read_u32()?;

        if local_size != string_table_size {
            return Err(CoalescedError::StringTableSizeMismatch);
        }

        let count = string_table_block.read_u32()?;

        let mut offsets: Vec<(u32, u32)> = Vec::new();

        for _ in 0..count {
            let hash = string_table_block.read_u32()?;
            let offset = string_table_block.read_u32()?;
            offsets.push((offset, hash))
        }

        let mut values = Vec::new();
        for (offset, hash) in offsets {
            string_table_block.seek((8 + offset) as usize)?;

            let length = string_table_block.read_u16()?;
            let bytes = string_table_block.read_bytes(length as usize)?;
            let text: Cow<str> = String::from_utf8_lossy(bytes);
            let text: String = text.to_string();

            if hash_crc32(text.as_bytes()) != hash {
                return Err(CoalescedError::StringTableHashMismatch);
            }

            values.push(text);
        }

        values
    };

    // Read the huffman tree
    let huffman_tree: Vec<(i32, i32)> = {
        let mut huffman_tree_block = r.take_slice(huffman_size as usize)?;

        // Read the length of the tree
        let count = huffman_tree_block.read_u16()?;

        let mut values = Vec::with_capacity(count as usize);

        for _ in 0..count {
            let left = huffman_tree_block.read_i32()?;
            let right = huffman_tree_block.read_i32()?;
            values.push((left, right))
        }

        values
    };

    // Read the index block
    let mut index_block: ReadBuffer = r.take_slice(index_size as usize)?;

    let data_block: &[u8] = {
        // Read the total bits count
        let _total_bits = r.read_u32()?;

        // Read the data block
        let block = r.take_slice(data_size as usize)?;
        block.buffer
    };

    // Read the number of files
    let files_count = index_block.read_u16()?;

    let mut files: Vec<CoalFile> = Vec::with_capacity(files_count as usize);

    // Read the file offsets
    let mut file_offsets: Vec<(String, usize)> = Vec::with_capacity(files_count as usize);

    for _ in 0..files_count {
        // Read the file name and get it from the string table
        let file_name_index = index_block.read_u16()?;
        let file_name = string_table
            .get(file_name_index as usize)
            .ok_or(CoalescedError::InvalidNameOffset)?;

        // Read the file offset
        let file_offset = index_block.read_u32()?;

        file_offsets.push((file_name.to_string(), file_offset as usize));
    }

    for (file_name, file_offset) in file_offsets {
        // Seek the index to the file
        index_block.seek(file_offset)?;

        // Read the number of sections
        let sections_count = index_block.read_u16()?;

        let mut sections: Vec<Section> = Vec::with_capacity(sections_count as usize);
        let mut section_offsets: Vec<(String, usize)> = Vec::with_capacity(sections_count as usize);

        for _ in 0..sections_count {
            // Read the section name and get it from the string table
            let section_name_index = index_block.read_u16()?;
            let section_name = string_table
                .get(section_name_index as usize)
                .ok_or(CoalescedError::InvalidNameOffset)?;

            // Read the section offset
            let section_offset = index_block.read_u32()?;

            section_offsets.push((section_name.to_string(), section_offset as usize));
        }

        for (section_name, section_offset) in section_offsets {
            // Seek the index to the section
            index_block.seek(file_offset + section_offset)?;

            let values_count = index_block.read_u16()? as usize;
            let mut properties: Vec<Property> = Vec::with_capacity(values_count);
            let mut value_offsets: Vec<(String, usize)> = Vec::with_capacity(values_count);

            for _ in 0..values_count {
                // Read the value name and get it from the string table
                let value_name_index = index_block.read_u16()?;
                let value_name = string_table
                    .get(value_name_index as usize)
                    .ok_or(CoalescedError::InvalidNameOffset)?;

                // Read the value offset
                let value_offset = index_block.read_u32()?;
                value_offsets.push((value_name.to_string(), value_offset as usize));
            }

            for (property_name, value_offset) in value_offsets {
                // Seek the index to the value
                index_block.seek(file_offset + section_offset + value_offset)?;

                let item_count = index_block.read_u16()? as usize;
                let mut items: Vec<Value> = Vec::with_capacity(values_count);

                for _ in 0..item_count {
                    // Read the item offset
                    let item_offset = index_block.read_u32()?;

                    // Split the type and offset
                    let ty = (item_offset & 0xE0000000) >> 29;
                    let item_offset = item_offset & 0x1fffffff;

                    let ty = ValueType::try_from(ty as u8)
                        .map_err(|_| CoalescedError::UnknownValueType)?;

                    let text = match ty {
                        ValueType::RemoveProperty => None,
                        _ => {
                            let text = Huffman::decode(
                                data_block,
                                &huffman_tree,
                                item_offset as usize,
                                max_value_length as usize,
                            )?;

                            Some(text)
                        }
                    };

                    items.push(Value { ty, text });
                }

                properties.push(Property {
                    name: property_name,
                    values: items,
                });
            }

            sections.push(Section {
                name: section_name,
                properties,
            });
        }

        files.push(CoalFile {
            path: file_name,
            sections,
        })
    }

    let coalesced = Coalesced { version, files };

    Ok(coalesced)
}

pub fn deserialize_tlk(input: &[u8]) -> CoalResult<Tlk> {
    let mut r = ReadBuffer::new(input);

    let magic = r.read_u32()?;

    if magic != TLK_MAGIC {
        return Err(CoalescedError::UnknownFileMagic);
    }

    // Header block
    let version = r.read_u32()?;
    let min_version = r.read_u32()?;
    let male_entry_count = r.read_u32()?;
    let female_entry_count = r.read_u32()?;
    let tree_node_count = r.read_u32()?;
    let data_length = r.read_u32()?;

    let mut male_refs = Vec::<(u32, u32)>::with_capacity(male_entry_count as usize);
    let mut female_refs = Vec::<(u32, u32)>::with_capacity(female_entry_count as usize);

    // Read the male refs
    for _ in 0..male_entry_count {
        let left = r.read_u32()?;
        let right = r.read_u32()?;

        male_refs.push((left, right));
    }

    // Read the female refs
    for _ in 0..female_entry_count {
        let left = r.read_u32()?;
        let right = r.read_u32()?;

        female_refs.push((left, right));
    }

    let mut huffman_tree: Vec<(i32, i32)> = Vec::with_capacity(tree_node_count as usize);

    // Read the huffman tree
    for _ in 0..tree_node_count {
        let left = r.read_i32()?;
        let right = r.read_i32()?;
        huffman_tree.push((left, right))
    }

    invert_huffman_tree(&mut huffman_tree);

    // Read the data block
    let data_block: &[u8] = r.take_slice(data_length as usize)?.buffer;

    let mut male_values: Vec<TlkString> = Vec::with_capacity(male_refs.len());
    let mut female_values: Vec<TlkString> = Vec::with_capacity(female_refs.len());

    // Decode the male ref values
    for (key, offset) in male_refs {
        let text = HuffmanUtf16::decode(data_block, &huffman_tree, offset as usize, usize::MAX)?;
        male_values.push(TlkString {
            id: key,
            value: text,
        })
    }

    // Decode the female ref values
    for (key, offset) in female_refs {
        let text = HuffmanUtf16::decode(data_block, &huffman_tree, offset as usize, usize::MAX)?;
        female_values.push(TlkString {
            id: key,
            value: text,
        })
    }

    Ok(Tlk {
        version,
        min_version,
        male_values,
        female_values,
    })
}
