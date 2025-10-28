use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Catalog {
    #[serde(default)]
    products: HashMap<String, super::Product>,
}

impl Catalog {
    pub fn products(&self) -> &HashMap<String, super::Product> {
        &self.products
    }
}
