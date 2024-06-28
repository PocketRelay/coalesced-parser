use std::{
    fs::File,
    io::{Read, Write},
};

use me3_coalesced_parser::{deserialize_tlk, serialize_tlk};

/// Tests that a valid tlk can be parsed, encoded, and parsed again
/// without any errors.
#[test]
fn test_tlk_rebuild() {
    // Only run test if valid coalesced is present
    if std::fs::metadata("./private/en.tlk").is_err() {
        println!("Skipping coalesced test");
        return;
    }

    let mut file = File::open("./private/en.tlk").expect("Failed to open tlk file");
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .expect("Failed to read coalesced");

    // Parse
    let tlk = deserialize_tlk(&bytes).expect("Failed to parse tlk");

    // Encode
    let bytes = serialize_tlk(&tlk);

    // Parse
    let tlk = deserialize_tlk(&bytes).expect("Failed to parse tlk");

    let mut out = File::create("./private/tlk_en.json").unwrap();
    out.write_all(serde_json::to_string_pretty(&tlk).unwrap().as_bytes())
        .unwrap();
}
