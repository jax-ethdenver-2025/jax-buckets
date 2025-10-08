use async_trait::async_trait;
use thumbs_up::prelude::{EcKey, PrivateKey, PublicKey};

use crate::{AppState, Op};

#[derive(Debug, clap::Args, Clone)]
pub struct Key {
    // NOTE: not used in exexute, but when initializing the app state
    #[clap(short, long)]
    pub key_path: String,
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("default error: {0}")]
    Default(#[from] anyhow::Error),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("thumbs up error: {0}")]
    ThumbsUp(#[from] thumbs_up::prelude::KeyError),
}

#[async_trait]
impl Op for Key {
    type Error = KeyError;
    type Output = String;

    async fn execute(&self, _state: &AppState) -> Result<Self::Output, Self::Error> {
        let key = EcKey::generate()?;
        let private_key_pem = key.export()?;
        let public_key_pem = key.public_key()?.export()?;
        let pretty_key_id = key.public_key()?.pretty_key_id()?;
        // Check if the path is directory
        let path = std::path::Path::new(&self.key_path);
        if path.is_dir() {
            let private_key_path = path.join(&pretty_key_id);
            let public_key_path = path.join(format!("{}.pem", pretty_key_id));
            std::fs::write(private_key_path, private_key_pem)?;
            std::fs::write(public_key_path, public_key_pem)?;
            return Ok(format!(
                "key pair generated at: {}/{}",
                path.display(),
                pretty_key_id
            ));
        }
        Ok("key path is not a directory".to_string())
    }
}
