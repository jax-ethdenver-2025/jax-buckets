use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Extension;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use common::bucket::BucketData;
use common::linked_data::BlockEncoded;

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
#[template(path = "bucket_explorer.html")]
pub struct BucketExplorerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub bucket_link: String,
    pub bucket_link_short: String,
    pub previous_link: Option<String>,
    pub previous_link_full: String,
    pub previous_link_short: String,
    pub bucket_data_formatted: String,
    pub current_path: String,
    pub path_segments: Vec<PathSegment>,
    pub parent_path_url: String,
    pub items: Vec<FileDisplayInfo>,
    pub read_only: bool,
    pub api_url: String,
    pub sync_status: String,
    pub sync_status_class: String,
    pub sync_error: String,
}

#[derive(Debug, Clone)]
pub struct PathSegment {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct FileDisplayInfo {
    pub name: String,
    pub path: String,
    pub link: String,
    pub is_dir: bool,
    pub mime_type: String,
}

#[derive(Debug, Deserialize)]
pub struct ExplorerQuery {
    #[serde(default)]
    pub path: Option<String>,
}

#[instrument(skip(state, config))]
pub async fn handler(
    State(state): State<ServiceState>,
    Extension(config): Extension<Config>,
    headers: HeaderMap,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<ExplorerQuery>,
) -> askama_axum::Response {
    // Check if request host matches configured hostname
    let read_only = if let Some(host_header) = headers.get("host") {
        if let Ok(host_str) = host_header.to_str() {
            let config_host = config.hostname.host_str().unwrap_or("localhost");
            let config_port = config.hostname.port().unwrap_or(8080);
            let expected_host = format!("{}:{}", config_host, config_port);
            host_str != expected_host && host_str != "localhost:8080"
        } else {
            true
        }
    } else {
        true
    };

    let current_path = query.path.unwrap_or_else(|| "/".to_string());

    // Get bucket info using mount_ops
    let bucket = match mount_ops::get_bucket_info(bucket_id, &state).await {
        Ok(bucket) => bucket,
        Err(e) => return error_response(&format!("{}", e)),
    };

    // List bucket contents
    let items =
        match mount_ops::list_bucket_contents(bucket_id, Some(current_path.clone()), false, &state)
            .await
        {
            Ok(items) => items,
            Err(e) => {
                tracing::error!("Failed to list bucket contents: {}", e);
                return error_response("Failed to load bucket contents");
            }
        };

    // Build path segments for breadcrumb
    let path_segments = build_path_segments(&current_path);

    // Build parent path URL
    let parent_path_url = build_parent_path_url(&current_path, &bucket_id);

    // Convert to display format
    let display_items: Vec<FileDisplayInfo> = items
        .into_iter()
        .map(|item| FileDisplayInfo {
            name: item.name,
            path: item.path,
            link: item.link.hash().to_string(),
            is_dir: item.is_dir,
            mime_type: item.mime_type,
        })
        .collect();

    let api_url = config
        .api_url
        .clone()
        .unwrap_or_else(|| "http://localhost:3000".to_string());

    let (status_text, status_class) = status_badge_class(&bucket.sync_status);

    // Format bucket link for display
    let bucket_link = bucket.link.hash().to_string();
    let bucket_link_short = if bucket_link.len() > 16 {
        format!(
            "{}...{}",
            &bucket_link[..8],
            &bucket_link[bucket_link.len() - 8..]
        )
    } else {
        bucket_link.clone()
    };

    // Load the full bucket data from blobs to get previous link and format it
    let blobs = state.node().blobs();
    let (previous_link, previous_link_full, previous_link_short, bucket_data_formatted) =
        match blobs.get(bucket.link.hash()).await {
            Ok(data) => match BucketData::decode(&data) {
                Ok(bucket_data) => {
                    // Format bucket data as pretty JSON
                    let formatted = serde_json::to_string_pretty(&bucket_data)
                        .unwrap_or_else(|_| format!("{:#?}", bucket_data));

                    // Extract previous link if it exists
                    let (prev_opt, prev_full, prev_short) =
                        if let Some(prev) = bucket_data.previous() {
                            let prev_hash = prev.hash().to_string();
                            let prev_short = if prev_hash.len() > 16 {
                                format!(
                                    "{}...{}",
                                    &prev_hash[..8],
                                    &prev_hash[prev_hash.len() - 8..]
                                )
                            } else {
                                prev_hash.clone()
                            };
                            (Some(prev_hash.clone()), prev_hash, prev_short)
                        } else {
                            (None, String::new(), String::new())
                        };

                    (prev_opt, prev_full, prev_short, formatted)
                }
                Err(e) => {
                    tracing::warn!("Failed to decode bucket data: {}", e);
                    (
                        None,
                        String::new(),
                        String::new(),
                        format!("Error decoding bucket data: {}", e),
                    )
                }
            },
            Err(e) => {
                tracing::warn!("Failed to load bucket data from blobs: {}", e);
                (
                    None,
                    String::new(),
                    String::new(),
                    format!("Error loading bucket data: {}", e),
                )
            }
        };

    let template = BucketExplorerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        bucket_link,
        bucket_link_short,
        previous_link,
        previous_link_full,
        previous_link_short,
        bucket_data_formatted,
        current_path,
        path_segments,
        parent_path_url,
        items: display_items,
        read_only,
        api_url,
        sync_status: status_text.to_string(),
        sync_status_class: status_class.to_string(),
        sync_error: bucket.sync_error.unwrap_or_default(),
    };

    template.into_response()
}

fn build_path_segments(path: &str) -> Vec<PathSegment> {
    if path == "/" {
        return vec![];
    }

    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut segments = Vec::new();
    let mut accumulated = String::new();

    for part in parts {
        accumulated.push('/');
        accumulated.push_str(part);
        segments.push(PathSegment {
            name: part.to_string(),
            path: accumulated.clone(),
        });
    }

    segments
}

fn build_parent_path_url(current_path: &str, bucket_id: &Uuid) -> String {
    if current_path == "/" {
        return format!("/buckets/{}", bucket_id);
    }

    let parent = std::path::Path::new(current_path)
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("/");

    if parent == "/" {
        format!("/buckets/{}", bucket_id)
    } else {
        format!("/buckets/{}?path={}", bucket_id, parent)
    }
}

fn error_response(message: &str) -> askama_axum::Response {
    (
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        format!("Error: {}", message),
    )
        .into_response()
}
