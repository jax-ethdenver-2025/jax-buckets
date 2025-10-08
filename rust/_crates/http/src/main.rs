#[tokio::main]
async fn main() {
    use std::time::Duration;

    use futures::future::join_all;
    use tokio::time::timeout;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::{EnvFilter, Layer};

    use leaky_server::app::{AppState, Config};

    const FINAL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(30);

    // Get the configuration from the environment
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Error loading configuration: {}", e);
            std::process::exit(2);
        }
    };

    // Set up logging
    // TODO: conditional text decoration depending on the environment
    let (non_blocking_writer, _guard) = tracing_appender::non_blocking(std::io::stdout());
    let env_filter = EnvFilter::builder()
        .with_default_directive((*config.log_level()).into())
        .from_env_lossy();

    let stderr_layer = tracing_subscriber::fmt::layer()
        .compact()
        .with_writer(non_blocking_writer)
        .with_filter(env_filter);

    tracing_subscriber::registry().with(stderr_layer).init();

    leaky_server::register_panic_logger();
    leaky_server::report_version();

    // Create the app state
    let state = match AppState::from_config(&config).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Error creating app state: {}", e);
            std::process::exit(3);
        }
    };

    let (graceful_waiter, shutdown_rx) = leaky_server::graceful_shutdown_blocker();
    let mut handles = Vec::new();

    let server = leaky_server::server(config, state, shutdown_rx).await;
    handles.push(server);

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
