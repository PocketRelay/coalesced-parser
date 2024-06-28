use crate::{
    crc32::hash_crc32,
    huffman::{FrequencyMap, Huffman},
    invert_huffman_tree,
    shared::{Coalesced, ValueType, ME3_MAGIC},
    Tlk, WChar, TLK_MAGIC,
};
use bitvec::{access::BitSafeU8, order::Lsb0, store::BitStore, vec::BitVec};
use std::collections::HashSet;

/// Seekable buffer implementation. Can seek beyond the end of the buffer. Writes
/// past the end of the buffer grow the underlying buffer to match
#[derive(Default)]
struct WriteBuffer {
    /// The underlying byte buffer
    buffer: Vec<u8>,
    /// The current cursor position
    cursor: usize,
    /// The length of the buffer that has been written to
    length: usize,
}

impl WriteBuffer {
    pub fn into_vec(mut self) -> Vec<u8> {
        self.buffer.truncate(self.length);
        self.buffer
    }

    pub fn write_u32(&mut self, value: u32) {
        self.write_slice(&value.to_le_bytes());
    }

    pub fn write_u16(&mut self, value: u16) {
        self.write_slice(&value.to_le_bytes());
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_slice(&value.to_le_bytes());
    }

    pub fn write_slice(&mut self, value: &[u8]) {
        let data = self.get_slice_mut(value.len());
        data.copy_from_slice(value);
        self.cursor += value.len();

        if self.cursor > self.length {
            self.length = self.cursor;
        }
    }

    pub fn seek(&mut self, cursor: usize) {
        self.cursor = cursor;
    }

    pub fn get_slice_mut(&mut self, length: usize) -> &mut [u8] {
        let start = self.cursor;
        let end = self.cursor + length;

        let buffer_length = self.buffer.len();

        // If the end point is past the buffer grow the buffer
        if start > buffer_length || end > buffer_length {
            self.buffer.resize(end, 0);
        }

        &mut self.buffer[start..end]
    }
}

/// Serializes the provided coalesced into bytes
pub fn serialize_coalesced(coalesced: &Coalesced) -> Vec<u8> {
    let mut keys: HashSet<&str> = HashSet::new();

    let mut max_value_length = 0;

    let huffman: Huffman<char> = {
        let mut freq = FrequencyMap::<char>::default();

        // Collect all keys for the string table
        for file in &coalesced.files {
            keys.insert(&file.path);

            for section in &file.sections {
                keys.insert(&section.name);

                for value in &section.properties {
                    keys.insert(&value.name);

                    for item in &value.values {
                        if let Some(text) = &item.text {
                            // Collect blob of values for huffman encoded data
                            freq.push_iter(text.chars());
                            freq.push('\0');

                            let value_length = text.len();
                            if value_length > max_value_length {
                                max_value_length = value_length;
                            }
                        }
                    }
                }
            }
        }

        Huffman::new(freq)
    };

    // Sort the keys
    let mut keys: Vec<&str> = keys.into_iter().collect();
    keys.sort_by_key(|a| hash_crc32(a.as_bytes()));

    // Determine the max key length
    let mut max_key_length = 0;
    for key in &keys {
        let key_len = key.len();
        if key_len > max_key_length {
            max_key_length = key_len;
        }
    }

    // Build the string table buffer
    let string_table_buffer: Vec<u8> = {
        let mut string_table_buffer = WriteBuffer::default();
        string_table_buffer.seek(4); // Skip writing length till later
        string_table_buffer.write_u32(keys.len() as u32); // Total number of keys

        string_table_buffer.seek(4 + 4 + (8 * keys.len()));

        let mut offsets: Vec<(u32, u32)> = Vec::new();

        // Write the data table
        for key in &keys {
            let offset = string_table_buffer.cursor as u32;

            let bytes: &[u8] = key.as_bytes();
            let bytes_len = bytes.len();

            let hash = hash_crc32(bytes);

            string_table_buffer.write_u16(bytes_len as u16);
            string_table_buffer.write_slice(bytes);

            offsets.push((hash, offset))
        }

        // Seek to start of table
        string_table_buffer.seek(8);

        // Write the offsets
        for (hash, offset) in offsets {
            string_table_buffer.write_u32(hash);
            string_table_buffer.write_u32(offset - 8);
        }

        // Return to start and write length
        string_table_buffer.seek(0);
        string_table_buffer.write_u32(string_table_buffer.length as u32);

        string_table_buffer.into_vec()
    };

    let huffman_buffer = {
        let mut huffman_buffer: WriteBuffer = WriteBuffer::default();

        let pairs = huffman.get_pairs();

        //Write the length of pairs
        huffman_buffer.write_u16(pairs.len() as u16);

        // Write the pairs
        for (left, right) in pairs {
            huffman_buffer.write_i32(*left);
            huffman_buffer.write_i32(*right);
        }

        huffman_buffer.into_vec()
    };

    let huffman_size: usize = huffman_buffer.len();

    let mut data_buffer: BitVec<BitSafeU8, Lsb0> = BitVec::new();

    let index_buffer = {
        let mut index_buffer: WriteBuffer = WriteBuffer::default();

        let mut file_data_offset = 2 /* file counts */ + (coalesced.files.len() * 6);

        let mut file_offsets: Vec<(u16, u32)> = Vec::new();

        for file in &coalesced.files {
            file_offsets.push((
                keys.iter()
                    .position(|key| key.eq(&file.path))
                    .expect("Missing file name key") as u16,
                file_data_offset as u32,
            ));

            let mut section_data_offset = 2 + (file.sections.len() * 6);
            let mut section_offset: Vec<(u16, u32)> = Vec::new();

            for section in &file.sections {
                section_offset.push((
                    keys.iter()
                        .position(|key| key.eq(&section.name))
                        .expect("Missing section name key") as u16,
                    section_data_offset as u32,
                ));

                let mut value_data_offset = 2 + (section.properties.len() * 6);
                let mut property_offsets: Vec<(u16, u32)> = Vec::new();

                for property in &section.properties {
                    index_buffer.seek(file_data_offset + section_data_offset + value_data_offset);

                    property_offsets.push((
                        keys.iter()
                            .position(|key| key.eq(&property.name))
                            .expect("Missing property name key") as u16,
                        value_data_offset as u32,
                    ));

                    index_buffer.write_u16(property.values.len() as u16);
                    value_data_offset += 2;

                    for item in &property.values {
                        let bit_offset = data_buffer.len();
                        let text: Option<&String> = match item.ty {
                            ValueType::RemoveProperty => None,
                            _ => item.text.as_ref(),
                        };

                        // Combine the type and the offset
                        index_buffer
                            .write_u32(((item.ty as u8 as u32) << 29) | (bit_offset as u32));

                        if let Some(text) = text {
                            huffman.encode(text.chars(), &mut data_buffer);
                            huffman.encode_null(&mut data_buffer);
                        }

                        value_data_offset += 4;
                    }
                }

                index_buffer.seek(file_data_offset + section_data_offset);

                index_buffer.write_u16(property_offsets.len() as u16);
                section_data_offset += 2;

                for (name_index, offset) in property_offsets {
                    index_buffer.write_u16(name_index);
                    index_buffer.write_u32(offset);
                    section_data_offset += 6;
                }

                section_data_offset += value_data_offset;
            }

            index_buffer.seek(file_data_offset);

            index_buffer.write_u16(section_offset.len() as u16);
            file_data_offset += 2;

            for (name_index, offset) in section_offset {
                index_buffer.write_u16(name_index);
                index_buffer.write_u32(offset);
                file_data_offset += 6;
            }

            file_data_offset += section_data_offset;
        }

        index_buffer.seek(0);

        index_buffer.write_u16(file_offsets.len() as u16);

        for (name_index, offset) in file_offsets {
            index_buffer.write_u16(name_index);
            index_buffer.write_u32(offset);
        }

        index_buffer.into_vec()
    };

    let index_size: usize = index_buffer.len();

    let total_bits = data_buffer.len();
    let data_bytes = bit_to_bytes(data_buffer);
    let data_size: usize = data_bytes.len();
    let string_table_length = string_table_buffer.len();

    let mut out = WriteBuffer::default();

    // Write the headers
    out.write_u32(ME3_MAGIC);
    out.write_u32(coalesced.version);
    out.write_u32(max_key_length as u32);
    out.write_u32(max_value_length as u32);
    out.write_u32(string_table_length as u32);
    out.write_u32(huffman_size as u32);
    out.write_u32(index_size as u32);
    out.write_u32(data_size as u32);

    // Write the contents
    out.write_slice(&string_table_buffer);
    out.write_slice(&huffman_buffer);
    out.write_slice(&index_buffer);
    out.write_u32(total_bits as u32);
    out.write_slice(&data_bytes);

    out.into_vec()
}

fn bit_to_bytes(mut bits: BitVec<BitSafeU8, Lsb0>) -> Vec<u8> {
    // Convert the bits to bytes
    bits.set_uninitialized(false);
    bits.into_vec()
        .into_iter()
        .map(|value| value.load_value())
        .collect()
}

pub fn serialize_tlk(tlk: &Tlk) -> Vec<u8> {
    let mut out = WriteBuffer::default();

    let male_entry_count: u32 = tlk.male_values.len() as u32;
    let female_entry_count: u32 = tlk.female_values.len() as u32;

    let huffman: Huffman<WChar> = {
        let mut freq = FrequencyMap::<WChar>::default();

        // Create a frequency map for the huffman tree with all the values
        tlk.male_values
            .iter()
            .chain(tlk.female_values.iter())
            .for_each(|value| {
                freq.push_iter(value.value.iter().copied());
                freq.push(0)
            });

        Huffman::new(freq)
    };

    let (huffman_buffer, tree_node_count) = {
        let mut huffman_buffer: WriteBuffer = WriteBuffer::default();

        let mut pairs = huffman.get_pairs().to_vec();
        invert_huffman_tree(&mut pairs);

        let tree_node_count = pairs.len() as u32;

        // Write the pairs
        for (left, right) in pairs {
            huffman_buffer.write_i32(left);
            huffman_buffer.write_i32(right);
        }

        (huffman_buffer.into_vec(), tree_node_count)
    };

    let mut data_buffer: BitVec<BitSafeU8, Lsb0> = BitVec::new();
    let mut ref_buffer = WriteBuffer::default();

    {
        tlk.male_values
            .iter()
            .chain(tlk.female_values.iter())
            .for_each(|value| {
                let bit_offset: usize = data_buffer.len();

                huffman.encode(value.value.iter().copied(), &mut data_buffer);
                huffman.encode_null(&mut data_buffer);

                ref_buffer.write_u32(value.id);
                ref_buffer.write_u32(bit_offset as u32);
            });
    }

    let data_bytes = bit_to_bytes(data_buffer);

    // Write the headers
    out.write_u32(TLK_MAGIC);
    out.write_u32(tlk.version);
    out.write_u32(tlk.min_version);
    out.write_u32(male_entry_count);
    out.write_u32(female_entry_count);
    out.write_u32(tree_node_count);
    out.write_u32(data_bytes.len() as u32);

    // Write the contents
    out.write_slice(&ref_buffer.buffer);
    out.write_slice(&huffman_buffer);
    out.write_slice(&data_bytes);

    out.into_vec()
}
