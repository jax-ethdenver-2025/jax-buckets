use std::env;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Version {
    build_profile: String,
    build_features: String,
    version: String,
}

impl Default for Version {
    fn default() -> Self {
        Self::new()
    }
}

impl Version {
    pub fn new() -> Self {
        Self {
            build_profile: env!("BUILD_PROFILE").to_string(),
            build_features: env!("BUILD_FEATURES").to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    pub fn build_profile(&self) -> &str {
        &self.build_profile
    }

    pub fn build_features(&self) -> &str {
        &self.build_features
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}
