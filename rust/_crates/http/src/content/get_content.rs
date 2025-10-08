use axum::extract::{Json, Path as AxumPath, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use image::{imageops::FilterType, ImageFormat};
use regex::Regex;
use std::io::Cursor;
use std::path::Path;
use std::path::PathBuf;
use url::Url;

use leaky_common::prelude::*;

use crate::app::AppState;

const MAX_WIDTH: u32 = 300;
const MAX_HEIGHT: u32 = 300;

#[derive(Debug, serde::Deserialize)]
pub struct GetContentQuery {
    pub html: Option<bool>,
    pub thumbnail: Option<bool>,
    pub deep: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
struct Item {
    cid: String,
    path: String,
    is_dir: bool,
    object: Option<Object>,
}

#[derive(Debug, serde::Serialize)]
struct LsResponse(Vec<Item>);

#[axum::debug_handler]
pub async fn handler(
    State(state): State<AppState>,
    AxumPath(path): AxumPath<PathBuf>,
    Query(query): Query<GetContentQuery>,
) -> Result<impl IntoResponse, GetContentError> {
    let path_clone = path.clone();
    tracing::info!(
        "Starting content request for path: {:?} with query: {:?}",
        path_clone,
        query
    );

    let mount = state.mount_for_reading();
    let path = PathBuf::from("/").join(path);
    tracing::debug!("Absolute path: {:?}", path);

    let ls_result = if query.deep.unwrap_or(false) {
        // For deep listing
        match mount.ls_deep(&path).await {
            Ok((links, _schemas)) => {
                // Convert to same format as regular ls
                Ok((links, None))
            }
            Err(e) => Err(e),
        }
    } else {
        // For regular listing
        mount.ls(&path).await
    };

    tracing::debug!("ls result for {:?}: {:?}", path, ls_result);

    match ls_result {
        Ok((ls, _)) => {
            if !ls.is_empty() {
                tracing::debug!(
                    "Found directory listing with {} items for path: {:?}",
                    ls.len(),
                    path
                );
                return Ok((
                    http::StatusCode::OK,
                    [(CONTENT_TYPE, "application/json")],
                    Json(LsResponse(
                        ls.into_iter()
                            .map(|(path, link)| Item {
                                cid: link.cid().to_string(),
                                path: path.to_str().unwrap().to_string(),
                                is_dir: match link {
                                    NodeLink::Node(_) => true,
                                    NodeLink::Data(_, _) => false,
                                },
                                object: match link {
                                    NodeLink::Node(_) => None,
                                    NodeLink::Data(_, object) => object,
                                },
                            })
                            .collect(),
                    )),
                )
                    .into_response());
            }
            tracing::debug!("Empty directory listing for path: {:?}", path);
        }
        Err(MountError::PathNotDir(_)) => {
            tracing::debug!("Path is not a directory: {:?}", path);
        }
        Err(MountError::PathNotFound(_)) => {
            tracing::debug!("Path not found: {:?}", path);
            return Err(GetContentError::NotFound);
        }
        Err(e) => {
            tracing::error!("Mount error for path {:?}: {:?}", path, e);
            return Err(GetContentError::Mount(e));
        }
    }

    let ext = path
        .extension()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default();
    tracing::debug!(
        "Processing file with extension: {} for path: {:?}",
        ext,
        path
    );

    match ext {
        "md" => {
            tracing::debug!("Processing markdown file: {:?}", path);
            if query.html.unwrap_or(false) {
                let base_path = path.parent().unwrap_or_else(|| Path::new(""));
                let get_content_url = state.get_content_forwarding_url().join("content").unwrap();

                let data = {
                    let future = mount.cat(&path);
                    future.await
                }
                .map_err(|_| GetContentError::NotFound)?;

                let html = markdown_to_html(data, base_path, &get_content_url);
                Ok((http::StatusCode::OK, [(CONTENT_TYPE, "text/html")], html).into_response())
            } else {
                let data = {
                    let future = mount.cat(&path);
                    future.await
                }
                .map_err(|_| GetContentError::NotFound)?;

                Ok((http::StatusCode::OK, [(CONTENT_TYPE, "text/plain")], data).into_response())
            }
        }
        "png" | "jpg" | "jpeg" | "gif" => {
            tracing::debug!(
                "Processing image file: {:?} with thumbnail: {:?}",
                path,
                query.thumbnail
            );
            let data = {
                let future = mount.cat(&path);
                future.await
            }
            .map_err(|_| GetContentError::NotFound)?;
            if query.thumbnail.unwrap_or(false) && ext != "gif" {
                let resized_image = resize_image(&data, ext)?;
                Ok((
                    http::StatusCode::OK,
                    [(CONTENT_TYPE, format!("image/{}", ext))],
                    resized_image,
                )
                    .into_response())
            } else {
                Ok((
                    http::StatusCode::OK,
                    [(CONTENT_TYPE, format!("image/{}", ext))],
                    data,
                )
                    .into_response())
            }
        }
        _ => {
            tracing::debug!("Processing generic file: {:?}", path);
            let data = {
                let future = mount.cat(&path);
                future.await
            }
            .map_err(|e| {
                tracing::error!("Failed to read file {:?}: {:?}", path, e);
                GetContentError::NotFound
            })?;
            tracing::debug!("Successfully read {} bytes from {:?}", data.len(), path);
            Ok((
                http::StatusCode::OK,
                [(CONTENT_TYPE, "application/octet-stream")],
                data,
            )
                .into_response())
        }
    }
}

fn resize_image(img_data: &[u8], format: &str) -> Result<Vec<u8>, GetContentError> {
    let img = image::load_from_memory(img_data)
        .map_err(|e| GetContentError::ImageProcessing(e.to_string()))?;

    let (width, height) = calculate_dimensions(img.width(), img.height());
    let resized = img.resize(width, height, FilterType::Lanczos3);

    let mut cursor = Cursor::new(Vec::new());
    let format = match format {
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        _ => return Err(GetContentError::UnsupportedImageFormat),
    };

    resized
        .write_to(&mut cursor, format)
        .map_err(|e| GetContentError::ImageProcessing(e.to_string()))?;

    Ok(cursor.into_inner())
}

fn calculate_dimensions(width: u32, height: u32) -> (u32, u32) {
    let aspect_ratio = width as f32 / height as f32;
    if width > height {
        let new_width = MAX_WIDTH.min(width);
        let new_height = (new_width as f32 / aspect_ratio) as u32;
        (new_width, new_height)
    } else {
        let new_height = MAX_HEIGHT.min(height);
        let new_width = (new_height as f32 * aspect_ratio) as u32;
        (new_width, new_height)
    }
}

pub fn markdown_to_html(data: Vec<u8>, base_path: &Path, get_content_url: &Url) -> String {
    let content = String::from_utf8(data).unwrap();

    let mut options = pulldown_cmark::Options::empty();
    options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);
    options.insert(pulldown_cmark::Options::ENABLE_TABLES);
    options.insert(pulldown_cmark::Options::ENABLE_FOOTNOTES);
    options.insert(pulldown_cmark::Options::ENABLE_SMART_PUNCTUATION);
    options.insert(pulldown_cmark::Options::ENABLE_TASKLISTS);

    let parser = pulldown_cmark::Parser::new_ext(&content, options);
    let mut html = String::new();
    pulldown_cmark::html::push_html(&mut html, parser);

    let re = Regex::new(r#"src="./([^"]+)"#).unwrap();
    let mut result = html.clone();

    for caps in re.captures_iter(&html) {
        if let Some(cap) = caps.get(1) {
            let path = PathBuf::from(cap.as_str());
            let path = normalize_path(base_path.join(path));
            let url = get_content_url.join(path.to_str().unwrap()).unwrap();
            let old = format!(r#"src="./{}""#, cap.as_str());
            let new = format!(r#"src="{}""#, url);
            result = result.replace(&old, &new);
        }
    }

    result
}

fn normalize_path(path: PathBuf) -> PathBuf {
    let mut normalized_path = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::ParentDir => {
                normalized_path.pop();
            }
            _ => {
                normalized_path.push(component);
            }
        }
    }
    normalized_path
}

#[derive(Debug, thiserror::Error)]
pub enum GetContentError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("root CID error: {0}")]
    RootCid(#[from] crate::database::models::RootCidError),
    #[error("not found")]
    NotFound,
    #[error("mount error: {0}")]
    Mount(#[from] MountError),
    #[error("Image processing error: {0}")]
    ImageProcessing(String),
    #[error("Unsupported image format")]
    UnsupportedImageFormat,
    #[error("Failed to acquire semaphore: {0}")]
    Semaphore(#[from] tokio::sync::AcquireError),
}

impl IntoResponse for GetContentError {
    fn into_response(self) -> Response {
        match self {
            GetContentError::Mount(_)
            | GetContentError::RootCid(_)
            | GetContentError::Database(_)
            | GetContentError::ImageProcessing(_) => {
                tracing::error!("{:?}", self);
                (
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    [(CONTENT_TYPE, "text/plain")],
                    "Internal server error",
                )
                    .into_response()
            }
            GetContentError::NotFound => (
                http::StatusCode::NOT_FOUND,
                [(CONTENT_TYPE, "text/plain")],
                "Not found",
            )
                .into_response(),
            GetContentError::UnsupportedImageFormat => (
                http::StatusCode::UNSUPPORTED_MEDIA_TYPE,
                [(CONTENT_TYPE, "text/plain")],
                "Unsupported image format",
            )
                .into_response(),
            GetContentError::Semaphore(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                [(CONTENT_TYPE, "text/plain")],
                "Failed to acquire semaphore",
            )
                .into_response(),
        }
    }
}
