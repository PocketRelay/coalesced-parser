use std::{
    borrow::Cow,
    cell::RefCell,
    cmp::Ordering,
    collections::{BinaryHeap, HashMap, HashSet, VecDeque},
    hash::Hash,
    ptr::NonNull,
    rc::Rc,
};

use bitvec::{access::BitSafeU8, index, order::Lsb0, store::BitStore, vec::BitVec};

use crate::error::{DecodeError, DecodeResult};

#[derive(Default)]
pub struct Serializer {
    pub buffer: Vec<u8>,
    pub cursor: usize,
    pub length: usize,
}

impl Serializer {
    pub fn to_vec(mut self) -> Vec<u8> {
        self.buffer.truncate(self.length);
        self.buffer
    }

    pub fn write_u32(&mut self, value: u32) {
        let data = self.get_slice_mut(4);
        data.copy_from_slice(&value.to_le_bytes());
        self.cursor += 4;

        if self.cursor > self.length {
            self.length = self.cursor;
        }
    }

    pub fn write_u16(&mut self, value: u16) {
        let data = self.get_slice_mut(2);
        data.copy_from_slice(&value.to_le_bytes());
        self.cursor += 2;

        if self.cursor > self.length {
            self.length = self.cursor;
        }
    }

    pub fn write_i32(&mut self, value: i32) {
        let data = self.get_slice_mut(4);
        data.copy_from_slice(&value.to_le_bytes());
        self.cursor += 4;

        if self.cursor > self.length {
            self.length = self.cursor;
        }
    }

    pub fn write_i16(&mut self, value: i16) {
        let data = self.get_slice_mut(2);
        data.copy_from_slice(&value.to_le_bytes());
        self.cursor += 2;

        if self.cursor > self.length {
            self.length = self.cursor;
        }
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

        if start > buffer_length || end > buffer_length {
            self.buffer.resize(end, 0);
        }

        &mut self.buffer[start..end]
    }
}

pub struct Deserializer<'de> {
    /// Buffer storing the bytes to be deserialized
    buffer: &'de [u8],
    /// Cursor representing the current offset within the buffer
    cursor: usize,
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

    pub fn take_slice(&mut self, length: usize) -> DecodeResult<Deserializer<'de>> {
        Ok(Self::new(self.read_bytes(length)?))
    }

    pub fn read_u32(&mut self) -> DecodeResult<u32> {
        let bytes = self.read_fixed::<4>()?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub fn read_u16(&mut self) -> DecodeResult<u16> {
        let bytes = self.read_fixed::<2>()?;
        Ok(u16::from_le_bytes(bytes))
    }

    pub fn read_i32(&mut self) -> DecodeResult<i32> {
        let bytes = self.read_fixed::<4>()?;
        Ok(i32::from_le_bytes(bytes))
    }

    pub fn read_i16(&mut self) -> DecodeResult<i16> {
        let bytes = self.read_fixed::<2>()?;
        Ok(i16::from_le_bytes(bytes))
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Coalesced {
    pub version: u32,
    pub files: Vec<CoalFile>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CoalFile {
    pub name: String,
    pub sections: Vec<Section>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Section {
    pub name: String,
    pub values: Vec<Value>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Value {
    pub name: String,
    pub properties: Vec<PropertyValue>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PropertyValue {
    /// Value type
    pub ty: u32,
    /// Associated text value
    pub text: Option<String>,
}

pub fn serialize_coalesced(coalesced: Coalesced) -> Vec<u8> {
    let mut keys: HashSet<&str> = HashSet::new();

    let mut blob = String::new();
    let mut max_value_length = 0;

    for file in &coalesced.files {
        keys.insert(&file.name);

        for section in &file.sections {
            keys.insert(&section.name);

            for value in &section.values {
                keys.insert(&value.name);

                for item in &value.properties {
                    if let Some(text) = &item.text {
                        // Blob of null terminated values
                        blob.push_str(text);
                        blob.push('\0');

                        let value_length = text.len();
                        if value_length > max_value_length {
                            max_value_length = value_length;
                        }
                    }
                }
            }
        }
    }

    let mut keys: Vec<&str> = keys.into_iter().collect();
    keys.sort_by_key(|a| hash_crc32(a.as_bytes()));

    let mut max_key_length = 0;
    for key in &keys {
        let key_len = key.len();
        if key_len > max_key_length {
            max_key_length = key_len;
        }
    }

    // Build the string table buffer
    let string_table_buffer: Vec<u8> = {
        let mut string_table_buffer = Serializer::default();
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

        string_table_buffer.to_vec()
    };

    let huffman = Huffman::new(&blob);

    let huffman_buffer = {
        let mut huffman_buffer: Serializer = Serializer::default();

        let pairs = huffman.collect_pairs();
        // let pairs2 = flatten_huffman_tree(huffman.tree.clone());

        println!("Write pairs: {:?}", pairs);
        // println!("Write pairs 2: {:?}", pairs2);

        //Write the length of pairs
        huffman_buffer.write_u16(pairs.len() as u16);

        // Write the pairs
        for (left, right) in pairs {
            huffman_buffer.write_i32(left);
            huffman_buffer.write_i32(right);
        }

        huffman_buffer.to_vec()
    };

    let huffman_size: usize = huffman_buffer.len();

    let mut data_buffer: BitVec<BitSafeU8, Lsb0> = BitVec::new();

    let index_buffer = {
        let mut index_buffer: Serializer = Serializer::default();

        let mut file_data_offset = 2 /* file counts */ + (coalesced.files.len() * 6);

        let mut files: Vec<(u16, u32)> = Vec::new();

        for file in &coalesced.files {
            files.push((
                keys.iter().position(|key| key.eq(&file.name)).unwrap() as u16,
                file_data_offset as u32,
            ));

            let mut section_data_offset = 2 + (file.sections.len() * 6);
            let mut sections: Vec<(u16, u32)> = Vec::new();

            for section in &file.sections {
                sections.push((
                    keys.iter().position(|key| key.eq(&section.name)).unwrap() as u16,
                    section_data_offset as u32,
                ));

                let mut value_data_offset = 2 + (section.values.len() * 6);
                let mut values: Vec<(u16, u32)> = Vec::new();

                for value in &section.values {
                    index_buffer.seek(file_data_offset + section_data_offset + value_data_offset);

                    values.push((
                        keys.iter().position(|key| key.eq(&value.name)).unwrap() as u16,
                        value_data_offset as u32,
                    ));

                    index_buffer.write_u16(value.properties.len() as u16);
                    value_data_offset += 2;

                    for item in &value.properties {
                        let bit_offset = data_buffer.len();

                        match item.ty {
                            1 => index_buffer.write_u32((1 << 29) | (bit_offset as u32)),
                            0 | 2 | 3 | 4 => {
                                index_buffer.write_u32((item.ty << 29) | (bit_offset as u32));
                                let mut value = item.text.clone().unwrap_or_default();
                                value.push('\0');

                                encode_huffman(&value, &huffman.mapping, &mut data_buffer);
                            }
                            _ => panic!("Unknown type"),
                        }

                        value_data_offset += 4;
                    }
                }

                index_buffer.seek(file_data_offset + section_data_offset);

                index_buffer.write_u16(values.len() as u16);

                section_data_offset += 2;

                for value in values {
                    index_buffer.write_u16(value.0);
                    index_buffer.write_u32(value.1);

                    section_data_offset += 6;
                }

                section_data_offset += value_data_offset;
            }

            index_buffer.seek(file_data_offset);

            index_buffer.write_u16(sections.len() as u16);
            file_data_offset += 2;

            for section in sections {
                index_buffer.write_u16(section.0);
                index_buffer.write_u32(section.1);

                file_data_offset += 6;
            }

            file_data_offset += section_data_offset;
        }

        index_buffer.seek(0);

        index_buffer.write_u16(files.len() as u16);

        for file in files {
            index_buffer.write_u16(file.0);
            index_buffer.write_u32(file.1);
        }

        index_buffer.to_vec()
    };

    let index_size: usize = index_buffer.len();

    let total_bits = data_buffer.len();
    let data_bytes = bit_to_bytes(data_buffer);
    let data_size: usize = data_bytes.len();

    println!("Write data bits: {}", total_bits);

    let mut out = Vec::new();

    // Write the headers
    out.extend_from_slice(&0x666D726Du32.to_le_bytes());
    out.extend_from_slice(&coalesced.version.to_le_bytes());
    out.extend_from_slice(&(max_key_length as u32).to_le_bytes());
    out.extend_from_slice(&(max_value_length as u32).to_le_bytes());
    out.extend_from_slice(&(string_table_buffer.len() as u32).to_le_bytes());
    out.extend_from_slice(&(huffman_size as u32).to_le_bytes());
    out.extend_from_slice(&(index_size as u32).to_le_bytes());
    out.extend_from_slice(&(data_size as u32).to_le_bytes());

    println!("Write dat size: {}", data_size);

    // Write all the block buffers
    out.extend_from_slice(&string_table_buffer);
    out.extend_from_slice(&huffman_buffer);
    out.extend_from_slice(&index_buffer);
    out.extend_from_slice(&(total_bits as u32).to_le_bytes());
    out.extend_from_slice(&data_bytes);

    out
}

fn bit_to_bytes(mut bits: BitVec<BitSafeU8, Lsb0>) -> Vec<u8> {
    // Convert the bits to bytes
    bits.set_uninitialized(false);
    bits.into_vec()
        .into_iter()
        .map(|value| value.load_value())
        .collect()
}

pub fn read_coalesced(r: &mut Deserializer) -> DecodeResult<Coalesced> {
    // Read the file header
    let magic = r.read_u32()?;

    if magic != 0x666D726D {
        return Err(DecodeError::Other("Not a ME3 coalesced file"));
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
            return Err(DecodeError::Other("String table size mismatch"));
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
                return Err(DecodeError::Other("String table hash mismatch"));
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

    println!("Read pairs {huffman_tree:?}");

    // Read the index block
    let mut index_block: Deserializer = r.take_slice(index_size as usize)?;

    let data_block: &[u8] = {
        // Read the total bits count
        let _total_bits = r.read_u32()?;

        println!("Read data bits: {} {}", _total_bits as f32 / 8., data_size);
        println!("Read dat size: {}", data_size);

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
            .ok_or(DecodeError::Other("Invalid file name offset"))?;

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
                .ok_or(DecodeError::Other("Invalid file name offset"))?;

            // Read the section offset
            let section_offset = index_block.read_u32()?;

            section_offsets.push((section_name.to_string(), section_offset as usize));
        }

        for (section_name, section_offset) in section_offsets {
            // Seek the index to the section
            index_block.seek(file_offset + section_offset)?;

            let values_count = index_block.read_u16()? as usize;
            let mut values: Vec<Value> = Vec::with_capacity(values_count);
            let mut value_offsets: Vec<(String, usize)> = Vec::with_capacity(values_count);

            for _ in 0..values_count {
                // Read the value name and get it from the string table
                let value_name_index = index_block.read_u16()?;
                let value_name = string_table
                    .get(value_name_index as usize)
                    .ok_or(DecodeError::Other("Invalid file name offset"))?;

                // Read the value offset
                let value_offset = index_block.read_u32()?;
                value_offsets.push((value_name.to_string(), value_offset as usize));
            }

            for (value_name, value_offset) in value_offsets {
                // Seek the index to the value
                index_block.seek(file_offset + section_offset + value_offset)?;

                let item_count = index_block.read_u16()? as usize;
                let mut items: Vec<PropertyValue> = Vec::with_capacity(values_count);

                for _ in 0..item_count {
                    // Read the item offset
                    let item_offset = index_block.read_u32()?;

                    // Split the type and offset
                    let ty = (item_offset & 0xE0000000) >> 29;
                    let item_offset = item_offset & 0x1fffffff;

                    match ty {
                        1 => items.push(PropertyValue { ty: 1, text: None }),
                        0 | 2 | 3 | 4 => {
                            let text = huffman_decode(
                                data_block,
                                &huffman_tree,
                                item_offset as usize,
                                max_value_length as usize,
                            );
                            items.push(PropertyValue {
                                ty,
                                text: Some(text),
                            })
                        }
                        _ => return Err(DecodeError::Other("Unknown property value type")),
                    }
                }

                values.push(Value {
                    name: value_name,
                    properties: items,
                })
            }

            sections.push(Section {
                name: section_name,
                values,
            })
        }

        files.push(CoalFile {
            name: file_name,
            sections,
        })
    }

    let coalesced = Coalesced { version, files };

    Ok(coalesced)
}

static CRC32_TABLE: &[u32] = &[
    0x00000000, 0x04C11DB7, 0x09823B6E, 0x0D4326D9, 0x130476DC, 0x17C56B6B, 0x1A864DB2, 0x1E475005,
    0x2608EDB8, 0x22C9F00F, 0x2F8AD6D6, 0x2B4BCB61, 0x350C9B64, 0x31CD86D3, 0x3C8EA00A, 0x384FBDBD,
    0x4C11DB70, 0x48D0C6C7, 0x4593E01E, 0x4152FDA9, 0x5F15ADAC, 0x5BD4B01B, 0x569796C2, 0x52568B75,
    0x6A1936C8, 0x6ED82B7F, 0x639B0DA6, 0x675A1011, 0x791D4014, 0x7DDC5DA3, 0x709F7B7A, 0x745E66CD,
    0x9823B6E0, 0x9CE2AB57, 0x91A18D8E, 0x95609039, 0x8B27C03C, 0x8FE6DD8B, 0x82A5FB52, 0x8664E6E5,
    0xBE2B5B58, 0xBAEA46EF, 0xB7A96036, 0xB3687D81, 0xAD2F2D84, 0xA9EE3033, 0xA4AD16EA, 0xA06C0B5D,
    0xD4326D90, 0xD0F37027, 0xDDB056FE, 0xD9714B49, 0xC7361B4C, 0xC3F706FB, 0xCEB42022, 0xCA753D95,
    0xF23A8028, 0xF6FB9D9F, 0xFBB8BB46, 0xFF79A6F1, 0xE13EF6F4, 0xE5FFEB43, 0xE8BCCD9A, 0xEC7DD02D,
    0x34867077, 0x30476DC0, 0x3D044B19, 0x39C556AE, 0x278206AB, 0x23431B1C, 0x2E003DC5, 0x2AC12072,
    0x128E9DCF, 0x164F8078, 0x1B0CA6A1, 0x1FCDBB16, 0x018AEB13, 0x054BF6A4, 0x0808D07D, 0x0CC9CDCA,
    0x7897AB07, 0x7C56B6B0, 0x71159069, 0x75D48DDE, 0x6B93DDDB, 0x6F52C06C, 0x6211E6B5, 0x66D0FB02,
    0x5E9F46BF, 0x5A5E5B08, 0x571D7DD1, 0x53DC6066, 0x4D9B3063, 0x495A2DD4, 0x44190B0D, 0x40D816BA,
    0xACA5C697, 0xA864DB20, 0xA527FDF9, 0xA1E6E04E, 0xBFA1B04B, 0xBB60ADFC, 0xB6238B25, 0xB2E29692,
    0x8AAD2B2F, 0x8E6C3698, 0x832F1041, 0x87EE0DF6, 0x99A95DF3, 0x9D684044, 0x902B669D, 0x94EA7B2A,
    0xE0B41DE7, 0xE4750050, 0xE9362689, 0xEDF73B3E, 0xF3B06B3B, 0xF771768C, 0xFA325055, 0xFEF34DE2,
    0xC6BCF05F, 0xC27DEDE8, 0xCF3ECB31, 0xCBFFD686, 0xD5B88683, 0xD1799B34, 0xDC3ABDED, 0xD8FBA05A,
    0x690CE0EE, 0x6DCDFD59, 0x608EDB80, 0x644FC637, 0x7A089632, 0x7EC98B85, 0x738AAD5C, 0x774BB0EB,
    0x4F040D56, 0x4BC510E1, 0x46863638, 0x42472B8F, 0x5C007B8A, 0x58C1663D, 0x558240E4, 0x51435D53,
    0x251D3B9E, 0x21DC2629, 0x2C9F00F0, 0x285E1D47, 0x36194D42, 0x32D850F5, 0x3F9B762C, 0x3B5A6B9B,
    0x0315D626, 0x07D4CB91, 0x0A97ED48, 0x0E56F0FF, 0x1011A0FA, 0x14D0BD4D, 0x19939B94, 0x1D528623,
    0xF12F560E, 0xF5EE4BB9, 0xF8AD6D60, 0xFC6C70D7, 0xE22B20D2, 0xE6EA3D65, 0xEBA91BBC, 0xEF68060B,
    0xD727BBB6, 0xD3E6A601, 0xDEA580D8, 0xDA649D6F, 0xC423CD6A, 0xC0E2D0DD, 0xCDA1F604, 0xC960EBB3,
    0xBD3E8D7E, 0xB9FF90C9, 0xB4BCB610, 0xB07DABA7, 0xAE3AFBA2, 0xAAFBE615, 0xA7B8C0CC, 0xA379DD7B,
    0x9B3660C6, 0x9FF77D71, 0x92B45BA8, 0x9675461F, 0x8832161A, 0x8CF30BAD, 0x81B02D74, 0x857130C3,
    0x5D8A9099, 0x594B8D2E, 0x5408ABF7, 0x50C9B640, 0x4E8EE645, 0x4A4FFBF2, 0x470CDD2B, 0x43CDC09C,
    0x7B827D21, 0x7F436096, 0x7200464F, 0x76C15BF8, 0x68860BFD, 0x6C47164A, 0x61043093, 0x65C52D24,
    0x119B4BE9, 0x155A565E, 0x18197087, 0x1CD86D30, 0x029F3D35, 0x065E2082, 0x0B1D065B, 0x0FDC1BEC,
    0x3793A651, 0x3352BBE6, 0x3E119D3F, 0x3AD08088, 0x2497D08D, 0x2056CD3A, 0x2D15EBE3, 0x29D4F654,
    0xC5A92679, 0xC1683BCE, 0xCC2B1D17, 0xC8EA00A0, 0xD6AD50A5, 0xD26C4D12, 0xDF2F6BCB, 0xDBEE767C,
    0xE3A1CBC1, 0xE760D676, 0xEA23F0AF, 0xEEE2ED18, 0xF0A5BD1D, 0xF464A0AA, 0xF9278673, 0xFDE69BC4,
    0x89B8FD09, 0x8D79E0BE, 0x803AC667, 0x84FBDBD0, 0x9ABC8BD5, 0x9E7D9662, 0x933EB0BB, 0x97FFAD0C,
    0xAFB010B1, 0xAB710D06, 0xA6322BDF, 0xA2F33668, 0xBCB4666D, 0xB8757BDA, 0xB5365D03, 0xB1F740B4,
];

/// calc crc32 for binary data
fn hash_crc32(bin_data: &[u8]) -> u32 {
    let mut hash = !0;
    for t in bin_data {
        hash = CRC32_TABLE[((hash >> 24) as u8 ^ t) as usize] ^ (hash << 8);
    }
    !hash
}

#[derive(Debug)]
enum HuffmanTree {
    Node(Rc<HuffmanTree>, Rc<HuffmanTree>),
    Leaf(char, u32),
}

impl HuffmanTree {
    fn frequency(&self) -> u32 {
        match *self {
            HuffmanTree::Node(ref left, ref right) => left.frequency() + right.frequency(),
            HuffmanTree::Leaf(_, freq) => freq,
        }
    }
}

impl PartialEq for HuffmanTree {
    fn eq(&self, other: &Self) -> bool {
        self.frequency().eq(&other.frequency())
    }
}

impl Eq for HuffmanTree {}

impl Ord for HuffmanTree {
    fn cmp(&self, other: &Self) -> Ordering {
        self.frequency().cmp(&other.frequency()).reverse()
    }
}

impl PartialOrd for HuffmanTree {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

// fn flatten_huffman_tree(tree: Rc<HuffmanTree>) -> Vec<(i32, i32)> {
//     let mut result = Vec::new();
//     let mut queue = VecDeque::new();
//     let mut node_index_map = HashMap::new();
//     let mut current_index = 0;

//     queue.push_back(tree.clone());
//     node_index_map.insert(tree, current_index);
//     current_index += 1;

//     while let Some(node) = queue.pop_front() {
//         match &*node {
//             HuffmanTree::Leaf(symbol, _) => {
//                 result.push((-1 - *symbol as i32, current_index as i32));
//             }
//             HuffmanTree::Node(left, right) => {
//                 let left_index = *node_index_map.entry(left.clone()).or_insert_with(|| {
//                     queue.push_back(left.clone());
//                     let idx = current_index;
//                     current_index += 1;
//                     idx
//                 });
//                 let right_index = *node_index_map.entry(right.clone()).or_insert_with(|| {
//                     queue.push_back(right.clone());
//                     let idx = current_index;
//                     current_index += 1;
//                     idx
//                 });
//                 result.push((left_index as i32, right_index as i32));
//             }
//         }
//     }

//     // Ensure the last leaf's right-hand side index is set correctly
//     if let Some((_, last)) = result.last_mut() {
//         *last = -1; // Set to -1 to indicate the end
//     }

//     result
// }

fn build_huffman_tree(text: &str) -> HuffmanTree {
    let mut frequency_map = HashMap::new();

    for c in text.chars() {
        *frequency_map.entry(c).or_insert(0) += 1;
    }

    let mut heap = BinaryHeap::new();

    for (char, freq) in frequency_map {
        heap.push(HuffmanTree::Leaf(char, freq));
    }

    while heap.len() > 1 {
        let left = heap.pop().unwrap();
        let right = heap.pop().unwrap();

        heap.push(HuffmanTree::Node(Rc::new(left), Rc::new(right)));
    }

    heap.pop().unwrap()
}

fn generate_huffman_codes(node: &HuffmanTree, prefix: BitVec, codes: &mut HashMap<char, BitVec>) {
    match node {
        HuffmanTree::Node(left, right) => {
            let mut left_prefix = prefix.clone();
            left_prefix.push(false);
            generate_huffman_codes(left, left_prefix, codes);

            let mut right_prefix = prefix;
            right_prefix.push(true);
            generate_huffman_codes(right, right_prefix, codes);
        }
        HuffmanTree::Leaf(char, _) => {
            codes.insert(*char, prefix);
        }
    }
}

// Encode the input text
fn encode_huffman(text: &str, codes: &HashMap<char, BitVec>, output: &mut BitVec<BitSafeU8, Lsb0>) {
    for character in text.chars() {
        if let Some(code) = codes.get(&character) {
            output.extend(code);
        }
    }
}

pub struct Huffman {
    tree: Rc<HuffmanTree>,
    mapping: HashMap<char, BitVec>,
}

impl Huffman {
    pub fn new(str: &str) -> Self {
        let huffman_tree = build_huffman_tree(str);
        let mut huffman_mapping = HashMap::new();
        generate_huffman_codes(&huffman_tree, BitVec::new(), &mut huffman_mapping);
        Self {
            tree: Rc::new(huffman_tree),
            mapping: huffman_mapping,
        }
    }

    /// Flattens the tree of huffman nodes into pairs where negative values are the symbols and
    /// positive values are the next node index
    pub fn collect_pairs(&self) -> Vec<(i32, i32)> {
        let mut pairs: Vec<Rc<RefCell<(i32, i32)>>> = Vec::new();
        let mut mapping: HashMap<*const HuffmanTree, Rc<RefCell<(i32, i32)>>> = HashMap::new();
        let mut queue: VecDeque<&HuffmanTree> = VecDeque::new();

        let root_pair = Rc::new(RefCell::new((0, 0)));

        mapping.insert(self.tree.as_ref(), root_pair.clone());
        queue.push_back(&self.tree);

        while let Some(node) = queue.pop_front() {
            let item = mapping.get(&(node as *const _)).unwrap().clone();

            if let HuffmanTree::Node(left_node, right_node) = node {
                if let HuffmanTree::Leaf(symbol, _) = left_node.as_ref() {
                    item.borrow_mut().0 = -1 - *symbol as i32;
                } else {
                    let left = Rc::new(RefCell::new((0, 0)));

                    // Add empty left pair
                    mapping.insert(left_node.as_ref(), left.clone());
                    pairs.push(left.clone());

                    // Queue the left node
                    queue.push_back(left_node.as_ref());

                    {
                        item.borrow_mut().0 = (pairs.len() - 1) as i32;
                    }
                }

                if let HuffmanTree::Leaf(symbol, _) = right_node.as_ref() {
                    item.borrow_mut().1 = -1 - *symbol as i32;
                } else {
                    let right = Rc::new(RefCell::new((0, 0)));

                    // Add empty right pair
                    mapping.insert(right_node.as_ref(), right.clone());
                    pairs.push(right.clone());

                    queue.push_back(right_node.as_ref());

                    {
                        item.borrow_mut().1 = (pairs.len() - 1) as i32;
                    }
                }
            } else {
                panic!("Invalid operation: leaf node in queue");
            }
        }
        pairs.push(root_pair);

        let pairs = pairs.into_iter().map(|value| *value.borrow()).collect();
        pairs
    }
}

pub fn huffman_decode(
    compressed_data: &[u8],
    pairs: &[(i32, i32)],
    position: usize,
    max_length: usize,
) -> String {
    let mut sb = String::new();
    let mut cur_node = pairs.len() - 1;
    let end = compressed_data.len() * 8;

    let mut pos = position;

    while pos < end && sb.len() < max_length {
        let sample = compressed_data[pos / 8] & (1 << (pos % 8));
        let next = pairs[cur_node];
        let next = if sample != 0 { next.1 } else { next.0 };

        if next < 0 {
            let ch = (-1 - next) as u16;
            if ch == 0 {
                break;
            }
            sb.push(ch as u8 as char);
            cur_node = pairs.len() - 1;
        } else {
            cur_node = next as usize;
            if cur_node > pairs.len() {
                panic!("The decompression nodes are malformed.");
            }
        }

        pos += 1;
    }

    sb
}
