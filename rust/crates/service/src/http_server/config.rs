use std::net::SocketAddr;

use url::Url;

#[derive(Debug, Clone)]
pub struct Config {
    // Listen address
    pub listen_addr: SocketAddr,
    // Host name for generating content URLs
    pub hostname: Url,
    // API server URL (for HTML server to reference)
    pub api_url: Option<String>,
    // log level for http tracing
    pub log_level: tracing::Level,
    // Run HTML UI in read-only mode
    pub read_only: bool,
}

impl Config {
    pub fn new(listen_addr: SocketAddr, api_url: Option<String>, read_only: bool) -> Self {
        let hostname = Url::parse(&format!("http://localhost:{}", listen_addr.port())).unwrap();
        Self {
            listen_addr,
            hostname,
            api_url,
            log_level: tracing::Level::INFO,
            read_only,
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Invalid Socket Address: {0}")]
    ListenAddr(#[from] std::net::AddrParseError),
}
