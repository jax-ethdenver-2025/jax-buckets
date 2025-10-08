use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use super::version::Version;
use super::{Cid, Ipld};

const VERSION_KEY: &str = "version";
const PREVIOUS_KEY: &str = "previous";
const DATA_KEY: &str = "data";

/// Manifest
#[derive(Default, Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// Build version
    version: Version,
    /// Previous manifest CID
    previous: Cid,
    /// data node CID
    data: Cid,
}

impl From<Manifest> for Ipld {
    fn from(manifest: Manifest) -> Self {
        let mut map = std::collections::BTreeMap::new();
        map.insert(VERSION_KEY.to_string(), manifest.version.into());
        map.insert(PREVIOUS_KEY.to_string(), Ipld::Link(manifest.previous));
        map.insert(DATA_KEY.to_string(), Ipld::Link(manifest.data));
        Ipld::Map(map)
    }
}

impl TryFrom<Ipld> for Manifest {
    type Error = ManifestError;
    fn try_from(ipld: Ipld) -> Result<Self, ManifestError> {
        match ipld {
            Ipld::Map(map) => {
                let version = match map.get(VERSION_KEY) {
                    Some(ipld) => Version::try_from(ipld.clone())?,
                    None => return Err(ManifestError::MissingField(VERSION_KEY.to_string())),
                };
                let previous = match map.get(PREVIOUS_KEY) {
                    Some(Ipld::Link(cid)) => *cid,
                    _ => return Err(ManifestError::MissingField(PREVIOUS_KEY.to_string())),
                };
                let data = match map.get(DATA_KEY) {
                    Some(Ipld::Link(cid)) => *cid,
                    _ => return Err(ManifestError::MissingField(DATA_KEY.to_string())),
                };

                Ok(Manifest {
                    version,
                    previous,
                    data,
                })
            }
            _ => Err(ManifestError::MissingField("map".to_string())),
        }
    }
}

impl Manifest {
    pub fn version(&self) -> &Version {
        &self.version
    }

    pub fn previous(&self) -> &Cid {
        &self.previous
    }

    pub fn data(&self) -> &Cid {
        &self.data
    }

    pub fn set_data(&mut self, cid: Cid) {
        self.data = cid;
    }

    pub fn set_previous(&mut self, cid: Cid) {
        self.previous = cid;
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("version error")]
    VersionError(#[from] super::version::VersionError),
    #[error("missing field: {0}")]
    MissingField(String),
}
