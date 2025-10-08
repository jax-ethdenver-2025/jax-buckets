use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use axum_extra::headers::ContentType;
use axum_extra::TypedHeader;

pub async fn not_found_handler(content_type: Option<TypedHeader<ContentType>>) -> Response {
    match content_type {
        Some(TypedHeader(content_type)) => {
            let content_type = content_type.to_string();
            match content_type.as_str() {
                "application/json" => {
                    let err_msg = serde_json::json!({"msg": "not found"});
                    (StatusCode::NOT_FOUND, Json(err_msg)).into_response()
                }
                "text/html" => {
                    let body = "<h1>Not Found</h1>";
                    (
                        StatusCode::NOT_FOUND,
                        [(axum::http::header::CONTENT_TYPE, "text/html")],
                        body,
                    )
                        .into_response()
                }
                _ => (
                    StatusCode::NOT_FOUND,
                    [(axum::http::header::CONTENT_TYPE, "text/plain")],
                    "not found",
                )
                    .into_response(),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            [(axum::http::header::CONTENT_TYPE, "text/plain")],
            "not found",
        )
            .into_response(),
    }
}
