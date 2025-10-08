use std::env;
use std::net::SocketAddr;
use std::str::FromStr;

use dotenvy::dotenv;

use url::Url;

#[derive(Debug)]
pub struct Config {
    // Listen address
    listen_addr: SocketAddr,

    // TODO: there's prolly a way to do this with headers
    //  but for now we'll just make it a config option
    /// Get Content Forwarding Url
    get_content_forwarding_url: Url,

    // Database Config
    sqlite_database_url: Url,

    // Ipfs Rpc Url
    ipfs_rpc_url: Url,

    // Logging Level
    log_level: tracing::Level,
}

impl Config {
    pub fn from_env() -> Result<Config, ConfigError> {
        if dotenv().is_err() {
            tracing::warn!("No .env file found");
        }

        let listen_addr_str = match env::var("LISTEN_ADDR") {
            Ok(addr) => addr,
            Err(_e) => {
                tracing::warn!("No LISTEN_ADDR found in .env. Using default");
                "0.0.0.0:3000".to_string()
            }
        };
        let listen_addr = listen_addr_str.parse()?;

        let get_content_forwarding_url_str = match env::var("GET_CONTENT_FORWARDING_URL") {
            Ok(url) => url,
            Err(_e) => {
                tracing::warn!("No GET_CONTENT_FORWARDING_URL found in .env. Using default");
                "http://localhost:3000".to_string()
            }
        };
        let get_content_forwarding_url = Url::parse(&get_content_forwarding_url_str)?;

        let sqlite_database_url_str = match env::var("SQLITE_DATABASE_URL") {
            Ok(url) => url,
            Err(_e) => {
                tracing::warn!("No SQLITE_DATABASE_URL found in .env. Using default");
                "sqlite://./data/server.db".to_string()
            }
        };
        let sqlite_database_url = Url::parse(&sqlite_database_url_str)?;

        let ipfs_rpc_url_str = match env::var("IPFS_RPC_URL") {
            Ok(url) => url,
            Err(_e) => {
                tracing::warn!("No IPFS_API_URL found in .env");
                "http://localhost:3000".to_string()
            }
        };
        let ipfs_rpc_url = Url::parse(&ipfs_rpc_url_str)?;

        let log_level_str = match env::var("LOG_LEVEL") {
            Ok(level) => level,
            Err(_e) => {
                tracing::warn!("No LOG_LEVEL found in .env. Using default");
                "info".to_string()
            }
        };
        let log_level = match tracing::Level::from_str(&log_level_str) {
            Ok(level) => level,
            Err(_e) => {
                tracing::warn!("Invalid LOG_LEVEL found in .env. Using default");
                tracing::Level::INFO
            }
        };

        Ok(Config {
            listen_addr,
            get_content_forwarding_url,
            sqlite_database_url,
            ipfs_rpc_url,
            log_level,
        })
    }

    pub fn sqlite_database_url(&self) -> &Url {
        &self.sqlite_database_url
    }

    pub fn ipfs_rpc_url(&self) -> &Url {
        &self.ipfs_rpc_url
    }

    pub fn log_level(&self) -> &tracing::Level {
        &self.log_level
    }

    pub fn listen_addr(&self) -> &SocketAddr {
        &self.listen_addr
    }

    pub fn get_content_forwarding_url(&self) -> &Url {
        &self.get_content_forwarding_url
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Missing Env: {0}")]
    Env(#[from] env::VarError),
    #[error("Invalid Socket Address: {0}")]
    ListenAddr(#[from] std::net::AddrParseError),
}
