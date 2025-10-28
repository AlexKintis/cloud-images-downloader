pub mod models;
pub use models::{DebianProvider, ImageAsset, ImageRequest, Provider};

use regex::Regex;
use reqwest::Client;
use sha2::{Digest, Sha512};
use std::fs::write;

use crate::helpers::{arch_options_for, choose_one};
use crate::ubuntu::Image;
use anyhow::ensure;

pub async fn pick_debian(codename: &str) -> anyhow::Result<Image> {
    // 1) Arch (use your existing helper; ensure it includes amd64/arm64 at least)
    let arch = choose_one("Select Architecture", arch_options_for("Debian"))?;

    // 2) Fetch images for the chosen arch (treat `codename` like "bookworm", "trixie", or "stable")
    let mut images: Vec<Image> = debian_list(codename, &arch, /*include_testing=*/ false)
        .await
        .with_context(|| format!("fetch debian images for codename='{codename}' arch='{arch}'"))?;

    ensure!(!images.is_empty(), "No Debian images found for codename={codename} arch={arch}");

    // 3) Distro major version (e.g., "12", "13")
    let mut distro_versions = images.iter().map(|i| i.distro_version().to_string()).collect::<Vec<_>>();
    distro_versions.sort();
    distro_versions.reverse();
    distro_versions.dedup();

    let distro_version = choose_one("Select Distro Version", distro_versions)?;
    images = images.into_iter().filter(|i| i.distro_version() == distro_version).collect();
    ensure!(!images.is_empty(), "No Debian images found for distro_version={distro_version}");

    // 4) Image version / build (e.g., point release or date-stamped build)
    let mut image_versions: Vec<String> = images.iter().map(|i| i.version().to_string()).collect::<Vec<_>>();
    image_versions.sort();
    image_versions.reverse();
    image_versions.dedup();

    let image_version = choose_one("Select Image Version", image_versions)?;
    images = images.into_iter().filter(|i| i.version() == image_version).collect();
    ensure!(
        !images.is_empty(),
        "No Debian images for distro_version={distro_version} and version={image_version}"
    );

    // 5) Image type / variant (Debian usually: "genericcloud" or "nocloud")
    let mut image_types: Vec<String> = images.iter().map(|i| i.image_type().to_string()).collect();
    image_types.sort();
    image_types.dedup();

    let image_type = choose_one("Select image type (variant)", image_types)?;
    images = images.into_iter().filter(|i| i.image_type() == image_type).collect();
    ensure!(
        !images.is_empty(),
        "No Debian images found for distro_version={distro_version}, version={image_version}, type={image_type}"
    );

    // 6) If multiple artifacts remain (qcow2/raw), let user pick the exact one
    let labelize = |i: &Image| format!("{} | {} | {}", i.name(), i.arch(), i.url());
    let chosen_label = choose_one("Select Image Artifact", images.iter().map(|i| labelize(i)).collect())?;

    let idx = images
        .iter()
        .position(|i| labelize(i) == chosen_label)
        .expect("selected label must match one candidate");

    Ok(images[idx].clone())
}

/// Adapt this to your actual constructor/factory.
/// Replace with `Image::new(...)` or similar in your codebase.
fn make_image(name: String, url: String, arch: String, image_type: String, version: String, distro_version: String) -> Image {
    Image::from_parts(name, url, arch, image_type, version, distro_version)
}

/// List Debian cloud images for a given codename & arch.
///
/// - `codename`: "bookworm", "trixie", or "stable" (etc)
/// - `arch`: "amd64" | "arm64" (accepts "x86_64" and normalizes to "amd64")
/// - `include_testing`: currently unused (kept for API symmetry)
pub async fn debian_list(codename: &str, arch: &str, _include_testing: bool) -> Result<Vec<Image>> {
    let client = Client::new();

    // Debian calls x86_64 -> amd64
    let want_arch = match arch {
        "x86_64" => "amd64",
        other => other,
    }
    .to_string();

    // e.g. https://cloud.debian.org/images/cloud/bookworm/
    let base = format!("https://cloud.debian.org/images/cloud/{}/", codename);

    // 1) Fetch directory index and extract subdirs: latest/ and YYYYMMDD-HHMM/
    let index_html = client
        .get(&base)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .with_context(|| format!("fetch directory listing: {base}"))?;

    let dir_re = Regex::new(r#"href="((?:latest|\d{8}-\d{4}))/""#)?;
    let mut dirs: Vec<String> = dir_re.captures_iter(&index_html).map(|c| c[1].to_string()).collect();

    // Dedup + keep latest first for nice UX (latest, then most recent builds)
    dirs.sort();
    dirs.dedup();
    // Move "latest" to front if present
    if let Some(pos) = dirs.iter().position(|d| d == "latest") {
        let latest = dirs.remove(pos);
        dirs.insert(0, latest);
    }
    // Reverse to have newest date dirs first (after "latest")
    // (If "latest" exists, it's already at index 0.)
    if dirs.len() > 1 {
        let (head, tail) = dirs.split_at_mut(1);
        tail.reverse();
        // head (latest) + reversed tail
        dirs = head.iter().cloned().chain(tail.iter().cloned()).collect();
    }

    // 2) For each subdir, read SHA256SUMS and parse artifacts
    // Filenames look like:
    //   debian-12-genericcloud-amd64.qcow2
    //   debian-12-nocloud-amd64.qcow2
    // We’ll capture:
    //   distro_version = 12
    //   image_type     = genericcloud|nocloud
    //   arch           = amd64|arm64
    //   ext            = qcow2|raw (you can keep/filter later)
    //
    // SHA256SUMS lines are typically:
    //   <sha256>  debian-12-genericcloud-amd64.qcow2
    //
    let line_re = Regex::new(
        r#"(?i)^(?P<sha>[a-f0-9]{64})\s+\*?(?P<file>debian-(?P<dver>\d+)-(?P<variant>[a-z0-9]+)-(?P<arch>amd64|arm64)\.(?P<ext>qcow2|raw))$"#,
    )?;

    let mut out = Vec::new();

    for d in dirs {
        let sums_url = format!("{base}{d}/SHA256SUMS");
        let sums = match client.get(&sums_url).send().await {
            Ok(resp) => match resp.error_for_status() {
                Ok(ok) => ok.text().await.unwrap_or_default(),
                Err(_) => continue, // no SHA256SUMS in this dir; skip
            },
            Err(_) => continue,
        };

        for line in sums.lines() {
            if let Some(c) = line_re.captures(line.trim()) {
                let file_arch = c.name("arch").unwrap().as_str();
                if file_arch != want_arch {
                    continue;
                }

                let filename = c.name("file").unwrap().as_str().to_string();
                let distro_version = c.name("dver").unwrap().as_str().to_string();
                let variant = c.name("variant").unwrap().as_str().to_string();

                // You can choose to filter by ext here if you only want qcow2:
                // let ext = c.name("ext").unwrap().as_str();
                // if ext != "qcow2" { continue; }

                let url = format!("{base}{d}/{}", filename);

                // "version" in your picker is the build dir (e.g., "latest" or "20241013-1744")
                // "image_type" is the Debian variant (e.g., "genericcloud", "nocloud")
                out.push(make_image(filename, url, want_arch.clone(), variant, d.clone(), distro_version));
            }
        }
    }

    Ok(out)
}

#[async_trait::async_trait]
impl Provider for DebianProvider {
    async fn resolve(&self, r: &ImageRequest, client: &Client) -> anyhow::Result<ImageAsset> {
        let base = format!("https://cloud.debian.org/images/cloud/{}/latest/", r.codename_or_major);

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
                    sha512: sha,
                    filename,
                });
            }
        }
        anyhow::bail!("No matching Debian image for filters");
    }
}

// Download + verify
pub async fn download_and_verify(client: &Client, asset: &ImageAsset, out_path: &std::path::Path) -> anyhow::Result<()> {
    let bytes = client.get(&asset.url).send().await?.error_for_status()?.bytes().await?;
    let mut hasher = Sha512::new();
    hasher.update(&bytes);
    let got = hex::encode(hasher.finalize());
    if got != asset.sha512.to_lowercase() {
        anyhow::bail!("SHA256 mismatch: expected {}, got {}", asset.sha512, got);
    }

    write(out_path, &bytes).unwrap();

    Ok(())
}
