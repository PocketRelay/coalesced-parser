use std::{fs::File, io::Read};

use me3_coalesced_parser::{deserialize_coalesced, serialize_coalesced};

/// Tests that a valid coalesced can be parsed, encoded, and parsed again
/// without any errors.
#[test]
fn test_coalesced_rebuild() {
    // Only run test if valid coalesced is present
    if std::fs::metadata("./private/coalesced.bin").is_err() {
        println!("Skipping coalesced test");
        return;
    }

    let mut file = File::open("./private/coalesced.bin").expect("Failed to open coalesced file");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Failed to read coalesced");

    // Parse
    let coalesced = deserialize_coalesced(&bytes).expect("Failed to parse coalesced");

    // Encode
    let bytes = serialize_coalesced(&coalesced);

    // Parse
    let _coalesced = deserialize_coalesced(&bytes).expect("Failed to parse coalesced");
}
