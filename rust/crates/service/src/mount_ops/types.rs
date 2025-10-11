use common::prelude::Link;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::database::models::SyncStatus;

#[derive(Debug, Clone)]
pub struct BucketInfo {
    pub bucket_id: Uuid,
    pub name: String,
    pub link: Link,
    pub created_at: OffsetDateTime,
    pub sync_status: SyncStatus,
    pub last_sync_attempt: Option<OffsetDateTime>,
    pub sync_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub link: Link,
    pub is_dir: bool,
    pub mime_type: String,
}
