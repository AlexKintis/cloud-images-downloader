use serde::Deserialize;
use std::collections::HashMap;

/// Wraps the list of artifact items for a specific product version.
#[derive(Debug, Deserialize)]
pub struct Version {
    #[serde(default)]
    items: HashMap<String, super::Item>,
}

impl Version {
    /// Borrow all artifacts keyed by their alias (e.g. `disk1.img`).
    pub fn items(&self) -> &HashMap<String, super::Item> {
        &self.items
    }
}
