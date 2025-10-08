use std::str::FromStr;

use axum::extract::{Json, State};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

use leaky_common::prelude::Cid;

use crate::app::AppState;
use crate::database::models::RootCid;

#[derive(Deserialize)]
pub struct PushRootRequest {
    cid: String,
    previous_cid: String,
}

#[derive(Serialize)]
pub struct PushRootResponse {
    previous_cid: String,
    cid: String,
}

impl From<RootCid> for PushRootResponse {
    fn from(root_cid: RootCid) -> Self {
        PushRootResponse {
            previous_cid: root_cid.previous_cid().to_string(),
            cid: root_cid.cid().to_string(),
        }
    }
}

pub async fn handler(
    State(state): State<AppState>,
    Json(push_root): Json<PushRootRequest>,
) -> Result<impl IntoResponse, PushRootError> {
    let cid = Cid::from_str(&push_root.cid)?;
    let previous_cid = Cid::from_str(&push_root.previous_cid)?;

    let db = state.sqlite_database();
    let mut conn = db.begin().await?;

    let root_cid = RootCid::push(&cid, &previous_cid, &mut conn).await?;
    conn.commit().await?;

    let mount = state.mount();
    let cid = root_cid.cid();

    tokio::task::spawn_blocking(move || {
        let mut guard = mount.write();
        futures::executor::block_on(guard.update(cid))
    })
    .await
    .map_err(PushRootError::JoinError)?
    .map_err(PushRootError::MountError)?;

    Ok((http::StatusCode::OK, Json(PushRootResponse::from(root_cid))).into_response())
}

#[derive(Debug, thiserror::Error)]
pub enum PushRootError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("invalid CID: {0}")]
    Cid(#[from] leaky_common::error::CidError),
    #[error("root CID error: {0}")]
    RootCid(#[from] crate::database::models::RootCidError),
    #[error("mount error: {0}")]
    MountError(#[from] leaky_common::error::MountError),
    #[error("join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

impl IntoResponse for PushRootError {
    fn into_response(self) -> Response {
        match self {
            PushRootError::MountError(_) | PushRootError::Database(_) => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "unknown server error",
            )
                .into_response(),
            PushRootError::Cid(_err) => {
                (http::StatusCode::BAD_REQUEST, "invalid cid").into_response()
            }
            PushRootError::RootCid(ref err) => match err {
                crate::database::models::RootCidError::Sqlx(err) => {
                    tracing::error!("database error: {}", err);
                    (
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        "unknown server error",
                    )
                        .into_response()
                }
                crate::database::models::RootCidError::InvalidLink(_, _) => {
                    (http::StatusCode::BAD_REQUEST, "invalid link").into_response()
                }
                crate::database::models::RootCidError::Conflict(_, _) => {
                    (http::StatusCode::CONFLICT, "conflict").into_response()
                }
            },
            PushRootError::JoinError(e) => {
                tracing::error!("join error: {}", e);
                (
                    http::StatusCode::INTERNAL_SERVER_ERROR,
                    "unknown server error",
                )
                    .into_response()
            }
        }
    }
}
