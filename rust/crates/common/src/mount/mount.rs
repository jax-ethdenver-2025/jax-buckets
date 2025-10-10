use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use uuid::Uuid;

use crate::bucket::{BucketData, Node, NodeError, NodeLink};
use crate::crypto::{Secret, SecretError, SecretKey, Share};
use crate::linked_data::{BlockEncoded, CodecError, Link};
use crate::peer::{BlobsStore, BlobsStoreError};

pub fn clean_path(path: &Path) -> PathBuf {
    if !path.is_absolute() {
        panic!("path is not absolute");
    }
    path.iter()
        .skip(1)
        .map(|part| part.to_string_lossy().to_string())
        .collect::<PathBuf>()
}

#[derive(Clone)]
pub struct MountInner {
    pub link: Link,
    pub bucket_data: BucketData,
    pub root_node: Node,
}

impl MountInner {
    pub fn link(&self) -> &Link {
        &self.link
    }
    pub fn bucket_data(&self) -> &BucketData {
        &self.bucket_data
    }
}

#[derive(Clone)]
pub struct Bucket(Arc<Mutex<MountInner>>);

#[derive(Debug, thiserror::Error)]
pub enum BucketError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("link not found")]
    LinkNotFound(Link),
    #[error("path not found: {0}")]
    PathNotFound(PathBuf),
    #[error("path is not a node: {0}")]
    PathNotNode(PathBuf),
    #[error("blobs store error: {0}")]
    BlobsStore(#[from] BlobsStoreError),
    #[error("secret error: {0}")]
    Secret(#[from] SecretError),
    #[error("node error: {0}")]
    Node(#[from] NodeError),
    #[error("codec error: {0}")]
    Codec(#[from] CodecError),
    #[error("share error: {0}")]
    Share(#[from] crate::crypto::ShareError),
}

impl Bucket {
    pub fn inner(&self) -> MountInner {
        self.0.lock().clone()
    }

    pub fn link(&self) -> Link {
        let inner = self.0.lock();

        inner.link.clone()
    }

    /// Save the current mount state by updating the bucket in blobs
    /// Returns the new bucket link that should be stored in the database
    pub async fn save(
        &self,
        secret_key: &SecretKey,
        blobs: &BlobsStore,
    ) -> Result<Link, BucketError> {
        let mut inner = self.0.lock();

        // Create a new secret for the updated root
        let secret = Secret::generate();

        // Put the current root node into blobs with the new secret
        let new_root_link = Self::_put_node_in_blobs(&inner.root_node, &secret, blobs).await?;

        // Update the bucket's share with the new root link
        // (add_share creates the Share internally)
        let mut updated_bucket = inner.bucket_data.clone();
        updated_bucket.add_share(secret_key.public(), new_root_link.clone(), secret)?;

        // Put the updated bucket into blobs
        let new_bucket_link = Self::_put_bucket_in_blobs(&updated_bucket, blobs).await?;

        // Update our internal state
        inner.link = new_root_link;
        inner.bucket_data = updated_bucket;

        Ok(new_bucket_link)
    }

    pub async fn init(
        id: Uuid,
        name: String,
        owner: &SecretKey,
        blobs: &BlobsStore,
    ) -> Result<Self, BucketError> {
        // create a new root node for the bucket
        let node = Node::default();
        // create a new secret for the owner
        let secret = Secret::generate();
        // put the node in the blobs store for the secret
        let root = Self::_put_node_in_blobs(&node, &secret, blobs).await?;
        // share the secret with the owner
        let share = Share::new(&secret, &owner.public())?;
        // construct the new bucket
        let bucket_data = BucketData::init(id, name, owner.public(), share, root);
        // put the bucket in the blobs store for the secret
        let link = Self::_put_bucket_in_blobs(&bucket_data, blobs).await?;
        // return the new mount
        Ok(Bucket(Arc::new(Mutex::new(MountInner {
            link,
            bucket_data,
            root_node: node,
        }))))
    }

    pub async fn load(
        link: &Link,
        secret_key: &SecretKey,
        blobs: &BlobsStore,
    ) -> Result<Self, BucketError> {
        let public_key = &secret_key.public();

        let bucket = Self::_get_bucket_from_blobs(link, blobs).await?;

        let _bucket_share = bucket.get_share(public_key);

        if _bucket_share.is_none() {
            return Err(BucketError::LinkNotFound(link.clone()));
        }

        let bucket_share = _bucket_share.unwrap();
        let share = bucket_share.share();
        let secret = share.recover(secret_key)?;

        let link = bucket_share.root().clone();
        if link == Link::default() {
            // TODO (amiller68): be better
            panic!("default link found")
        } else {
            let root_node =
                Self::_get_node_from_blobs(&NodeLink::Dir(link.clone(), secret), blobs).await?;
            Ok(Bucket(Arc::new(Mutex::new(MountInner {
                link,
                bucket_data: bucket,
                root_node,
            }))))
        }
    }

    #[allow(clippy::await_holding_lock)]
    pub async fn add<R>(
        &mut self,
        path: &Path,
        data: R,
        blobs: &BlobsStore,
    ) -> Result<(), BucketError>
    where
        R: Read + Send + Sync + 'static + Unpin,
    {
        let secret = Secret::generate();

        let encrypted_reader = secret.encrypt_reader(data)?;

        // TODO (amiller68): this is incredibly dumb
        use bytes::Bytes;
        use futures::stream;
        let encrypted_bytes = {
            let mut buf = Vec::new();
            let mut reader = encrypted_reader;
            reader.read_to_end(&mut buf).map_err(SecretError::Io)?;
            buf
        };

        let stream = Box::pin(stream::once(async move {
            Ok::<_, std::io::Error>(Bytes::from(encrypted_bytes))
        }));

        let hash = blobs.put_stream(stream).await?;

        let link = Link::new(
            crate::linked_data::LD_RAW_CODEC,
            hash,
            iroh_blobs::BlobFormat::Raw,
        );

        let node_link = NodeLink::new_data_from_path(link, secret, path);

        let mut inner = self.0.lock();
        let root_node = inner.root_node.clone();
        let updated_link = Self::_set_node_link_at_path(root_node, node_link, path, blobs).await?;

        if let NodeLink::Dir(new_root_link, new_secret) = updated_link {
            inner.root_node = Self::_get_node_from_blobs(
                &NodeLink::Dir(new_root_link.clone(), new_secret),
                blobs,
            )
            .await?;
            inner.link = new_root_link;
        }

        Ok(())
    }

    #[allow(clippy::await_holding_lock)]
    pub async fn rm(&mut self, path: &Path, blobs: &BlobsStore) -> Result<(), BucketError> {
        let path = clean_path(path);
        let parent_path = path
            .parent()
            .ok_or_else(|| BucketError::Default(anyhow::anyhow!("Cannot remove root")))?;

        let inner = self.0.lock();
        let root_node = inner.root_node.clone();
        drop(inner);

        let mut parent_node = if parent_path == Path::new("") {
            root_node.clone()
        } else {
            Self::_get_node_at_path(&root_node, parent_path, blobs).await?
        };

        let file_name = path.file_name().unwrap().to_string_lossy().to_string();

        if parent_node.del(&file_name).is_none() {
            return Err(BucketError::PathNotFound(path.to_path_buf()));
        }

        if parent_path == Path::new("") {
            let secret = Secret::generate();
            let link = Self::_put_node_in_blobs(&parent_node, &secret, blobs).await?;

            let mut inner = self.0.lock();
            inner.root_node = parent_node;
            inner.link = link;
        } else {
            // Save the modified parent node to blobs
            let secret = Secret::generate();
            let parent_link = Self::_put_node_in_blobs(&parent_node, &secret, blobs).await?;
            let node_link = NodeLink::new_dir(parent_link, secret);

            let mut inner = self.0.lock();
            // Convert parent_path back to absolute for _set_node_link_at_path
            let abs_parent_path = Path::new("/").join(parent_path);
            let updated_link = Self::_set_node_link_at_path(
                inner.root_node.clone(),
                node_link,
                &abs_parent_path,
                blobs,
            )
            .await?;

            if let NodeLink::Dir(new_root_link, new_secret) = updated_link {
                inner.root_node = Self::_get_node_from_blobs(
                    &NodeLink::Dir(new_root_link.clone(), new_secret),
                    blobs,
                )
                .await?;
                inner.link = new_root_link;
            }
        }

        Ok(())
    }

    #[allow(clippy::await_holding_lock)]
    pub async fn ls(
        &self,
        path: &Path,
        blobs: &BlobsStore,
    ) -> Result<BTreeMap<PathBuf, NodeLink>, BucketError> {
        let mut items = BTreeMap::new();
        let path = clean_path(path);

        let inner = self.0.lock();
        let root_node = inner.root_node.clone();
        drop(inner);

        let node = if path == Path::new("") {
            root_node
        } else {
            match Self::_get_node_at_path(&root_node, &path, blobs).await {
                Ok(node) => node,
                Err(BucketError::LinkNotFound(_)) => {
                    return Err(BucketError::PathNotNode(path.to_path_buf()))
                }
                Err(err) => return Err(err),
            }
        };

        for (name, link) in node.get_links() {
            let mut full_path = path.clone();
            full_path.push(name);
            items.insert(full_path, link.clone());
        }

        Ok(items)
    }

    pub async fn ls_deep(
        &self,
        path: &Path,
        blobs: &BlobsStore,
    ) -> Result<BTreeMap<PathBuf, NodeLink>, BucketError> {
        let base_path = clean_path(path);
        self._ls_deep(path, &base_path, blobs).await
    }

    async fn _ls_deep(
        &self,
        path: &Path,
        base_path: &Path,
        blobs: &BlobsStore,
    ) -> Result<BTreeMap<PathBuf, NodeLink>, BucketError> {
        let mut all_items = BTreeMap::new();

        // get the initial items at the given path
        let items = self.ls(path, blobs).await?;

        for (item_path, link) in items {
            // Make path relative to the base_path
            let relative_path = if base_path == Path::new("") {
                item_path.clone()
            } else {
                item_path
                    .strip_prefix(base_path)
                    .unwrap_or(&item_path)
                    .to_path_buf()
            };
            all_items.insert(relative_path.clone(), link.clone());

            if link.is_dir() {
                // Recurse using the absolute path
                let abs_item_path = Path::new("/").join(&item_path);
                let sub_items = Box::pin(self._ls_deep(&abs_item_path, base_path, blobs)).await?;

                // Sub items already have correct relative paths from base_path
                for (sub_path, sub_link) in sub_items {
                    all_items.insert(sub_path, sub_link);
                }
            }
        }

        Ok(all_items)
    }

    #[allow(clippy::await_holding_lock)]
    pub async fn cat(&self, path: &Path, blobs: &BlobsStore) -> Result<Vec<u8>, BucketError> {
        let path = clean_path(path);

        let inner = self.0.lock();
        let root_node = inner.root_node.clone();
        drop(inner);

        let (parent_path, file_name) = if let Some(parent) = path.parent() {
            (
                parent,
                path.file_name().unwrap().to_string_lossy().to_string(),
            )
        } else {
            return Err(BucketError::PathNotFound(path.to_path_buf()));
        };

        let parent_node = if parent_path == Path::new("") {
            root_node
        } else {
            Self::_get_node_at_path(&root_node, parent_path, blobs).await?
        };

        let link = parent_node
            .get_link(&file_name)
            .ok_or_else(|| BucketError::PathNotFound(path.to_path_buf()))?;

        match link {
            NodeLink::Data(link, secret, _) => {
                let encrypted_data = blobs.get(link.hash()).await?;
                let data = secret.decrypt(&encrypted_data)?;
                Ok(data)
            }
            NodeLink::Dir(_, _) => Err(BucketError::PathNotNode(path.to_path_buf())),
        }
    }

    /// Get the NodeLink for a file at a given path
    #[allow(clippy::await_holding_lock)]
    pub async fn get(&self, path: &Path, blobs: &BlobsStore) -> Result<NodeLink, BucketError> {
        let path = clean_path(path);

        let inner = self.0.lock();
        let root_node = inner.root_node.clone();
        drop(inner);

        let (parent_path, file_name) = if let Some(parent) = path.parent() {
            (
                parent,
                path.file_name().unwrap().to_string_lossy().to_string(),
            )
        } else {
            return Err(BucketError::PathNotFound(path.to_path_buf()));
        };

        let parent_node = if parent_path == Path::new("") {
            root_node
        } else {
            Self::_get_node_at_path(&root_node, parent_path, blobs).await?
        };

        parent_node
            .get_link(&file_name)
            .cloned()
            .ok_or_else(|| BucketError::PathNotFound(path.to_path_buf()))
    }

    async fn _get_node_at_path(
        node: &Node,
        path: &Path,
        blobs: &BlobsStore,
    ) -> Result<Node, BucketError> {
        let mut current_node = node.clone();
        let mut consumed_path = PathBuf::from("/");

        for part in path.iter() {
            consumed_path.push(part);
            let next = part.to_string_lossy().to_string();
            let next_link = current_node
                .get_link(&next)
                .ok_or(BucketError::PathNotFound(consumed_path.clone()))?;
            current_node = Self::_get_node_from_blobs(next_link, blobs).await?
        }
        Ok(current_node)
    }

    pub async fn _set_node_link_at_path(
        node: Node,
        node_link: NodeLink,
        path: &Path,
        blobs: &BlobsStore,
    ) -> Result<NodeLink, BucketError> {
        let path = clean_path(path);
        let mut visited_nodes = Vec::new();
        let mut name = path.file_name().unwrap().to_string_lossy().to_string();
        let parent_path = path.parent().unwrap_or(Path::new(""));

        let mut consumed_path = PathBuf::from("/");
        let mut node = node;
        visited_nodes.push((consumed_path.clone(), node.clone()));

        for part in parent_path.iter() {
            let next = part.to_string_lossy().to_string();
            let next_link = node.get_link(&next);
            if let Some(next_link) = next_link {
                consumed_path.push(part);
                match next_link {
                    NodeLink::Dir(..) => {
                        node = Self::_get_node_from_blobs(next_link, blobs).await?
                    }
                    NodeLink::Data(..) => {
                        return Err(BucketError::PathNotNode(consumed_path.clone()));
                    }
                }
                visited_nodes.push((consumed_path.clone(), node.clone()));
            } else {
                // Create a new directory node
                node = Node::default();
                consumed_path.push(part);
                visited_nodes.push((consumed_path.clone(), node.clone()));
            }
        }

        let mut node_link = node_link;
        for (path, mut node) in visited_nodes.into_iter().rev() {
            node.insert(name, node_link.clone());
            let secret = Secret::generate();
            let link = Self::_put_node_in_blobs(&node, &secret, blobs).await?;
            node_link = NodeLink::Dir(link, secret);
            name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
        }

        Ok(node_link)
    }

    async fn _get_bucket_from_blobs(link: &Link, blobs: &BlobsStore) -> Result<BucketData, BucketError> {
        if !(blobs.stat(link.hash()).await?) {
            return Err(BucketError::LinkNotFound(link.clone()));
        };
        let data = blobs.get(link.hash()).await?;
        Ok(BucketData::decode(&data)?)
    }

    async fn _get_node_from_blobs(
        node_link: &NodeLink,
        blobs: &BlobsStore,
    ) -> Result<Node, BucketError> {
        let link = node_link.link();
        let secret = node_link.secret();
        if !(blobs.stat(link.hash()).await?) {
            return Err(BucketError::LinkNotFound(link.clone()));
        };
        let blob = blobs.get(link.hash()).await?;
        let data = secret.decrypt(&blob)?;

        Ok(Node::decode(&data)?)
    }

    // TODO (amiller68): you should inline a Link
    //  into the node when we store encrypt it,
    //  so that we have an integrity check
    async fn _put_node_in_blobs(
        node: &Node,
        secret: &Secret,
        blobs: &BlobsStore,
    ) -> Result<Link, BucketError> {
        let _data = node.encode()?;
        let data = secret.encrypt(&_data)?;
        let hash = blobs.put(data).await?;
        // NOTE (amiller68): nodes are always stored as raw
        //  since they are encrypted blobs
        let link = Link::new(
            crate::linked_data::LD_RAW_CODEC,
            hash,
            iroh_blobs::BlobFormat::Raw,
        );
        Ok(link)
    }

    pub async fn _put_bucket_in_blobs(
        bucket_data: &BucketData,
        blobs: &BlobsStore,
    ) -> Result<Link, BucketError> {
        let data = bucket_data.encode()?;
        let hash = blobs.put(data).await?;
        // NOTE (amiller68): buckets are unencrypted, so they can inherit
        //  the codec of the bucket itself (which is currently always cbor)
        let link = Link::new(bucket_data.codec(), hash, iroh_blobs::BlobFormat::Raw);
        Ok(link)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    async fn setup_test_env() -> (Bucket, BlobsStore, crate::crypto::SecretKey, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let blob_path = temp_dir.path().join("blobs");

        let secret_key = crate::crypto::SecretKey::generate();
        let iroh_secret_key = iroh::SecretKey::generate(rand_core::OsRng);
        let endpoint = iroh::Endpoint::builder()
            .secret_key(iroh_secret_key)
            .bind()
            .await
            .unwrap();
        let blobs = BlobsStore::load(&blob_path, endpoint).await.unwrap();
        let bucket = BucketData::new("test-bucket".to_string(), secret_key.public());

        let root_node = Node::default();
        let root_secret = Secret::generate();

        let root_data = root_node.encode().unwrap();
        let encrypted_root = root_secret.encrypt(&root_data).unwrap();
        let root_hash = blobs.put(encrypted_root).await.unwrap();
        let root_link = Link::new(root_node.codec(), root_hash, iroh_blobs::BlobFormat::Raw);

        let mut bucket_with_share = bucket.clone();
        bucket_with_share
            .add_share(secret_key.public(), root_link.clone(), root_secret.clone())
            .unwrap();

        let bucket_data = bucket_with_share.encode().unwrap();

        let bucket_hash = blobs.put(bucket_data).await.unwrap();

        let bucket_link = Link::new(
            bucket_with_share.codec(),
            bucket_hash,
            iroh_blobs::BlobFormat::Raw,
        );

        let mount = Bucket::load(&bucket_link, &secret_key, &blobs)
            .await
            .unwrap();

        (mount, blobs, secret_key, temp_dir)
    }

    #[tokio::test]
    async fn test_add_and_cat() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        let data = b"Hello, world!";
        let path = PathBuf::from("/test.txt");

        mount
            .add(&path, Cursor::new(data.to_vec()), &blobs)
            .await
            .unwrap();

        let result = mount.cat(&path, &blobs).await.unwrap();
        assert_eq!(result, data);
    }

    #[tokio::test]
    async fn test_add_with_metadata() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        let data = b"{ \"key\": \"value\" }";
        let path = PathBuf::from("/data.json");

        mount
            .add(&path, Cursor::new(data.to_vec()), &blobs)
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/"), &blobs).await.unwrap();
        assert_eq!(items.len(), 1);

        let (file_path, link) = items.iter().next().unwrap();
        assert_eq!(file_path, &PathBuf::from("data.json"));

        if let Some(data_info) = link.data() {
            assert!(data_info.mime().is_some());
            assert_eq!(data_info.mime().unwrap().as_ref(), "application/json");
        } else {
            panic!("Expected data link with metadata");
        }
    }

    #[tokio::test]
    async fn test_ls() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        mount
            .add(
                &PathBuf::from("/file1.txt"),
                Cursor::new(b"data1".to_vec()),
                &blobs,
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/file2.txt"),
                Cursor::new(b"data2".to_vec()),
                &blobs,
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir/file3.txt"),
                Cursor::new(b"data3".to_vec()),
                &blobs,
            )
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/"), &blobs).await.unwrap();
        assert_eq!(items.len(), 3);

        assert!(items.contains_key(&PathBuf::from("file1.txt")));
        assert!(items.contains_key(&PathBuf::from("file2.txt")));
        assert!(items.contains_key(&PathBuf::from("dir")));

        let sub_items = mount.ls(&PathBuf::from("/dir"), &blobs).await.unwrap();
        assert_eq!(sub_items.len(), 1);
        assert!(sub_items.contains_key(&PathBuf::from("dir/file3.txt")));
    }

    #[tokio::test]
    async fn test_ls_deep() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        mount
            .add(&PathBuf::from("/a.txt"), Cursor::new(b"a".to_vec()), &blobs)
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir1/b.txt"),
                Cursor::new(b"b".to_vec()),
                &blobs,
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir1/dir2/c.txt"),
                Cursor::new(b"c".to_vec()),
                &blobs,
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/dir1/dir2/dir3/d.txt"),
                Cursor::new(b"d".to_vec()),
                &blobs,
            )
            .await
            .unwrap();

        let all_items = mount.ls_deep(&PathBuf::from("/"), &blobs).await.unwrap();

        assert!(all_items.contains_key(&PathBuf::from("a.txt")));
        assert!(all_items.contains_key(&PathBuf::from("dir1")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/b.txt")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/c.txt")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/dir3")));
        assert!(all_items.contains_key(&PathBuf::from("dir1/dir2/dir3/d.txt")));
    }

    #[tokio::test]
    async fn test_rm() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        mount
            .add(
                &PathBuf::from("/file1.txt"),
                Cursor::new(b"data1".to_vec()),
                &blobs,
            )
            .await
            .unwrap();
        mount
            .add(
                &PathBuf::from("/file2.txt"),
                Cursor::new(b"data2".to_vec()),
                &blobs,
            )
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/"), &blobs).await.unwrap();
        assert_eq!(items.len(), 2);

        mount
            .rm(&PathBuf::from("/file1.txt"), &blobs)
            .await
            .unwrap();

        let items = mount.ls(&PathBuf::from("/"), &blobs).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items.contains_key(&PathBuf::from("file2.txt")));
        assert!(!items.contains_key(&PathBuf::from("file1.txt")));

        let result = mount.cat(&PathBuf::from("/file1.txt"), &blobs).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_nested_operations() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        let files = vec![
            ("/root.txt", b"root" as &[u8]),
            ("/docs/readme.md", b"readme" as &[u8]),
            ("/docs/guide.pdf", b"guide" as &[u8]),
            ("/src/main.rs", b"main" as &[u8]),
            ("/src/lib.rs", b"lib" as &[u8]),
            ("/src/tests/unit.rs", b"unit" as &[u8]),
            ("/src/tests/integration.rs", b"integration" as &[u8]),
        ];

        for (path, data) in &files {
            mount
                .add(&PathBuf::from(path), Cursor::new(data.to_vec()), &blobs)
                .await
                .unwrap();
        }

        for (path, expected_data) in &files {
            let data = mount.cat(&PathBuf::from(path), &blobs).await.unwrap();
            assert_eq!(data, expected_data.to_vec());
        }

        mount
            .rm(&PathBuf::from("/src/tests/unit.rs"), &blobs)
            .await
            .unwrap();

        let result = mount
            .cat(&PathBuf::from("/src/tests/unit.rs"), &blobs)
            .await;
        assert!(result.is_err());

        let data = mount
            .cat(&PathBuf::from("/src/tests/integration.rs"), &blobs)
            .await
            .unwrap();
        assert_eq!(data, b"integration");
    }

    #[tokio::test]
    async fn test_various_file_types() {
        let (mut mount, blobs, _, _temp) = setup_test_env().await;

        let test_files = vec![
            ("/image.png", "image/png"),
            ("/video.mp4", "video/mp4"),
            ("/style.css", "text/css"),
            ("/script.js", "application/javascript"),
            ("/data.json", "application/json"),
            ("/archive.zip", "application/zip"),
            ("/document.pdf", "application/pdf"),
            ("/code.rs", "text/rust"),
        ];

        for (path, expected_mime) in test_files {
            mount
                .add(&PathBuf::from(path), Cursor::new(b"test".to_vec()), &blobs)
                .await
                .unwrap();

            let items = mount.ls(&PathBuf::from("/"), &blobs).await.unwrap();
            let link = items.values().find(|l| l.is_data()).unwrap();

            if let Some(data_info) = link.data() {
                assert!(data_info.mime().is_some());
                assert_eq!(data_info.mime().unwrap().as_ref(), expected_mime);
            }

            mount.rm(&PathBuf::from(path), &blobs).await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_error_cases() {
        let (mount, blobs, _, _temp) = setup_test_env().await;

        let result = mount
            .cat(&PathBuf::from("/does_not_exist.txt"), &blobs)
            .await;
        assert!(result.is_err());

        let result = mount.ls(&PathBuf::from("/does_not_exist"), &blobs).await;
        assert!(result.is_err() || result.unwrap().is_empty());

        let (mut mount, blobs, _, _temp) = setup_test_env().await;
        mount
            .add(
                &PathBuf::from("/dir/file.txt"),
                Cursor::new(b"data".to_vec()),
                &blobs,
            )
            .await
            .unwrap();

        let result = mount.cat(&PathBuf::from("/dir"), &blobs).await;
        assert!(result.is_err());
    }
}
