# Download cloud images

This project is supposed to get cloud images for kvm from the official indexes.
It should do the following:

1. Get each distro the index.
2. Pass it into an fzf.
3. The one that is choosen, download the img or qcow2 img.

## Repositories links

1. [Ubuntu](https://cloud-images.ubuntu.com/releases/streams/v1/index.json)

## Archive

**Description**: Serialize struct's instance with serde and print a json.

``` rust
println!("{:?}", serde_json::to_string(&product_metadata).unwrap());
```

### Intel

#### Working example of downloading qcow2 image for debian

``` rust
// Cargo.toml
// [dependencies]
// anyhow = "1.0.100"
// hex = "0.4.3"
// regex = "1.12.2"
// reqwest = "0.12.24"
// sha2 = "0.10.9"
// thiserror = "2.0.17"
// tokio = { version = "1.48.0", features = ["macros", "rt-multi-thread"] }

use anyhow::Result;
use regex::Regex;
use reqwest::Client;
use sha2::{Digest, Sha512};
use std::fs::write;

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
    pub sha256: String,
    pub filename: String,
}

async fn resolve(r: &ImageRequest, client: &Client) -> anyhow::Result<ImageAsset> {
    let base = format!("https://cloud.debian.org/images/cloud/{}/latest/", r.codename_or_major);
    // Fetch SHA256SUMS
    let sums_url = format!("{base}SHA512SUMS");
    let sums = client.get(&sums_url).send().await?.error_for_status()?.text().await?;

    // Example names: debian-12-genericcloud-amd64.qcow2 | debian-12-nocloud-amd64.qcow2
    let name_re = Regex::new(&format!(
        r"(debian-\d+-{}-{}\.{})",
        regex::escape(&r.variant.to_lowercase()),
        if r.arch == "x86_64" { "amd64" } else { &r.arch },
        regex::escape(&r.format)
    ))?;

    // Parse "SHA256SUMS": "abc123...  filename"
    for line in sums.lines() {
        if let Some(mat) = name_re.captures(line) {
            let filename = mat.get(1).unwrap().as_str().to_string();
            let sha = line.split_whitespace().next().unwrap().to_string();
            return Ok(ImageAsset {
                url: format!("{base}{filename}"),
                sha256: sha,
                filename,
            });
        }
    }
    anyhow::bail!("No matching Debian image for filters");
}

// Download + verify
pub async fn download_and_verify(client: &Client, asset: &ImageAsset, out_path: &std::path::Path) -> anyhow::Result<()> {
    let bytes = client.get(&asset.url).send().await?.error_for_status()?.bytes().await?;
    let mut hasher = Sha512::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    if got != asset.sha256.to_lowercase() {
        anyhow::bail!("SHA512 mismatch: expected {}, got {}", asset.sha256, got);
    }

    write(out_path, &bytes).unwrap();

    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Debian bookworm genericcloud qcow2 (amd64)
    let req = ImageRequest {
        distro: "debian".into(),
        codename_or_major: "bookworm".into(),
        arch: "amd64".into(),
        variant: "genericcloud".into(), // or "nocloud"
        format: "qcow2".into(),
    };

    let client = reqwest::Client::new();
    let asset = resolve(&req, &client).await?; // URL + SHA256 from CHECKSUM
    download_and_verify(&client, &asset, std::path::Path::new(&asset.filename)).await?;

    Ok(())
}
```
