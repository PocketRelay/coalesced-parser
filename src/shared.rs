// Magic bytes for ME3
pub const ME3_MAGIC: u32 = 0x666D726D;

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
