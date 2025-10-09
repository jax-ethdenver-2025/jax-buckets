use uuid::Uuid;

use crate::ServiceState;

use super::error::MountOpsError;
use super::load_mount::load_mount_for_bucket;

#[derive(Debug, Clone)]
pub struct FileContent {
    pub data: Vec<u8>,
    pub mime_type: String,
}

/// Get file content from a bucket
pub async fn get_file_content(
    bucket_id: Uuid,
    path: String,
    state: &ServiceState,
) -> Result<FileContent, MountOpsError> {
    let mount = load_mount_for_bucket(bucket_id, state).await?;

    let path_buf = std::path::PathBuf::from(&path);
    if !path_buf.is_absolute() {
        return Err(MountOpsError::InvalidPath("Path must be absolute".into()));
    }

    let blobs = state.node().blobs();
    let blobs_clone = blobs.clone();
    let path_buf_clone = path_buf.clone();

    // Read file and get node info in blocking task
    let (data, mime_type) = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(async {
            // Get file data
            let data = mount.cat(&path_buf_clone, &blobs_clone).await?;

            // Get node link to extract MIME type
            let node_link = mount.get(&path_buf_clone, &blobs_clone).await?;
            let mime_type = node_link
                .data()
                .and_then(|data| data.mime())
                .map(|mime| mime.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            Ok::<(Vec<u8>, String), common::prelude::MountError>((data, mime_type))
        })
    })
    .await
    .map_err(|e| MountOpsError::Mount(common::prelude::MountError::Default(anyhow::anyhow!(e))))??;

    Ok(FileContent { data, mime_type })
}
