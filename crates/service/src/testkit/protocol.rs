use common::peer::{announce_to_peer, fetch_bucket, ping_peer, NodeAddr};
use uuid::Uuid;

use crate::ServiceState;

/// Test-only wrappers that enrich NodeAddr from the test registry when possible.
pub async fn test_ping_peer(
    state: &ServiceState,
    peer_public_key_hex: &str,
    bucket_id: Uuid,
    current_link: common::linked_data::Link,
) -> anyhow::Result<common::peer::SyncStatus> {
    let endpoint = state.node().endpoint();
    let addr = super::registry::lookup(peer_public_key_hex)
        .unwrap_or_else(|| NodeAddr::new(*state.node().secret().public()));
    Ok(ping_peer(endpoint, &addr, bucket_id, current_link).await?)
}

/// Same as test_ping_peer but accepts a NodeAddr; we replace it if a richer addr is registered.
pub async fn test_ping_peer_addr(
    state: &ServiceState,
    peer_addr: &NodeAddr,
    bucket_id: Uuid,
    current_link: common::linked_data::Link,
) -> anyhow::Result<common::peer::SyncStatus> {
    let endpoint = state.node().endpoint();
    let peer_hex = peer_addr.node_id.to_string();
    let effective = super::registry::lookup(&peer_hex).unwrap_or_else(|| peer_addr.clone());
    Ok(ping_peer(endpoint, &effective, bucket_id, current_link).await?)
}

pub async fn test_fetch_bucket(
    state: &ServiceState,
    peer_public_key_hex: &str,
    bucket_id: Uuid,
) -> anyhow::Result<Option<common::linked_data::Link>> {
    let endpoint = state.node().endpoint();
    let addr = super::registry::lookup(peer_public_key_hex)
        .unwrap_or_else(|| NodeAddr::new(*state.node().secret().public()));
    Ok(fetch_bucket(endpoint, &addr, bucket_id).await?)
}

pub async fn test_fetch_bucket_addr(
    state: &ServiceState,
    peer_addr: &NodeAddr,
    bucket_id: Uuid,
) -> anyhow::Result<Option<common::linked_data::Link>> {
    let endpoint = state.node().endpoint();
    let peer_hex = peer_addr.node_id.to_string();
    let effective = super::registry::lookup(&peer_hex).unwrap_or_else(|| peer_addr.clone());
    Ok(fetch_bucket(endpoint, &effective, bucket_id).await?)
}

pub async fn test_announce_to_peer(
    state: &ServiceState,
    peer_public_key_hex: &str,
    bucket_id: Uuid,
    new_link: common::linked_data::Link,
    previous_link: Option<common::linked_data::Link>,
) -> anyhow::Result<()> {
    let endpoint = state.node().endpoint();
    let addr = super::registry::lookup(peer_public_key_hex)
        .unwrap_or_else(|| NodeAddr::new(*state.node().secret().public()));
    Ok(announce_to_peer(endpoint, &addr, bucket_id, new_link, previous_link).await?)
}

pub async fn test_announce_to_peer_addr(
    state: &ServiceState,
    peer_addr: &NodeAddr,
    bucket_id: Uuid,
    new_link: common::linked_data::Link,
    previous_link: Option<common::linked_data::Link>,
) -> anyhow::Result<()> {
    let endpoint = state.node().endpoint();
    let peer_hex = peer_addr.node_id.to_string();
    let effective = super::registry::lookup(&peer_hex).unwrap_or_else(|| peer_addr.clone());
    Ok(announce_to_peer(endpoint, &effective, bucket_id, new_link, previous_link).await?)
}


