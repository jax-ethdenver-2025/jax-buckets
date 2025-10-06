use serde::{Deserialize, Serialize};
use std::fmt;

pub type Version = String;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildInfo {
    pub version: Version,
    pub git_hash: String,
    pub build_profile: String,
    pub build_features: String,
    pub build_timestamp: String,
    pub rust_version: String,
    pub target: String,
    pub host: String,
}

impl BuildInfo {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            git_hash: option_env!("REPO_VERSION").unwrap_or("unknown").to_string(),
            build_profile: option_env!("BUILD_PROFILE")
                .unwrap_or("unknown")
                .to_string(),
            build_features: option_env!("BUILD_FEATURES").unwrap_or("none").to_string(),
            build_timestamp: option_env!("BUILD_TIMESTAMP")
                .unwrap_or("unknown")
                .to_string(),
            rust_version: option_env!("RUST_VERSION").unwrap_or("unknown").to_string(),
            target: option_env!("BUILD_TARGET").unwrap_or("unknown").to_string(),
            host: option_env!("BUILD_HOST").unwrap_or("unknown").to_string(),
        }
    }

    pub fn is_debug(&self) -> bool {
        self.build_profile == "debug"
    }

    pub fn is_release(&self) -> bool {
        self.build_profile == "release"
    }

    pub fn has_feature(&self, feature: &str) -> bool {
        self.build_features.split(',').any(|f| f.trim() == feature)
    }

    pub fn features(&self) -> Vec<&str> {
        if self.build_features == "none" {
            Vec::new()
        } else {
            self.build_features.split(',').map(|f| f.trim()).collect()
        }
    }

    pub fn short_hash(&self) -> &str {
        if self.git_hash.len() > 7 {
            &self.git_hash[..7]
        } else {
            &self.git_hash
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.git_hash.contains("-dirty")
    }
}

impl Default for BuildInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for BuildInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) built with {} on {} for {}",
            self.version,
            self.short_hash(),
            self.rust_version,
            self.build_timestamp,
            self.target
        )
    }
}

#[macro_export]
macro_rules! build_info {
    () => {
        $crate::version::BuildInfo::new()
    };
}

#[macro_export]
macro_rules! version_string {
    () => {
        format!(
            "{} ({})",
            env!("CARGO_PKG_VERSION"),
            option_env!("REPO_VERSION").unwrap_or("unknown")
        )
    };
}

pub fn version() -> String {
    version_string!()
}

pub fn build_info() -> BuildInfo {
    build_info!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_info_creation() {
        let info = BuildInfo::new();
        assert!(!info.version.is_empty());
        assert!(!info.git_hash.is_empty());
        assert!(!info.build_profile.is_empty());
    }

    #[test]
    fn test_version_string() {
        let version = version();
        assert!(version.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn test_has_feature() {
        let mut info = BuildInfo::new();
        info.build_features = "serde,async,tokio".to_string();

        assert!(info.has_feature("serde"));
        assert!(info.has_feature("async"));
        assert!(info.has_feature("tokio"));
        assert!(!info.has_feature("nonexistent"));
    }

    #[test]
    fn test_features_list() {
        let mut info = BuildInfo::new();
        info.build_features = "serde,async,tokio".to_string();

        let features = info.features();
        assert_eq!(features.len(), 3);
        assert!(features.contains(&"serde"));
        assert!(features.contains(&"async"));
        assert!(features.contains(&"tokio"));
    }

    #[test]
    fn test_no_features() {
        let mut info = BuildInfo::new();
        info.build_features = "none".to_string();

        let features = info.features();
        assert!(features.is_empty());
        assert!(!info.has_feature("serde"));
    }

    #[test]
    fn test_short_hash() {
        let mut info = BuildInfo::new();
        info.git_hash = "abcdef123456789".to_string();

        assert_eq!(info.short_hash(), "abcdef1");
    }

    #[test]
    fn test_is_dirty() {
        let mut info = BuildInfo::new();
        info.git_hash = "abcdef123456-dirty".to_string();

        assert!(info.is_dirty());

        info.git_hash = "abcdef123456".to_string();
        assert!(!info.is_dirty());
    }

    #[test]
    fn test_build_profile_checks() {
        let mut info = BuildInfo::new();

        info.build_profile = "debug".to_string();
        assert!(info.is_debug());
        assert!(!info.is_release());

        info.build_profile = "release".to_string();
        assert!(!info.is_debug());
        assert!(info.is_release());
    }
}
