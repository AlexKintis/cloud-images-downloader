use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Item {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    sha256: Option<String>,
    // ftype exists but we won’t rely on it; keep optional for completeness
    #[serde(default)]
    ftype: Option<String>,
}

#[allow(unused)]
impl Item {
    pub fn path(&self) -> &Option<String> {
        &self.path
    }

    pub fn sha256(&self) -> &Option<String> {
        &self.sha256
    }

    #[allow(dead_code)]
    pub fn ftype(&self) -> &Option<String> {
        &self.ftype
    }
}
