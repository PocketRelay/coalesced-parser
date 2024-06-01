use std::{
    error::Error,
    fs::File,
    io::{Read, Write},
};

use reader::{read_coalesced, serialize_coalesced, Deserializer};

pub mod error;
pub mod reader;

fn main() -> Result<(), Box<dyn Error>> {
    let mut file = File::open("./private/coalesced.bin").unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();

    let mut r = Deserializer::new(&buf);

    let file = read_coalesced(&mut r)?;

    // dbg!(&file);
    let output = serde_json::to_string_pretty(&file).unwrap();

    File::create("./private/coalesced.json")
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();

    let encoded = serialize_coalesced(file);
    let decoded = read_coalesced(&mut Deserializer::new(&encoded))?;

    println!("{:?} {:?}", encoded.len(), buf.len());

    File::create("./private/coalesced_re.bin")
        .unwrap()
        .write_all(encoded.as_ref())
        .unwrap();

    // dbg!(&file);
    let output = serde_json::to_string_pretty(&decoded).unwrap();

    File::create("./private/coalesced_re.json")
        .unwrap()
        .write_all(output.as_bytes())
        .unwrap();

    Ok(())
}
