use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum MountOpsError {
    #[error("Bucket not found: {0}")]
    BucketNotFound(Uuid),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error("Database error: {0}")]
    Database(String),
    #[error("Mount error: {0}")]
    Mount(#[from] common::prelude::MountError),
    #[error("Share not found")]
    ShareNotFound,
    #[error("Crypto error: {0}")]
    CryptoError(String),
    #[error("Share error: {0}")]
    ShareError(String),
}
