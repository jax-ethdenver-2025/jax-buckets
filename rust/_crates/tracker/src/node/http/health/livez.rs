use axum::Json;
use serde::Serialize;

use crate::node::State as NodeState;

#[derive(Serialize)]
pub struct LivenessResponse {
    status: String,
}

pub async fn handler(_state: axum::extract::State<NodeState>) -> Json<LivenessResponse> {
    Json(LivenessResponse {
        status: "ok".to_string(),
    })
}
