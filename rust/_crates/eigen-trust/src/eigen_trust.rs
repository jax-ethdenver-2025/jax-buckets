use anyhow::{anyhow, Result};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use crate::trust_fetcher::TrustFetcher;

/// Implementation of the basic EigenTrust algorithm
/// with support for fetching remote trust values and dynamic peer management
pub struct EigenTrust<F: TrustFetcher> {
    peers: HashSet<F::NodeId>,
    local_trust: HashMap<F::NodeId, f64>,
    trust_cache: HashMap<(F::NodeId, F::NodeId), f64>,
    trust_fetcher: F,
    epsilon: f64,
    max_iterations: usize,
    pre_trusted: HashMap<F::NodeId, f64>,
}

impl<F: TrustFetcher> EigenTrust<F>
where
    F::NodeId: Clone + Hash + Eq,
{
    pub fn new(trust_fetcher: F) -> Self {
        let peers = HashSet::new();

        EigenTrust {
            peers,
            local_trust: HashMap::new(),
            trust_cache: HashMap::new(),
            trust_fetcher,
            epsilon: 0.001,
            max_iterations: 100,
            pre_trusted: HashMap::new(),
        }
    }

    pub fn set_epsilon(&mut self, epsilon: f64) -> &mut Self {
        self.epsilon = epsilon;
        self
    }

    pub fn set_max_iterations(&mut self, max_iterations: usize) -> &mut Self {
        self.max_iterations = max_iterations;
        self
    }

    pub fn add_pre_trusted(&mut self, peer_id: F::NodeId, value: f64) -> &mut Self {
        assert!(value >= 0.0, "Trust values must be non-negative");
        self.pre_trusted.insert(peer_id, value);
        self.add_peer(peer_id);
        self
    }

    pub fn clear_pre_trusted(&mut self) -> &mut Self {
        self.pre_trusted.clear();
        self
    }

    pub fn add_local_trust(&mut self, j: F::NodeId, value: f64) -> &mut Self {
        assert!(value >= 0.0, "Trust values must be non-negative");
        self.local_trust.insert(j, value);
        self.add_peer(j);
        self
    }

    pub fn update_local_trust(&mut self, j: F::NodeId, new_value: f64, weight: f64) -> &mut Self {
        assert!(new_value >= 0.0, "Trust values must be non-negative");
        assert!(
            (0.0..=1.0).contains(&weight),
            "Weight must be between 0 and 1"
        );

        let current = self.local_trust.get(&j).cloned().unwrap_or(0.0);
        let updated = (1.0 - weight) * current + weight * new_value;
        self.local_trust.insert(j, updated);
        self.add_peer(j);
        self
    }

    pub fn remove_local_trust(&mut self, j: F::NodeId) -> &mut Self {
        self.local_trust.remove(&j);
        self
    }

    pub fn add_peer(&mut self, peer_id: F::NodeId) -> &mut Self {
        self.peers.insert(peer_id);
        self
    }

    pub fn remove_peer(&mut self, peer_id: F::NodeId) -> &mut Self {
        self.peers.remove(&peer_id);
        self.local_trust.remove(&peer_id);
        self.trust_cache
            .retain(|&(i, j), _| i != peer_id && j != peer_id);
        self.pre_trusted.remove(&peer_id);
        self
    }

    pub fn get_peers(&self) -> &HashSet<F::NodeId> {
        &self.peers
    }

    pub async fn discover_peers(&mut self, max_depth: usize) -> Result<()> {
        if max_depth == 0 {
            return Ok(());
        }

        let mut explored = HashSet::new();
        let mut to_explore: Vec<F::NodeId> = self.peers.iter().cloned().collect();
        let mut current_depth = 0;

        while !to_explore.is_empty() && current_depth < max_depth {
            let peers_at_current_level = to_explore.clone();
            to_explore.clear();

            for peer_id in peers_at_current_level {
                if explored.contains(&peer_id) {
                    continue;
                }

                explored.insert(peer_id);

                match self.trust_fetcher.discover_peers(&peer_id).await {
                    Ok(discovered) => {
                        for new_peer in discovered {
                            self.add_peer(new_peer);
                            if !explored.contains(&new_peer) {
                                to_explore.push(new_peer);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to discover peers from {}: {}", peer_id, e);
                    }
                }
            }

            current_depth += 1;
        }

        Ok(())
    }

    async fn get_trust(&mut self, i: &F::NodeId, j: &F::NodeId) -> Result<f64> {
        if !self.peers.contains(i) || !self.peers.contains(j) {
            return Ok(0.0);
        }

        if let Some(&value) = self.trust_cache.get(&(*i, *j)) {
            return Ok(value);
        }

        let value = self.trust_fetcher.fetch_trust(i, j).await?;
        self.trust_cache.insert((*i, *j), value);
        Ok(value)
    }

    pub fn clear_cache(&mut self) -> &mut Self {
        self.trust_cache.clear();
        self
    }

    async fn normalize_local_trust(&mut self) -> Result<HashMap<(F::NodeId, F::NodeId), f64>> {
        let mut c = HashMap::new();
        let peers_vec: Vec<F::NodeId> = self.peers.iter().cloned().collect();

        for i in &peers_vec {
            let mut sum = 0.0;

            for j in &peers_vec {
                let trust = self.get_trust(i, j).await?;
                sum += trust;
            }

            if sum > 0.0 {
                for j in &peers_vec {
                    let trust = self.get_trust(i, j).await?;
                    c.insert((*i, *j), trust / sum);
                }
            } else {
                let uniform_trust = 1.0 / peers_vec.len() as f64;
                for j in &peers_vec {
                    c.insert((*i, *j), uniform_trust);
                }
            }
        }

        Ok(c)
    }

    pub async fn compute_global_trust(&mut self) -> Result<HashMap<F::NodeId, f64>> {
        if self.peers.is_empty() {
            return Err(anyhow!("No peers available for trust computation"));
        }

        let peers_vec: Vec<F::NodeId> = self.peers.iter().cloned().collect();
        let n = peers_vec.len();

        let peer_to_index: HashMap<F::NodeId, usize> = peers_vec
            .iter()
            .enumerate()
            .map(|(idx, peer_id)| (*peer_id, idx))
            .collect();

        let index_to_peer: HashMap<usize, F::NodeId> = peers_vec
            .iter()
            .enumerate()
            .map(|(idx, peer_id)| (idx, *peer_id))
            .collect();

        let mut t = vec![0.0; n];

        if !self.pre_trusted.is_empty() {
            let sum: f64 = self.pre_trusted.values().sum();
            if sum > 0.0 {
                for (peer_id, &value) in &self.pre_trusted {
                    if let Some(&idx) = peer_to_index.get(peer_id) {
                        t[idx] = value / sum;
                    }
                }
            }
        } else {
            let uniform_value = 1.0 / n as f64;
            t.fill(uniform_value);
        }

        let c = self.normalize_local_trust().await?;

        let p = if !self.pre_trusted.is_empty() {
            let sum: f64 = self.pre_trusted.values().sum();
            let mut p_vec = vec![0.0; n];

            if sum > 0.0 {
                for (peer_id, &value) in &self.pre_trusted {
                    if let Some(&idx) = peer_to_index.get(peer_id) {
                        p_vec[idx] = value / sum;
                    }
                }
            } else {
                let uniform_value = 1.0 / n as f64;
                p_vec.fill(uniform_value);
            }

            p_vec
        } else {
            vec![1.0 / n as f64; n]
        };

        let alpha = 0.1;

        let mut iterations = 0;
        loop {
            let mut t_new = vec![0.0; n];

            for i in 0..n {
                let peer_i = index_to_peer[&i];

                let mut sum = 0.0;
                for j in 0..n {
                    let peer_j = index_to_peer[&j];
                    let c_ji = *c.get(&(peer_j, peer_i)).unwrap_or(&0.0);
                    sum += c_ji * t[j];
                }

                t_new[i] = (1.0 - alpha) * sum + alpha * p[i];
            }

            let diff = (0..n)
                .map(|i| (t_new[i] - t[i]).abs())
                .fold(0.0, |max, val| if val > max { val } else { max });

            t = t_new;
            iterations += 1;

            if diff < self.epsilon || iterations >= self.max_iterations {
                break;
            }
        }

        println!("Converged after {} iterations", iterations);

        let mut result = HashMap::new();
        for i in 0..n {
            let peer_id = index_to_peer[&i];
            result.insert(peer_id, t[i]);
        }

        Ok(result)
    }

    pub fn get_fetcher(&self) -> Option<&F> {
        Some(&self.trust_fetcher)
    }

    pub fn get_fetcher_mut(&mut self) -> Option<&mut F> {
        Some(&mut self.trust_fetcher)
    }

    pub fn get_local_trust(&self, node_id: &F::NodeId) -> Option<f64> {
        self.local_trust.get(node_id).copied()
    }
}
