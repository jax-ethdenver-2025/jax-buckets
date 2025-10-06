use askama::Template;
use askama_axum::IntoResponse;
use tracing::instrument;

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub node_id: String,
    pub eth_address: String,
    pub eth_balance: String,
}

#[instrument]
pub async fn handler() -> askama_axum::Response {
    let template = IndexTemplate {
        node_id: "test-node-123".to_string(),
        eth_address: "0x1234567890abcdef".to_string(),
        eth_balance: "1000000000000000000".to_string(), // 1 ETH in wei
    };

    template.into_response()
}
