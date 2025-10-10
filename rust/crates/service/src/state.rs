use std::sync::Arc;
use url::Url;

use super::config::Config;
use super::database::{Database, DatabaseSetupError};
use super::jax_state::JaxState;

use common::prelude::*;

#[derive(Clone)]
pub struct State {
    node: Peer,
    database: Database,
}

impl State {
    pub async fn from_config(config: &Config) -> Result<Self, StateSetupError> {
        let sqlite_database_url = match config.sqlite_path {
            Some(ref path) => {
                // check that the path exists
                if !path.exists() {
                    return Err(StateSetupError::DatabasePathDoesNotExist);
                }
                // parse the path into a URL
                Url::parse(&format!("sqlite://{}", path.display()))
                    .map_err(|_| StateSetupError::InvalidDatabaseUrl)
            }
            // otherwise just set up an in-memory database
            None => Url::parse("sqlite::memory:").map_err(|_| StateSetupError::InvalidDatabaseUrl),
        }?;
        tracing::info!("Database URL: {:?}", sqlite_database_url);
        let database = Database::connect(&sqlite_database_url).await?;

        // Create JAX protocol state first
        // Note: JaxState doesn't need blobs store at construction time,
        // only when check_bucket_sync is called
        let jax_state = Arc::new(JaxState::new(
            database.clone(),
        ));

        // build our node with the protocol state
        let mut node_builder = Peer::builder()
            .protocol_state(jax_state.clone());

        // set the socket addr if specified
        if config.node_listen_addr.is_some() {
            node_builder = node_builder.socket_addr(config.node_listen_addr.unwrap());
        }
        // attempt to read the secret key if specified
        if config.node_secret.is_some() {
            node_builder = node_builder.secret_key(config.node_secret.clone().unwrap());
        }
        // set the blobs store path if specified
        if config.node_blobs_store_path.is_some() {
            node_builder =
                node_builder.blobs_store_path(config.node_blobs_store_path.clone().unwrap());
        }

        // Build the node once with protocol state
        let node = node_builder.build().await;
        tracing::info!("Node id: {} (with JAX protocol)", node.id());

        // Now that the node is built, set the blobs store in JaxState
        jax_state.set_blobs(node.blobs().clone());

        Ok(Self { node, database })
    }

    pub fn node(&self) -> &Peer {
        &self.node
    }

    pub fn database(&self) -> &Database {
        &self.database
    }
}

impl AsRef<Peer> for State {
    fn as_ref(&self) -> &Peer {
        &self.node
    }
}

impl AsRef<Database> for State {
    fn as_ref(&self) -> &Database {
        &self.database
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StateSetupError {
    #[error("Database path does not exist")]
    DatabasePathDoesNotExist,
    #[error("Database setup error")]
    DatabaseSetupError(#[from] DatabaseSetupError),
    #[error("Invalid database URL")]
    InvalidDatabaseUrl,
}
