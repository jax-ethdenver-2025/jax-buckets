use std::convert::TryFrom;

use aes_kw::KekAes256 as Kek;
use serde::{Deserialize, Serialize};

use super::keys::{KeyError, PublicKey, SecretKey, PUBLIC_KEY_SIZE};
use super::secret::{Secret, SecretError, SECRET_SIZE};

// Aes-Kw nonce size (just eight bytes)
pub const KW_NONCE_SIZE: usize = 8;
// Total expected byte lenght for the Share, which is a:
//  - ephemeral public key,(32 bytes)
//  - aes-kw nonce, (8 bytes)
//  - and wrapped key (same length as the original aes secret, 32 bytes)
// for a total of 72 bytes
pub const SHARE_SIZE: usize = KW_NONCE_SIZE + PUBLIC_KEY_SIZE + SECRET_SIZE;

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("share error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("key error: {0}")]
    Key(#[from] KeyError),
    #[error("secret error: {0}")]
    Secret(#[from] SecretError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Share(pub(crate) [u8; SHARE_SIZE]);

impl Serialize for Share {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for Share {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{Error, Visitor};
        use std::fmt;

        struct ShareVisitor;

        impl<'de> Visitor<'de> for ShareVisitor {
            type Value = Share;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a byte array or sequence of SHARE_SIZE")
            }

            fn visit_bytes<E>(self, v: &[u8]) -> Result<Self::Value, E>
            where
                E: Error,
            {
                if v.len() != SHARE_SIZE {
                    return Err(E::invalid_length(
                        v.len(),
                        &format!("expected {} bytes", SHARE_SIZE).as_str(),
                    ));
                }
                let mut array = [0u8; SHARE_SIZE];
                array.copy_from_slice(v);
                Ok(Share(array))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    bytes.push(byte);
                }
                if bytes.len() != SHARE_SIZE {
                    return Err(A::Error::invalid_length(
                        bytes.len(),
                        &format!("expected {} bytes", SHARE_SIZE).as_str(),
                    ));
                }
                let mut array = [0u8; SHARE_SIZE];
                array.copy_from_slice(&bytes);
                Ok(Share(array))
            }
        }

        // Try bytes first (for CBOR/bincode), fallback to seq (for JSON)
        deserializer.deserialize_byte_buf(ShareVisitor)
    }
}

impl Default for Share {
    fn default() -> Self {
        Share([0; SHARE_SIZE])
    }
}

impl From<[u8; SHARE_SIZE]> for Share {
    fn from(bytes: [u8; SHARE_SIZE]) -> Self {
        Share(bytes)
    }
}

impl From<Share> for [u8; SHARE_SIZE] {
    fn from(share: Share) -> Self {
        share.0
    }
}

impl TryFrom<&[u8]> for Share {
    type Error = ShareError;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != SHARE_SIZE {
            return Err(anyhow::anyhow!(
                "invalid share size, expected {}, got {}",
                SHARE_SIZE,
                bytes.len()
            )
            .into());
        }
        let mut share = Share::default();
        share.0.copy_from_slice(bytes);
        Ok(share)
    }
}

impl Share {
    /**
     * Decode a Share from a hex string.
     *  Optionally allows a '0x' prefix.
     *  Supports hex-only encoding as well.
     */
    pub fn from_hex(hex: &str) -> Result<Self, ShareError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut buff = [0; SHARE_SIZE];
        hex::decode_to_slice(hex, &mut buff).map_err(|_| anyhow::anyhow!("hex decode error"))?;
        Ok(Share::from(buff))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }

    /**
     * Generate a new share from a secret and target recipient
     *  This will:
     *   - generate an ephemeral key pair 'E' to use with the share
     *   - create a shared secret using 'E' for the target recipient 'R'
     */
    pub fn new(secret: &Secret, recipient: &PublicKey) -> Result<Self, ShareError> {
        // Generate ephemeral Ed25519 keypair
        let ephemeral_private = SecretKey::generate();
        let ephemeral_public = ephemeral_private.public();

        // Convert both keys to X25519 for ECDH
        let ephemeral_x25519_private = ephemeral_private.to_x25519();
        let recipient_x25519_public = recipient.to_x25519()?;

        // Perform ECDH to get shared secret
        let shared_secret = ephemeral_x25519_private.diffie_hellman(&recipient_x25519_public);

        // Use shared secret as KEK for AES-KW
        // copy the bytes to a fixed array
        let mut shared_secret_bytes = [0; SECRET_SIZE];
        shared_secret_bytes.copy_from_slice(shared_secret.as_bytes());
        let kek = Kek::from(shared_secret_bytes);
        let wrapped = kek
            .wrap_vec(secret.bytes())
            .map_err(|_| anyhow::anyhow!("AES-KW wrap error"))?;

        // Build share: ephemeral_public_key || wrapped_secret
        let mut share = Share::default();
        let ephemeral_bytes = ephemeral_public.to_bytes();

        // sanity check we're getting `SHARE_SIZE` bytes here
        if ephemeral_bytes.len() + wrapped.len() != SHARE_SIZE {
            return Err(anyhow::anyhow!("expected share size is incorrect").into());
        };

        // Copy the bytes in
        share.0[..PUBLIC_KEY_SIZE].copy_from_slice(&ephemeral_bytes);
        share.0[PUBLIC_KEY_SIZE..PUBLIC_KEY_SIZE + wrapped.len()].copy_from_slice(&wrapped);

        Ok(share)
    }

    /**
     * Recover a share using the recipient's private key
     */
    pub fn recover(&self, recipient_secret: &SecretKey) -> Result<Secret, ShareError> {
        // Extract the ephemeral public key
        let ephemeral_public_bytes = &self.0[..PUBLIC_KEY_SIZE];
        let ephemeral_public = PublicKey::try_from(ephemeral_public_bytes)?;

        // Convert keys to X25519 for ECDH
        let recipient_x25519_private = recipient_secret.to_x25519();
        let ephemeral_x25519_public = ephemeral_public.to_x25519()?;

        // Perform ECDH to get same shared secret
        let shared_secret = recipient_x25519_private.diffie_hellman(&ephemeral_x25519_public);

        // Use shared secret as KEK for AES-KW unwrapping
        let shared_secret_bytes = *shared_secret.as_bytes();
        let kek = Kek::from(shared_secret_bytes);
        let wrapped_data = &self.0[PUBLIC_KEY_SIZE..];

        // Find the actual length of wrapped data (AES-KW adds padding)
        let unwrapped = kek
            .unwrap_vec(wrapped_data)
            .map_err(|_| anyhow::anyhow!("AES-KW unwrap error"))?;

        if unwrapped.len() != SECRET_SIZE {
            return Err(anyhow::anyhow!("unwrapped secret has wrong size").into());
        }

        let mut secret_bytes = [0; SECRET_SIZE];
        secret_bytes.copy_from_slice(&unwrapped);
        Ok(Secret::from(secret_bytes))
    }

    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_share_secret() {
        let secret = Secret::from_slice(&[42u8; SECRET_SIZE]).unwrap();
        let private_key = SecretKey::generate();
        let public_key = private_key.public();
        let share = Share::new(&secret, &public_key).unwrap();
        let recovered_secret = share.recover(&private_key).unwrap();
        assert_eq!(secret, recovered_secret);
    }

    #[test]
    fn test_share_different_keys() {
        let secret = Secret::generate();
        let alice_private = SecretKey::generate();
        let alice_public = alice_private.public();
        let bob_private = SecretKey::generate();
        // Alice creates a share for Bob
        let share = Share::new(&secret, &alice_public).unwrap();
        // Alice can recover the secret
        let recovered_by_alice = share.recover(&alice_private).unwrap();
        assert_eq!(secret, recovered_by_alice);
        // Bob cannot recover the secret (should fail)
        let result = share.recover(&bob_private);
        assert!(result.is_err());
    }

    #[test]
    fn test_share_hex_roundtrip() {
        let secret = Secret::generate();
        let private_key = SecretKey::generate();
        let public_key = private_key.public();
        let share = Share::new(&secret, &public_key).unwrap();
        let hex = share.to_hex();
        let recovered_share = Share::from_hex(&hex).unwrap();
        assert_eq!(share, recovered_share);
        let recovered_secret = recovered_share.recover(&private_key).unwrap();
        assert_eq!(secret, recovered_secret);
    }

    #[test]
    fn test_share_serde_json_roundtrip() {
        let secret = Secret::generate();
        let private_key = SecretKey::generate();
        let public_key = private_key.public();
        let share = Share::new(&secret, &public_key).unwrap();

        // Serialize to JSON
        let json = serde_json::to_string(&share).unwrap();

        // Deserialize from JSON
        let recovered_share: Share = serde_json::from_str(&json).unwrap();

        // Verify the share is identical
        assert_eq!(share, recovered_share);

        // Verify we can still recover the original secret
        let recovered_secret = recovered_share.recover(&private_key).unwrap();
        assert_eq!(secret, recovered_secret);
    }

    #[test]
    fn test_share_serde_bincode_roundtrip() {
        let secret = Secret::generate();
        let private_key = SecretKey::generate();
        let public_key = private_key.public();
        let share = Share::new(&secret, &public_key).unwrap();

        // Serialize to binary
        let binary = bincode::serialize(&share).unwrap();

        // Deserialize from binary
        let recovered_share: Share = bincode::deserialize(&binary).unwrap();

        // Verify the share is identical
        assert_eq!(share, recovered_share);

        // Verify we can still recover the original secret
        let recovered_secret = recovered_share.recover(&private_key).unwrap();
        assert_eq!(secret, recovered_secret);
    }

    #[test]
    fn test_share_deserialize_invalid_length() {
        // Test with too short data
        let short_data = vec![0u8; SHARE_SIZE - 1];
        let result: Result<Share, _> =
            bincode::deserialize(&bincode::serialize(&short_data).unwrap());
        assert!(result.is_err());

        // Test with too long data
        let long_data = vec![0u8; SHARE_SIZE + 1];
        let result: Result<Share, _> =
            bincode::deserialize(&bincode::serialize(&long_data).unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_share_deserialize_exact_size() {
        // Test that exact size data can be deserialized
        let exact_data = vec![0u8; SHARE_SIZE];
        let serialized = bincode::serialize(&exact_data).unwrap();
        let result: Result<Share, _> = bincode::deserialize(&serialized);
        assert!(result.is_ok());

        let share = result.unwrap();
        assert_eq!(share.0, [0u8; SHARE_SIZE]);
    }

    #[test]
    fn test_share_serde_multiple_formats() {
        let secret = Secret::generate();
        let private_key = SecretKey::generate();
        let public_key = private_key.public();
        let original_share = Share::new(&secret, &public_key).unwrap();

        // Test JSON roundtrip
        let json = serde_json::to_string(&original_share).unwrap();
        let json_share: Share = serde_json::from_str(&json).unwrap();
        assert_eq!(original_share, json_share);

        // Test Bincode roundtrip
        let binary = bincode::serialize(&original_share).unwrap();
        let binary_share: Share = bincode::deserialize(&binary).unwrap();
        assert_eq!(original_share, binary_share);

        // Ensure all formats produce the same result
        assert_eq!(json_share, binary_share);

        // Verify all can recover the same secret
        let secret1 = json_share.recover(&private_key).unwrap();
        let secret2 = binary_share.recover(&private_key).unwrap();
        assert_eq!(secret, secret1);
        assert_eq!(secret, secret2);
        assert_eq!(secret1, secret2);
    }
}
