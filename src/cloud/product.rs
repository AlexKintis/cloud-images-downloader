use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Debug, Deserialize)]
pub struct Product {
    #[serde(default)]
    arch: Option<String>,

    #[serde(default)]
    os: Option<String>,

    #[serde(default)]
    release: Option<String>,

    #[serde(default)]
    release_codename: Option<String>,

    #[serde(rename = "version")]
    distro_version: Option<String>,

    #[serde(skip_serializing)]
    versions: HashMap<String, super::Version>,
}

#[allow(unused)]
impl Product {
    pub fn os(&self) -> Option<String> {
        self.os.clone()
    }

    pub fn release(&self) -> Option<String> {
        self.release.clone()
    }

    pub fn release_codename(&self) -> Option<String> {
        self.release_codename.clone()
    }

    pub fn distro_version(&self) -> Option<String> {
        self.distro_version.clone()
    }

    pub fn arch(&self) -> Option<String> {
        self.arch.clone()
    }

    pub fn versions(&self) -> &HashMap<String, super::Version> {
        &self.versions
    }
}
