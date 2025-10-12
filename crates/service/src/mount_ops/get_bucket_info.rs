use uuid::Uuid;

use crate::ServiceState;

use super::error::MountOpsError;
use super::types::BucketInfo;

/// Get information about a specific bucket by ID
pub async fn get_bucket_info(
    bucket_id: Uuid,
    state: &ServiceState,
) -> Result<BucketInfo, MountOpsError> {
    let buckets = super::list_buckets(state).await?;

    buckets
        .into_iter()
        .find(|b| b.bucket_id == bucket_id)
        .ok_or(MountOpsError::BucketNotFound(bucket_id))
}
