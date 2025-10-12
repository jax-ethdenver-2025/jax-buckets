use std::collections::HashSet;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::linked_data::Hash;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pins(HashSet<Hash>);

impl Deref for Pins {
    type Target = HashSet<Hash>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for Pins {
    fn default() -> Self {
        Self::new()
    }
}

impl Pins {
    /// Create a new empty pin set
    pub fn new() -> Self {
        Pins(HashSet::new())
    }

    /// Insert a single hash into the pin set
    pub fn insert(&mut self, hash: Hash) -> bool {
        self.0.insert(hash)
    }

    /// Extend the pin set with an iterator of hashes
    pub fn extend<I>(&mut self, hashes: I)
    where
        I: IntoIterator<Item = Hash>,
    {
        self.0.extend(hashes)
    }

    /// Get the number of pinned hashes
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the pin set is empty
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Check if a hash is pinned
    pub fn contains(&self, hash: &Hash) -> bool {
        self.0.contains(hash)
    }

    /// Convert pins to a Vec for serialization
    pub fn to_vec(&self) -> Vec<Hash> {
        self.0.iter().copied().collect()
    }

    /// Create pins from a Vec
    pub fn from_vec(hashes: Vec<Hash>) -> Self {
        Pins(hashes.into_iter().collect())
    }

    /// Get an iterator over the hashes
    pub fn iter(&self) -> impl Iterator<Item = &Hash> {
        self.0.iter()
    }
}
