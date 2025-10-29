use serde::Deserialize;
use std::collections::HashMap;

/// Top-level container for the Simplestreams catalogue returned by each
/// repository.
#[derive(Debug, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    products: HashMap<String, super::Product>,
}

impl Catalog {
    /// Borrow the catalogue entries keyed by their product identifier.
    pub fn products(&self) -> &HashMap<String, super::Product> {
        &self.products
    }
}
