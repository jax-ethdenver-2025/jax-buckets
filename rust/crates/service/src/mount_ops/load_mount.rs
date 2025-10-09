use common::prelude::{Link, Mount};
use uuid::Uuid;

use crate::database::models::Bucket as BucketModel;
use crate::ServiceState;

use super::error::MountOpsError;

/// Load a mount for a specific bucket
pub async fn load_mount_for_bucket(
    bucket_id: Uuid,
    state: &ServiceState,
) -> Result<Mount, MountOpsError> {
    // Get bucket from database
    let bucket = BucketModel::get_by_id(&bucket_id, state.database())
        .await
        .map_err(|e| MountOpsError::Database(e.to_string()))?
        .ok_or(MountOpsError::BucketNotFound(bucket_id))?;

    // Load mount
    let bucket_link: Link = bucket.link.into();
    let secret_key = state.node().secret();
    let blobs = state.node().blobs();

    Mount::load(&bucket_link, secret_key, blobs)
        .await
        .map_err(MountOpsError::Mount)
}
