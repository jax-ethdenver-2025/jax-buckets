use alloy::primitives::Address;
use alloy::primitives::U256;
use askama::Template;
use askama_axum::IntoResponse;
use axum::extract::State;
use http::header;
use iroh::NodeId;
use iroh_blobs::Hash;

use crate::node::State as NodeState;

#[derive(Template)]
#[template(path = "pools.html")]
struct PoolsTemplate {
    pools: Vec<(Address, Hash, U256, Vec<(NodeId, f64)>)>,
    eth_balance: U256,
}

#[axum::debug_handler]
pub async fn pools_handler(State(state): State<NodeState>) -> impl IntoResponse {
    let eth_address = state.eth_address();
    let tracker = state.tracker();
    let eth_balance = tracker
        .get_address_balance(eth_address)
        .await
        .expect("failed to get balance");

    let pools = state
        .tracker()
        .list_pools_with_trust()
        .await
        .unwrap_or_default();

    let mut pools_vec: Vec<(Address, Hash, U256, Vec<(NodeId, f64)>)> = pools
        .into_iter()
        .map(|(key, peers)| {
            let mut peers_vec = peers.into_iter().collect::<Vec<_>>();
            // Sort peers by trust score (descending), then by node ID
            peers_vec.sort_by(|(id_a, score_a), (id_b, score_b)| {
                score_b
                    .partial_cmp(score_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| id_a.cmp(id_b))
            });
            (key.key.address, key.key.hash, key.balance, peers_vec)
        })
        .collect();

    // Sort pools by highest trust score
    pools_vec.sort_by(|a, b| {
        let a_max =
            a.3.iter()
                .map(|(_, score)| score)
                .fold(0_f64, |acc, &x| f64::max(acc, x));
        let b_max =
            b.3.iter()
                .map(|(_, score)| score)
                .fold(0_f64, |acc, &x| f64::max(acc, x));
        b_max
            .partial_cmp(&a_max)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let template = PoolsTemplate {
        pools: pools_vec,
        eth_balance,
    };

    // Convert template to HTML and return with proper content type
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        template.render().unwrap_or_default(),
    )
}
