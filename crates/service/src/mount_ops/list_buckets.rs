use crate::database::models::Bucket as BucketModel;
use crate::ServiceState;

use super::error::MountOpsError;
use super::types::BucketInfo;

/// List all buckets from the database
pub async fn list_buckets(state: &ServiceState) -> Result<Vec<BucketInfo>, MountOpsError> {
    let buckets = BucketModel::list(None, None, state.database())
        .await
        .map_err(|e| MountOpsError::Database(e.to_string()))?;

    Ok(buckets
        .into_iter()
        .map(|b| BucketInfo {
            bucket_id: b.id,
            name: b.name,
            link: b.link.into(),
            created_at: b.created_at,
            sync_status: b.sync_status,
            last_sync_attempt: b.last_sync_attempt,
            sync_error: b.sync_error,
        })
        .collect())
}
