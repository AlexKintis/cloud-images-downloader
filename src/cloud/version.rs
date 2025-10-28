use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Version {
    #[serde(default)]
    items: HashMap<String, super::Item>,
}

impl Version {
    pub fn items(&self) -> &HashMap<String, super::Item> {
        &self.items
    }
}
