use common::crypto::PublicKey;
use common::prelude::{Link, Mount};
use uuid::Uuid;

use crate::database::models::Bucket as BucketModel;
use crate::sync_manager::SyncEvent;
use crate::ServiceState;

use super::error::MountOpsError;

/// Share a bucket with a peer by adding them to the bucket's shares
/// Returns the new bucket link after adding the share
pub async fn share_bucket(
    bucket_id: Uuid,
    peer_public_key: PublicKey,
    state: &ServiceState,
) -> Result<Link, MountOpsError> {
    // Get bucket from database
    let bucket = BucketModel::get_by_id(&bucket_id, state.database())
        .await
        .map_err(|e| MountOpsError::Database(e.to_string()))?
        .ok_or(MountOpsError::BucketNotFound(bucket_id))?;

    // Load mount
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    let mut mount = Mount::load(&bucket_link, secret_key, blobs)
        .await
        .map_err(MountOpsError::Mount)?;

    mount.share(peer_public_key).await?;

    let new_bucket_link = mount.save(blobs).await?;

    // Update bucket link in database
    bucket
        .update_link(new_bucket_link.clone(), state.database())
        .await
        .map_err(|e| MountOpsError::Database(e.to_string()))?;

    // Trigger push sync to announce the new share to all peers
    tracing::debug!(
        "Triggering push sync for bucket {} after adding share",
        bucket_id
    );
    if let Err(e) = state.send_sync_event(SyncEvent::Push {
        bucket_id,
        new_link: new_bucket_link.clone(),
    }) {
        tracing::warn!(
            "Failed to trigger push sync for bucket {}: {:?}",
            bucket_id,
            e
        );
        // Don't fail the request if sync event fails - the share was added successfully
    }

    Ok(new_bucket_link)
}
