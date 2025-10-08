use super::*;

#[tokio::test]
async fn test_basic_trust_computation() {
    let fetcher = mock::MockTrustFetcher::new();
    let mut eigentrust = eigen_trust::EigenTrust::new(fetcher);

    // Set local trust values for node 0
    eigentrust
        .add_local_trust(0, 1.0)
        .add_local_trust(1, 1.0)
        .add_local_trust(2, 0.5)
        .add_local_trust(3, 0.0);

    // Add pre-trusted peers
    eigentrust.add_pre_trusted(0, 1.0);

    let global_trust = eigentrust.compute_global_trust().await.unwrap();

    // Verify trust values are normalized
    let sum: f64 = global_trust.values().sum();
    assert!((sum - 1.0).abs() < 0.0001);

    // Verify expected trust relationships
    assert!(global_trust[&1] > global_trust[&2]); // Node 1 should be more trusted than 2
    assert!(global_trust[&2] > global_trust[&3]); // Node 2 should be more trusted than 3
}

#[tokio::test]
async fn test_peer_discovery() {
    let fetcher = mock::MockTrustFetcher::new();
    let mut eigentrust = eigen_trust::EigenTrust::new(fetcher);

    // Initial peer set should only contain node 0
    // add 0 to the peer set
    eigentrust.add_peer(0);
    assert_eq!(eigentrust.get_peers().len(), 1);
    assert!(eigentrust.get_peers().contains(&0));

    // Discover peers up to 2 hops away
    eigentrust.discover_peers(2).await.unwrap();

    // Verify discovered peers
    let peers = eigentrust.get_peers();
    assert!(peers.contains(&1));
    assert!(peers.contains(&2));
    assert!(peers.contains(&3));
    assert!(peers.contains(&4)); // Should discover extended network
}

#[tokio::test]
async fn test_peer_removal() {
    let fetcher = mock::MockTrustFetcher::new();
    let mut eigentrust = eigen_trust::EigenTrust::new(fetcher);

    // Add some peers and trust values
    eigentrust
        .add_local_trust(0, 1.0)
        .add_local_trust(1, 1.0)
        .add_local_trust(2, 0.5)
        .add_local_trust(3, 0.3);

    // Remove peer 2
    eigentrust.remove_peer(2);

    // Verify peer 2 is removed
    assert!(!eigentrust.get_peers().contains(&2));

    // Compute trust and verify peer 2 is not in results
    let global_trust = eigentrust.compute_global_trust().await.unwrap();
    assert!(!global_trust.contains_key(&2));
}
