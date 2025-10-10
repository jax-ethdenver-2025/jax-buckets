use uuid::Uuid;

use crate::ServiceState;

use super::error::MountOpsError;

#[derive(Debug, Clone)]
pub struct ShareInfo {
    pub public_key: String,
    pub role: String,
    pub root_link: String,
}

/// Get all shares (peers) for a bucket
pub async fn get_bucket_shares(
    bucket_id: Uuid,
    state: &ServiceState,
) -> Result<Vec<ShareInfo>, MountOpsError> {
    // Load the mount for this bucket
    let bucket_mount = super::load_mount::load_mount_for_bucket(bucket_id, state).await?;

    // Get the bucket data from the mount
    let inner = bucket_mount.inner();
    let bucket_data = inner.bucket_data();

    // Convert shares to ShareInfo
    let shares: Vec<ShareInfo> = bucket_data
        .shares()
        .values()
        .map(|share| ShareInfo {
            public_key: share.principal().identity.to_hex(),
            role: format!("{:?}", share.principal().role),
            root_link: share.root().hash().to_string(),
        })
        .collect();

    Ok(shares)
}
