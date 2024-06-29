# TLK file format

## 1. Overview

The tlk file format is used to store translation data for ME3

## 2. File Header

The file header is 28 bytes long and contains the following fields:

| Offset | Type | Field              | Description                                         |
| ------ | ---- | ------------------ | --------------------------------------------------- |
| 0      | u32  | Magic              | Magic (`0x006B6C54` for "Tlk\0")                    |
| 4      | u32  | Version            | File version                                        |
| 8      | u32  | Min Version        | File min version                                    |
| 12     | u32  | Male Entry Count   | Number of encoded male entries                      |
| 16     | u32  | Female Entry Count | Number of encoded female entries                    |
| 20     | u32  | Tree Node Count    | Number of encoded huffman pairs                     |
| 24     | u32  | Data Bytes Length  | Length in bytes of the huffman encoded data section |


## 3. Data Structures

### Entry

Each male & female entry consists of the following fields:

| Offset | Type | Field      | Description                          |
| ------ | ---- | ---------- | ------------------------------------ |
| 0      | u32  | Entry ID   | Unique identifier for the entry      |
| 4      | u32  | Bit Offset | Offset into the huffman encoded bits |

### Huffman Node

Each huffman node consists of the following fields:

| Offset | Type | Field | Description                |
| ------ | ---- | ----- | -------------------------- |
| 0      | i32  | Left  | The left half of the node  |
| 4      | i32  | Right | The right half of the node |

The huffman nodes are used when decoding, the bit value of the encoded string is used
to determine which side of the node to follow.

When following a node the following logic is used:

- Negative values are interpreted as the value literals
- Positive values are the index to the next node within the list of nodes 

## 4. Byte Order (Endianness)
All multi-byte values are stored in little-endian format.

## 5. Sections or Segments
The file is divided into three main sections:

1. **Header:** Contains file metadata (28 bytes).
2. **Male Entries:** Contains multiple [Entries](#entry) fields with the amount specified in the header Male Entry Count 
2. **Female Entries:** Contains multiple [Entries](#entry) fields with the amount specified in the header Female Entry Count
3. **Huffman Nodes:** Contains multiple [Huffman Nodes](#huffman-node) with the amount specified in the header Tree Node Count
4. **Data Bytes:** The huffman encoded bit values (Array of bytes to the length specified in the header)

## 6. Data Encoding
- **Numeric Data:** All numeric values are stored as little-endian.
- **Text Data:** Values are UTF-16 encoded strings, padded with null bytes.
- **Compression:** Values are compressed using huffman encoding
