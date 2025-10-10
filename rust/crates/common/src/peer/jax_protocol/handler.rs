use std::sync::Arc;

use anyhow::anyhow;
use futures::future::BoxFuture;
use iroh::endpoint::Connection;
use iroh::protocol::AcceptError;

use super::messages::{PingRequest, PingResponse};
use super::state::BucketStateProvider;

/// ALPN identifier for the JAX protocol
pub const JAX_ALPN: &[u8] = b"/iroh-jax/1";

/// Protocol handler for the JAX protocol
///
/// Accepts incoming connections and handles ping requests
#[derive(Clone, Debug)]
pub struct JaxProtocol {
    state: Arc<dyn BucketStateProvider>,
}

impl JaxProtocol {
    /// Create a new JAX protocol handler with the given state provider
    pub fn new(state: Arc<dyn BucketStateProvider>) -> Self {
        Self { state }
    }

    /// Handle an incoming connection
    ///
    /// This is called by the iroh router for each incoming connection
    /// with the JAX ALPN.
    pub fn handle_connection(
        self,
        conn: Connection,
    ) -> BoxFuture<'static, Result<(), AcceptError>> {
        Box::pin(async move {
            // Accept the first bidirectional stream from the connection
            let (mut send, mut recv) = conn.accept_bi().await.map_err(AcceptError::from)?;

            // Read the request from the stream
            let request_bytes = recv.read_to_end(1024 * 1024).await.map_err(|e| {
                AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?; // 1MB limit
            let request: PingRequest = bincode::deserialize(&request_bytes).map_err(|e| {
                let err: Box<dyn std::error::Error + Send + Sync> = anyhow!("Failed to deserialize request: {}", e).into();
                AcceptError::from(err)
            })?;

            tracing::debug!(
                "Received ping request for bucket {} with link {:?}",
                request.bucket_id,
                request.current_link
            );

            // Check the bucket sync status using the state provider
            let status = self
                .state
                .check_bucket_sync(request.bucket_id, &request.current_link)
                .await
                .unwrap_or_else(|e| {
                    tracing::error!("Error checking bucket sync: {}", e);
                    super::messages::SyncStatus::NotFound
                });

            let response = PingResponse::new(status);

            // Serialize and send the response
            let response_bytes = bincode::serialize(&response).map_err(|e| {
                let err: Box<dyn std::error::Error + Send + Sync> = anyhow!("Failed to serialize response: {}", e).into();
                AcceptError::from(err)
            })?;
            send.write_all(&response_bytes).await.map_err(|e| {
                AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;
            send.finish().map_err(|e| {
                AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;

            tracing::debug!("Sent ping response: {:?}", response);

            Ok(())
        })
    }
}

// Implement the iroh protocol handler trait
// This allows the router to accept connections for this protocol
impl iroh::protocol::ProtocolHandler for JaxProtocol {
    #[allow(refining_impl_trait)]
    fn accept(
        &self,
        conn: iroh::endpoint::Connection,
    ) -> BoxFuture<'static, Result<(), AcceptError>> {
        let this = self.clone();
        this.handle_connection(conn)
    }
}
