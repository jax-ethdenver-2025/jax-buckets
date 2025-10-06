use axum::response::{IntoResponse, Response};
use axum::Json;
use http::StatusCode;

use common::prelude::build_info;

#[tracing::instrument]
pub async fn handler() -> Response {
    (StatusCode::OK, Json(build_info())).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handler_direct() {
        let response = handler().await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
