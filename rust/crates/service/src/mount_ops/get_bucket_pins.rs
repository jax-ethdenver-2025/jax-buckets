use uuid::Uuid;

use crate::ServiceState;

use super::error::MountOpsError;

/// Get all pinned hashes for a bucket
pub async fn get_bucket_pins(
    bucket_id: Uuid,
    state: &ServiceState,
) -> Result<Vec<common::linked_data::Hash>, MountOpsError> {
    // Load the mount for this bucket
    let bucket_mount = super::load_mount::load_mount_for_bucket(bucket_id, state).await?;

    // Get the pins from the mount
    let inner = bucket_mount.inner();
    let pins = inner.pins();

    // Convert to Vec for easier handling
    Ok(pins.to_vec())
}
