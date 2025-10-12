use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::Extension;
use tracing::instrument;

use crate::database::models::SyncStatus;
use crate::http_server::Config;
use crate::mount_ops;
use crate::ServiceState;

/// Get status badge styling for a given sync status
fn status_badge_class(status: &SyncStatus) -> (&'static str, &'static str) {
    match status {
        SyncStatus::Synced => ("Synced", "status-badge status-synced"),
        SyncStatus::OutOfSync => ("Out of Sync", "status-badge status-out-of-sync"),
        SyncStatus::Syncing => ("Syncing", "status-badge status-syncing"),
        SyncStatus::Failed => ("Failed", "status-badge status-failed"),
    }
}

#[derive(Template)]
#[template(path = "buckets.html")]
pub struct BucketsTemplate {
    pub buckets: Vec<BucketDisplayInfo>,
    pub read_only: bool,
    pub api_url: String,
}

#[derive(Debug, Clone)]
pub struct BucketDisplayInfo {
    pub bucket_id: String,
    pub name: String,
    pub created_at: String,
    pub sync_status: String,
    pub sync_status_class: String,
    pub last_sync_attempt: String,
    pub sync_error: String,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    _headers: HeaderMap,
) -> askama_axum::Response {
    // Use the read_only flag from config
    let read_only = config.read_only;

    // Load buckets from database using mount_ops
    let buckets = match mount_ops::list_buckets(&state).await {
        Ok(buckets) => buckets,
        Err(e) => {
            tracing::error!("Failed to list buckets: {}", e);
            return error_response("Failed to load buckets");
        }
    };

    // Convert to display format
    let display_buckets: Vec<BucketDisplayInfo> = buckets
        .into_iter()
        .map(|b| {
            let (status_text, status_class) = status_badge_class(&b.sync_status);
            BucketDisplayInfo {
                bucket_id: b.bucket_id.to_string(),
                name: b.name,
                created_at: format_timestamp(b.created_at),
                sync_status: status_text.to_string(),
                sync_status_class: status_class.to_string(),
                last_sync_attempt: b
                    .last_sync_attempt
                    .map(format_timestamp)
                    .unwrap_or_else(|| "Never".to_string()),
                sync_error: b.sync_error.unwrap_or_default(),
            }
        })
        .collect();

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    let template = BucketsTemplate {
        buckets: display_buckets,
        read_only,
        api_url,
    };

    template.into_response()
}

fn format_timestamp(ts: time::OffsetDateTime) -> String {
    ts.format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| ts.to_string())
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
