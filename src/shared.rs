/// Magic bytes for ME3
pub const ME3_MAGIC: u32 = 0x666D726D;
/// Magic bytes for the ME3 tlk file
pub const TLK_MAGIC: u32 = 0x006B6C54;

/// Tlk file
#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct Tlk {
    pub version: u32,
    pub min_version: u32,

    /// Male tlk strings
    pub male_values: Vec<TlkString>,
    /// Female tlk strings
    pub female_values: Vec<TlkString>,
}

impl Tlk {
    /// Replaces a string with the provided ID with a new value
    pub fn replace_male(&mut self, id: u32, value: String) -> bool {
        if let Some(entry) = self.male_values.iter_mut().find(|value| value.id == id) {
            entry.value = value;
            true
        } else {
            false
        }
    }

    /// Inserts a value into the tlk attempting to replace an existing one
    pub fn insert_male(&mut self, id: u32, value: String) {
        if self.replace_male(id, value.clone()) {
            return;
        }

        self.male_values.push(TlkString { id, value })
    }

    /// Replaces a string with the provided ID with a new value
    pub fn replace_female(&mut self, id: u32, value: String) -> bool {
        if let Some(entry) = self.female_values.iter_mut().find(|value| value.id == id) {
            entry.value = value;
            true
        } else {
            false
        }
    }

    /// Inserts a value into the tlk attempting to replace an existing one
    pub fn insert_female(&mut self, id: u32, value: String) {
        if self.replace_female(id, value.clone()) {
            return;
        }

        self.female_values.push(TlkString { id, value })
    }
}

/// String within a tlk file
#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct TlkString {
    /// ID of the value
    pub id: u32,
    /// The string value itself
    pub value: String,
}

/// Coalesced file
#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct Coalesced {
    /// Coalesced version
    pub version: u32,
    /// Files within the coalesced
    pub files: Vec<CoalFile>,
}

/// File within the coalesced
#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct CoalFile {
    /// The relative file path
    pub path: String,
    /// The sections within the file
    pub sections: Vec<Section>,
}

#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct Section {
    /// The section name
    pub name: String,
    /// Properties within the section
    pub properties: Vec<Property>,
}

#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct Property {
    /// The name of the property
    pub name: String,
    /// The values for this property
    pub values: Vec<Value>,
}

#[derive(Debug, Hash, serde::Serialize, serde::Deserialize)]
pub struct Value {
    /// Value type
    pub ty: ValueType,
    /// Associated text value
    pub text: Option<String>,
}

#[derive(Debug, Hash, serde::Serialize, serde::Deserialize, Clone, Copy)]
#[repr(u8)]
pub enum ValueType {
    // Overwrite
    New = 0,
    // Remove entirely
    RemoveProperty = 1,
    // Add always
    Add = 2,
    // Add if unique
    AddUnique = 3,
    // Remove if same
    Remove = 4,
}

pub struct UnknownValueType;

impl TryFrom<u8> for ValueType {
    type Error = UnknownValueType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::New,
            1 => Self::RemoveProperty,
            2 => Self::Add,
            3 => Self::AddUnique,
            4 => Self::Remove,
            _ => return Err(UnknownValueType),
        })
    }
}

/// Invests the order of the provided huffman pairs
///
/// The TLK format encodes them in the opposite direction
/// to the Coalesced file so its easier to just flip them
/// than write separate implementations
pub(crate) fn invert_huffman_tree(pairs: &mut Vec<(i32, i32)>) {
    let last_index = (pairs.len() - 1) as i32;

    // Reverse the pair order
    pairs.reverse();

    // Update the pair indexes to match the new order
    for pair in pairs {
        if pair.0 > -1 {
            pair.0 = last_index - pair.0
        }

        if pair.1 > -1 {
            pair.1 = last_index - pair.1
        }
    }
}
