use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Extension;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::http_server::Config;
use crate::mount_ops;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "bucket_explorer.html")]
pub struct BucketExplorerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub current_path: String,
    pub path_segments: Vec<PathSegment>,
    pub parent_path_url: String,
    pub items: Vec<FileDisplayInfo>,
    pub read_only: bool,
    pub api_url: String,
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

    let template = BucketExplorerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        current_path,
        path_segments,
        parent_path_url,
        items: display_items,
        read_only,
        api_url,
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
