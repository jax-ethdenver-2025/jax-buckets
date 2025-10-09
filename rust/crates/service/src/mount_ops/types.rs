use common::prelude::Link;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct BucketInfo {
    pub bucket_id: Uuid,
    pub name: String,
    pub link: Link,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: String,
    pub name: String,
    pub link: Link,
    pub is_dir: bool,
    pub mime_type: String,
}
