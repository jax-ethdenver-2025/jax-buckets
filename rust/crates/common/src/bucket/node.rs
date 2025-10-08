#![allow(clippy::doc_lazy_continuation)]

use std::collections::BTreeMap;
use std::path::Path;

use mime::Mime;
use serde::{Deserialize, Serialize};

use crate::crypto::Secret;
use crate::linked_data::{BlockEncoded, DagCborCodec, Link, LinkedData};

use super::maybe_mime::MaybeMime;

/**
 * Nodes
 * =====
 * Nodes are the building blocks of a bucket's file structure.
 *  (Maybe bucket is not a good name for this project, since Nodes are
 *   *NOT* just key / value pairs, but a full DAG structure)
 *  At a high level, a node is just a description of links to
 *   to other nodes, which fall into two categories:
 *  - Data Links: links to terminal nodes in the DAG i.e. actual files
 *  - Dir Links: links to other nodes in the DAG i.e. directories
 * Nodes are always DAG-CBOR encoded, and may be encrypted
 */

// Describes links to terminal nodes in the DAG i.e. actual
//  files
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Data {
    // NOTE (amiller68): this is its own type st we can implement
    //  sed / de for the option cleanly
    // Data Links may have a MIME type associated with them,
    //  if it can be determined
    mime: MaybeMime,
    // Data Links may have metadata built for them, which are parsed
    //  from the links data at inclusion time
    metadata: Option<BTreeMap<String, LinkedData>>,
}

impl Default for Data {
    fn default() -> Self {
        Self::new()
    }
}

impl Data {
    /// Create a new Data with no metadata
    pub fn new() -> Self {
        Self {
            mime: MaybeMime(None),
            metadata: None,
        }
    }

    /// Create a Data with mime type detected from file path
    pub fn from_path(path: &Path) -> Self {
        let mime = Self::detect_mime_from_path(path);
        let metadata = BTreeMap::new();

        Self {
            mime: MaybeMime(mime),
            metadata: if metadata.is_empty() {
                None
            } else {
                Some(metadata)
            },
        }
    }

    /// Detect MIME type from file extension
    fn detect_mime_from_path(path: &Path) -> Option<Mime> {
        let extension = path.extension()?.to_str()?.to_lowercase();

        // Common file extensions to MIME types
        let mime_str = match extension.as_str() {
            // Text files
            "txt" => "text/plain",
            "html" | "htm" => "text/html",
            "css" => "text/css",
            "js" | "mjs" => "application/javascript",
            "json" => "application/json",
            "xml" => "application/xml",
            "md" | "markdown" => "text/markdown",
            "csv" => "text/csv",
            "yaml" | "yml" => "application/x-yaml",
            "toml" => "application/toml",

            // Images
            "jpg" | "jpeg" => "image/jpeg",
            "png" => "image/png",
            "gif" => "image/gif",
            "bmp" => "image/bmp",
            "svg" => "image/svg+xml",
            "webp" => "image/webp",
            "ico" => "image/x-icon",

            // Videos
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "avi" => "video/x-msvideo",
            "mkv" => "video/x-matroska",
            "mov" => "video/quicktime",

            // Audio
            "mp3" => "audio/mpeg",
            "wav" => "audio/wav",
            "ogg" => "audio/ogg",
            "flac" => "audio/flac",
            "aac" => "audio/aac",

            // Archives
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" | "gzip" => "application/gzip",
            "7z" => "application/x-7z-compressed",
            "rar" => "application/x-rar-compressed",

            // Documents
            "pdf" => "application/pdf",
            "doc" => "application/msword",
            "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            "xls" => "application/vnd.ms-excel",
            "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            "ppt" => "application/vnd.ms-powerpoint",
            "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",

            // Programming
            "rs" => "text/rust",
            "py" => "text/x-python",
            "c" => "text/x-c",
            "cpp" | "cc" | "cxx" => "text/x-c++",
            "h" | "hpp" => "text/x-c-header",
            "java" => "text/x-java",
            "go" => "text/x-go",
            "sh" => "application/x-sh",

            // Fonts
            "ttf" => "font/ttf",
            "otf" => "font/otf",
            "woff" => "font/woff",
            "woff2" => "font/woff2",

            _ => "application/octet-stream",
        };

        mime_str.parse().ok()
    }

    /// Set custom metadata
    pub fn set_metadata(&mut self, key: String, value: LinkedData) {
        if let Some(ref mut metadata) = self.metadata {
            metadata.insert(key, value);
        } else {
            let mut metadata = BTreeMap::new();
            metadata.insert(key, value);
            self.metadata = Some(metadata);
        }
    }

    /// Get the MIME type if present
    pub fn mime(&self) -> Option<&Mime> {
        self.mime.0.as_ref()
    }

    /// Get the metadata if present
    pub fn metadata(&self) -> Option<&BTreeMap<String, LinkedData>> {
        self.metadata.as_ref()
    }
}

// Lastly, we have a node, which is either a data link,
//  or a link to another node.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeLink {
    Data(Link, Secret, Data),
    Dir(Link, Secret),
}

impl NodeLink {
    /// Create a new Data node link with automatic metadata detection from path
    pub fn new_data_from_path(link: Link, secret: Secret, path: &Path) -> Self {
        let data = Data::from_path(path);
        NodeLink::Data(link, secret, data)
    }

    /// Create a new Data node link without metadata
    pub fn new_data(link: Link, secret: Secret) -> Self {
        NodeLink::Data(link, secret, Data::new())
    }

    /// Create a new Dir node link
    pub fn new_dir(link: Link, secret: Secret) -> Self {
        NodeLink::Dir(link, secret)
    }

    pub fn link(&self) -> &Link {
        match self {
            NodeLink::Data(link, _, _) => link,
            NodeLink::Dir(link, _) => link,
        }
    }

    pub fn secret(&self) -> &Secret {
        match self {
            NodeLink::Data(_, secret, _) => secret,
            NodeLink::Dir(_, secret) => secret,
        }
    }

    /// Get data info if this is a Data link
    pub fn data(&self) -> Option<&Data> {
        match self {
            NodeLink::Data(_, _, data) => Some(data),
            NodeLink::Dir(_, _) => None,
        }
    }

    /// Check if this is a directory link
    pub fn is_dir(&self) -> bool {
        matches!(self, NodeLink::Dir(_, _))
    }

    /// Check if this is a data/file link
    pub fn is_data(&self) -> bool {
        matches!(self, NodeLink::Data(_, _, _))
    }
}

// And a node is just a map of names to links.
//  When traversing the DAG, path names are just
//  /-joined names of links in nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Node {
    links: BTreeMap<String, NodeLink>,
}

#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("link not found")]
    LinkNotFound(String),
}

impl BlockEncoded<DagCborCodec> for Node {}

impl Node {
    pub fn new() -> Self {
        Node {
            links: BTreeMap::new(),
        }
    }

    pub fn get_link(&self, name: &str) -> Option<&NodeLink> {
        self.links.get(name)
    }

    pub fn insert(&mut self, name: String, link: NodeLink) -> Option<NodeLink> {
        self.links.insert(name, link)
    }

    pub fn get_links(&self) -> &BTreeMap<String, NodeLink> {
        &self.links
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
mod test {
    use super::*;

    #[test]
    fn test_node_encode_decode() {
        let mut node = Node::default();
        node.links.insert(
            "example".to_string(),
            NodeLink::Data(
                Link::default(),
                Secret::default(),
                Data {
                    metadata: None,
                    mime: MaybeMime(None),
                },
            ),
        );

        let encoded = node.encode().unwrap();
        let decoded = Node::decode(&encoded).unwrap();

        assert_eq!(node, decoded);
    }

    #[test]
    fn test_data_from_path() {
        use std::path::PathBuf;

        // Test with .json file
        let path = PathBuf::from("/test/file.json");
        let data = Data::from_path(&path);
        assert_eq!(data.mime().map(|m| m.as_ref()), Some("application/json"));

        // Test with .rs file
        let path = PathBuf::from("/src/main.rs");
        let data = Data::from_path(&path);
        assert_eq!(data.mime().map(|m| m.as_ref()), Some("text/rust"));

        // Test with unknown extension
        let path = PathBuf::from("/test/file.xyz");
        let data = Data::from_path(&path);
        assert_eq!(
            data.mime().map(|m| m.as_ref()),
            Some("application/octet-stream")
        );

        // Test with no extension
        let path = PathBuf::from("/test/README");
        let data = Data::from_path(&path);
        assert_eq!(data.mime(), None);
    }

    #[test]
    fn test_node_link_constructors() {
        use std::path::PathBuf;

        let link = Link::default();
        let secret = Secret::default();
        let path = PathBuf::from("/test/image.png");

        // Test new_data_from_path
        let node_link = NodeLink::new_data_from_path(link.clone(), secret.clone(), &path);
        assert!(node_link.is_data());
        assert!(!node_link.is_dir());
        let data = node_link.data().unwrap();
        assert_eq!(data.mime().map(|m| m.as_ref()), Some("image/png"));

        // Test new_data without path
        let node_link = NodeLink::new_data(link.clone(), secret.clone());
        assert!(node_link.is_data());
        let data = node_link.data().unwrap();
        assert_eq!(data.mime(), None);
        assert_eq!(data.metadata(), None);

        // Test new_dir
        let node_link = NodeLink::new_dir(link.clone(), secret.clone());
        assert!(node_link.is_dir());
        assert!(!node_link.is_data());
        assert!(node_link.data().is_none());
    }
}
