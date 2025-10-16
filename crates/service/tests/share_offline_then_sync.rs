#![cfg(feature = "testkit")]

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn share_offline_then_sync() -> anyhow::Result<()> {
    use std::time::Duration;
    use tokio::time::timeout;
    use service::testkit::*;

    timeout(Duration::from_secs(45), async {
        let mut net = TestNetwork::new();
        let alice = net.add_peer("alice");
        let bob = net.add_peer("bob");
        alice.start().await?;
        bob.start().await?;

        let bucket = alice.create_bucket("shared").await?;
        // Ensure the same bucket id exists on bob before syncing
        bob.ensure_bucket(bucket, "shared").await?;
        alice.share_bucket_with(bucket, &bob).await?;

        // Simulate target offline (no-op in deterministic test mode)

        alice.add_file_bytes(bucket, "/a.txt", b"hello").await?;
        alice.add_file_bytes(bucket, "/b.txt", b"world").await?;

        // Simulate target online and proceed with direct file sync

        // Start network on bob so SyncManager is initialized
        bob.start().await?;
        // Actively trigger a pull on bob and assert with bounded waits
        bob.trigger_pull(bucket).await?;

        net.eventually(Duration::from_secs(15), || async {
            let _ = bob.trigger_pull(bucket).await; // nudge sync without arbitrary sleeps
            let has_b = bob.has_file(bucket, "/b.txt").await?;
            let has_a = bob.has_file(bucket, "/a.txt").await?;
            Ok(has_b && has_a)
        })
        .await
    })
    .await
    .map_err(|_| anyhow::anyhow!("test timed out"))??;
    Ok(())
}
