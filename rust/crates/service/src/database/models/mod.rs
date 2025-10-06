// use sqlx::FromRow;

// use leaky_common::prelude::Cid;

// use crate::database::types::DCid;
// use crate::database::DatabaseConnection;

// /*
// CREATE TABLE root_cids (
//     id SERIAL PRIMARY KEY,
//     cid VARCHAR(255) NOT NULL,
//     previous_cid VARCHAR(255) NOT NULL,
//     created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
// );

// CREATE UNIQUE INDEX root_cids_cid_previous_cid ON root_cids (cid, previous_cid);
// */
// #[derive(FromRow, Debug)]
// pub struct RootCid {
//     cid: DCid,
//     previous_cid: DCid,
// }

// impl RootCid {
//     pub async fn push(
//         cid: &Cid,
//         previous_cid: &Cid,
//         conn: &mut DatabaseConnection,
//     ) -> Result<RootCid, RootCidError> {
//         // Read the current root cid
//         let maybe_root_cid = RootCid::pull(conn).await?;
//         if let Some(root_cid) = maybe_root_cid {
//             if root_cid.cid() != *previous_cid {
//                 return Err(RootCidError::InvalidLink(root_cid.cid(), *previous_cid));
//             }
//         } else if Cid::default() != *previous_cid {
//             return Err(RootCidError::InvalidLink(Cid::default(), *previous_cid));
//         }

//         let dcid: DCid = (*cid).into();
//         let dprevious_cid: DCid = (*previous_cid).into();
//         let root_cid = sqlx::query_as!(
//             RootCid,
//             r#"
//             INSERT INTO root_cids (
//                 cid,
//                 previous_cid,
//                 created_at
//             )
//             VALUES (
//                 $1,
//                 $2,
//                 CURRENT_TIMESTAMP
//             )
//             RETURNING cid as "cid: DCid", previous_cid as "previous_cid: DCid"
//             "#,
//             dcid,
//             dprevious_cid
//         )
//         .fetch_one(conn)
//         .await
//         .map_err(|e| match e {
//             sqlx::Error::Database(ref db_error) => {
//                 if db_error.constraint().unwrap_or("") == "root_cids_cid_previous_cid" {
//                     RootCidError::Conflict(*cid, *previous_cid)
//                 } else {
//                     e.into()
//                 }
//             }
//             _ => e.into(),
//         })?;
//         Ok(root_cid)
//     }

//     pub async fn pull(conn: &mut DatabaseConnection) -> Result<Option<RootCid>, RootCidError> {
//         let root_cid = sqlx::query_as!(
//             RootCid,
//             r#"
//             SELECT
//                 cid as "cid: DCid",
//                 previous_cid as "previous_cid: DCid"
//             FROM root_cids
//             ORDER BY
//                 created_at DESC
//             LIMIT 1
//             "#
//         )
//         .fetch_optional(conn)
//         .await?;
//         Ok(root_cid)
//     }

//     pub fn cid(&self) -> Cid {
//         self.cid.into()
//     }

//     pub fn previous_cid(&self) -> Cid {
//         self.previous_cid.into()
//     }
// }

// #[derive(Debug, thiserror::Error)]
// pub enum RootCidError {
//     #[error("sqlx: {0}")]
//     Sqlx(#[from] sqlx::Error),
//     #[error("wrong previous cid: {0:?} != {1:?}")]
//     InvalidLink(Cid, Cid),
//     #[error("conflicting Update: {0:?} -> {1:?}")]
//     Conflict(Cid, Cid),
// }
