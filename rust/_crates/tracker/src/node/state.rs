use std::sync::Arc;

use alloy::primitives::Address;
use alloy::signers::local::PrivateKeySigner;
use anyhow::Result;
use ed25519::Signature;
use iroh::Endpoint;
use iroh::NodeId;
use iroh::SecretKey;

use crate::config::{Config, ConfigError};

use super::iroh::{await_relay_region, create_endpoint, BlobsService};
use super::tracker::Tracker;

use tokio::sync::watch;

#[derive(Clone)]
pub struct State {
    iroh_secret_key: SecretKey,
    eth_signer: PrivateKeySigner,
    endpoint: Arc<Endpoint>,
    blobs_service: BlobsService,
    tracker: Tracker,
}

#[derive(Debug, thiserror::Error)]
pub enum StateSetupError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("config error: {0}")]
    Config(#[from] ConfigError),
}

impl State {
    pub async fn from_config(
        config: &Config,
        shutdown_rx: watch::Receiver<()>,
    ) -> Result<Self, StateSetupError> {
        // set up our endpoint
        let endpoint_socket_addr = config.endpoint_listen_addr();
        let iroh_secret_key = config.iroh_secret_key()?;
        let iroh_node_id = iroh_secret_key.public();
        let _endpoint = create_endpoint(*endpoint_socket_addr, iroh_secret_key.clone()).await;
        let endpoint = Arc::new(_endpoint);
        // await making sure the endpoint is setup
        let _ = await_relay_region(endpoint.as_ref().clone()).await;

        // set up a blob service
        let blobs_path = config.blobs_path();
        let blobs_service = BlobsService::load(blobs_path, endpoint.as_ref().clone())
            .await
            .map_err(StateSetupError::Default)?;

        let beneficiary_address = config.eth_signer().expect("valid eth signer").address();
        println!("beneficiary_address: {:?}", beneficiary_address);
        let iroh_signature = iroh_secret_key.sign(beneficiary_address.into_array().as_ref());

        let tracker = Tracker::new(
            shutdown_rx.clone(),
            config.eth_ws_rpc_url().clone(),
            config.factory_contract_address(),
            iroh_node_id,
            config.eth_signer().expect("valid eth signer"),
            blobs_service.clone(),
            iroh_signature,
        )
        .await
        .expect("valid tracker");

        // Create state with all components
        let state = Self {
            iroh_secret_key,
            eth_signer: config.eth_signer().expect("valid eth signer"),
            endpoint,
            blobs_service,
            tracker,
        };

        Ok(state)
    }

    pub fn iroh_signature(&self) -> Signature {
        let node_id = self.iroh_node_id();
        let secret_key = &self.iroh_secret_key;
        let node_id_bytes = node_id.as_bytes();

        secret_key.sign(node_id_bytes)
    }

    pub fn iroh_node_id(&self) -> NodeId {
        self.iroh_secret_key.public()
    }

    pub fn eth_address(&self) -> Address {
        self.eth_signer.address()
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn blobs_service(&self) -> &BlobsService {
        &self.blobs_service
    }

    pub fn tracker(&self) -> &Tracker {
        &self.tracker
    }
}
