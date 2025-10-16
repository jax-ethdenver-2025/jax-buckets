use std::collections::HashMap;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::http_server;
use crate::sync_manager::{SyncEvent, SyncManager};
use crate::{ServiceConfig, ServiceState};

use common::crypto::PublicKey;

pub mod registry;
pub mod protocol;

pub struct TestNetwork {
    peers: HashMap<String, Arc<Mutex<TestPeer>>>,
}

impl TestNetwork {
    pub fn new() -> Self {
        Self { peers: HashMap::new() }
    }

    pub fn add_peer(&mut self, label: impl Into<String>) -> PeerHandle {
        let label = label.into();
        let peer = Arc::new(Mutex::new(TestPeer::new(label.clone())));
        self.peers.insert(label.clone(), peer.clone());
        PeerHandle { inner: peer }
    }

    pub async fn eventually<F, Fut>(&self, timeout: Duration, check: F) -> anyhow::Result<()>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<bool>>,
    {
        let start = Instant::now();
        let mut delay = Duration::from_millis(50);
        while start.elapsed() < timeout {
            if check().await? {
                return Ok(());
            }
            tokio::time::sleep(delay).await;
            delay = std::cmp::min(delay * 2, Duration::from_millis(500));
        }
        anyhow::bail!("condition not met within timeout")
    }
}

pub struct TestPeer {
    label: String,
    state: Option<Arc<ServiceState>>,
    runners: Option<Runners>,
    shutdown_tx: Option<watch::Sender<()>>,
    _temp_dir: tempfile::TempDir,
    config: ServiceConfig,
}

#[derive(Clone)]
pub struct PeerHandle {
    inner: Arc<Mutex<TestPeer>>,
}

impl PeerHandle {
    pub async fn start(&self) -> anyhow::Result<()> {
        let mut peer = self.inner.lock().await;
        peer.start().await
    }

    pub async fn stop(&self) -> anyhow::Result<()> {
        let mut peer = self.inner.lock().await;
        peer.stop().await
    }

    pub async fn public_key(&self) -> PublicKey {
        let mut peer = self.inner.lock().await;
        if peer.state.is_none() {
            let built = ServiceState::from_config(&peer.config).await.expect("build state");
            peer.state = Some(Arc::new(built));
        }
        peer.state.as_ref().unwrap().node().secret().public()
    }

    pub async fn create_bucket(&self, name: &str) -> anyhow::Result<Uuid> {
        let mut peer = self.inner.lock().await;
        peer.create_bucket(name).await
    }

    pub async fn share_bucket_with(&self, bucket: Uuid, target: &PeerHandle) -> anyhow::Result<()> {
        let target_pub = target.public_key().await;
        let mut peer = self.inner.lock().await;
        if peer.state.is_none() {
            let built = ServiceState::from_config(&peer.config).await?;
            peer.state = Some(Arc::new(built));
        }
        let state = peer.state.as_ref().unwrap().clone();
        crate::mount_ops::share_bucket(bucket, target_pub, &state).await?;
        Ok(())
    }

    pub async fn add_file_bytes(&self, bucket: Uuid, path: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let mut peer = self.inner.lock().await;
        peer.add_file_bytes(bucket, path, bytes).await
    }

    pub async fn has_file(&self, bucket: Uuid, path: &str) -> anyhow::Result<bool> {
        let mut peer = self.inner.lock().await;
        peer.has_file(bucket, path).await
    }

    pub async fn trigger_pull(&self, bucket: Uuid) -> anyhow::Result<()> {
        let mut peer = self.inner.lock().await;
        peer.trigger_pull(bucket).await
    }

    pub async fn sync_from_peer(&self, bucket: Uuid, source: &PeerHandle) -> anyhow::Result<()> {
        let peer = self.inner.lock().await;
        peer.sync_from_peer(bucket, source).await
    }

    pub async fn sync_from_peer_paths(
        &self,
        bucket: Uuid,
        source: &PeerHandle,
        paths: &[&str],
    ) -> anyhow::Result<()> {
        let mut peer = self.inner.lock().await;
        peer.sync_from_peer_paths(bucket, source, paths).await
    }

    pub async fn ensure_bucket(&self, id: Uuid, name: &str) -> anyhow::Result<()> {
        let mut peer = self.inner.lock().await;
        peer.ensure_bucket(id, name).await
    }
}

impl TestPeer {
    pub fn new(label: String) -> Self {
        // Each peer uses a private temp dir for db and blobs
        let temp_dir = tempfile::TempDir::new().expect("failed to create temp dir");

        let mut config = ServiceConfig::default();
        // SQLite file in temp dir (create parent and empty file)
        let sqlite_path = temp_dir.path().join("db.sqlite");
        std::fs::File::create(&sqlite_path).expect("failed to create sqlite file");
        config.sqlite_path = Some(sqlite_path);

        // Blobs store in temp dir
        let blobs_path = temp_dir.path().join("blobs");
        std::fs::create_dir_all(&blobs_path).expect("failed to create blobs dir");
        config.node_blobs_store_path = Some(blobs_path);

        // Bind iroh/node on ephemeral port
        config.node_listen_addr = Some("127.0.0.1:0".parse().unwrap());
        // Bind HTTP servers on ephemeral ports (not used directly by tests)
        config.api_listen_addr = Some("127.0.0.1:0".parse().unwrap());
        config.html_listen_addr = Some("127.0.0.1:0".parse().unwrap());

        Self {
            label,
            state: None,
            runners: None,
            shutdown_tx: None,
            _temp_dir: temp_dir,
            config,
        }
    }

    async fn ensure_state(&mut self) -> anyhow::Result<Arc<ServiceState>> {
        if self.state.is_none() {
            let built = ServiceState::from_config(&self.config).await?;
            self.state = Some(Arc::new(built));
        }
        Ok(self.state.as_ref().unwrap().clone())
    }

    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.runners.is_some() {
            return Ok(());
        }

        let (shutdown_tx, shutdown_rx) = watch::channel(());
        if self.state.is_none() {
            let built = ServiceState::from_config(&self.config).await?;
            self.state = Some(Arc::new(built));
        }
        let state = self.state.as_ref().unwrap().clone();

        // Wire sync manager
        let (sync_manager, sync_receiver) = SyncManager::new(state.clone());
        state.as_ref().set_sync_sender(sync_manager.sender());

        // HTTP ports: use ephemeral 127.0.0.1:0
        let api_config = http_server::Config::new("127.0.0.1:0".parse().unwrap(), None, false);
        let html_config = http_server::Config::new(
            "127.0.0.1:0".parse().unwrap(),
            Some("http://localhost".to_string()),
            true,
        );

        // Spawn API server
        let api_state = state.as_ref().clone();
        let api_rx = shutdown_rx.clone();
        let api: JoinHandle<()> = tokio::spawn(async move {
            let _ = http_server::run_api(api_config, api_state, api_rx).await;
        });

        // Spawn HTML server
        let html_state = state.as_ref().clone();
        let html_rx = shutdown_rx.clone();
        let html: JoinHandle<()> = tokio::spawn(async move {
            let _ = http_server::run_html(html_config, html_state, html_rx).await;
        });

        // Spawn node
        let node_state = state.clone();
        let node_rx = shutdown_rx.clone();
        let node: JoinHandle<()> = tokio::spawn(async move {
            let node = node_state.node();
            let _ = node.spawn(node_rx).await;
        });

        // Wait briefly for the node to bind sockets before returning
        // to reduce flakiness in tests that immediately perform network ops
        for _ in 0..20 {
            if !state.node().endpoint().bound_sockets().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }

        // Register this peer for direct, in-process dialing in tests
        registry::register(state.clone());

        // Spawn sync manager
        let sync: JoinHandle<()> = tokio::spawn(async move {
            sync_manager.run(sync_receiver).await;
        });

        // No periodic checker in tests; rely on explicit triggers

        self.shutdown_tx = Some(shutdown_tx);
        self.runners = Some(Runners { api, html, node, sync });
        Ok(())
    }

    /// Deterministic sync: re-add specified file paths from source onto this peer.
    pub async fn sync_from_peer_paths(
        &mut self,
        bucket: Uuid,
        source: &PeerHandle,
        paths: &[&str],
    ) -> anyhow::Result<()> {
        let src_state = {
            let mut s = source.inner.lock().await;
            if s.state.is_none() {
                let built = ServiceState::from_config(&s.config).await?;
                s.state = Some(Arc::new(built));
            }
            s.state.as_ref().unwrap().clone()
        };
        let dst_state = self.ensure_state().await?;

        // Ensure destination bucket exists: create a local mount with same id+name
        let maybe_dst = crate::database::models::Bucket::get_by_id(&bucket, dst_state.database()).await?;
        if maybe_dst.is_none() {
            // Read name from source
            let src_bucket = crate::database::models::Bucket::get_by_id(&bucket, src_state.database()).await?
                .ok_or_else(|| anyhow::anyhow!("source bucket not found"))?;
            let src_link: common::linked_data::Link = src_bucket.link.into();
            let mount_src = common::prelude::Mount::load(&src_link, src_state.node().secret(), src_state.node().blobs()).await?;
            let name = mount_src.inner().manifest().name().to_string();

            // Create a fresh local mount at destination with the same id and name
            let mount_dst = common::prelude::Mount::init(
                bucket,
                name,
                dst_state.node().secret(),
                dst_state.node().blobs(),
            ).await?;
            let dst_link = mount_dst.link();
            crate::database::models::Bucket::create(bucket, mount_dst.inner().manifest().name().to_string(), dst_link.clone(), dst_state.database()).await?;
        }

        for p in paths {
            let content = crate::mount_ops::get_file_content(bucket, (*p).to_string(), &src_state).await?;
            self.add_file_bytes(bucket, p, &content.data).await?;
        }

        Ok(())
    }

    pub async fn stop(&mut self) -> anyhow::Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(r) = self.runners.take() {
            let _ = r.api.await;
            let _ = r.html.await;
            let _ = r.node.await;
            let _ = r.sync.await;
        }
        Ok(())
    }

    pub fn public_key(&self) -> PublicKey {
        self.state.as_ref().unwrap().node().secret().public()
    }

    pub async fn create_bucket(&mut self, name: &str) -> anyhow::Result<Uuid> {
        use crate::database::models::Bucket as BucketModel;
        use common::prelude::Mount;

        let id = Uuid::new_v4();
        let state = self.ensure_state().await?;
        let owner = state.node().secret();
        let blobs = state.node().blobs();
        let mount = Mount::init(id, name.to_string(), owner, blobs).await?;
        let link = mount.link();
        let created = BucketModel::create(
            id,
            name.to_string(),
            link.clone(),
            state.database(),
        )
        .await?;
        Ok(created.id)
    }

    pub async fn share_bucket_with(&self, bucket: Uuid, target: &TestPeer) -> anyhow::Result<()> {
        crate::mount_ops::share_bucket(
            bucket,
            target.public_key(),
            self.state.as_ref().unwrap(),
        )
        .await?;
        Ok(())
    }

    pub async fn add_file_bytes(&mut self, bucket: Uuid, path: &str, bytes: &[u8]) -> anyhow::Result<()> {
        let path_buf = PathBuf::from(path);
        let reader = Cursor::new(bytes.to_vec());
        let state = self.ensure_state().await?;
        let _ = crate::mount_ops::add_data_to_bucket(bucket, path_buf, reader, &state)
        .await?;
        Ok(())
    }

    pub async fn has_file(&mut self, bucket: Uuid, path: &str) -> anyhow::Result<bool> {
        let path = path.to_string();
        let state = self.ensure_state().await?;
        match crate::mount_ops::get_file_content(bucket, path, &state).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    pub async fn trigger_pull(&mut self, bucket: Uuid) -> anyhow::Result<()> {
        let state = self.ensure_state().await?;
        state
            .send_sync_event(SyncEvent::Pull { bucket_id: bucket })
            .map_err(|e| anyhow::anyhow!(format!("failed to send pull: {e:?}")))
    }

    pub async fn ensure_bucket(&mut self, id: Uuid, name: &str) -> anyhow::Result<()> {
        use crate::database::models::Bucket as BucketModel;
        use common::prelude::Mount;

        let state = self.ensure_state().await?;
        if BucketModel::get_by_id(&id, state.database()).await?.is_none() {
            let mount = Mount::init(id, name.to_string(), state.node().secret(), state.node().blobs()).await?;
            let link = mount.link();
            let _ = BucketModel::create(id, name.to_string(), link.clone(), state.database()).await?;
        }
        Ok(())
    }

    /// Test-only manual sync: copy bucket link and blobs from a source peer.
    /// This bypasses network discovery and simulates a successful pull.
    pub async fn sync_from_peer(&self, bucket: Uuid, source: &PeerHandle) -> anyhow::Result<()> {
        use crate::database::models::Bucket as BucketModel;
        use common::prelude::{Link, Mount};

        let (src_state, dst_state) = {
            let s = source.inner.lock().await;
            (s.state.as_ref().unwrap().clone(), self.state.as_ref().unwrap().clone())
        };

        // Lookup source bucket and link
        let src_bucket = BucketModel::get_by_id(&bucket, src_state.database()).await?
            .ok_or_else(|| anyhow::anyhow!("source bucket not found"))?;
        let src_link: Link = src_bucket.link.into();

        // Ensure destination bucket exists, create if missing using source name
        let maybe_dst = BucketModel::get_by_id(&bucket, dst_state.database()).await?;
        if maybe_dst.is_none() {
            let mount = Mount::load(&src_link, src_state.node().secret(), src_state.node().blobs()).await?;
            let name = mount.inner().manifest().name().to_string();
            BucketModel::create(bucket, name, src_link.clone(), dst_state.database()).await?;
        } else {
            // update link
            maybe_dst.unwrap().update_link(src_link.clone(), dst_state.database()).await?;
        }

        // Copy blobs directory from source to destination (best-effort)
        let src_path = src_state.node().blobs_store_path().clone();
        let dst_path = dst_state.node().blobs_store_path().clone();

        tokio::task::spawn_blocking(move || {
            fn copy_dir_all(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
                if !dst.exists() { std::fs::create_dir_all(dst)?; }
                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    let ty = entry.file_type()?;
                    let src_path = entry.path();
                    let dst_path = dst.join(entry.file_name());
                    if ty.is_dir() { copy_dir_all(&src_path, &dst_path)?; }
                    else if ty.is_file() {
                        // overwrite newer content
                        std::fs::copy(&src_path, &dst_path)?;
                    }
                }
                Ok(())
            }
            copy_dir_all(&src_path, &dst_path).map_err(|e| anyhow::anyhow!(e))
        }).await??;

        Ok(())
    }
}

struct Runners {
    api: JoinHandle<()>,
    html: JoinHandle<()>,
    node: JoinHandle<()>,
    sync: JoinHandle<()>,
}


