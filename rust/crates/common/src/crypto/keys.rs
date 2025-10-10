use std::ops::Deref;

use curve25519_dalek::edwards::CompressedEdwardsY;
use iroh::{PublicKey as PPublicKey, SecretKey as SSecretKey};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

pub const PRIVATE_KEY_SIZE: usize = 32;
pub const PUBLIC_KEY_SIZE: usize = 32; // Ed25519 public key size

// TODO (amiller68): be alot less lazy about this
#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("id key error: {0}")]
    Default(#[from] anyhow::Error),
}

/**
 * Public keys for identity, sharing, and
 *  update provenance.
 * Just a light wrapper around iroh's PublicKey,
 *  which is the public part of an ed25519 keypair.
 */
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Copy)]
pub struct PublicKey(PPublicKey);

impl Deref for PublicKey {
    type Target = PPublicKey;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<PPublicKey> for PublicKey {
    fn from(key: PPublicKey) -> Self {
        PublicKey(key)
    }
}

impl From<PublicKey> for PPublicKey {
    fn from(key: PublicKey) -> Self {
        key.0
    }
}

impl From<[u8; PUBLIC_KEY_SIZE]> for PublicKey {
    fn from(bytes: [u8; PUBLIC_KEY_SIZE]) -> Self {
        PublicKey(PPublicKey::from_bytes(&bytes).expect("valid public key"))
    }
}

impl TryFrom<&[u8]> for PublicKey {
    type Error = KeyError;
    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() != PUBLIC_KEY_SIZE {
            return Err(anyhow::anyhow!(
                "invalid public key size, expected {}, got {}",
                PUBLIC_KEY_SIZE,
                bytes.len()
            )
            .into());
        }
        let mut buff = [0; PUBLIC_KEY_SIZE];
        buff.copy_from_slice(bytes);
        Ok(buff.into())
    }
}

impl PublicKey {
    pub fn from_hex(hex: &str) -> Result<Self, KeyError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut buff = [0; PUBLIC_KEY_SIZE];
        hex::decode_to_slice(hex, &mut buff)
            .map_err(|_| anyhow::anyhow!("public key hex decode error"))?;
        Ok(buff.into())
    }

    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_SIZE] {
        *self.0.as_bytes()
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /**
     * Convert our Ed25519 key into its X25519 curve equivalent
     *  for the purpose of doing key sharing
     */
    #[allow(clippy::wrong_self_convention)]
    pub(crate) fn to_x25519(&self) -> Result<X25519PublicKey, KeyError> {
        let edwards_bytes = self.to_bytes();
        let edwards_point = CompressedEdwardsY::from_slice(&edwards_bytes)
            .map_err(|_| anyhow::anyhow!("public key invalid edwards point"))?
            .decompress()
            .ok_or_else(|| anyhow::anyhow!("public key failed to decompress edwards point"))?;

        let montgomery_point = edwards_point.to_montgomery();
        Ok(X25519PublicKey::from(montgomery_point.to_bytes()))
    }
}

/**
 * Private keys for signing and decryption.
 * Just a light wrapper around iroh's SecretKey,
 *  which is the private part of an ed25519 keypair.
 */
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretKey(pub SSecretKey);

impl From<[u8; PRIVATE_KEY_SIZE]> for SecretKey {
    fn from(secret: [u8; PRIVATE_KEY_SIZE]) -> Self {
        Self(SSecretKey::from_bytes(&secret))
    }
}

impl Deref for SecretKey {
    type Target = SSecretKey;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SecretKey {
    pub fn from_hex(hex: &str) -> Result<Self, KeyError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut buff = [0; PRIVATE_KEY_SIZE];
        hex::decode_to_slice(hex, &mut buff)
            .map_err(|_| anyhow::anyhow!("priuvate key hex decode error"))?;
        Ok(Self::from(buff))
    }

    // TODO (amiller68): i think some of these are actually
    //  kinda useless since we get them for free through the deref
    //  ... but fine for now
    pub fn generate() -> Self {
        let mut bytes = [0u8; PRIVATE_KEY_SIZE];
        getrandom::getrandom(&mut bytes).expect("failed to generate random bytes");
        Self::from(bytes)
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.public())
    }

    pub fn to_bytes(&self) -> [u8; PRIVATE_KEY_SIZE] {
        self.0.to_bytes()
    }

    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    pub fn to_pem(&self) -> String {
        let pem = pem::Pem::new("PRIVATE KEY", self.to_bytes());
        pem::encode(&pem)
    }

    pub fn from_pem(pem_str: &str) -> Result<Self, KeyError> {
        let pem = pem::parse(pem_str).map_err(|e| anyhow::anyhow!("failed to parse PEM: {}", e))?;

        if pem.tag() != "PRIVATE KEY" {
            return Err(anyhow::anyhow!("invalid PEM tag, expected PRIVATE KEY").into());
        }

        let contents = pem.contents();
        if contents.len() != PRIVATE_KEY_SIZE {
            return Err(anyhow::anyhow!(
                "invalid private key size in PEM, expected {}, got {}",
                PRIVATE_KEY_SIZE,
                contents.len()
            )
            .into());
        }

        let mut bytes = [0u8; PRIVATE_KEY_SIZE];
        bytes.copy_from_slice(contents);
        Ok(Self::from(bytes))
    }

    // Convert Ed25519 private key to X25519 private key for ECDH
    pub(crate) fn to_x25519(&self) -> StaticSecret {
        // Access the underlying ed25519_dalek::SigningKey
        let signing_key = self.0.secret();
        // Use the scalar bytes for X25519 (this is the correct way)
        let scalar_bytes = signing_key.to_scalar_bytes();
        StaticSecret::from(scalar_bytes)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_keypair_generation() {
        let private_key = SecretKey::generate();
        let public_key = private_key.public();

        // Test round-trip conversion
        let private_hex = private_key.to_hex();
        let recovered_private = SecretKey::from_hex(&private_hex).unwrap();
        assert_eq!(private_key.to_bytes(), recovered_private.to_bytes());

        let public_hex = public_key.to_hex();
        let recovered_public = PublicKey::from_hex(&public_hex).unwrap();
        assert_eq!(public_key.to_bytes(), recovered_public.to_bytes());
    }

    #[test]
    fn test_pem_serialization() {
        let private_key = SecretKey::generate();

        // Test round-trip PEM conversion
        let pem = private_key.to_pem();
        let recovered_private = SecretKey::from_pem(&pem).unwrap();
        assert_eq!(private_key.to_bytes(), recovered_private.to_bytes());

        // Verify the recovered key can produce the same public key
        assert_eq!(
            private_key.public().to_bytes(),
            recovered_private.public().to_bytes()
        );
    }
}
