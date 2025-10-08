use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::database::{types::DCid, Database};

use common::prelude::Link;

#[derive(FromRow, Debug, Clone)]
pub struct Bucket {
    pub id: Uuid,
    pub name: String,
    pub link: DCid,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl Bucket {
    pub async fn create(
        id: Uuid,
        name: String,
        link: Link,
        db: &Database,
    ) -> Result<Bucket, BucketError> {
        let dcid: DCid = link.into();
        let bucket = sqlx::query_as!(
            Bucket,
            r#"
            INSERT INTO buckets (id, name, link, created_at, updated_at)
            VALUES ($1, $2, $3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            RETURNING id as "id!: Uuid", name as "name!", link as "link!: DCid", created_at as "created_at!", updated_at as "updated_at!"
            "#,
            id,
            name,
            dcid
        )
        .fetch_one(&**db)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_error) => {
                if db_error.constraint().is_some() {
                    BucketError::AlreadyExists(name.clone())
                } else {
                    BucketError::Database(e)
                }
            }
            _ => BucketError::Database(e),
        })?;

        Ok(bucket)
    }

    pub async fn get_by_id(id: &Uuid, db: &Database) -> Result<Option<Bucket>, BucketError> {
        let bucket = sqlx::query_as!(
            Bucket,
            r#"
            SELECT id as "id!: Uuid", name as "name!", link as "link!: DCid", created_at as "created_at!", updated_at as "updated_at!"
            FROM buckets
            WHERE id = $1
            "#,
            id
        )
        .fetch_optional(&**db)
        .await?;

        Ok(bucket)
    }

    pub async fn list(
        prefix: Option<String>,
        limit: Option<u32>,
        db: &Database,
    ) -> Result<Vec<Bucket>, BucketError> {
        let limit = limit.unwrap_or(100).min(1000) as i64;

        let buckets = if let Some(prefix) = prefix {
            let pattern = format!("{}%", prefix);
            sqlx::query_as!(
                Bucket,
                r#"
                SELECT id as "id!: Uuid", name as "name!", link as "link!: DCid", created_at as "created_at!", updated_at as "updated_at!"
                FROM buckets
                WHERE name LIKE $1
                ORDER BY created_at DESC
                LIMIT $2
                "#,
                pattern,
                limit
            )
            .fetch_all(&**db)
            .await?
        } else {
            sqlx::query_as!(
                Bucket,
                r#"
                SELECT id as "id!: Uuid", name as "name!", link as "link!: DCid", created_at as "created_at!", updated_at as "updated_at!"
                FROM buckets
                ORDER BY created_at DESC
                LIMIT $1
                "#,
                limit
            )
            .fetch_all(&**db)
            .await?
        };

        Ok(buckets)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BucketError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Bucket already exists: {0}")]
    AlreadyExists(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn setup_test_db() -> Database {
        // Create in-memory database
        let pool = SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory database");

        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE buckets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                link VARCHAR(255) NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            );
            CREATE UNIQUE INDEX buckets_id_name ON buckets (id, name);
            "#,
        )
        .execute(&pool)
        .await
        .expect("Failed to create table");

        Database::new(pool)
    }

    #[tokio::test]
    async fn test_create_bucket() {
        let db = setup_test_db().await;

        let id = Uuid::new_v4();
        let bucket = Bucket::create(id, "test-bucket".to_string(), Link::default(), &db)
            .await
            .unwrap();

        assert_eq!(bucket.id, id);
        assert_eq!(bucket.name, "test-bucket");
        assert_eq!(bucket.link, DCid::default());
    }

    #[tokio::test]
    async fn test_create_duplicate_bucket() {
        let db = setup_test_db().await;

        let id = Uuid::new_v4();
        Bucket::create(id, "test-bucket".to_string(), Link::default(), &db)
            .await
            .expect("Failed to create first bucket");

        let result = Bucket::create(id, "test-bucket".to_string(), Link::default(), &db).await;

        // Should fail due to PRIMARY KEY constraint on id
        match result {
            Err(BucketError::AlreadyExists(name)) => {
                assert_eq!(name, "test-bucket");
            }
            Err(BucketError::Database(e)) => {
                // Sometimes constraint violation comes through as generic DB error
                eprintln!("Got database error: {}", e);
                assert!(e.to_string().contains("UNIQUE") || e.to_string().contains("constraint"));
            }
            Ok(_) => panic!("Expected error but got Ok"),
        }
    }

    #[tokio::test]
    async fn test_get_by_id() {
        let db = setup_test_db().await;

        let id = Uuid::new_v4();
        let bucket = Bucket::create(id, "test-bucket".to_string(), Link::default(), &db)
            .await
            .unwrap();

        let bucket = Bucket::get_by_id(&id, &db)
            .await
            .expect("Failed to get bucket")
            .expect("Bucket not found");

        assert_eq!(bucket.id, id);
        assert_eq!(bucket.name, "test-bucket");

        let not_found = Bucket::get_by_id(&Uuid::new_v4(), &db)
            .await
            .expect("Failed to query");

        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_list_buckets() {
        let db = setup_test_db().await;

        // Create multiple buckets
        for i in 1..=5 {
            let id = Uuid::new_v4();
            Bucket::create(id, format!("bucket-{}", i), Link::default(), &db)
                .await
                .expect("Failed to create bucket");
        }

        // List all
        let buckets = Bucket::list(None, None, &db)
            .await
            .expect("Failed to list buckets");

        assert_eq!(buckets.len(), 5);

        // List with limit
        let buckets = Bucket::list(None, Some(3), &db)
            .await
            .expect("Failed to list buckets");

        assert_eq!(buckets.len(), 3);
    }

    #[tokio::test]
    async fn test_list_with_prefix() {
        let db = setup_test_db().await;

        // Create buckets with different prefixes
        let id = Uuid::new_v4();
        Bucket::create(id, "prod-bucket-1".to_string(), Link::default(), &db)
            .await
            .expect("Failed to create bucket");

        let id = Uuid::new_v4();
        Bucket::create(id, "prod-bucket-2".to_string(), Link::default(), &db)
            .await
            .expect("Failed to create bucket");

        let id = Uuid::new_v4();
        Bucket::create(id, "dev-bucket-1".to_string(), Link::default(), &db)
            .await
            .expect("Failed to create bucket");

        let id = Uuid::new_v4();
        Bucket::create(id, "dev-bucket-2".to_string(), Link::default(), &db)
            .await
            .expect("Failed to create bucket");

        // List with prefix
        let buckets = Bucket::list(Some("prod".to_string()), None, &db)
            .await
            .expect("Failed to list buckets");

        assert_eq!(buckets.len(), 2);
        assert!(buckets.iter().all(|b| b.name.starts_with("prod")));

        let dev_buckets = Bucket::list(Some("dev".to_string()), None, &db)
            .await
            .expect("Failed to list buckets");

        assert_eq!(dev_buckets.len(), 1);
    }
}
