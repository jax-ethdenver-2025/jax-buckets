use anyhow::{anyhow, Result};
use iroh::{Endpoint, NodeAddr};
use uuid::Uuid;

use crate::linked_data::Link;

use super::messages::{PingRequest, PingResponse, SyncStatus};
use super::JAX_ALPN;

/// Ping a peer to check the sync status of a bucket
///
/// This function connects to a peer using the JAX protocol ALPN and sends
/// a ping request containing the bucket ID and the local peer's current link.
/// The remote peer will respond with the sync status.
///
/// # Arguments
/// * `endpoint` - The local iroh endpoint to use for the connection
/// * `peer_addr` - The address of the peer to ping
/// * `bucket_id` - The UUID of the bucket to check
/// * `current_link` - The current link/hash of the bucket on this peer
///
/// # Returns
/// The sync status from the remote peer's perspective
pub async fn ping_peer(
    endpoint: &Endpoint,
    peer_addr: &NodeAddr,
    bucket_id: Uuid,
    current_link: Link,
) -> Result<SyncStatus> {
    // Connect to the peer using the JAX ALPN
    let conn = endpoint
        .connect(peer_addr.clone(), JAX_ALPN)
        .await
        .map_err(|e| anyhow!("Failed to connect to peer: {}", e))?;

    // Open a bidirectional stream
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| anyhow!("Failed to open bidirectional stream: {}", e))?;

    // Create and serialize the ping request
    let request = PingRequest {
        bucket_id,
        current_link,
    };
    let request_bytes = bincode::serialize(&request)
        .map_err(|e| anyhow!("Failed to serialize ping request: {}", e))?;

    // Send the request
    send.write_all(&request_bytes)
        .await
        .map_err(|e| anyhow!("Failed to write request: {}", e))?;
    send.finish()
        .map_err(|e| anyhow!("Failed to finish sending request: {}", e))?;

    // Read the response
    let response_bytes = recv
        .read_to_end(1024 * 1024)
        .await
        .map_err(|e| anyhow!("Failed to read response: {}", e))?;

    // Deserialize the response
    let response: PingResponse = bincode::deserialize(&response_bytes)
        .map_err(|e| anyhow!("Failed to deserialize ping response: {}", e))?;

    tracing::debug!("Received ping response: {:?}", response);

    Ok(response.status)
}
