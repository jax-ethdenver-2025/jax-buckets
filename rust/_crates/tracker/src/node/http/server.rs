use anyhow::Result;
use axum::Router;
use std::net::SocketAddr;
use tokio::sync::watch::Receiver as WatchReceiver;
use tower_http::trace::{DefaultOnFailure, DefaultOnResponse, TraceLayer};
use tower_http::LatencyUnit;

use crate::node::State as NodeState;

use super::api;
use super::html;
// use super::error_handlers;
use super::health;

const HTML_PREFIX: &str = "/";
const HEALTH_PREFIX: &str = "/_status";
const API_PREFIX: &str = "/api/v0";

pub async fn run(
    remote_listen_addr: SocketAddr,
    log_level: tracing::Level,
    state: NodeState,
    mut shutdown_rx: WatchReceiver<()>,
) -> Result<()> {
    let trace_layer = TraceLayer::new_for_http()
        .on_response(
            DefaultOnResponse::new()
                .include_headers(false)
                .level(log_level)
                .latency_unit(LatencyUnit::Micros),
        )
        .on_failure(DefaultOnFailure::new().latency_unit(LatencyUnit::Micros));

    let root_router = Router::new()
        .nest(HTML_PREFIX, html::router(state.clone()))
        .nest(API_PREFIX, api::router(state.clone()))
        .nest(HEALTH_PREFIX, health::router(state.clone()))
        .with_state(state)
        // .fallback(error_handlers::not_found_handler)
        .layer(trace_layer);

    tracing::info!(addr = ?remote_listen_addr, "server listening");
    let listener = tokio::net::TcpListener::bind(remote_listen_addr).await?;

    axum::serve(listener, root_router)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.changed().await;
        })
        .await?;

    Ok(())
}
