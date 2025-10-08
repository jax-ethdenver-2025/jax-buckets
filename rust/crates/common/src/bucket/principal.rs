#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::doc_overindented_list_items)]

use serde::{Deserialize, Serialize};

use crate::crypto::PublicKey;

// TODO (amiller68): it would be cool if I could gaurantee
//  that principal shares are
//  - valid (point to the up-to-date entrypoint)
//  - point to the same entry link
// For now I assume no one is sharing bogus data / only
//  add principals that you trust.
/**
 * Principals
 * ==========
 * Principals are a fancy name for public keys that
 *  have permissions on the bucket.
 * Principals:
 *  - descrive a public key
 *  - which has a role on the bucket (for now we just support 'owner')
 *  - and have a share into the bucket's encryption key
 * To be clear:
 *  - there is no cryptographic validation of the role *other* than
 *     whether or not clients are willing to accept updates given
 *     the prior state of a bucket. It is the responsibility of clients
 *     to check that bucket updates respect principal roles at a given
 *     update
 *  - shares may be assumed to point to the entry of a bucket for
 *     each principal. It is the responsibility of the updater to
 *     share to all principals st they may read the bucket
 */

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrincipalRole {
    Owner,
}

// NOTE (amiller68): we omit the key from the Principal struct
//  since we use it to index into the Principals map
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Principal {
    pub role: PrincipalRole,
    pub identity: PublicKey,
}
