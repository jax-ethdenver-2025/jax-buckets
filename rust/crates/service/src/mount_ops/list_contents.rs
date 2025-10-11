use uuid::Uuid;

use crate::ServiceState;

use super::error::MountOpsError;
use super::load_mount::load_mount_for_bucket;
use super::types::FileInfo;

/// List contents of a bucket at a specific path
pub async fn list_bucket_contents(
    bucket_id: Uuid,
    path: Option<String>,
    deep: bool,
    state: &ServiceState,
) -> Result<Vec<FileInfo>, MountOpsError> {
    let mount = load_mount_for_bucket(bucket_id, state).await?;

    let path_str = path.as_deref().unwrap_or("/");
    let path_buf = std::path::PathBuf::from(path_str);

    let blobs = state.node().blobs();
    let blobs_clone = blobs.clone();
    let path_buf_clone = path_buf.clone();

    // List items in blocking task
    let items = tokio::task::spawn_blocking(move || {
        tokio::runtime::Handle::current().block_on(async {
            if deep {
                mount.ls_deep(&path_buf_clone, &blobs_clone).await
            } else {
                mount.ls(&path_buf_clone, &blobs_clone).await
            }
        })
    })
    .await
    .map_err(|e| {
        MountOpsError::Mount(common::prelude::MountError::Default(anyhow::anyhow!(e)))
    })??;

    // Convert to FileInfo - paths from mount are relative, make them absolute
    Ok(items
        .into_iter()
        .map(|(path, node_link)| {
            // Mount returns relative paths, prepend "/" to make them absolute
            let absolute_path = std::path::Path::new("/").join(&path);
            let path_str = absolute_path.to_string_lossy().to_string();
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());

            let mime_type = if node_link.is_dir() {
                "inode/directory".to_string()
            } else {
                // Get MIME type from node data if available
                node_link
                    .data()
                    .and_then(|data| data.mime())
                    .map(|mime| mime.to_string())
                    .unwrap_or_else(|| "application/octet-stream".to_string())
            };

            FileInfo {
                path: path_str,
                name,
                link: node_link.link().clone(),
                is_dir: node_link.is_dir(),
                mime_type,
            }
        })
        .collect())
}
