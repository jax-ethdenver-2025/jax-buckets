use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::node::State as NodeState;

#[derive(Serialize)]
pub struct ReadinessResponse {
    node_id: String,
    eth_address: String,
}

pub async fn handler(state: State<NodeState>) -> Json<ReadinessResponse> {
    let node_id = state.iroh_node_id();
    let eth_address = state.eth_address();
    Json(ReadinessResponse {
        node_id: node_id.to_string(),
        eth_address: eth_address.to_string(),
    })
}
