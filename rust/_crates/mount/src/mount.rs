use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::io::Read;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::ipfs_rpc::{IpfsRpc, IpfsRpcError};
use crate::types::NodeLink;
use crate::types::Schema;
use crate::types::{ipld_to_cid, NodeError, Object};
use crate::types::{Cid, Ipld, Manifest, Node};

// NOTE: this is really just used as a node cache, but right now it has some
//  mixed responsibilities
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct BlockCache(pub HashMap<String, Ipld>);

impl Deref for BlockCache {
    type Target = HashMap<String, Ipld>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BlockCache {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

// NOTE: the mount api requires absolute paths, but we transalte those into
//  relative paths to make iteration slightly easier
// Kinda janky but it works for now
pub fn clean_path(path: &Path) -> PathBuf {
    if !path.is_absolute() {
        panic!("path is not absolute");
    }

    path.iter()
        .skip(1)
        .map(|part| part.to_string_lossy().to_string())
        .collect::<PathBuf>()
}

// TODO: ipfs rpc and block cache should not be apart of the mount struct
//  they are less state than injectable dependencies
#[derive(Clone)]
pub struct Mount {
    cid: Cid,
    manifest: Arc<Mutex<Manifest>>,
    block_cache: Arc<Mutex<BlockCache>>,
    ipfs_rpc: IpfsRpc,
}

impl Mount {
    // getters

    pub fn cid(&self) -> &Cid {
        &self.cid
    }

    pub fn previous_cid(&self) -> Cid {
        *self.manifest.lock().previous()
    }

    pub fn manifest(&self) -> Manifest {
        self.manifest.lock().clone()
    }

    pub fn block_cache(&self) -> BlockCache {
        self.block_cache.lock().clone()
    }

    // setters

    pub fn set_previous(&mut self, previous: Cid) {
        self.manifest.lock().set_previous(previous);
    }

    // mount sync

    /// Initialize a fresh mount against a given ipfs rpc
    pub async fn init(ipfs_rpc: &IpfsRpc) -> Result<Self, MountError> {
        let mut manifest = Manifest::default();
        let block_cache = Arc::new(Mutex::new(BlockCache::default()));
        let node = Node::default();
        let data_cid = Self::put_cache::<Node>(&node, &block_cache).await?;
        manifest.set_data(data_cid);
        let cid = Self::put::<Manifest>(&manifest, ipfs_rpc).await?;

        Ok(Self {
            cid,
            manifest: Arc::new(Mutex::new(manifest)),
            block_cache,
            ipfs_rpc: ipfs_rpc.clone(),
        })
    }

    /// Pull a mount from a given ipfs rpc using its root cid
    pub async fn pull(cid: Cid, ipfs_rpc: &IpfsRpc) -> Result<Self, MountError> {
        let manifest = Self::get::<Manifest>(&cid, ipfs_rpc).await?;
        let block_cache = Arc::new(Mutex::new(BlockCache::default()));

        Self::pull_nodes(manifest.data(), &block_cache, Some(ipfs_rpc)).await?;

        Ok(Self {
            cid,
            manifest: Arc::new(Mutex::new(manifest)),
            block_cache,
            ipfs_rpc: ipfs_rpc.clone(),
        })
    }

    /// update an existing mount against an updated ipfs rpc
    pub async fn update(&mut self, cid: Cid) -> Result<(), MountError> {
        let manifest = Self::get::<Manifest>(&cid, &self.ipfs_rpc).await?;
        // purge the block cache
        let block_cache = Arc::new(Mutex::new(BlockCache::default()));
        // pull the nodes
        Self::pull_nodes(manifest.data(), &block_cache, Some(&self.ipfs_rpc)).await?;
        // update the manifest and block cache
        self.manifest = Arc::new(Mutex::new(manifest));
        self.cid = cid;
        self.block_cache = block_cache;
        Ok(())
    }

    /// push state against our ipfs rpc
    pub async fn push(&mut self) -> Result<(), MountError> {
        let ipfs_rpc = &self.ipfs_rpc;
        let block_cache_data = self.block_cache.lock().clone();
        // iterate through the block cache and push each block in the cache
        for (cid_str, ipld) in block_cache_data.iter() {
            let cid = Self::put::<Ipld>(ipld, ipfs_rpc).await?;
            assert_eq!(cid.to_string(), cid_str.to_string());
        }

        let manifest = self.manifest.lock().clone();
        self.cid = Self::put::<Manifest>(&manifest, ipfs_rpc).await?;

        Ok(())
    }

    // mount operations api

    /// add or upsert data at a given path within the mount.
    ///  Does and should not handle inserting object or schema
    ///  metadata into the mount.
    ///
    /// # Arguments
    ///
    /// * `path` - the path to add the data at
    /// * `(data, hash_only)` - the data to add and a flag to indicate if we should write
    ///     the data to ipfs or just hash it
    ///
    /// # Returns
    ///
    /// * `Ok(())` - if the data was added successfully
    /// * `Err(MountError)` - if the data could not be added
    pub async fn add<R>(&mut self, path: &Path, data: (R, bool)) -> Result<(), MountError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let ipfs_rpc = &self.ipfs_rpc;
        let maybe_object = match self.get_node_link_at_path(path).await {
            Ok(NodeLink::Data(_, object)) => object,
            Ok(NodeLink::Node(_)) => return Err(MountError::PathNotFile(path.to_path_buf())),
            Err(MountError::PathNotFound(_)) => None,
            Err(err) => return Err(err),
        };

        // get a cid link to insert regardles of if we are hashing or not
        let link = match data {
            (d, true) => Self::hash_data(d, ipfs_rpc).await?,
            (d, false) => Self::add_data(d, ipfs_rpc).await?,
        };
        // see if the link exists and persist metadata
        let link = NodeLink::Data(link, maybe_object);
        self.upsert_node_link_at_path(path, link).await?;

        Ok(())
    }

    /// remove data or node at a given path within the mount
    ///  Will remove objects and schemas at the given path
    ///  if removing a node
    ///
    /// # Arguments
    ///
    /// * `path` - the path to remove the data at
    ///
    /// # Returns
    ///
    /// * `Ok(())` - if the data was removed successfully
    /// * `Err(MountError)` - if the data could not be removed
    pub async fn rm(&mut self, path: &Path) -> Result<(), MountError> {
        let parent_path = path.parent().unwrap();
        let mut node = self.get_node_at_path(parent_path).await?;
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        match node.del(&file_name) {
            Some(_) => (),
            None => return Err(MountError::PathNotFound(path.to_path_buf())),
        }
        self.upsert_node_at_path(parent_path, node).await?;
        Ok(())
    }

    /// Get all node links and the schema at a given path
    ///
    /// # Arguments
    ///
    /// * `path` - the path to get the node links and schema at
    ///
    /// # Returns
    ///
    /// * `Ok((links, schema))` - if the node links and schema were found
    /// * `Err(MountError)` - if the node links and schema could not be found
    pub async fn ls(
        &self,
        path: &Path,
    ) -> Result<(BTreeMap<PathBuf, NodeLink>, Option<Schema>), MountError> {
        let mut items = BTreeMap::new();
        let node = match self.get_node_at_path(path).await {
            Ok(node) => node,
            // TODO: this is not super precise, but it works for now
            Err(MountError::BlockCacheMiss(_)) => {
                return Err(MountError::PathNotDir(path.to_path_buf()))
            }
            Err(err) => return Err(err),
        };
        for (name, link) in node.get_links() {
            items.insert(name.clone().into(), link.clone());
        }

        Ok((items, node.schema().cloned()))
    }

    /// Get all child nodes and schemas at a given path
    ///
    /// # Arguments
    ///
    /// * `path` - the path to get the child nodes and schemas at
    ///
    /// # Returns
    ///
    /// * `Ok((nodes, schemas))` - if the child nodes and schemas were found
    /// * `Err(MountError)` - if the child nodes and schemas could not be found
    pub async fn ls_deep(
        &self,
        path: &Path,
    ) -> Result<(BTreeMap<PathBuf, NodeLink>, BTreeMap<PathBuf, Schema>), MountError> {
        self.get_nodes_links_and_schemas_at_path(path).await
    }

    /// cat data at a given path within the mount
    ///  Does and should not handle getting object or schema
    ///  metadata from the mount
    ///
    /// # Arguments
    ///
    /// * `path` - the path to get the data at
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<u8>)` - if the data was retrieved successfully
    /// * `Err(MountError)` - if the data could not be retrieved
    pub async fn cat(&self, path: &Path) -> Result<Vec<u8>, MountError> {
        let ipfs_rpc = &self.ipfs_rpc;
        let link = self.get_node_link_at_path(path).await?;
        match link {
            NodeLink::Data(cid, _) => {
                let data = Self::cat_data(&cid, ipfs_rpc).await?;
                Ok(data)
            }
            NodeLink::Node(_) => Err(MountError::PathNotFile(path.to_path_buf())),
        }
    }

    /// add a schema at a given path within the mount
    ///  Does and should not handle inserting object or data into the mount
    ///
    /// # Arguments
    ///
    /// * `path` - the path to add the schema at
    /// * `schema` - the schema to add
    ///
    /// # Returns
    ///
    /// * `Ok(())` - if the schema was added successfully
    /// * `Err(MountError)` - if the schema could not be added
    pub async fn set_schema(&mut self, path: &Path, schema: Schema) -> Result<(), MountError> {
        let mut node = match self.get_node_at_path(path).await {
            Ok(node) => node,
            Err(MountError::PathNotFound(_)) => Node::default(),
            Err(err) => return Err(err),
        };
        node.set_schema(schema);
        self.upsert_node_at_path(path, node).await?;
        Ok(())
    }

    /// remove a schema at a given path within the mount
    ///  Does and should not handle removing object or data from the mount
    ///
    /// # Arguments
    ///
    /// * `path` - the path to remove the schema at
    ///
    /// # Returns
    ///
    /// * `Ok(())` - if the schema was removed successfully
    /// * `Err(MountError)` - if the schema could not be removed
    pub async fn unset_schema(&mut self, path: &Path) -> Result<(), MountError> {
        let mut node = self.get_node_at_path(path).await?;
        node.unset_schema();
        self.upsert_node_at_path(path, node).await?;
        Ok(())
    }

    pub async fn tag(&mut self, path: &Path, object: Object) -> Result<(), MountError> {
        let parent_path = path.parent().unwrap();
        let mut node = self.get_node_at_path(parent_path).await?;
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        node.put_object(&file_name, &object)?;
        self.upsert_node_at_path(parent_path, node).await?;
        Ok(())
    }

    pub async fn rm_tag(&mut self, path: &Path) -> Result<(), MountError> {
        let parent_path = path.parent().unwrap();
        let mut node = self.get_node_at_path(parent_path).await?;
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        node.rm_object(&file_name)?;
        self.upsert_node_at_path(parent_path, node).await?;
        Ok(())
    }

    /// Get a node at a given path
    ///  Nodes are returned if the path ends in a node.
    ///
    /// # Arguments
    ///
    /// * `path` - the path to get the node at
    ///
    /// # Returns
    ///
    /// * `Ok((node, node_link))` - if the node or node link was found
    /// * `Err(MountError)` - if the node or node link could not be found
    async fn get_node_at_path(&self, path: &Path) -> Result<Node, MountError> {
        let block_cache = &self.block_cache;
        let path = clean_path(path);

        // get our entry into the mount
        let data_node_cid = *self.manifest.lock().data();
        let node = Self::get_cache::<Node>(&data_node_cid, block_cache).await?;
        Self::_get_node_at_path(&node, &path, block_cache).await
    }

    /// Get all child node links and schemas at a given path
    ///
    /// # Arguments
    ///
    /// * `path` - the path to get the node links and schemas at
    ///
    /// # Returns
    ///
    /// * `Ok((links, schemas))` - if the links and schemas were found
    /// * `Err(MountError)` - if the links and schemas could not be found
    pub async fn get_nodes_links_and_schemas_at_path(
        &self,
        path: &Path,
    ) -> Result<(BTreeMap<PathBuf, NodeLink>, BTreeMap<PathBuf, Schema>), MountError> {
        let block_cache = &self.block_cache;
        // let path = clean_path(path);
        let node = self.get_node_at_path(path).await?;
        Self::_get_nodes_links_and_schemas_at_path(&node, &PathBuf::from("/"), block_cache).await
    }

    /// Traverse a node in order to the the target node at the given path
    ///
    /// # Arguments
    ///
    /// * `node` - the node to traverse
    /// * `path` - the path to traverse to
    /// * `block_cache` - the block cache to use
    ///
    /// # Returns
    ///
    /// * `Ok(node)` - if the node was found
    /// * `Err(MountError)` - if the node could not be found
    async fn _get_node_at_path(
        node: &Node,
        path: &Path,
        block_cache: &Arc<Mutex<BlockCache>>,
    ) -> Result<Node, MountError> {
        let mut current_node = node.clone();
        // keep track of our consumed path and remaining path
        let mut consumed_path = PathBuf::from("/");

        // iterate through the path and get the node at each step
        for part in path.iter() {
            consumed_path.push(part);
            let next = part.to_string_lossy().to_string();
            // get the next link
            let next_link = current_node
                .get_link(&next)
                .ok_or(MountError::PathNotFound(consumed_path.clone()))?;
            // get the next node from the cache
            current_node = match Self::get_cache::<Node>(next_link.cid(), block_cache).await {
                // this is just a node
                Ok(n) => n,
                Err(err) => match err {
                    // this was not a node
                    MountError::Ipld => {
                        return Err(MountError::PathNotNode(consumed_path.clone()));
                    }
                    // the path was not found
                    MountError::PathNotFound(_) => {
                        return Err(MountError::PathNotFound(consumed_path.clone()));
                    }
                    // otherwise
                    err => return Err(err),
                },
            };
        }
        // return the node
        Ok(current_node)
    }

    /// Get all child node links and schemas at a given path
    ///
    /// # Arguments
    ///
    /// * `node` - the node to traverse
    /// * `path` - the path to traverse to
    /// * `block_cache` - the block cache to use
    ///
    /// # Returns
    ///
    /// * `Ok((links, schemas))` - if the links and schemas were found
    /// * `Err(MountError)` - if the links and schemas could not be found
    #[async_recursion::async_recursion]
    async fn _get_nodes_links_and_schemas_at_path(
        node: &Node,
        consumed_path: &Path,
        block_cache: &Arc<Mutex<BlockCache>>,
    ) -> Result<(BTreeMap<PathBuf, NodeLink>, BTreeMap<PathBuf, Schema>), MountError> {
        let mut links = BTreeMap::new();
        let mut schemas = BTreeMap::new();
        // append our schema if it exists
        if let Some(schema) = node.schema() {
            schemas.insert(consumed_path.to_path_buf(), schema.clone());
        }
        // iterate over the links and recurse
        for (name, link) in node.get_links() {
            if let NodeLink::Node(cid) = link {
                let node = Self::get_cache::<Node>(cid, block_cache).await?;
                let (mut _links, mut _schemas) = Self::_get_nodes_links_and_schemas_at_path(
                    &node,
                    &consumed_path.join(name),
                    block_cache,
                )
                .await?;
                links.extend(_links);
                schemas.extend(_schemas);
            }
            // if the link is a data link, we need to add it to the links
            if let NodeLink::Data(_, _) = link {
                links.insert(consumed_path.join(name), link.clone());
            }
        }
        Ok((links, schemas))
    }

    /// Get a node link at a given path
    ///
    /// # Arguments
    ///
    /// * `path` - the path to get the node link at
    ///
    /// # Returns
    ///
    /// * `Ok((node_link, node_link_cid))` - if the node link was found
    /// * `Err(MountError)` - if the node link could not be found
    async fn get_node_link_at_path(&self, path: &Path) -> Result<NodeLink, MountError> {
        // split off the file name from the path
        // TODO: this is kinda hacky, not sure if this unwrap is safe
        let maybe_file_name = path
            .file_name()
            .ok_or_else(|| MountError::PathNotFound(path.to_path_buf()))?
            .to_string_lossy()
            .to_string();
        let parent_path = path
            .parent()
            .ok_or_else(|| MountError::PathNotFound(path.to_path_buf()))?;
        let parent_node = self.get_node_at_path(parent_path).await?;

        // Get the final link
        let link = parent_node
            .get_link(&maybe_file_name)
            .ok_or(MountError::PathNotFound(path.to_path_buf()))?;

        Ok(link.clone())
    }

    #[async_recursion::async_recursion]
    async fn pull_nodes(
        cid: &Cid,
        block_cache: &Arc<Mutex<BlockCache>>,
        ipfs_rpc: Option<&IpfsRpc>,
    ) -> Result<(), MountError> {
        let node = if let Some(ipfs_rpc) = ipfs_rpc {
            Self::get::<Node>(cid, ipfs_rpc).await?
        } else {
            Self::get_cache::<Node>(cid, block_cache).await?
        };
        block_cache
            .lock()
            .insert(cid.to_string(), node.clone().into());

        // Iterate over links using get_links()
        for (_, link) in node.get_links().iter() {
            if let NodeLink::Node(cid) = link {
                Self::pull_nodes(cid, block_cache, ipfs_rpc).await?;
            }
        }

        Ok(())
    }

    /// Upsert a node at a given path within the mount with a single traversal
    ///
    /// # Arguments
    ///
    /// * `path` - the path to upsert the node at
    /// * `node` - the node to upsert
    ///
    /// # Returns
    ///
    /// * `Ok(())` - if the node was upserted successfully
    /// * `Err(MountError)` - if the node could not be upserted
    pub async fn upsert_node_at_path(&mut self, path: &Path, node: Node) -> Result<(), MountError> {
        let block_cache = &self.block_cache;
        let path = clean_path(path);

        // Get our entry into the mount
        let data_node_cid = *self.manifest.lock().data();
        let mut current_node = Self::get_cache::<Node>(&data_node_cid, block_cache).await?;

        // Keep track of visited nodes and their paths
        let mut visited_nodes = Vec::new();
        let mut consumed_path = PathBuf::from("/");

        // Do a single traversal, collecting nodes
        for part in path.iter() {
            consumed_path.push(part);
            let next = part.to_string_lossy().to_string();

            // Save current node before moving to next
            visited_nodes.push((consumed_path.clone(), current_node.clone()));

            // Try to get next node or create new one
            // NOTE (amiller68): for now, by the default, treat this as if it was called right after
            //  mkdir -p. This means that if the path doesn't exist, we create it.
            current_node = match current_node.get_link(&next) {
                Some(NodeLink::Node(cid)) => Self::get_cache::<Node>(cid, block_cache).await?,
                _ => Node::default(),
            };
        }

        // Put our target node in the cache
        let mut current_cid = Self::put_cache::<Node>(&node, block_cache).await?;

        // Work backwards through visited nodes, updating links
        for (path, mut parent_node) in visited_nodes.into_iter().rev() {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            parent_node.put_link(&name, current_cid)?;
            current_cid = Self::put_cache::<Node>(&parent_node, block_cache).await?;
        }

        // Update manifest with root node
        self.manifest.lock().set_data(current_cid);
        let manifest = self.manifest.lock().clone();
        self.cid = Self::put::<Manifest>(&manifest, &self.ipfs_rpc).await?;

        Ok(())
    }

    pub async fn upsert_node_link_at_path(
        &mut self,
        path: &Path,
        node_link: NodeLink,
    ) -> Result<(), MountError> {
        let path = clean_path(path);
        let parent_path = path.parent().unwrap();
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let block_cache = &self.block_cache;

        // Get our entry into the mount
        let data_node_cid = *self.manifest.lock().data();
        let mut current_node = Self::get_cache::<Node>(&data_node_cid, block_cache).await?;

        // Keep track of visited nodes and their paths
        let mut visited_nodes = Vec::new();
        let mut consumed_path = PathBuf::from("");

        // Do a single traversal, collecting nodes
        for part in parent_path.iter() {
            consumed_path.push(part);
            let next = part.to_string_lossy().to_string();

            // Save current node before moving to next
            visited_nodes.push((consumed_path.clone(), current_node.clone()));

            // Try to get next node or create new one
            // NOTE (amiller68): for now, by the default, treat this as if it was called right after
            //  mkdir -p. This means that if the path doesn't exist, we create it.
            current_node = match current_node.get_link(&next) {
                Some(NodeLink::Node(cid)) => Self::get_cache::<Node>(cid, block_cache).await?,
                _ => Node::default(),
            };
        }

        // Put our target node in the cache
        current_node.put_link(&file_name, *node_link.cid())?;
        if let NodeLink::Data(_, Some(object)) = node_link {
            current_node.put_object(&file_name, &object)?;
        }
        let mut current_cid = Self::put_cache::<Node>(&current_node, block_cache).await?;
        // Work backwards through visited nodes, updating links
        for (path, mut parent_node) in visited_nodes.into_iter().rev() {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            parent_node.put_link(&name, current_cid)?;
            current_cid = Self::put_cache::<Node>(&parent_node, block_cache).await?;
        }

        // Update manifest with root node
        self.manifest.lock().set_data(current_cid);
        let manifest = self.manifest.lock().clone();
        self.cid = Self::put::<Manifest>(&manifest, &self.ipfs_rpc).await?;

        Ok(())
    }

    pub async fn hash_data<R>(data: R, ipfs_rpc: &IpfsRpc) -> Result<Cid, MountError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let cid = ipfs_rpc.hash_data(data).await?;
        Ok(cid)
    }

    pub async fn add_data<R>(data: R, ipfs_rpc: &IpfsRpc) -> Result<Cid, MountError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let cid = ipfs_rpc.add_data(data).await?;
        Ok(cid)
    }

    async fn cat_data(cid: &Cid, ipfs_rpc: &IpfsRpc) -> Result<Vec<u8>, MountError> {
        let data = ipfs_rpc.cat_data(cid).await?;
        Ok(data)
    }

    async fn get<B>(cid: &Cid, ipfs_rpc: &IpfsRpc) -> Result<B, MountError>
    where
        B: TryFrom<Ipld> + Send,
    {
        let ipld = ipfs_rpc.get_ipld(cid).await?;
        let object = B::try_from(ipld).map_err(|_| MountError::Ipld)?;
        Ok(object)
    }

    async fn put<B>(ipld: &B, ipfs_rpc: &IpfsRpc) -> Result<Cid, MountError>
    where
        B: Into<Ipld> + Clone,
    {
        let cid = ipfs_rpc.put_ipld(ipld.clone()).await?;
        Ok(cid)
    }

    async fn get_cache<B>(cid: &Cid, block_cache: &Arc<Mutex<BlockCache>>) -> Result<B, MountError>
    where
        B: TryFrom<Ipld> + Send,
    {
        let cid_str = cid.to_string();
        let ipld = block_cache
            .lock()
            .get(&cid_str)
            .cloned()
            .ok_or(MountError::BlockCacheMiss(*cid))?;
        let object = B::try_from(ipld).map_err(|_| MountError::Ipld)?;
        Ok(object)
    }

    async fn put_cache<B>(ipld: &B, block_cache: &Arc<Mutex<BlockCache>>) -> Result<Cid, MountError>
    where
        B: Into<Ipld> + Clone,
    {
        // convert our ipld able thing to a block
        //  in order to determine the cid
        let ipld: Ipld = ipld.clone().into();
        let cid = ipld_to_cid(ipld.clone());

        block_cache.lock().insert(cid.to_string(), ipld);
        Ok(cid)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MountError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("path not found: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a node: {0}")]
    PathNotNode(PathBuf),
    #[error("path is not a node link: {0}")]
    PathNotNodeLink(PathBuf),
    #[error("previous cid mismatch: {0} != {1}")]
    PreviousCidMismatch(Cid, Cid),
    #[error("block cache miss: {0}")]
    BlockCacheMiss(Cid),
    #[error("node error: {0}")]
    Node(#[from] NodeError),
    #[error("ipfs rpc error: {0}")]
    IpfsRpc(#[from] IpfsRpcError),
    #[error("could not convert Ipld to type")]
    Ipld,
    #[error("cid is not set")]
    NoCid,
    #[error("path is not directory: {0}")]
    PathNotDir(PathBuf),
    #[error("path is not file: {0}")]
    PathNotFile(PathBuf),
    #[error("block creation failed")]
    BlockCreation,
    #[error("block decoding failed")]
    BlockDecode,
    #[error("block encoding failed")]
    BlockEncoding,
}

#[cfg(test)]
mod test {
    use crate::types::{SchemaProperty, SchemaType};

    use super::*;

    async fn empty_mount() -> Mount {
        let ipfs_rpc = IpfsRpc::default();
        Mount::init(&ipfs_rpc).await.unwrap()
    }

    #[tokio::test]
    async fn add() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/foo"), (data, true))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn add_with_metadata() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        let mut object = Object::default();
        object.insert("foo".to_string(), Ipld::String("bar".to_string()));
        let mut schema = Schema::default();
        schema.insert(
            "foo".to_string(),
            SchemaProperty {
                property_type: SchemaType::String,
                description: Some("foo".to_string()),
                required: true,
            },
        );
        mount
            .add(&PathBuf::from("/foo"), (data, true))
            .await
            .unwrap();
        mount.set_schema(&PathBuf::from("/"), schema).await.unwrap();
        mount.tag(&PathBuf::from("/foo"), object).await.unwrap();
        let (links, schemas) = mount.ls_deep(&PathBuf::from("/")).await.unwrap();
        assert_eq!(links.len(), 1);
        assert!(links.get(&PathBuf::from("/foo")).is_some());
        assert_eq!(schemas.len(), 1);
        assert!(schemas.get(&PathBuf::from("/")).is_some());
    }

    #[tokio::test]
    async fn add_cat() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/bar"), (data, false))
            .await
            .unwrap();
        let get_data = mount.cat(&PathBuf::from("/bar")).await.unwrap();
        assert_eq!(data, get_data.as_slice());
    }

    #[tokio::test]
    async fn add_ls() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/bar"), (data, true))
            .await
            .unwrap();
        let (links, _) = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(links.len(), 1);
    }

    #[tokio::test]
    async fn add_deep() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/foo/bar/buzz"), (data, true))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn add_deep_ls_deep() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        let schema_path = PathBuf::from("/foo/bar");
        let schema = Schema::default();

        mount
            .add(&PathBuf::from("/foo/bar/buzz"), (data, true))
            .await
            .unwrap();
        mount.set_schema(&schema_path, schema).await.unwrap();
        let (links, schemas) = mount.ls_deep(&PathBuf::from("/")).await.unwrap();
        assert_eq!(links.len(), 1);
        assert!(links.get(&PathBuf::from("/foo/bar/buzz")).is_some());
        assert_eq!(schemas.len(), 1);
        assert!(schemas.get(&PathBuf::from("/foo/bar")).is_some());
    }

    #[tokio::test]
    async fn add_rm() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/foo/bar"), (data, true))
            .await
            .unwrap();
        mount.rm(&PathBuf::from("/foo/bar")).await.unwrap();
    }

    #[tokio::test]
    async fn add_pull_ls() {
        let mut mount = empty_mount().await;
        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/bar"), (data, true))
            .await
            .unwrap();
        let cid = *mount.cid();
        mount.push().await.unwrap();

        let mount = Mount::pull(cid, &IpfsRpc::default()).await.unwrap();
        let (links, _) = mount.ls(&PathBuf::from("/")).await.unwrap();
        assert_eq!(links.len(), 1);
    }

    #[tokio::test]
    async fn add_add_deep() {
        let mut mount = empty_mount().await;

        let data = "foo".as_bytes();
        mount
            .add(&PathBuf::from("/foo/bar"), (data, true))
            .await
            .unwrap();

        let data = "bang".as_bytes();
        mount
            .add(&PathBuf::from("/foo/bug"), (data, true))
            .await
            .unwrap();
    }
}
