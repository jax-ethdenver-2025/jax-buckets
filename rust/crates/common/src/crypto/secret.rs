use std::io::Read;
use std::ops::Deref;

use chacha20poly1305::Key;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use serde::{Deserialize, Serialize};

// ChaCha20-Poly1305 uses 12-byte nonces
pub const NONCE_SIZE: usize = 12;
// ChaCha20-Poly1305 uses 32-byte keys
pub const SECRET_SIZE: usize = 32;
// Chunk size for streaming (can be any size)
#[allow(dead_code)]
pub const CHUNK_SIZE: usize = 4096;

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("secret error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Secret([u8; SECRET_SIZE]);

impl Default for Secret {
    fn default() -> Self {
        Secret([0; SECRET_SIZE])
    }
}

impl Deref for Secret {
    type Target = [u8; SECRET_SIZE];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<[u8; SECRET_SIZE]> for Secret {
    fn from(bytes: [u8; SECRET_SIZE]) -> Self {
        Secret(bytes)
    }
}

impl Secret {
    pub fn generate() -> Self {
        let mut buff = [0; SECRET_SIZE];
        getrandom::getrandom(&mut buff).expect("failed to generate random bytes");
        Self(buff)
    }

    pub fn from_slice(data: &[u8]) -> Result<Self, SecretError> {
        if data.len() != SECRET_SIZE {
            return Err(anyhow::anyhow!(
                "invalid secret size, expected {}, got {}",
                SECRET_SIZE,
                data.len()
            )
            .into());
        }
        let mut buff = [0; SECRET_SIZE];
        buff.copy_from_slice(data);
        Ok(buff.into())
    }

    pub fn bytes(&self) -> &[u8] {
        self.0.as_ref()
    }

    // Small payload encryption (keeps your existing API)
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        let key = Key::from_slice(self.bytes());
        let cipher = ChaCha20Poly1305::new(key);

        // Generate nonce manually
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        getrandom::getrandom(&mut nonce_bytes)
            .map_err(|e| anyhow::anyhow!("failed to generate nonce: {}", e))?;
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(&nonce, data.as_ref())
            .map_err(|_| anyhow::anyhow!("encrypt error"))?;

        let mut out = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        out.extend_from_slice(nonce.as_ref());
        out.extend_from_slice(ciphertext.as_ref());

        Ok(out)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, SecretError> {
        if data.len() < NONCE_SIZE {
            return Err(anyhow::anyhow!("data too short for nonce").into());
        }

        let key = Key::from_slice(self.bytes());
        let nonce = Nonce::from_slice(&data[..NONCE_SIZE]);
        let cipher = ChaCha20Poly1305::new(key);
        let decrypted = cipher
            .decrypt(nonce, &data[NONCE_SIZE..])
            .map_err(|_| anyhow::anyhow!("decrypt error"))?;

        Ok(decrypted.to_vec())
    }

    // Create an encrypted reader that can be consumed by a stream
    pub fn encrypt_reader<R>(&self, reader: R) -> Result<impl Read, SecretError>
    where
        R: Read,
    {
        // For now, we'll read all data and encrypt it
        // This could be optimized to a true streaming implementation later
        let mut data = Vec::new();
        let mut reader = reader;
        reader.read_to_end(&mut data).map_err(SecretError::Io)?;

        let encrypted = self.encrypt(&data)?;
        Ok(std::io::Cursor::new(encrypted))
    }

    // Create a decrypted reader from encrypted data
    pub fn decrypt_reader<R>(&self, reader: R) -> Result<impl Read, SecretError>
    where
        R: Read,
    {
        // For now, we'll read all data and decrypt it
        // This could be optimized to a true streaming implementation later
        let mut encrypted_data = Vec::new();
        let mut reader = reader;
        reader
            .read_to_end(&mut encrypted_data)
            .map_err(SecretError::Io)?;

        let decrypted = self.decrypt(&encrypted_data)?;
        Ok(std::io::Cursor::new(decrypted))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_secret_encrypt_decrypt() {
        let secret = Secret::generate();
        let data = b"hello world, this is a test message for encryption";

        let encrypted = secret.encrypt(data).unwrap();
        let decrypted = secret.decrypt(&encrypted).unwrap();

        assert_eq!(data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_encrypt_decrypt_reader() {
        let secret = Secret::generate();
        let data = b"hello world, this is a test message for reader encryption and decryption";

        // Create encrypted reader
        let reader = Cursor::new(data.to_vec());
        let mut encrypted_reader = secret.encrypt_reader(reader).unwrap();

        // Read encrypted data
        let mut encrypted_data = Vec::new();
        encrypted_reader.read_to_end(&mut encrypted_data).unwrap();

        // Decrypt using reader
        let encrypted_cursor = Cursor::new(encrypted_data);
        let mut decrypted_reader = secret.decrypt_reader(encrypted_cursor).unwrap();

        let mut decrypted_data = Vec::new();
        decrypted_reader.read_to_end(&mut decrypted_data).unwrap();

        assert_eq!(data.to_vec(), decrypted_data);
    }

    #[test]
    fn test_secret_size_validation() {
        let too_short = [1u8; 16];
        let too_long = [1u8; 64];

        assert!(Secret::from_slice(&too_short).is_err());
        assert!(Secret::from_slice(&too_long).is_err());

        let just_right = [1u8; SECRET_SIZE];
        assert!(Secret::from_slice(&just_right).is_ok());
    }
}
