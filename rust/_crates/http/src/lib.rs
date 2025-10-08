mod api;
pub mod app;
mod content;
mod database;
mod health;
mod server;
mod version;

use std::time::Duration;

use tokio::signal::unix::{signal, SignalKind};
use tokio::sync::watch;
use tokio::task::JoinHandle;

const REQUEST_GRACE_PERIOD: Duration = Duration::from_secs(10);

pub fn graceful_shutdown_blocker() -> (JoinHandle<()>, watch::Receiver<()>) {
    let mut sigint = signal(SignalKind::interrupt()).unwrap();
    let mut sigterm = signal(SignalKind::terminate()).unwrap();

    let (tx, rx) = tokio::sync::watch::channel(());

    let handle = tokio::spawn(async move {
        tokio::select! {
            _ = sigint.recv() => {
                tracing::debug!("gracefully exiting immediately on SIGINT");
            }
            _ = sigterm.recv() => {
                tokio::time::sleep(REQUEST_GRACE_PERIOD).await;
                tracing::debug!("initiaing graceful shutdown with delay on SIGTERM");
            }
        }

        // Time to start signaling any services that care about gracefully shutting down that the
        // time is at hand.
        let _ = tx.send(());
    });

    (handle, rx)
}

pub async fn server(
    config: crate::app::Config,
    state: crate::app::AppState,
    shutdown_rx: watch::Receiver<()>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        match crate::server::run(config, state, shutdown_rx).await {
            Ok(_) => tracing::info!("shutting down normally"),
            Err(err) => tracing::error!("http server exited with an error: {err}"),
        }
    })
}

pub fn register_panic_logger() {
    std::panic::set_hook(Box::new(|panic| match panic.location() {
        Some(loc) => {
            tracing::error!(
                message = %panic,
                panic.file = loc.file(),
                panic.line = loc.line(),
                panic.column = loc.column(),
            );
        }
        None => tracing::error!(message = %panic),
    }));
}

pub fn report_version() {
    let version = crate::version::Version::new();

    tracing::info!(
        build_profile = ?version.build_profile(),
        features = ?version.build_features(),
        version = ?version.version(),
        "service starting up"
    );
}
