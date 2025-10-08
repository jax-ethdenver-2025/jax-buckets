// use alloy::primitives::U256;
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;

use crate::node::State as NodeState;

#[derive(Template)]
#[template(path = "share.html")]
pub struct ShareTemplate {
    message: Option<String>,
    // TODO (amiller68): add this back in
    // eth_balance: U256,
}

pub async fn share_handler(State(_state): State<NodeState>) -> impl IntoResponse {
    // let eth_address = state.eth_address();
    // let tracker = state.tracker();
    // let eth_balance = tracker
    //     .get_address_balance(eth_address)
    //     .await
    //     .expect("failed to get balance");

    ShareTemplate {
        message: None,
        // eth_balance,
    }
}
