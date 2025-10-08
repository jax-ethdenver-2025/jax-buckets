use crate::trust_fetcher::TrustFetcher;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};

pub struct MockTrustFetcher {
    mock_trust: Vec<(usize, usize, f64)>,
    mock_connections: HashMap<usize, HashSet<usize>>,
}

impl Default for MockTrustFetcher {
    fn default() -> Self {
        Self::new()
    }
}

impl MockTrustFetcher {
    pub fn new() -> Self {
        // Add self-trust values to the mock data
        let mock_trust = vec![
            // Self trust values
            (0, 0, 1.0),
            (1, 1, 1.0),
            (2, 2, 1.0),
            (3, 3, 1.0),
            (4, 4, 1.0),
            (5, 5, 1.0),
            (6, 6, 1.0),
            (7, 7, 1.0),
            // Existing trust values
            (0, 1, 1.0),
            (0, 2, 0.5),
            (0, 3, 0.0),
            (1, 0, 0.75),
            (1, 2, 1.0),
            (1, 3, 0.25),
            (2, 0, 0.25),
            (2, 1, 0.5),
            (2, 3, 1.0),
            (3, 0, 0.5),
            (3, 1, 0.25),
            (3, 2, 0.5),
            // Extended network
            (4, 0, 0.8),
            (4, 1, 0.2),
            (4, 5, 1.0),
            (5, 4, 0.9),
            (5, 6, 0.7),
            (6, 5, 0.8),
            (6, 7, 0.6),
            (7, 6, 0.5),
            (7, 0, 0.3),
        ];

        // Initialize peer connections
        let mut mock_connections = HashMap::new();

        // Peer 0 knows peers 1, 2, 3, 4
        let mut peers_0 = HashSet::new();
        peers_0.extend([1, 2, 3, 4]);
        mock_connections.insert(0, peers_0);

        // Peer 1 knows peers 0, 2, 3, 4
        let mut peers_1 = HashSet::new();
        peers_1.extend([0, 2, 3, 4]);
        mock_connections.insert(1, peers_1);

        // Peer 2 knows peers 0, 1, 3
        let mut peers_2 = HashSet::new();
        peers_2.extend([0, 1, 3]);
        mock_connections.insert(2, peers_2);

        // Peer 3 knows peers 0, 1, 2
        let mut peers_3 = HashSet::new();
        peers_3.extend([0, 1, 2]);
        mock_connections.insert(3, peers_3);

        // Extended network connections
        let mut peers_4 = HashSet::new();
        peers_4.extend([0, 1, 5]);
        mock_connections.insert(4, peers_4);

        let mut peers_5 = HashSet::new();
        peers_5.extend([4, 6]);
        mock_connections.insert(5, peers_5);

        let mut peers_6 = HashSet::new();
        peers_6.extend([5, 7]);
        mock_connections.insert(6, peers_6);

        let mut peers_7 = HashSet::new();
        peers_7.extend([6, 0]);
        mock_connections.insert(7, peers_7);

        MockTrustFetcher {
            mock_trust,
            mock_connections,
        }
    }

    async fn fetch_trust(&self, i: &usize, j: &usize) -> Result<f64> {
        if let Some(trust_map) = self
            .mock_trust
            .iter()
            .find(|&&(peer_i, peer_j, _)| peer_i == *i && peer_j == *j)
        {
            Ok(trust_map.2)
        } else {
            Err(anyhow!("No trust value found for peer {} -> {}", i, j))
        }
    }

    async fn discover_peers(&self, peer_id: &usize) -> Result<HashSet<usize>> {
        if let Some(peers) = self.mock_connections.get(peer_id) {
            Ok(peers.clone())
        } else {
            Err(anyhow!("No peer connections found for peer {}", peer_id))
        }
    }
}

#[async_trait]
impl TrustFetcher for MockTrustFetcher {
    type NodeId = usize;

    async fn fetch_trust(&self, i: &Self::NodeId, j: &Self::NodeId) -> Result<f64> {
        self.fetch_trust(i, j).await
    }

    async fn discover_peers(&self, peer_id: &Self::NodeId) -> Result<HashSet<Self::NodeId>> {
        self.discover_peers(peer_id).await
    }
}
