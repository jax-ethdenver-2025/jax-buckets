use leaky_common::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use time::OffsetDateTime;

/// A human-friendly version of Object for editing in markdown files
#[derive(Debug, Serialize, Deserialize)]
pub struct EditableObject {
    #[serde(with = "time::serde::rfc3339")]
    created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    updated_at: OffsetDateTime,
    properties: BTreeMap<String, Ipld>,
}

impl From<Object> for EditableObject {
    fn from(obj: Object) -> Self {
        Self {
            created_at: *obj.created_at(),
            updated_at: *obj.updated_at(),
            properties: obj.properties().clone(),
        }
    }
}

impl From<EditableObject> for Object {
    fn from(obj: EditableObject) -> Self {
        let mut object = Object::new(Some(&obj.properties)).unwrap();
        object.set_created_at(obj.created_at);
        object
    }
}
