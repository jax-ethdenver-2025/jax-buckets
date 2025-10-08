use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::crypto::{PublicKey, Secret, Share};
use crate::linked_data::{BlockEncoded, DagCborCodec, Link};
use crate::version::Version;

use super::principal::{Principal, PrincipalRole};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BucketShare {
    principal: Principal,
    share: Share,
    root: Link,
}

impl BucketShare {
    pub fn new(root: Link, share: Share, public_key: PublicKey) -> Self {
        Self {
            principal: Principal {
                role: PrincipalRole::Owner,
                identity: public_key,
            },
            share,
            root,
        }
    }

    pub fn principal(&self) -> &Principal {
        &self.principal
    }

    pub fn share(&self) -> &Share {
        &self.share
    }

    pub fn root(&self) -> &Link {
        &self.root
    }
}

/**
* Buckets
* =======
* Buckets are the top level structure in a JaxBucket.
*  They are essenitially just a pointer to a:
*   - an identifier for the bucket. this is global and static,
*      and essentially provides a common namespace for all versions
*      of a given bucket.
*     clients may use this as a canonical identifier for the bucket.
*   - a friendly name for the bucket (not used for provenance, just for display)
*   - the entry point of the bucket
*   - the previous version of the bucket
*/
#[allow(clippy::doc_overindented_list_items)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bucket {
    // Buckets have a global unique identifier
    //  that clients should respect
    id: Uuid,
    // They also have a friendly name,
    // buckets are identified via unique pairs
    //  of <name, pk>
    name: String,
    // the set of principals that have access to the bucket
    //  and their roles
    // Using String as key for CBOR compatibility
    shares: BTreeMap<String, BucketShare>,
    // a pointer to a HashSeq block describing the pin set
    //  for the bucket
    pins: Option<Link>,
    // and a point to the previous version of the bucket
    previous: Option<Link>,
    // specify the software version as a sanity check
    version: Version,
}

impl BlockEncoded<DagCborCodec> for Bucket {}

impl Bucket {
    /// Create a new bucket with a name, owner, and share, and entry node link
    pub fn init(id: Uuid, name: String, owner: PublicKey, share: Share, root: Link) -> Self {
        Bucket {
            id,
            name,
            shares: BTreeMap::from([(
                owner.to_hex(),
                BucketShare {
                    principal: Principal {
                        role: PrincipalRole::Owner,
                        identity: owner,
                    },
                    share,
                    root,
                },
            )]),
            pins: None,
            previous: None,
            version: Version::default(),
        }
    }

    /// @deprecated -- don't use this
    pub fn new(name: String, owner: PublicKey) -> Self {
        Bucket {
            id: Uuid::new_v4(),
            name,
            shares: BTreeMap::from([(
                owner.to_hex(),
                BucketShare {
                    principal: Principal {
                        role: PrincipalRole::Owner,
                        identity: owner,
                    },
                    share: Share::default(),
                    root: Link::default(),
                },
            )]),
            pins: None,
            previous: None,
            version: Version::default(),
        }
    }

    pub fn get_share(&self, public_key: &PublicKey) -> Option<&BucketShare> {
        self.shares.get(&public_key.to_hex())
    }

    pub fn add_share(
        &mut self,
        public_key: PublicKey,
        root: Link,
        secret: Secret,
    ) -> Result<(), anyhow::Error> {
        let share = Share::new(&secret, &public_key)?;
        let bucket_share = BucketShare::new(root, share, public_key);
        self.shares.insert(public_key.to_hex(), bucket_share);
        Ok(())
    }

    pub fn id(&self) -> &Uuid {
        &self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn shares(&self) -> &BTreeMap<String, BucketShare> {
        &self.shares
    }

    pub fn version(&self) -> &Version {
        &self.version
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::crypto::{PublicKey, Secret};

    #[test]
    fn test_bucket_encode_decode() {
        // Create a bucket
        let owner = crate::crypto::SecretKey::generate().public();
        let bucket = Bucket::new("test-bucket".to_string(), owner.clone());

        // Encode
        let encoded = bucket.encode().unwrap();
        assert!(!encoded.is_empty());

        // Decode
        let decoded = Bucket::decode(&encoded).unwrap();

        // Verify fields match
        assert_eq!(bucket.name(), decoded.name());
        assert_eq!(bucket.id(), decoded.id());
        assert_eq!(bucket.version(), decoded.version());
        assert_eq!(bucket.shares().len(), decoded.shares().len());
    }

    #[test]
    fn test_bucket_with_shares_encode_decode() {
        // Create a bucket with shares
        let owner = crate::crypto::SecretKey::generate().public();
        let mut bucket = Bucket::new("test-bucket".to_string(), owner.clone());

        // Add a share
        let secret = Secret::generate();
        let root = Link::default();
        bucket
            .add_share(owner.clone(), root.clone(), secret)
            .unwrap();

        // Encode
        let encoded = bucket.encode().unwrap();
        assert!(!encoded.is_empty());

        // Decode
        let decoded = Bucket::decode(&encoded).unwrap();

        // Verify fields match
        assert_eq!(bucket.name(), decoded.name());
        assert_eq!(bucket.id(), decoded.id());
        assert_eq!(bucket.version(), decoded.version());
        // We should have the same number of shares
        assert_eq!(bucket.shares().len(), decoded.shares().len());
        assert_eq!(decoded.shares().len(), 1);
    }

    #[test]
    fn test_bucket_codec_value() {
        let owner = crate::crypto::SecretKey::generate().public();
        let bucket = Bucket::new("test-bucket".to_string(), owner);

        // Check the codec value matches DagCborCodec
        assert_eq!(bucket.codec(), 0x71); // DAG-CBOR codec
    }

    #[test]
    fn test_share_serialize() {
        use ipld_core::codec::Codec;
        use serde_ipld_dagcbor::codec::DagCborCodec;

        let share = Share::default();

        // Try to encode/decode just the Share
        let encoded = DagCborCodec::encode_to_vec(&share).unwrap();
        println!(
            "Share encoded bytes: {:?}",
            &encoded[..encoded.len().min(32)]
        );
        let decoded: Share = DagCborCodec::decode_from_slice(&encoded).unwrap();

        assert_eq!(share, decoded);
    }

    #[test]
    fn test_principal_serialize() {
        use ipld_core::codec::Codec;
        use serde_ipld_dagcbor::codec::DagCborCodec;

        let public_key = crate::crypto::SecretKey::generate().public();
        let principal = Principal {
            role: PrincipalRole::Owner,
            identity: public_key,
        };

        // Try to encode/decode just the Principal
        let encoded = DagCborCodec::encode_to_vec(&principal).unwrap();
        println!(
            "Principal encoded bytes: {:?}",
            &encoded[..encoded.len().min(32)]
        );
        let decoded: Principal = DagCborCodec::decode_from_slice(&encoded).unwrap();

        assert_eq!(principal, decoded);
    }

    #[test]
    fn test_bucket_share_serialize() {
        use ipld_core::codec::Codec;
        use serde_ipld_dagcbor::codec::DagCborCodec;

        let public_key = crate::crypto::SecretKey::generate().public();
        let share = Share::default();
        let root = Link::default();

        let bucket_share = BucketShare::new(root.clone(), share, public_key);

        // Try to encode/decode just the BucketShare
        let encoded = DagCborCodec::encode_to_vec(&bucket_share).unwrap();
        let decoded: BucketShare = DagCborCodec::decode_from_slice(&encoded).unwrap();

        assert_eq!(bucket_share, decoded);
    }
}
