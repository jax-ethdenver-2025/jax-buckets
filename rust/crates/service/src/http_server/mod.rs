use axum::body::Body;
use axum::extract::DefaultBodyLimit;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Router};
use rust_embed::RustEmbed;
use tokio::sync::watch;
use tower_http::trace::TraceLayer;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse};
use tower_http::LatencyUnit;

pub mod api;
mod config;
mod handlers;
mod health;
mod html;

pub use config::Config;

use crate::ServiceState;

const API_PREFIX: &str = "/api";
const STATUS_PREFIX: &str = "/_status";

#[derive(RustEmbed)]
#[folder = "static"]
struct StaticAssets;

async fn static_handler(uri: Uri) -> impl IntoResponse {
    let path = uri
        .path()
        .trim_start_matches('/')
        .trim_start_matches("static/");

    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data.to_vec()))
                .unwrap()
        }
        None => {
            // Serve 404.html if file not found
            match StaticAssets::get("404.html") {
                Some(content) => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .header(header::CONTENT_TYPE, "text/html")
                    .body(Body::from(content.data.to_vec()))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not Found"))
                    .unwrap(),
            }
        }
    }
}

/// Run the HTML server on port 8080
pub async fn run_html(
    config: Config,
    state: ServiceState,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<(), HttpServerError> {
    let listen_addr = config.listen_addr;
    let log_level = config.log_level;
    let trace_layer = TraceLayer::new_for_http()
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(log_level)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    tracing::info!("Static files embedded in binary");

    let html_router = Router::new()
        .nest(STATUS_PREFIX, health::router(state.clone()))
        .route("/static/*path", axum::routing::get(static_handler))
        .merge(html::router(state.clone()))
        .fallback(handlers::not_found_handler)
        .layer(Extension(config.clone()))
        .with_state(state)
        .layer(trace_layer);

    tracing::info!(addr = ?listen_addr, "HTML server listening");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, html_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

/// Run the API server on port 3000
pub async fn run_api(
    config: Config,
    state: ServiceState,
    mut shutdown_rx: watch::Receiver<()>,
) -> Result<(), HttpServerError> {
    let listen_addr = config.listen_addr;
    let log_level = config.log_level;
    let trace_layer = TraceLayer::new_for_http()
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(log_level)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    let api_router = Router::new()
        .nest(STATUS_PREFIX, health::router(state.clone()))
        .nest(API_PREFIX, api::router(state.clone()))
        .fallback(handlers::not_found_handler)
        .layer(DefaultBodyLimit::max(500 * 1024 * 1024)) // 500MB limit for file uploads
        .with_state(state)
        .layer(trace_layer);

    tracing::info!(addr = ?listen_addr, "API server listening");
    let listener = tokio::net::TcpListener::bind(listen_addr).await?;

    axum::serve(listener, api_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum HttpServerError {
    #[error("an error occurred running the HTTP server: {0}")]
    ServingFailed(#[from] std::io::Error),
}
