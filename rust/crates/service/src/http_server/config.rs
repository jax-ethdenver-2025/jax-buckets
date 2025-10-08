use std::net::SocketAddr;
use std::str::FromStr;

use url::Url;

#[derive(Debug)]
pub struct Config {
    // Listen address
    pub listen_addr: SocketAddr,
    // Host name for generating content URLs
    pub hostname: Url,

    // log level for http tracing
    pub log_level: tracing::Level,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from_str("127.0.0.1:3000").unwrap(),
            hostname: Url::parse("http://localhost:3000").unwrap(),
            log_level: tracing::Level::INFO,
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
