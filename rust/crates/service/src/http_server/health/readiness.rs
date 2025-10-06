use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use std::time::Duration;
use tokio::time::timeout;

use super::data_source::*;

const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(10);

#[tracing::instrument]
pub async fn handler(data_src: StateDataSource) -> Response {
    match timeout(HEALTH_CHECK_TIMEOUT, data_src.is_ready()).await {
        Ok(result) => match result {
            Ok(_) => {
                let msg = serde_json::json!({"status": "ok"});
                (StatusCode::OK, Json(msg)).into_response()
            }
            Err(e) => handle_error(e),
        },
        Err(_) => {
            let msg = serde_json::json!({
                "status": "failure",
                "message": "health check timed out"
            });
            (StatusCode::SERVICE_UNAVAILABLE, Json(msg)).into_response()
        }
    }
}

fn handle_error(err: DataSourceError) -> Response {
    match err {
        DataSourceError::DependencyFailure => {
            let msg = serde_json::json!({"status": "failure", "message": "one or more dependencies aren't available"});
            (StatusCode::SERVICE_UNAVAILABLE, Json(msg)).into_response()
        }
        DataSourceError::ShuttingDown => {
            let msg =
                serde_json::json!({"status": "failure", "message": "service is shutting down"});
            (StatusCode::SERVICE_UNAVAILABLE, Json(msg)).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    use crate::http_server::health::data_source::tests::*;

    #[tokio::test]
    async fn test_handler_direct() {
        let response = handler(StateDataSource::new(Arc::new(MockReadiness::Ready))).await;
        assert_eq!(response.status(), StatusCode::OK);

        let response = handler(StateDataSource::new(Arc::new(
            MockReadiness::DependencyFailure,
        )))
        .await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let response = handler(StateDataSource::new(Arc::new(MockReadiness::ShuttingDown))).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }
}
