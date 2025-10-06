use std::collections::HashSet;
use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::linked_data::Hash;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pins(HashSet<Hash>);

impl Deref for Pins {
    type Target = HashSet<Hash>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Pins {
    // TODO (amiller68): stream out pins as a HashSeq block
}
