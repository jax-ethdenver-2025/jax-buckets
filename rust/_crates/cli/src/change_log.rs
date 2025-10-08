use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use leaky_common::prelude::*;

use serde::{Deserialize, Serialize};

// TODO: this is an akward way to do this, i could probably
// constructs diffs better

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum StagedType {
    Added,
    Modified,
    Removed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChangeType {
    Base {
        last_check: Option<SystemTime>,
    },
    Added {
        modified: bool,
        last_check: Option<SystemTime>,
    },
    Modified {
        processed: bool,
        last_check: Option<SystemTime>,
    },
    Removed {
        processed: bool,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum FileType {
    Regular,
    Schema,
    Object,
}

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Base { .. } => "\x1b[0;32mBase\x1b[0m",
            Self::Added { .. } => "\x1b[0;32mAdded\x1b[0m",
            Self::Modified { .. } => "\x1b[0;33mModified\x1b[0m",
            Self::Removed { .. } => "\x1b[0;31mRemoved\x1b[0m",
        };
        write!(f, "{}", s)
    }
}

/// Tracks what files are in the local clone and their hashes
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ChangeLog {
    regular: BTreeMap<PathBuf, (Cid, ChangeType)>,
    schema: BTreeMap<PathBuf, (Cid, ChangeType)>,
    object: BTreeMap<PathBuf, (Cid, ChangeType)>,
}

impl ChangeLog {
    pub fn new() -> Self {
        Self {
            regular: BTreeMap::new(),
            schema: BTreeMap::new(),
            object: BTreeMap::new(),
        }
    }

    pub fn regular(&self) -> &BTreeMap<PathBuf, (Cid, ChangeType)> {
        &self.regular
    }

    pub fn schema(&self) -> &BTreeMap<PathBuf, (Cid, ChangeType)> {
        &self.schema
    }

    pub fn object(&self) -> &BTreeMap<PathBuf, (Cid, ChangeType)> {
        &self.object
    }

    pub fn insert(&mut self, path: PathBuf, entry: (Cid, ChangeType)) {
        match FileType::from_path(&path) {
            FileType::Regular => self.regular.insert(path, entry),
            FileType::Schema => self.schema.insert(path, entry),
            FileType::Object => self.object.insert(path, entry),
        };
    }

    pub fn remove(&mut self, path: &Path) -> Option<(Cid, ChangeType)> {
        match FileType::from_path(path) {
            FileType::Regular => self.regular.remove(path),
            FileType::Schema => self.schema.remove(path),
            FileType::Object => self.object.remove(path),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &(Cid, ChangeType))> {
        // Collect all entries into a vector and sort by path
        let mut all_entries: Vec<_> = self
            .regular
            .iter()
            .chain(self.schema.iter())
            .chain(self.object.iter())
            .collect();

        all_entries.sort_by(|(path_a, _), (path_b, _)| path_a.cmp(path_b));
        all_entries.into_iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&PathBuf, &mut (Cid, ChangeType))> {
        // Collect all entries into a vector and sort by path
        let mut all_entries: Vec<_> = self
            .regular
            .iter_mut()
            .chain(self.schema.iter_mut())
            .chain(self.object.iter_mut())
            .collect();

        all_entries.sort_by(|(path_a, _), (path_b, _)| path_a.cmp(path_b));
        all_entries.into_iter()
    }
}

impl FileType {
    pub fn from_path(path: &Path) -> Self {
        if path.file_name().is_some_and(|f| f == "schema.md") {
            FileType::Schema
        } else if path
            .file_name()
            .is_some_and(|f| f.to_str().unwrap().ends_with(".obj.md"))
        {
            FileType::Object
        } else {
            FileType::Regular
        }
    }

    pub fn is_schema(&self) -> bool {
        matches!(self, FileType::Schema)
    }

    pub fn is_object(&self) -> bool {
        matches!(self, FileType::Object)
    }
}
