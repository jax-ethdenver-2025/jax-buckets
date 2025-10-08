use std::collections::BTreeMap;
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use super::Ipld;

pub const OBJECT_CREATED_AT_KEY: &str = "created_at";
pub const OBJECT_UPDATED_AT_KEY: &str = "updated_at";
// NOTE: we keep things nested under metadata to keep
//  things backwards compatible -- otherwise we end up changing
//  the cid when we serialize back to ipld
pub const LEGACY_PROPERTIES_KEY: &str = "metadata";

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Object {
    // if not set when deserializing, set to current time
    #[serde(default = "OffsetDateTime::now_utc")]
    created_at: OffsetDateTime,
    #[serde(default = "OffsetDateTime::now_utc")]
    updated_at: OffsetDateTime,
    properties: BTreeMap<String, Ipld>,
}

impl Default for Object {
    fn default() -> Self {
        Self {
            created_at: OffsetDateTime::now_utc(),
            updated_at: OffsetDateTime::now_utc(),
            properties: BTreeMap::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ObjectError {
    #[error("not a map")]
    NotAMap,
    #[error("missing field: {0}")]
    MissingField(String),
    #[error("invalid datetime: {0}")]
    InvalidDateTime(#[from] time::error::ComponentRange),
    #[error("schema validation failed: {0}")]
    SchemaValidation(#[from] super::schema::SchemaError),
    #[error("no schema available")]
    NoSchema,
}

impl Object {
    /// Create a new object, validating properties against the provided schema
    pub fn new(properties: Option<&BTreeMap<String, Ipld>>) -> Result<Self, ObjectError> {
        let properties = properties.cloned().unwrap_or_default();
        let now = OffsetDateTime::now_utc();
        let obj = Self {
            created_at: now,
            updated_at: now,
            properties,
        };

        Ok(obj)
    }

    pub fn set_created_at(&mut self, created_at: OffsetDateTime) {
        self.created_at = created_at;
    }

    pub fn created_at(&self) -> &OffsetDateTime {
        &self.created_at
    }

    pub fn updated_at(&self) -> &OffsetDateTime {
        &self.updated_at
    }

    pub fn properties(&self) -> &BTreeMap<String, Ipld> {
        &self.properties
    }

    pub fn insert(&mut self, key: String, value: Ipld) {
        self.properties.insert(key, value);
    }
}

// IPLD serialization implementations remain unchanged
impl From<Object> for Ipld {
    fn from(object: Object) -> Self {
        let mut map = BTreeMap::new();

        map.insert(
            OBJECT_CREATED_AT_KEY.to_string(),
            Ipld::Integer(object.created_at.unix_timestamp_nanos()),
        );
        map.insert(
            OBJECT_UPDATED_AT_KEY.to_string(),
            Ipld::Integer(object.updated_at.unix_timestamp_nanos()),
        );

        map.insert(
            LEGACY_PROPERTIES_KEY.to_string(),
            Ipld::Map(object.properties),
        );

        Ipld::Map(map)
    }
}

impl TryFrom<Ipld> for Object {
    type Error = ObjectError;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut map = match ipld {
            Ipld::Map(m) => m,
            _ => return Err(ObjectError::NotAMap),
        };

        let created_at = match map.remove(OBJECT_CREATED_AT_KEY) {
            Some(Ipld::Integer(ts)) => OffsetDateTime::from_unix_timestamp_nanos(ts)?,
            _ => return Err(ObjectError::MissingField(OBJECT_CREATED_AT_KEY.to_string())),
        };

        let updated_at = match map.remove(OBJECT_UPDATED_AT_KEY) {
            Some(Ipld::Integer(ts)) => OffsetDateTime::from_unix_timestamp_nanos(ts)?,
            _ => return Err(ObjectError::MissingField(OBJECT_UPDATED_AT_KEY.to_string())),
        };

        // properties is just everything under the legacy properties key, or
        //  a new map of properties
        let mut properties = BTreeMap::new();
        if let Some(Ipld::Map(metadata)) = map.remove(LEGACY_PROPERTIES_KEY) {
            properties = metadata;
        }

        Ok(Self {
            created_at,
            updated_at,
            properties,
        })
    }
}

impl From<BTreeMap<String, Ipld>> for Object {
    fn from(properties: BTreeMap<String, Ipld>) -> Self {
        let now = OffsetDateTime::now_utc();
        Self {
            created_at: now,
            updated_at: now,
            properties,
        }
    }
}
