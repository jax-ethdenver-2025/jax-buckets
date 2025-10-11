use anyhow::{anyhow, Result};
use iroh::{Endpoint, NodeAddr};
use uuid::Uuid;

use crate::linked_data::Link;

use super::messages::{
    AnnounceMessage, FetchBucketRequest, PingRequest, Request, Response, SyncStatus,
};
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
    tracing::debug!(
        "ping_peer: Starting ping to peer {} for bucket {} with link {}",
        peer_addr.node_id,
        bucket_id,
        current_link.hash()
    );

    // Connect to the peer using the JAX ALPN
    // Use just the node_id so iroh can handle relay discovery automatically
    tracing::debug!("ping_peer: Connecting to peer {}", peer_addr.node_id);
    let conn = endpoint
        .connect(peer_addr.node_id, JAX_ALPN)
        .await
        .map_err(|e| {
            tracing::error!(
                "ping_peer: Failed to connect to peer {}: {}",
                peer_addr.node_id,
                e
            );
            anyhow!("Failed to connect to peer: {}", e)
        })?;
    tracing::debug!(
        "ping_peer: Successfully connected to peer {}",
        peer_addr.node_id
    );

    // Open a bidirectional stream
    tracing::debug!("ping_peer: Opening bidirectional stream");
    let (mut send, mut recv) = conn.open_bi().await.map_err(|e| {
        tracing::error!("ping_peer: Failed to open bidirectional stream: {}", e);
        anyhow!("Failed to open bidirectional stream: {}", e)
    })?;
    tracing::debug!("ping_peer: Bidirectional stream opened");

    // Create and serialize the ping request
    tracing::debug!("ping_peer: Creating ping request");
    let request = Request::Ping(PingRequest {
        bucket_id,
        current_link: current_link.clone(),
    });
    let request_bytes = bincode::serialize(&request).map_err(|e| {
        tracing::error!("ping_peer: Failed to serialize ping request: {}", e);
        anyhow!("Failed to serialize ping request: {}", e)
    })?;
    tracing::debug!(
        "ping_peer: Serialized ping request ({} bytes)",
        request_bytes.len()
    );

    // Send the request
    tracing::debug!("ping_peer: Sending ping request");
    send.write_all(&request_bytes).await.map_err(|e| {
        tracing::error!("ping_peer: Failed to write request: {}", e);
        anyhow!("Failed to write request: {}", e)
    })?;
    send.finish().map_err(|e| {
        tracing::error!("ping_peer: Failed to finish sending request: {}", e);
        anyhow!("Failed to finish sending request: {}", e)
    })?;
    tracing::debug!("ping_peer: Ping request sent, waiting for response");

    // Read the response
    let response_bytes = recv.read_to_end(1024 * 1024).await.map_err(|e| {
        tracing::error!("ping_peer: Failed to read response: {}", e);
        anyhow!("Failed to read response: {}", e)
    })?;
    tracing::debug!(
        "ping_peer: Received response ({} bytes)",
        response_bytes.len()
    );

    // Deserialize the response
    let response: Response = bincode::deserialize(&response_bytes).map_err(|e| {
        tracing::error!("ping_peer: Failed to deserialize response: {}", e);
        anyhow!("Failed to deserialize response: {}", e)
    })?;
    tracing::debug!("ping_peer: Successfully deserialized response");

    match response {
        Response::Ping(ping_response) => {
            tracing::info!(
                "ping_peer: Received ping response from {}: status={:?}",
                peer_addr.node_id,
                ping_response.status
            );
            Ok(ping_response.status)
        }
        _ => {
            tracing::error!("ping_peer: Unexpected response type for ping request");
            Err(anyhow!("Unexpected response type for ping request"))
        }
    }
}

/// Fetch the current bucket link from a peer
///
/// This function connects to a peer using the JAX protocol ALPN and sends
/// a fetch bucket request to retrieve the peer's current link for a bucket.
///
/// # Arguments
/// * `endpoint` - The local iroh endpoint to use for the connection
/// * `peer_addr` - The address of the peer to fetch from
/// * `bucket_id` - The UUID of the bucket to fetch
///
/// # Returns
/// The current link for the bucket on the peer (None if bucket not found)
pub async fn fetch_bucket(
    endpoint: &Endpoint,
    peer_addr: &NodeAddr,
    bucket_id: Uuid,
) -> Result<Option<Link>> {
    // Connect to the peer using the JAX ALPN
    // Use just the node_id so iroh can handle relay discovery automatically
    let conn = endpoint
        .connect(peer_addr.node_id, JAX_ALPN)
        .await
        .map_err(|e| anyhow!("Failed to connect to peer: {}", e))?;

    // Open a bidirectional stream
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| anyhow!("Failed to open bidirectional stream: {}", e))?;

    // Create and serialize the fetch bucket request
    let request = Request::FetchBucket(FetchBucketRequest::new(bucket_id));
    let request_bytes = bincode::serialize(&request)
        .map_err(|e| anyhow!("Failed to serialize fetch bucket request: {}", e))?;

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
    let response: Response = bincode::deserialize(&response_bytes)
        .map_err(|e| anyhow!("Failed to deserialize response: {}", e))?;

    match response {
        Response::FetchBucket(fetch_response) => {
            tracing::debug!("Received fetch bucket response: {:?}", fetch_response);
            Ok(fetch_response.current_link)
        }
        _ => Err(anyhow!("Unexpected response type for fetch bucket request")),
    }
}

/// Announce a new bucket version to a peer
///
/// This function connects to a peer using the JAX protocol ALPN and sends
/// an announce message to notify them of a new bucket version. This is a
/// one-way message with no response expected.
///
/// # Arguments
/// * `endpoint` - The local iroh endpoint to use for the connection
/// * `peer_addr` - The address of the peer to announce to
/// * `bucket_id` - The UUID of the bucket being announced
/// * `new_link` - The new link for the bucket
/// * `previous_link` - The previous link (for single-hop verification)
pub async fn announce_to_peer(
    endpoint: &Endpoint,
    peer_addr: &NodeAddr,
    bucket_id: Uuid,
    new_link: Link,
    previous_link: Option<Link>,
) -> Result<()> {
    // Connect to the peer using the JAX ALPN
    // Use just the node_id so iroh can handle relay discovery automatically
    let conn = endpoint
        .connect(peer_addr.node_id, JAX_ALPN)
        .await
        .map_err(|e| anyhow!("Failed to connect to peer: {}", e))?;

    // Open a bidirectional stream
    let (mut send, mut recv) = conn
        .open_bi()
        .await
        .map_err(|e| anyhow!("Failed to open bidirectional stream: {}", e))?;

    tracing::debug!(
        "Sending announce to peer for bucket {} with new link {:?}",
        bucket_id,
        new_link
    );

    // Create and serialize the announce message
    let request = Request::Announce(AnnounceMessage::new(bucket_id, new_link, previous_link));
    let request_bytes = bincode::serialize(&request)
        .map_err(|e| anyhow!("Failed to serialize announce message: {}", e))?;

    // Send the announce message
    send.write_all(&request_bytes)
        .await
        .map_err(|e| anyhow!("Failed to write announce message: {}", e))?;
    send.finish()
        .map_err(|e| anyhow!("Failed to finish sending announce message: {}", e))?;

    // Wait for server to close the stream (acknowledgment that message was received)
    // This ensures the server has time to read and process the announce message
    let _ = recv.read_to_end(0).await;

    Ok(())
}
