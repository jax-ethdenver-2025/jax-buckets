use std::sync::Arc;

use anyhow::anyhow;
use futures::future::BoxFuture;
use iroh::endpoint::Connection;
use iroh::protocol::AcceptError;

use super::messages::{FetchBucketResponse, PingResponse, Request, Response};
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
            tracing::debug!(
                "JAX handler: Accepted new connection from {:?}",
                conn.remote_node_id()
            );

            // Accept the first bidirectional stream from the connection
            tracing::debug!("JAX handler: Accepting bidirectional stream");
            let (mut send, mut recv) = conn.accept_bi().await.map_err(|e| {
                tracing::error!("JAX handler: Failed to accept bidirectional stream: {}", e);
                AcceptError::from(e)
            })?;
            tracing::debug!("JAX handler: Bidirectional stream accepted");

            // Get remote peer ID for announce handling
            let remote_node_id = conn.remote_node_id().map(|id| id.to_string());

            // Read the request from the stream
            tracing::debug!("JAX handler: Reading request from stream");
            let request_bytes = recv.read_to_end(1024 * 1024).await.map_err(|e| {
                tracing::error!("JAX handler: Failed to read request: {}", e);
                AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?; // 1MB limit
            tracing::debug!(
                "JAX handler: Read {} bytes from stream",
                request_bytes.len()
            );

            tracing::debug!("JAX handler: Deserializing request");
            let request: Request = bincode::deserialize(&request_bytes).map_err(|e| {
                tracing::error!("JAX handler: Failed to deserialize request: {}", e);
                let err: Box<dyn std::error::Error + Send + Sync> =
                    anyhow!("Failed to deserialize request: {}", e).into();
                AcceptError::from(err)
            })?;
            tracing::debug!("JAX handler: Successfully deserialized request");

            // Dispatch based on request type
            match request {
                Request::Ping(ping_req) => {
                    tracing::info!(
                        "JAX handler: Received ping request for bucket {} with link {:?}",
                        ping_req.bucket_id,
                        ping_req.current_link
                    );

                    // Check the bucket sync status using the state provider
                    tracing::debug!("JAX handler: Checking bucket sync status");
                    let status = self
                        .state
                        .check_bucket_sync(ping_req.bucket_id, &ping_req.current_link)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::error!("JAX handler: Error checking bucket sync: {}", e);
                            super::messages::SyncStatus::NotFound
                        });
                    tracing::debug!("JAX handler: Bucket sync status: {:?}", status);

                    let response = Response::Ping(PingResponse::new(status));
                    tracing::debug!("JAX handler: Created ping response");

                    // Serialize and send the response
                    tracing::debug!("JAX handler: Serializing ping response");
                    let response_bytes = bincode::serialize(&response).map_err(|e| {
                        tracing::error!("JAX handler: Failed to serialize response: {}", e);
                        let err: Box<dyn std::error::Error + Send + Sync> =
                            anyhow!("Failed to serialize response: {}", e).into();
                        AcceptError::from(err)
                    })?;
                    tracing::debug!(
                        "JAX handler: Serialized response to {} bytes",
                        response_bytes.len()
                    );

                    tracing::debug!("JAX handler: Writing response to stream");
                    send.write_all(&response_bytes).await.map_err(|e| {
                        tracing::error!("JAX handler: Failed to write response: {}", e);
                        AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;

                    tracing::debug!("JAX handler: Finishing send stream");
                    send.finish().map_err(|e| {
                        tracing::error!("JAX handler: Failed to finish send stream: {}", e);
                        AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;

                    tracing::debug!("JAX handler: Waiting for connection to close");
                    conn.closed().await;

                    tracing::info!(
                        "JAX handler: Successfully sent ping response: {:?}",
                        response
                    );
                }

                Request::FetchBucket(fetch_req) => {
                    tracing::debug!(
                        "Received fetch bucket request for bucket {}",
                        fetch_req.bucket_id
                    );

                    // Get the current bucket link
                    let current_link = self
                        .state
                        .get_bucket_link(fetch_req.bucket_id)
                        .await
                        .unwrap_or_else(|e| {
                            tracing::error!("Error fetching bucket link: {}", e);
                            None
                        });

                    let response = Response::FetchBucket(FetchBucketResponse::new(current_link));

                    // Serialize and send the response
                    let response_bytes = bincode::serialize(&response).map_err(|e| {
                        let err: Box<dyn std::error::Error + Send + Sync> =
                            anyhow!("Failed to serialize response: {}", e).into();
                        AcceptError::from(err)
                    })?;

                    send.write_all(&response_bytes).await.map_err(|e| {
                        AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;

                    send.finish().map_err(|e| {
                        AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;

                    conn.closed().await;

                    tracing::debug!("Sent fetch bucket response: {:?}", response);
                }

                Request::Announce(announce_msg) => {
                    let peer_id = remote_node_id.unwrap_or_else(|_| "unknown".to_string());

                    tracing::info!(
                        "Received announce from peer {} for bucket {} with new link {:?}",
                        peer_id,
                        announce_msg.bucket_id,
                        announce_msg.new_link
                    );

                    // Handle the announce message (triggers sync event)
                    if let Err(e) = self
                        .state
                        .handle_announce(
                            announce_msg.bucket_id,
                            peer_id,
                            announce_msg.new_link,
                            announce_msg.previous_link,
                        )
                        .await
                    {
                        tracing::error!("Error handling announce: {}", e);
                    }

                    // No response needed for announce - just finish the stream
                    send.finish().map_err(|e| {
                        AcceptError::from(std::io::Error::new(std::io::ErrorKind::Other, e))
                    })?;
                }
            }

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
