use alloy::primitives::{Address, U256};
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use iroh::NodeId;

use crate::node::State as NodeState;

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    node_id: NodeId,
    eth_address: Address,
    eth_balance: U256,
}

#[axum::debug_handler]
pub async fn index_handler(State(state): State<NodeState>) -> impl IntoResponse {
    let node_id = state.iroh_node_id();
    let eth_address = state.eth_address();
    let tracker = state.tracker();
    let eth_balance = tracker
        .get_address_balance(eth_address)
        .await
        .expect("failed to get balance");

    IndexTemplate {
        node_id,
        eth_address,
        eth_balance,
    }
}
