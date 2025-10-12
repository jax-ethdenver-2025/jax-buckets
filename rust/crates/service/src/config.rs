use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use common::prelude::SecretKey;

#[derive(Debug)]
pub struct Config {
    // peer configuration
    /// address for our jax peer to listen on,
    ///  if not set then an ephemeral port will be used
    pub node_listen_addr: Option<SocketAddr>,
    /// on system file path to our secret,
    ///  if not set then a new secret will be generated
    pub node_secret: Option<SecretKey>,
    /// the path to our blobs store, if not set then
    ///  a temporary directory will be used
    pub node_blobs_store_path: Option<PathBuf>,

    // http server configuration
    /// address for the HTML server to listen on.
    ///  if not set then 0.0.0.0:8080 will be used
    pub html_listen_addr: Option<SocketAddr>,
    /// address for the API server to listen on.
    ///  if not set then 0.0.0.0:3000 will be used
    pub api_listen_addr: Option<SocketAddr>,

    // data store configuration
    /// a path to a sqlite database, if not set then an
    ///  in-memory database will be used
    pub sqlite_path: Option<PathBuf>,

    // misc
    pub log_level: tracing::Level,

    // ui configuration
    /// run the HTML UI in read-only mode (hides write operations)
    pub ui_read_only: bool,
    /// API hostname to use for HTML UI
    pub api_hostname: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_listen_addr: None,
            node_secret: None,
            node_blobs_store_path: None,
            html_listen_addr: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080)),
            api_listen_addr: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 3000)),
            sqlite_path: None,
            log_level: tracing::Level::INFO,
            ui_read_only: false,
            api_hostname: None,
        }
    }
}

// TODO (amiller68): real error handling
#[allow(dead_code)]
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {}
