use anyhow::Result;
use iroh::{protocol::Router, Endpoint};
use tokio::sync::watch::Receiver as WatchReceiver;

use super::blobs_service::BlobsService;

const BLOBS_SERVICE_ALPN: &[u8] = iroh_blobs::ALPN;

pub async fn router(
    endpoint: Endpoint,
    blobs_service: BlobsService,
    mut shutdown_rx: WatchReceiver<()>,
) -> Result<()> {
    let inner_blobs = blobs_service.get_inner_blobs().clone();
    // Build the  router against the endpoint -> to our blobs service
    //  NOTE (amiller68): if you want to extend our iroh capabilities
    //   with more protocols and handlers, you'd do so here
    let router = Router::builder(endpoint)
        .accept(BLOBS_SERVICE_ALPN, inner_blobs)
        .spawn()
        .await?;

    tracing::info!("node::iroh::router: router started");

    // Wait for shutdown signal
    let _ = shutdown_rx.changed().await;

    // Gracefully shut down the router
    tracing::info!("node::iroh::router: shutting down router");
    router.shutdown().await?;

    tracing::info!("node::iroh::router: router shutdown complete");
    Ok(())
}
