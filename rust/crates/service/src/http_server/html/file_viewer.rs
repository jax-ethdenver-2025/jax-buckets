use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::{Path, Query, State};
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

use crate::mount_ops;
use crate::ServiceState;

#[derive(Template)]
#[template(path = "file_viewer.html")]
pub struct FileViewerTemplate {
    pub bucket_id: String,
    pub bucket_name: String,
    pub file_path: String,
    pub file_name: String,
    pub file_size: usize,
    pub mime_type: String,
    pub is_text: bool,
    pub content: String,
    pub back_url: String,
}

#[derive(Debug, Deserialize)]
pub struct ViewerQuery {
    pub path: String,
}

#[instrument(skip(state))]
pub async fn handler(
    State(state): State<ServiceState>,
    Path(bucket_id): Path<Uuid>,
    Query(query): Query<ViewerQuery>,
) -> askama_axum::Response {
    let file_path = query.path;

    // Get bucket info using mount_ops
    let bucket = match mount_ops::get_bucket_info(bucket_id, &state).await {
        Ok(bucket) => bucket,
        Err(e) => return error_response(&format!("{}", e)),
    };

    // Get file content
    let file_content = match mount_ops::get_file_content(bucket_id, file_path.clone(), &state).await {
        Ok(content) => content,
        Err(e) => {
            tracing::error!("Failed to get file content: {}", e);
            return error_response("Failed to load file content");
        }
    };

    // Extract file name
    let file_name = std::path::Path::new(&file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(&file_path)
        .to_string();

    // Determine how to display content based on MIME type
    let (is_text, content) = if file_content.mime_type.starts_with("image/")
        || file_content.mime_type.starts_with("video/")
        || file_content.mime_type.starts_with("audio/")
    {
        // Encode as base64 for embedded display
        (false, base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &file_content.data))
    } else {
        // Try to decode as UTF-8 text
        match String::from_utf8(file_content.data.clone()) {
            Ok(text) => (true, text),
            Err(_) => {
                // Binary content - show hex dump
                let hex = file_content.data
                    .chunks(16)
                    .enumerate()
                    .map(|(i, chunk)| {
                        let hex_part: String = chunk
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect::<Vec<_>>()
                            .join(" ");
                        let ascii_part: String = chunk
                            .iter()
                            .map(|&b| {
                                if b.is_ascii_graphic() || b == b' ' {
                                    b as char
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        format!("{:08x}  {:47}  |{}|", i * 16, hex_part, ascii_part)
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (false, hex)
            }
        }
    };

    // Build back URL (parent directory)
    let back_url = build_back_url(&file_path, &bucket_id);

    let template = FileViewerTemplate {
        bucket_id: bucket_id.to_string(),
        bucket_name: bucket.name,
        file_path,
        file_name,
        file_size: file_content.data.len(),
        mime_type: file_content.mime_type,
        is_text,
        content,
        back_url,
    };

    template.into_response()
}

fn build_back_url(file_path: &str, bucket_id: &Uuid) -> String {
    let parent = std::path::Path::new(file_path)
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
