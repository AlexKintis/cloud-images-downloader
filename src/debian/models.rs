use reqwest::Client;

#[derive(Debug, Clone)]
pub struct ImageRequest {
    pub distro: String,            // "debian" | "almalinux"
    pub codename_or_major: String, // e.g., "bookworm" or "9"
    pub arch: String,              // "amd64" | "x86_64"
    pub variant: String,           // "genericcloud" | "nocloud" | "GenericCloud"
    pub format: String,            // "qcow2" | "raw"
}

#[derive(Debug, Clone)]
pub struct ImageAsset {
    pub url: String,
    pub sha512: String,
    pub filename: String,
}

#[async_trait::async_trait]
pub trait Provider {
    async fn resolve(&self, req: &ImageRequest, client: &Client) -> anyhow::Result<ImageAsset>;
}

pub struct DebianProvider;
