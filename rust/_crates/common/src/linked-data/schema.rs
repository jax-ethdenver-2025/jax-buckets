use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;

use super::object::Object;
use super::Ipld;

/// Represents the type of a schema property
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchemaType {
    String,
    Integer,
    Float,
    Boolean,
    Link,
    List,
    Map,
    Null,
}

impl TryFrom<&Ipld> for SchemaType {
    type Error = SchemaError;

    fn try_from(ipld: &Ipld) -> Result<Self, Self::Error> {
        match ipld {
            Ipld::String(_) => Ok(SchemaType::String),
            Ipld::Integer(_) => Ok(SchemaType::Integer),
            Ipld::Float(_) => Ok(SchemaType::Float),
            Ipld::Bool(_) => Ok(SchemaType::Boolean),
            Ipld::Link(_) => Ok(SchemaType::Link),
            Ipld::List(_) => Ok(SchemaType::List),
            Ipld::Map(_) => Ok(SchemaType::Map),
            Ipld::Null => Ok(SchemaType::Null),
            _ => Err(SchemaError::UnsupportedType),
        }
    }
}

/// Defines a single property in a schema
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemaProperty {
    // TODO: nothing should be pub
    /// The type of the property
    #[serde(rename = "type")]
    pub property_type: SchemaType,
    /// Optional description of the property
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this property is required
    #[serde(default)]
    pub required: bool,
}

impl From<SchemaProperty> for Ipld {
    fn from(prop: SchemaProperty) -> Self {
        let mut map = BTreeMap::new();
        map.insert(
            "type".to_string(),
            Ipld::String(format!("{:?}", prop.property_type).to_lowercase()),
        );
        if let Some(desc) = prop.description {
            map.insert("description".to_string(), Ipld::String(desc));
        }
        map.insert("required".to_string(), Ipld::Bool(prop.required));
        Ipld::Map(map)
    }
}

impl TryFrom<Ipld> for SchemaProperty {
    type Error = SchemaError;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        match ipld {
            Ipld::Map(mut map) => {
                let type_str = match map.remove("type") {
                    Some(Ipld::String(s)) => s,
                    _ => return Err(SchemaError::MissingField("type".to_string())),
                };

                let property_type = match type_str.as_str() {
                    "string" => SchemaType::String,
                    "integer" => SchemaType::Integer,
                    "float" => SchemaType::Float,
                    "boolean" => SchemaType::Boolean,
                    "link" => SchemaType::Link,
                    "list" => SchemaType::List,
                    "map" => SchemaType::Map,
                    "null" => SchemaType::Null,
                    _ => return Err(SchemaError::InvalidType(type_str)),
                };

                let description = match map.remove("description") {
                    Some(Ipld::String(s)) => Some(s),
                    None => None,
                    _ => return Err(SchemaError::InvalidField("description".to_string())),
                };

                let required = match map.remove("required") {
                    Some(Ipld::Bool(b)) => b,
                    None => false,
                    _ => return Err(SchemaError::InvalidField("required".to_string())),
                };

                Ok(SchemaProperty {
                    property_type,
                    description,
                    required,
                })
            }
            _ => Err(SchemaError::NotAMap),
        }
    }
}

/// Represents a complete schema for object metadata
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Schema(BTreeMap<String, SchemaProperty>);

impl Schema {
    pub fn validate(&self, object: &Object) -> Result<(), SchemaError> {
        let metadata = object.properties();

        // Check for required properties
        for (prop_name, prop) in &self.0 {
            if prop.required && !metadata.contains_key(prop_name) {
                return Err(SchemaError::MissingRequiredField(prop_name.clone()));
            }
        }

        // Validate types of present properties
        for (key, value) in metadata {
            if let Some(prop) = self.0.get(key) {
                let value_type = SchemaType::try_from(value)?;
                if value_type != prop.property_type {
                    return Err(SchemaError::TypeMismatch {
                        field: key.clone(),
                        expected: prop.property_type.clone(),
                        found: value_type,
                    });
                }
            }
        }

        Ok(())
    }
}

// Implement Deref and DerefMut to keep the BTreeMap interface
impl std::ops::Deref for Schema {
    type Target = BTreeMap<String, SchemaProperty>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Schema {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// Update the From/TryFrom implementations
impl From<Schema> for Ipld {
    fn from(schema: Schema) -> Self {
        let props_map: BTreeMap<String, Ipld> =
            schema.0.into_iter().map(|(k, v)| (k, v.into())).collect();
        Ipld::Map(props_map)
    }
}

impl TryFrom<Ipld> for Schema {
    type Error = SchemaError;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        match ipld {
            Ipld::Map(props_map) => {
                let mut properties = BTreeMap::new();
                for (key, value) in props_map {
                    properties.insert(key, SchemaProperty::try_from(value)?);
                }
                Ok(Schema(properties))
            }
            _ => Err(SchemaError::NotAMap),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    #[error("not a map")]
    NotAMap,
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("invalid field: {0}")]
    InvalidField(String),
    #[error("invalid type: {0}")]
    InvalidType(String),
    #[error("unsupported type")]
    UnsupportedType,
    #[error("invalid ignore pattern")]
    InvalidIgnorePattern,
    #[error("missing required field in metadata: {0}")]
    MissingRequiredField(String),
    #[error("type mismatch for field {field}: expected {expected:?}, found {found:?}")]
    TypeMismatch {
        field: String,
        expected: SchemaType,
        found: SchemaType,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_serialization() {
        let mut schema = Schema::default();
        let title_prop = SchemaProperty {
            property_type: SchemaType::String,
            description: Some("The title of the item".to_string()),
            required: true,
        };
        schema.insert("title".to_string(), title_prop);

        // Convert to IPLD and back
        let ipld: Ipld = schema.clone().into();
        let decoded = Schema::try_from(ipld).unwrap();
        assert_eq!(schema, decoded);
    }

    #[test]
    fn test_schema_validation() {
        let mut schema = Schema::default();
        schema.insert(
            "title".to_string(),
            SchemaProperty {
                property_type: SchemaType::String,
                description: None,
                required: true,
            },
        );

        // Valid metadata
        let mut valid_metadata = BTreeMap::new();
        valid_metadata.insert("title".to_string(), Ipld::String("Test".to_string()));
        let obj = Object::new(Some(&valid_metadata)).unwrap();
        assert!(schema.validate(&obj).is_ok());

        // Missing required field
        let invalid_metadata = BTreeMap::new();
        let invalid_object = Object::new(Some(&invalid_metadata)).unwrap();
        assert!(matches!(
            schema.validate(&invalid_object),
            Err(SchemaError::MissingRequiredField(_))
        ));

        // Wrong type
        let mut wrong_type_metadata = BTreeMap::new();
        wrong_type_metadata.insert("title".to_string(), Ipld::Integer(42));
        let wrong_type_object = Object::new(Some(&wrong_type_metadata)).unwrap();
        assert!(matches!(
            schema.validate(&wrong_type_object),
            Err(SchemaError::TypeMismatch { .. })
        ));
    }
}
