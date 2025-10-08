use std::collections::BTreeMap;
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use super::ipld::{Block, Cid, DefaultParams, Ipld, IpldCodec};
use super::object::{Object, ObjectError};
use super::schema::{Schema, SchemaError};
use super::{DEFAULT_HASH_CODE, DEFAULT_IPLD_CODEC};

// TODO: not allowing node links to exist as nodes is a bit of a hack
//  and creates issues down the line writing logic around nodes vs node links

// Reserved object key for detailing what links
//  within have visible metatdata attached to them
// NOTE: i'd like to name this to .object, but this makes us compatible with
//  prior versions of the data format
const NODE_OBJECT_KEY: &str = ".metadata";
const NODE_SCHEMA_KEY: &str = ".schema";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeLink {
    Data(Cid, Option<Object>),
    Node(Cid),
}

impl NodeLink {
    pub fn cid(&self) -> &Cid {
        match self {
            NodeLink::Data(cid, _) | NodeLink::Node(cid) => cid,
        }
    }

    pub fn is_data(&self) -> bool {
        matches!(self, NodeLink::Data(_, _))
    }
}

impl From<NodeLink> for Ipld {
    fn from(link: NodeLink) -> Self {
        Ipld::Link(*link.cid())
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    /// Links to other nodes/data
    links: BTreeMap<String, NodeLink>,
    /// Object defs for data in this directory
    schema: Option<Schema>,
}

// TODO: might be nice to do some validation that data links point to
//  codecs that specify the correct cid type
impl From<Node> for Ipld {
    fn from(node: Node) -> Self {
        let mut map = BTreeMap::new();
        let mut objects = BTreeMap::new();

        // Add all links directly to the root map, and include objects if present
        for (name, link) in node.links {
            map.insert(name.clone(), link.clone().into());
            if let NodeLink::Data(_, Some(object)) = link {
                objects.insert(name, object.clone().into());
            }
        }

        // Add schema under .schema if present
        if let Some(schema) = node.schema {
            map.insert(NODE_SCHEMA_KEY.to_string(), schema.into());
        }

        // Add objects under .obj
        map.insert(NODE_OBJECT_KEY.to_string(), Ipld::Map(objects));

        Ipld::Map(map)
    }
}

impl TryFrom<Ipld> for Node {
    type Error = NodeError;

    fn try_from(ipld: Ipld) -> Result<Self, Self::Error> {
        let mut map = match ipld {
            Ipld::Map(m) => m,
            _ => return Err(NodeError::NotAMap("ipld".to_string())),
        };

        let mut links = BTreeMap::new();
        let mut objects = BTreeMap::new();
        let mut schema = None;

        // process the .obj key
        if let Some(object_map) = map.remove(NODE_OBJECT_KEY) {
            if let Ipld::Map(object_map) = object_map {
                for (name, obj_ipld) in object_map {
                    let object = Object::try_from(obj_ipld)?;
                    objects.insert(name, object);
                }
            } else {
                return Err(NodeError::NotAMap(NODE_OBJECT_KEY.to_string()));
            }
        }

        // process the .schema key
        if let Some(schema_ipld) = map.remove(NODE_SCHEMA_KEY) {
            schema = Some(Schema::try_from(schema_ipld)?);
        }

        // Process each entry in the map
        for (key, value) in map {
            if let Ipld::Link(cid) = value {
                // objects are just privileged data links
                match objects.remove(&key) {
                    // TODO: should probably sanity check that the codec is raw
                    Some(object) => links.insert(key, NodeLink::Data(cid, Some(object.clone()))),
                    // match on what codec is used
                    None => match IpldCodec::try_from(cid.codec()).unwrap() {
                        // this is just data without an object
                        IpldCodec::Raw | IpldCodec::DagPb => {
                            links.insert(key, NodeLink::Data(cid, None))
                        }

                        _ => links.insert(key, NodeLink::Node(cid)),
                    },
                };
            }
            // just skip non-link entries
        }

        // NOTE: objects won't be included in the node if the link is deleted
        //  we can maybe see this as just a special case which also implicitly
        //  deletes the object if the link is destroyed
        // I think that's fine for now

        Ok(Self { links, schema })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("block encoding failed")]
    BlockEncoding,
    #[error("file is not a map")]
    NotAMap(String),
    #[error("invalid object")]
    Object(#[from] ObjectError),
    #[error("invalid schema")]
    Schema(#[from] SchemaError),
    #[error("link uses reserved name: {0}")]
    ReservedName(String),
    #[error("link not found")]
    LinkNotFound(String),
}

impl Node {
    // Schema methods
    pub fn schema(&self) -> Option<&Schema> {
        self.schema.as_ref()
    }

    pub fn unset_schema(&mut self) {
        self.schema = None;
    }

    pub fn set_schema(&mut self, schema: Schema) {
        self.schema = Some(schema);
    }

    pub fn clear_schema(&mut self) {
        self.schema = None;
    }

    pub fn cid(&self) -> Cid {
        let ipld: Ipld = self.clone().into();
        let block = Block::<DefaultParams>::encode(DEFAULT_IPLD_CODEC, DEFAULT_HASH_CODE, &ipld)
            .map_err(|_| NodeError::BlockEncoding)
            .unwrap();
        *block.cid()
    }

    // put a link into the node
    pub fn put_link(&mut self, name: &str, cid: Cid) -> Result<(), NodeError> {
        // restrict reserved names -- don't allow anything named .schema or .obj
        if name == NODE_SCHEMA_KEY || name == NODE_OBJECT_KEY {
            return Err(NodeError::ReservedName(name.to_string()));
        }
        match IpldCodec::try_from(cid.codec()).unwrap() {
            IpldCodec::DagCbor => {
                self.links.insert(name.to_string(), NodeLink::Node(cid));
            }
            _ => {
                self.links
                    .insert(name.to_string(), NodeLink::Data(cid, None));
            }
        };
        Ok(())
    }

    pub fn get_link(&self, name: &str) -> Option<&NodeLink> {
        self.links.get(name)
    }

    pub fn get_links(&self) -> &BTreeMap<String, NodeLink> {
        &self.links
    }

    // Object/object methods
    pub fn put_object(&mut self, name: &str, object: &Object) -> Result<(), NodeError> {
        if name == NODE_SCHEMA_KEY || name == NODE_OBJECT_KEY {
            return Err(NodeError::ReservedName(name.to_string()));
        }
        let maybe_schema = self.schema();

        // get the link
        let object = object.clone();

        if let Some(NodeLink::Data(cid, _maybe_object)) = self.links.get(name) {
            // validate the object against the schema
            if let Some(schema) = maybe_schema {
                schema.validate(&object)?;
            }
            // and we'll overwrite the object in the link
            self.links
                .insert(name.to_string(), NodeLink::Data(*cid, Some(object)));
        } else {
            return Err(NodeError::LinkNotFound(name.to_string()));
        }

        Ok(())
    }

    pub fn rm_object(&mut self, name: &str) -> Result<(), NodeError> {
        let link = self.get_link(name);
        if let Some(NodeLink::Data(cid, _)) = link {
            self.links
                .insert(name.to_string(), NodeLink::Data(*cid, None));
            Ok(())
        } else {
            Err(NodeError::LinkNotFound(name.to_string()))
        }
    }

    pub fn del(&mut self, name: &str) -> Option<NodeLink> {
        // check if the link is an object
        self.links.remove(name)
    }

    pub fn size(&self) -> usize {
        self.links.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{SchemaProperty, SchemaType, RAW_IPLD_CODEC};

    fn test_cid() -> Cid {
        *Block::<DefaultParams>::encode(
            RAW_IPLD_CODEC,
            DEFAULT_HASH_CODE,
            &Ipld::Bytes("test".as_bytes().to_vec()),
        )
        .unwrap()
        .cid()
    }

    #[tokio::test]
    async fn test_schema_valid() {
        let mut node = Node::default();

        // Set up a schema
        let mut schema = Schema::default();
        schema.insert(
            "title".to_string(),
            SchemaProperty {
                property_type: SchemaType::String,
                description: None,
                required: true,
            },
        );

        node.set_schema(schema);

        // Test valid object
        let test_cid = test_cid();

        node.put_link("test.txt", test_cid).unwrap();
        let mut valid_object = Object::default();
        valid_object.insert("title".to_string(), Ipld::String("Test".to_string()));
        assert!(node.put_object("test.txt", &valid_object).is_ok());
    }

    #[tokio::test]
    async fn test_schema_invalid() {
        let mut node = Node::default();

        // Set up a schema
        let mut schema = Schema::default();
        schema.insert(
            "title".to_string(),
            SchemaProperty {
                property_type: SchemaType::String,
                description: None,
                required: true,
            },
        );

        node.set_schema(schema);

        // Test valid object
        let test_cid = test_cid();

        node.put_link("test.txt", test_cid).unwrap();
        let mut invalid_object = Object::default();
        invalid_object.insert("_title".to_string(), Ipld::String("Test".to_string()));
        assert!(node.put_object("test.txt", &invalid_object).is_err());
        let mut invalid_object = Object::default();
        invalid_object.insert("title".to_string(), Ipld::Integer(1));
        assert!(node.put_object("test.txt", &invalid_object).is_err());
    }

    #[tokio::test]
    async fn test_ipld_serialization() {
        let mut node = Node::default();
        // Add some test data
        let mut object = Object::default();
        let test_cid = test_cid();
        node.put_link("test.txt", test_cid).unwrap();

        object.insert("title".to_string(), Ipld::String("Test".to_string()));
        node.put_object("test.txt", &object).unwrap();

        // Convert to IPLD and back
        let ipld: Ipld = node.clone().into();
        let decoded = Node::try_from(ipld).unwrap();

        assert_eq!(node, decoded);
    }
}
