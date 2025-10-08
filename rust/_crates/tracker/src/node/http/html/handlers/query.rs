use askama::Template;
use askama_axum::IntoResponse;
use iroh::NodeId;

#[derive(Template)]
#[template(path = "query.html")]
struct QueryTemplate {
    // hash: Option<Hash>,
    #[allow(dead_code)]
    nodes: Option<Vec<(NodeId, f64)>>,
    #[allow(dead_code)]
    message: Option<String>,
}

pub async fn query_handler() -> impl IntoResponse {
    QueryTemplate {
        nodes: None,
        message: None,
    }
}
