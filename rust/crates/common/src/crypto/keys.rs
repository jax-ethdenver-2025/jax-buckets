use std::ops::Deref;

use curve25519_dalek::edwards::CompressedEdwardsY;
use iroh::{PublicKey as PPublicKey, SecretKey as SSecretKey};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret};

/// Size of Ed25519 private key in bytes
pub const PRIVATE_KEY_SIZE: usize = 32;
/// Size of Ed25519 public key in bytes
pub const PUBLIC_KEY_SIZE: usize = 32;

/// Errors that can occur during key operations
#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("key error: {0}")]
    Default(#[from] anyhow::Error),
}

/// Public key for peer identity, key sharing, and update provenance
///
/// A thin wrapper around Iroh's `PublicKey`, representing the public part of an Ed25519 keypair.
/// This key serves multiple purposes:
/// - **Peer Identity**: Uniquely identifies a peer in the network (equivalent to Iroh's NodeId)
/// - **Key Sharing**: Used in ECDH key exchange (after conversion to X25519)
/// - **Access Control**: Listed in bucket shares to grant access
///
/// # Examples
///
/// ```ignore
/// let secret_key = SecretKey::generate();
/// let public_key = secret_key.public();
///
/// // Serialize to hex for storage/transmission
/// let hex = public_key.to_hex();
/// let recovered = PublicKey::from_hex(&hex)?;
/// ```
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
    /// Parse a public key from a hexadecimal string
    ///
    /// Accepts both plain hex and "0x"-prefixed hex strings.
    pub fn from_hex(hex: &str) -> Result<Self, KeyError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut buff = [0; PUBLIC_KEY_SIZE];
        hex::decode_to_slice(hex, &mut buff)
            .map_err(|_| anyhow::anyhow!("public key hex decode error"))?;
        Ok(buff.into())
    }

    /// Convert public key to raw bytes
    pub fn to_bytes(&self) -> [u8; PUBLIC_KEY_SIZE] {
        *self.0.as_bytes()
    }

    /// Convert public key to hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Convert Ed25519 public key to X25519 (Montgomery curve) for ECDH
    ///
    /// This conversion is necessary for the key sharing protocol, which uses
    /// Elliptic Curve Diffie-Hellman (ECDH) to establish shared secrets.
    /// Ed25519 uses the Edwards curve, while ECDH requires the Montgomery curve (X25519).
    ///
    /// # Errors
    ///
    /// Returns an error if the Ed25519 point cannot be converted (invalid point).
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

/// Secret key for peer identity and key sharing
///
/// A thin wrapper around Iroh's `SecretKey`, representing the private part of an Ed25519 keypair.
/// This key should be kept secret and securely stored (e.g., in the local config file).
///
/// # Security Considerations
///
/// - Never share this key over the network
/// - Store encrypted or in a secure location (e.g., `~/.config/jax/secret.pem`)
/// - Generate a new key for each peer instance
///
/// # Examples
///
/// ```ignore
/// // Generate a new keypair
/// let secret_key = SecretKey::generate();
/// let public_key = secret_key.public();
///
/// // Persist to PEM format
/// let pem = secret_key.to_pem();
/// std::fs::write("secret.pem", pem)?;
///
/// // Load from PEM
/// let pem = std::fs::read_to_string("secret.pem")?;
/// let recovered = SecretKey::from_pem(&pem)?;
/// ```
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
    /// Parse a secret key from a hexadecimal string
    ///
    /// Accepts both plain hex and "0x"-prefixed hex strings.
    pub fn from_hex(hex: &str) -> Result<Self, KeyError> {
        let hex = hex.strip_prefix("0x").unwrap_or(hex);
        let mut buff = [0; PRIVATE_KEY_SIZE];
        hex::decode_to_slice(hex, &mut buff)
            .map_err(|_| anyhow::anyhow!("private key hex decode error"))?;
        Ok(Self::from(buff))
    }

    /// Generate a new random secret key using a cryptographically secure RNG
    pub fn generate() -> Self {
        let mut bytes = [0u8; PRIVATE_KEY_SIZE];
        getrandom::getrandom(&mut bytes).expect("failed to generate random bytes");
        Self::from(bytes)
    }

    /// Derive the public key from this secret key
    pub fn public(&self) -> PublicKey {
        PublicKey(self.0.public())
    }

    /// Convert secret key to raw bytes
    pub fn to_bytes(&self) -> [u8; PRIVATE_KEY_SIZE] {
        self.0.to_bytes()
    }

    /// Convert secret key to hexadecimal string
    pub fn to_hex(&self) -> String {
        hex::encode(self.to_bytes())
    }

    /// Encode secret key in PEM format for secure storage
    ///
    /// Returns a PEM-encoded string with tag "PRIVATE KEY".
    pub fn to_pem(&self) -> String {
        let pem = pem::Pem::new("PRIVATE KEY", self.to_bytes());
        pem::encode(&pem)
    }

    /// Parse a secret key from PEM format
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The PEM string is malformed
    /// - The PEM tag is not "PRIVATE KEY"
    /// - The key size is incorrect
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

    /// Convert Ed25519 secret key to X25519 (Montgomery curve) for ECDH
    ///
    /// This conversion is used internally for the key sharing protocol.
    /// The scalar bytes of the Ed25519 key are directly used as the X25519 private key.
    pub(crate) fn to_x25519(&self) -> StaticSecret {
        let signing_key = self.0.secret();
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
