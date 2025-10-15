mod utils;

use std::net::SocketAddr;
use std::str::FromStr;
use std::time::Duration;

use futures::future::join_all;
use tokio::time::timeout;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

use crate::http_server;
use crate::sync_coordinator::{SyncCoordinator, SyncEvent};
use crate::{ServiceConfig, ServiceState};

pub async fn spawn_service(service_config: &ServiceConfig) {
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let env_filter = EnvFilter::builder()
        .with_default_directive(service_config.log_level.into())
        .from_env_lossy();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(non_blocking_writer)
        .with_filter(env_filter);

    tracing_subscriber::registry().with(stderr_layer).init();

    utils::register_panic_logger();
    utils::report_build_info();

    let (graceful_waiter, shutdown_rx) = utils::graceful_shutdown_blocker();

    // Create sync channel first
    let (sync_sender, sync_receiver) = flume::unbounded::<SyncEvent>();

    // Create announce callback to send sync events
    let announce_sync_sender = sync_sender.clone();
    let announce_callback =
        std::sync::Arc::new(move |bucket_id, peer_id, new_link, previous_link| {
            let _ = announce_sync_sender.send(SyncEvent::PeerAnnounce {
                bucket_id,
                peer_id,
                new_link,
                previous_link,
            });
        });

    // Create state with sync sender and announce callback
    let state = match ServiceState::from_config(
        service_config,
        sync_sender.clone(),
        Some(announce_callback),
    )
    .await
    {
        Ok(state) => std::sync::Arc::new(state),
        Err(e) => {
            tracing::error!("error creating server state: {}", e);
            std::process::exit(3);
        }
    };

    let mut handles = Vec::new();

    // Get listen addresses from config
    let html_listen_addr = service_config
        .html_listen_addr
        .unwrap_or_else(|| SocketAddr::from_str("0.0.0.0:8080").unwrap());
    let api_listen_addr = service_config
        .api_listen_addr
        .unwrap_or_else(|| SocketAddr::from_str("0.0.0.0:3000").unwrap());

    // Start HTML server
    let html_state = state.as_ref().clone();
    let api_url = service_config
        .api_hostname
        .clone()
        .unwrap_or_else(|| format!("http://localhost:{}", api_listen_addr.port()));
    let html_config =
        http_server::Config::new(html_listen_addr, Some(api_url), service_config.ui_read_only);
    let html_rx = shutdown_rx.clone();
    let html_handle = tokio::spawn(async move {
        tracing::info!("Starting HTML server on {}", html_listen_addr);
        if let Err(e) = http_server::run_html(html_config, html_state, html_rx).await {
            tracing::error!("HTML server error: {}", e);
        }
    });
    handles.push(html_handle);

    // Start API server
    let api_state = state.as_ref().clone();
    let api_config = http_server::Config::new(api_listen_addr, None, false);
    let api_rx = shutdown_rx.clone();
    let api_handle = tokio::spawn(async move {
        tracing::info!("Starting API server on {}", api_listen_addr);
        if let Err(e) = http_server::run_api(api_config, api_state, api_rx).await {
            tracing::error!("API server error: {}", e);
        }
    });
    handles.push(api_handle);

    // spawn a router for the node
    let node_state = state.clone();
    let node_rx = shutdown_rx.clone();
    let node_handle = tokio::spawn(async move {
        let node = node_state.node();
        tracing::info!("Starting node");
        if let Err(e) = node.spawn(node_rx).await {
            tracing::error!("Node error: {}", e);
        }
    });
    handles.push(node_handle);

    // Spawn sync coordinator
    let sync_coordinator = SyncCoordinator::new(state.peer().clone(), state.peer_state().clone());
    let sync_handle = tokio::spawn(async move {
        sync_coordinator.run(sync_receiver).await;
    });
    handles.push(sync_handle);

    // Spawn periodic sync checker
    let periodic_state = state.clone();
    let mut periodic_rx = shutdown_rx.clone();
    let periodic_handle = tokio::spawn(async move {
        use crate::database::models::Bucket as BucketModel;
        use tokio::time::{interval, Duration};

        let mut interval_timer = interval(Duration::from_secs(60)); // Check every 60 seconds
        interval_timer.tick().await; // Skip first immediate tick

        tracing::info!("Periodic sync checker started");

        loop {
            tokio::select! {
                _ = interval_timer.tick() => {
                    tracing::debug!("Running periodic sync check");

                    // Get all buckets
                    match BucketModel::list(None, None, periodic_state.database()).await {
                        Ok(buckets) => {
                            for bucket in buckets {
                                tracing::debug!("Checking sync for bucket {}", bucket.id);

                                // Trigger pull sync for each bucket
                                if let Err(e) = periodic_state.send_sync_event(SyncEvent::Pull {
                                    bucket_id: bucket.id,
                                }) {
                                    tracing::warn!("Failed to trigger periodic sync for bucket {}: {:?}", bucket.id, e);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Failed to list buckets for periodic sync: {}", e);
                        }
                    }
                }
                _ = periodic_rx.changed() => {
                    tracing::info!("Periodic sync checker shutting down");
                    break;
                }
            }
        }
    });
    handles.push(periodic_handle);

    let _ = graceful_waiter.await;

    if timeout(FINAL_SHUTDOWN_TIMEOUT, join_all(handles))
        .await
        .is_err()
    {
        tracing::error!(
            "Failed to shut down within {} seconds",
            FINAL_SHUTDOWN_TIMEOUT.as_secs()
        );
        std::process::exit(4);
    }
}
