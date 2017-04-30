use schema::TextOptions;
use schema::U32Options;

use serde::{Serialize, Deserialize, Serializer, Deserializer};
use schema::FieldType;

/// A `FieldEntry` represents a field and its configuration.
/// `Schema` are a collection of `FieldEntry`
/// 
/// It consists of 
/// - a field name 
/// - a field type, itself wrapping up options describing 
/// how the field should be indexed.
#[derive(Clone, Debug)]
pub struct FieldEntry {
    name: String,
    field_type: FieldType,
}

impl FieldEntry {
    
    /// Creates a new u32 field entry in the schema, given
    /// a name, and some options.
    pub fn new_text(field_name: String, field_type: TextOptions) -> FieldEntry {
        FieldEntry {
            name: field_name,
            field_type: FieldType::Str(field_type),
        }
    }
    
    /// Creates a new u32 field entry in the schema, given
    /// a name, and some options.
    pub fn new_u32(field_name: String, field_type: U32Options) -> FieldEntry {
        FieldEntry {
            name: field_name,
            field_type: FieldType::U32(field_type),
        }
    }
    
    /// Returns the name of the field
    pub fn name(&self,) -> &String {
        &self.name
    }
        
    /// Returns the field type
    pub fn field_type(&self,) -> &FieldType {
        &self.field_type
    }
    
    /// Returns true iff the field is indexed
    pub fn is_indexed(&self,) -> bool {
        match self.field_type {
            FieldType::Str(ref options) => options.get_indexing_options().is_indexed(),
            FieldType::U32(ref options) => options.is_indexed(),
        }
    }
    
    /// Returns true iff the field is a u32 fast field
    pub fn is_u32_fast(&self,) -> bool {
        match self.field_type {
            FieldType::U32(ref options) => options.is_fast(),
            _ => false,
        }
    }
    
    /// Returns true iff the field is stored
    pub fn is_stored(&self,) -> bool {
        match self.field_type {
            FieldType::U32(ref options) => {
                options.is_stored()
            }
            FieldType::Str(ref options) => {
                options.is_stored()
            }
        }
    }
}

/*
{ 
    "name": "stuff",
    "type": "text",
    "options": {}
}
*/

impl Serialize for FieldEntry {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where S: Serializer
    {
        let mut s = serializer.serialize_struct("field_entry", 3)?;
        s.serialize_field("name", &self.name)?;
        s.serialize_field("type", self.field_type.type_name())?;

        match self.field_type {
            FieldType::Str(ref options) => s.serialize_field("options", options)?,
            FieldType::U32(ref options) => s.serialize_field("options", options)?
        }
        
        s.end()
    }
}

impl Decodable for FieldEntry {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        d.read_struct("field_entry", 3, |d| {
            let name = try!(d.read_struct_field("name", 0, |d| {
                d.read_str()
            }));
            let field_type: String = try!(d.read_struct_field("type", 1, |d| {
                d.read_str()
            }));
            d.read_struct_field("options", 2, |d| {
                match field_type.as_ref() {
                    "u32" => {
                        let u32_options = try!(U32Options::decode(d));
                        Ok(FieldEntry::new_u32(name, u32_options))
                    }
                    "text" => {
                        let text_options = try!(TextOptions::decode(d));
                        Ok(FieldEntry::new_text(name, text_options))
                    }
                    _ => {
                        Err(d.error(&format!("Field type {:?} unknown", field_type)))
                    }
                }
            })
        })
    }
}


#[cfg(test)]
mod tests {

    use super::*;
    use schema::TEXT;
    use rustc_serialize::json;
    
    #[test]
    fn test_json_serialization() {
        let field_value = FieldEntry::new_text(String::from("title"), TEXT);
        assert_eq!(format!("{}", json::as_pretty_json(&field_value)), r#"{
  "name": "title",
  "type": "text",
  "options": {
    "indexing": "position",
    "stored": false
  }
}"#);
    }
}
