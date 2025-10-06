use askama::Template;
use askama_axum::IntoResponse;
use tracing::instrument;

#[derive(Template)]
#[template(path = "blobs.html")]
pub struct BlobsTemplate {
    pub blobs: Vec<String>,
}

#[instrument]
pub async fn handler() -> askama_axum::Response {
    let template = BlobsTemplate {
        blobs: vec![
            "blob1".to_string(),
            "blob2".to_string(),
            "blob3".to_string(),
        ],
    };

    template.into_response()
}
