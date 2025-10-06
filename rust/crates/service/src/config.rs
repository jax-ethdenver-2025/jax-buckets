use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
};

use common::prelude::SecretKey;
use url::Url;

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
    // TODO (amiller68): maybe do some specific configuration error handling
    //  based on whether one and not the other is provided
    /// address for our http server to listen on.
    ///  if not set then 0.0.0.0:8080 will be used
    pub http_listen_addr: Option<SocketAddr>,
    /// hostname that our server will assume its
    ///  responding from. if not set then the listen
    ///  address will be used
    pub http_hostname: Option<Url>,

    // data store configuration
    /// a path to a sqlite database, if not set then an
    ///  in-memory database will be used
    pub sqlite_path: Option<PathBuf>,

    // misc
    pub log_level: tracing::Level,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            node_listen_addr: None,
            node_secret: None,
            node_blobs_store_path: None,
            http_listen_addr: Some(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)), 8080)),
            http_hostname: None,
            sqlite_path: None,
            log_level: tracing::Level::INFO,
        }
    }
}

// TODO (amiller68): real error handling
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {}
