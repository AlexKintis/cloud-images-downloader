use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Public model; serde is confined to this module tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repository {
    pub(crate) name: String,
    pub(crate) url: String,
    #[serde(rename = "parameters")]
    pub(crate) other_parameters: Option<HashMap<String, String>>,
}

impl Repository {
    #[allow(unused)]
    // Borrowing getters (no clones).
    pub fn name(&self) -> &str {
        &self.name
    }

    #[allow(unused)]
    pub fn url(&self) -> &str {
        &self.url
    }

    #[allow(unused)]
    pub fn other_parameters(&self) -> Option<&HashMap<String, String>> {
        self.other_parameters.as_ref()
    }
}
