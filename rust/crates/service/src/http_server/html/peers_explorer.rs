use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, State};
use tracing::instrument;
use uuid::Uuid;

use crate::mount_ops;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "peers_explorer.html")]
pub struct PeersExplorerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub peers: Vec<PeerInfo>,
    pub total_peers: usize,
}

#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub public_key: String,
    pub role: String,
    pub root_link: String,
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
) -> askama_axum::Response {
    // Get bucket info
    let bucket = match mount_ops::get_bucket_info(bucket_id, &state).await {
        Ok(bucket) => bucket,
        Err(e) => return error_response(&format!("Failed to load bucket: {}", e)),
    };

    // Get bucket shares using mount_ops
    let shares = match mount_ops::get_bucket_shares(bucket_id, &state).await {
        Ok(shares) => shares,
        Err(e) => {
            tracing::error!("Failed to get bucket shares: {}", e);
            return error_response("Failed to load bucket shares");
        }
    };

    // Convert to display format
    let peers: Vec<PeerInfo> = shares
        .into_iter()
        .map(|share| PeerInfo {
            public_key: share.public_key,
            role: share.role,
            root_link: share.root_link,
        })
        .collect();

    let total_peers = peers.len();

    let template = PeersExplorerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        peers,
        total_peers,
    };

    template.into_response()
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
