use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Display;
use std::hash::Hash;

use anyhow::Result;
use async_trait::async_trait;

/// Trait for fetching remote trust values
#[async_trait]
pub trait TrustFetcher {
    type NodeId: Clone + Hash + Eq + Debug + Copy + Display;

    /// Fetch the trust value that peer i has for peer j
    async fn fetch_trust(&self, i: &Self::NodeId, j: &Self::NodeId) -> Result<f64>;

    /// Discover peers connected to a given peer
    /// Returns a set of peer IDs that peer_id knows about
    async fn discover_peers(&self, peer_id: &Self::NodeId) -> Result<HashSet<Self::NodeId>>;
}
